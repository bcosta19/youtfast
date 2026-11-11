//! Drawing mpv's picture inside the egui window.
//!
//! mpv renders with OpenGL into a framebuffer of ours, sized to the video's
//! rectangle in physical pixels, and an egui_glow paint callback blits that
//! framebuffer onto the window. Everything here runs on the GL thread inside
//! the callback, with eframe's context current: the render context is made
//! on the first draw and freed in [`VideoSurface::destroy`], from
//! `App::on_exit`, where the context is current too.
//!
//! The framebuffer keeps its last picture between egui frames: a frame egui
//! draws for another reason (a hover, a tooltip) only blits it again; mpv
//! renders when it says a new frame is due, or when the size changes. Opening
//! another video deletes the target and holds the blit until mpv loads that
//! file and draws a frame that is not a redraw.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex, PoisonError};

use eframe::egui_glow;
use eframe::glow::{self, HasContext};

use super::{Error, Handle, ffi};

/// eframe's lookup of OpenGL functions, which mpv's renderer needs.
pub type GetProcAddress = Arc<dyn Fn(&CStr) -> *const c_void + Send + Sync>;

/// The windowing system's display, which lets mpv hand hardware-decoded
/// frames to OpenGL without copying them back through the CPU.
#[derive(Clone, Copy, Debug)]
pub enum NativeDisplay {
    None,
    /// A `wl_display*`.
    Wayland(*mut c_void),
    /// An Xlib `Display*`.
    X11(*mut c_void),
}

#[derive(Clone)]
pub struct VideoSurface {
    inner: Arc<Mutex<Inner>>,
}

/// Whether the target holds a frame that may be shown for the selected video.
#[derive(Default)]
struct Frame {
    /// mpv reported the selected file loaded; its frames may be shown.
    allowed: bool,
    /// The target holds a picture from the allowed file.
    current: bool,
    /// The target still holds the previous video's picture; delete it before
    /// the next draw so no later render can bring those pixels back.
    discard: bool,
}

impl Frame {
    /// A new load started: the target still holds the previous video.
    fn forget(&mut self) {
        self.allowed = false;
        self.current = false;
        self.discard = true;
    }

    /// mpv loaded the selected file: its frames may be shown now.
    fn allow(&mut self) {
        self.allowed = true;
    }

    fn allowed(&self) -> bool {
        self.allowed
    }

    fn discard_target(&mut self) -> bool {
        std::mem::take(&mut self.discard)
    }

    /// mpv rendered into the target. A frame from the previous file (or a
    /// resize-only render before the new file loaded) must not make the
    /// target visible.
    fn rendered(&mut self, new_frame: bool) {
        if self.allowed && new_frame {
            self.current = true;
        }
    }

    fn visible(&self) -> bool {
        self.current
    }
}

struct Inner {
    mpv: Arc<Handle>,
    get_proc_address: GetProcAddress,
    display: NativeDisplay,
    repaint: egui::Context,
    render: Option<RenderContext>,
    target: Option<Target>,
    frame: Frame,
    /// The libmpv build did not answer `NEXT_FRAME_INFO`; fall back to its
    /// update flag instead (logged once).
    frame_info_missing: bool,
    /// Creating the renderer failed once; the error is logged and the
    /// surface stays black rather than retrying every frame.
    failed: bool,
}

// SAFETY: the raw pointers (the display, the render context) are opaque
// handles. They are only dereferenced by mpv, on the GL thread, inside the
// paint callback or destroy, and the mutex serializes those.
unsafe impl Send for Inner {}

impl VideoSurface {
    pub fn new(
        mpv: Arc<Handle>,
        get_proc_address: GetProcAddress,
        display: NativeDisplay,
        repaint: egui::Context,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                mpv,
                get_proc_address,
                display,
                repaint,
                render: None,
                target: None,
                frame: Frame::default(),
                frame_info_missing: false,
                failed: false,
            })),
        }
    }

    /// Queues the video into `rect` of the current egui frame.
    pub fn paint(&self, ui: &egui::Ui, rect: egui::Rect) {
        let inner = Arc::clone(&self.inner);
        let callback = egui_glow::CallbackFn::new(move |info, painter| {
            let mut inner = inner.lock().unwrap_or_else(PoisonError::into_inner);
            inner.draw(&info, painter.gl());
        });
        ui.painter().add(egui::PaintCallback {
            rect,
            callback: Arc::new(callback),
        });
    }

    /// A new video is opening: the framebuffer still holds the old picture.
    pub fn forget(&self) {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .frame
            .forget();
    }

    /// mpv loaded the selected file: its frames may be shown now.
    pub fn allow_frames(&self) {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .frame
            .allow();
    }

    /// Whether the current video's first frame is still to come, and the
    /// renderer can still draw it.
    pub fn waiting_for_frame(&self) -> bool {
        let inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        !inner.frame.visible() && !inner.failed
    }

    /// Frees the renderer and the framebuffer. Call with the GL context
    /// current, before the [`super::Mpv`] it draws is dropped.
    pub fn destroy(&self, gl: &glow::Context) {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(render) = inner.render.take() {
            // SAFETY: the caller has the context current.
            unsafe { render.free() };
        }
        if let Some(target) = inner.target.take() {
            // SAFETY: as above.
            unsafe { target.delete(gl) };
        }
    }
}

impl Inner {
    fn draw(&mut self, info: &egui::PaintCallbackInfo, gl: &glow::Context) {
        if self.failed {
            return;
        }
        if self.render.is_none() {
            // SAFETY: called from the paint callback, context current.
            match unsafe {
                RenderContext::new(
                    &self.mpv,
                    &self.get_proc_address,
                    self.display,
                    &self.repaint,
                )
            } {
                Ok(render) => self.render = Some(render),
                Err(error) => {
                    log::error!("could not start the video renderer: {error}");
                    self.failed = true;
                    return;
                }
            }
        }
        // egui's viewport_in_pixels clamps to the screen. A scrolling video
        // still needs its full framebuffer and destination, otherwise mpv
        // changes the aspect ratio as the picture leaves the window.
        let viewport = video_viewport(info);
        let clip = info.clip_rect_in_pixels();
        let (width, height) = (viewport.width_px, viewport.height_px);
        if width <= 0 || height <= 0 || clip.width_px <= 0 || clip.height_px <= 0 {
            return;
        }
        // SAFETY: GL calls on the context egui_glow made current for this
        // callback. egui_glow restores its own state after the callback;
        // the framebuffer bindings are restored here because the blit's
        // destination is whatever egui was drawing into.
        unsafe {
            let previous_draw = gl.get_parameter_i32(glow::DRAW_FRAMEBUFFER_BINDING);
            let previous_read = gl.get_parameter_i32(glow::READ_FRAMEBUFFER_BINDING);

            if self.frame.discard_target()
                && let Some(old) = self.target.take()
            {
                old.delete(gl);
            }
            let resized = self
                .target
                .as_ref()
                .is_none_or(|target| target.width != width || target.height != height);
            if resized {
                if let Some(old) = self.target.take() {
                    old.delete(gl);
                }
                match Target::new(gl, width, height) {
                    Ok(target) => self.target = Some(target),
                    Err(error) => {
                        log::error!("could not make the video framebuffer: {error}");
                        restore(gl, previous_draw, previous_read);
                        return;
                    }
                }
            }
            let (Some(render), Some(target)) = (self.render.as_ref(), self.target.as_ref()) else {
                return;
            };

            let frame_due =
                ffi::mpv_render_context_update(render.ptr) & ffi::MPV_RENDER_UPDATE_FRAME != 0;
            if self.frame.allowed() && (frame_due || resized) {
                // SAFETY: called from the render callback.
                let next = render.next_frame_info();
                if frame_due && next.is_none() && !self.frame_info_missing {
                    self.frame_info_missing = true;
                    log::warn!("mpv did not report the next frame; using its update flag instead");
                }
                match render.render(target) {
                    Ok(()) => self
                        .frame
                        .rendered(next.map_or(frame_due, |frame| new_picture(frame.flags))),
                    Err(error) => {
                        log::warn!("mpv could not draw a frame: {error}");
                        if resized {
                            // The target was just recreated and nothing drew
                            // into it: blitting would show undefined pixels.
                            restore(gl, previous_draw, previous_read);
                            return;
                        }
                    }
                }
            }
            if !self.frame.visible() {
                // The target holds the previous video, or the new one has not
                // drawn a frame yet: leave the black backing egui drew instead
                // of blitting it.
                restore(gl, previous_draw, previous_read);
                return;
            }

            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(target.framebuffer));
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, framebuffer(previous_draw));
            // libmpv changes GL state, including the scissor test. Reinstate
            // egui's clip before the blit so video cannot cover the toolbar.
            gl.enable(glow::SCISSOR_TEST);
            gl.scissor(
                clip.left_px,
                clip.from_bottom_px,
                clip.width_px,
                clip.height_px,
            );
            gl.blit_framebuffer(
                0,
                0,
                width,
                height,
                viewport.left_px,
                viewport.from_bottom_px,
                viewport.left_px + width,
                viewport.from_bottom_px + height,
                glow::COLOR_BUFFER_BIT,
                glow::NEAREST,
            );
            restore(gl, previous_draw, previous_read);
        }
    }
}

/// Whether mpv is about to render a new picture, not redraw the previous one.
/// The previous one can still be the video that was just replaced.
fn new_picture(flags: u64) -> bool {
    flags & ffi::MPV_RENDER_FRAME_INFO_PRESENT != 0
        && flags & ffi::MPV_RENDER_FRAME_INFO_REDRAW == 0
}

fn video_viewport(info: &egui::PaintCallbackInfo) -> egui::epaint::ViewportInPixels {
    let rect = info.viewport;
    let scale = info.pixels_per_point;
    let left_px = (rect.left() * scale).round() as i32;
    let top_px = (rect.top() * scale).round() as i32;
    let width_px = (rect.width() * scale).round() as i32;
    let height_px = (rect.height() * scale).round() as i32;
    egui::epaint::ViewportInPixels {
        left_px,
        top_px,
        from_bottom_px: info.screen_size_px[1] as i32 - top_px - height_px,
        width_px,
        height_px,
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        // Freeing GL objects needs the context current, which a drop cannot
        // promise. destroy() runs at exit; anything left is the OS's to free.
        // The framebuffer is left the same way: Target has no Drop of its own.
        if let Some(render) = self.render.take() {
            log::debug!("video renderer left for the process exit");
            std::mem::forget(render);
        }
    }
}

/// # Safety
///
/// A GL context must be current.
unsafe fn restore(gl: &glow::Context, draw: i32, read: i32) {
    unsafe {
        gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, framebuffer(draw));
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, framebuffer(read));
    }
}

fn framebuffer(id: i32) -> Option<glow::NativeFramebuffer> {
    NonZeroU32::new(id as u32).map(glow::NativeFramebuffer)
}

/// The texture mpv draws into, and the framebuffer around it.
struct Target {
    framebuffer: glow::NativeFramebuffer,
    texture: glow::NativeTexture,
    width: i32,
    height: i32,
}

impl Target {
    /// # Safety
    ///
    /// A GL context must be current.
    unsafe fn new(gl: &glow::Context, width: i32, height: i32) -> Result<Self, String> {
        unsafe {
            let texture = gl.create_texture()?;
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                width,
                height,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );
            gl.bind_texture(glow::TEXTURE_2D, None);

            let framebuffer = match gl.create_framebuffer() {
                Ok(framebuffer) => framebuffer,
                Err(error) => {
                    gl.delete_texture(texture);
                    return Err(error);
                }
            };
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(texture),
                0,
            );
            let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
            let target = Self {
                framebuffer,
                texture,
                width,
                height,
            };
            if status != glow::FRAMEBUFFER_COMPLETE {
                target.delete(gl);
                return Err(format!("framebuffer incomplete (0x{status:x})"));
            }
            Ok(target)
        }
    }

    /// # Safety
    ///
    /// A GL context must be current.
    unsafe fn delete(self, gl: &glow::Context) {
        unsafe {
            gl.delete_framebuffer(self.framebuffer);
            gl.delete_texture(self.texture);
        }
    }
}

/// mpv's OpenGL renderer and the two contexts its callbacks borrow.
struct RenderContext {
    ptr: *mut ffi::mpv_render_context,
    get_proc_address: *mut GetProcAddress,
    repaint: *mut egui::Context,
    /// Keeps mpv alive until the renderer is freed, as libmpv requires.
    _mpv: Arc<Handle>,
}

impl RenderContext {
    /// # Safety
    ///
    /// The GL context the video is drawn with must be current.
    unsafe fn new(
        mpv: &Arc<Handle>,
        get_proc_address: &GetProcAddress,
        display: NativeDisplay,
        repaint: &egui::Context,
    ) -> Result<Self, Error> {
        let proc_context = Box::into_raw(Box::new(Arc::clone(get_proc_address)));
        let mut init = ffi::mpv_opengl_init_params {
            get_proc_address: Some(lookup),
            get_proc_address_ctx: proc_context.cast(),
        };
        let mut params = vec![
            ffi::mpv_render_param {
                type_: ffi::MPV_RENDER_PARAM_API_TYPE,
                data: ffi::MPV_RENDER_API_TYPE_OPENGL.as_ptr() as *mut c_void,
            },
            ffi::mpv_render_param {
                type_: ffi::MPV_RENDER_PARAM_OPENGL_INIT_PARAMS,
                data: (&raw mut init).cast(),
            },
        ];
        match display {
            NativeDisplay::Wayland(display) => params.push(ffi::mpv_render_param {
                type_: ffi::MPV_RENDER_PARAM_WL_DISPLAY,
                data: display,
            }),
            NativeDisplay::X11(display) => params.push(ffi::mpv_render_param {
                type_: ffi::MPV_RENDER_PARAM_X11_DISPLAY,
                data: display,
            }),
            NativeDisplay::None => {}
        }
        params.push(ffi::mpv_render_param {
            type_: ffi::MPV_RENDER_PARAM_INVALID,
            data: std::ptr::null_mut(),
        });

        let mut ptr = std::ptr::null_mut();
        // SAFETY: a valid handle and a terminated parameter list whose
        // pointers outlive the call; the caller has the context current.
        let code =
            unsafe { ffi::mpv_render_context_create(&mut ptr, mpv.ptr(), params.as_mut_ptr()) };
        if code < 0 {
            // SAFETY: mpv did not keep the pointer when creation failed.
            drop(unsafe { Box::from_raw(proc_context) });
            return Err(Error::new(code, "creating the OpenGL renderer"));
        }
        let repaint = Box::into_raw(Box::new(repaint.clone()));
        // SAFETY: the callback only asks egui for a repaint; free() removes
        // it before the context is dropped.
        unsafe {
            ffi::mpv_render_context_set_update_callback(ptr, Some(frame_ready), repaint.cast())
        };
        Ok(Self {
            ptr,
            get_proc_address: proc_context,
            repaint,
            _mpv: Arc::clone(mpv),
        })
    }

    /// Draws the current video frame into `target`, letterboxed to its size.
    ///
    /// # Safety
    ///
    /// The GL context must be current.
    unsafe fn render(&self, target: &Target) -> Result<(), Error> {
        let mut fbo = ffi::mpv_opengl_fbo {
            fbo: target.framebuffer.0.get() as c_int,
            w: target.width,
            h: target.height,
            internal_format: glow::RGBA8 as c_int,
        };
        // The framebuffer is blitted 1:1 onto egui's target, which has
        // OpenGL's bottom-up rows like a default framebuffer: mpv flips for
        // that. If the picture ever shows upside down, this is the switch.
        let mut flip_y: c_int = 1;
        let mut params = [
            ffi::mpv_render_param {
                type_: ffi::MPV_RENDER_PARAM_OPENGL_FBO,
                data: (&raw mut fbo).cast(),
            },
            ffi::mpv_render_param {
                type_: ffi::MPV_RENDER_PARAM_FLIP_Y,
                data: (&raw mut flip_y).cast(),
            },
            ffi::mpv_render_param {
                type_: ffi::MPV_RENDER_PARAM_INVALID,
                data: std::ptr::null_mut(),
            },
        ];
        // SAFETY: valid render context and parameters; context current.
        let code = unsafe { ffi::mpv_render_context_render(self.ptr, params.as_mut_ptr()) };
        if code < 0 {
            Err(Error::new(code, "rendering a frame"))
        } else {
            Ok(())
        }
    }

    /// The frame mpv is about to render, if this libmpv answers.
    ///
    /// # Safety
    ///
    /// The render context must be valid. Call it from the paint callback.
    unsafe fn next_frame_info(&self) -> Option<ffi::mpv_render_frame_info> {
        let mut info = ffi::mpv_render_frame_info::default();
        let param = ffi::mpv_render_param {
            type_: ffi::MPV_RENDER_PARAM_NEXT_FRAME_INFO,
            data: (&raw mut info).cast(),
        };
        // SAFETY: a valid render context; the call copies the queued frame's
        // description into `info`.
        let code = unsafe { ffi::mpv_render_context_get_info(self.ptr, param) };
        (code >= 0).then_some(info)
    }

    /// # Safety
    ///
    /// The GL context must be current.
    unsafe fn free(self) {
        unsafe {
            ffi::mpv_render_context_set_update_callback(self.ptr, None, std::ptr::null_mut());
            ffi::mpv_render_context_free(self.ptr);
            drop(Box::from_raw(self.repaint));
            drop(Box::from_raw(self.get_proc_address));
        }
        // `_mpv` drops here, after the renderer, as libmpv requires.
    }
}

/// mpv's OpenGL function lookup, answered by eframe's.
unsafe extern "C" fn lookup(context: *mut c_void, name: *const c_char) -> *mut c_void {
    // SAFETY: the context is the boxed GetProcAddress RenderContext::new
    // registered, and mpv passes a NUL-terminated name.
    let (get_proc_address, name) =
        unsafe { (&*(context as *const GetProcAddress), CStr::from_ptr(name)) };
    get_proc_address(name) as *mut c_void
}

/// mpv's update callback: a new frame is due. It may run on any thread and
/// must not call back into mpv.
unsafe extern "C" fn frame_ready(context: *mut c_void) {
    // SAFETY: the pointer is the boxed context RenderContext::new registered.
    let context = unsafe { &*(context as *const egui::Context) };
    context.request_repaint();
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Rect, pos2, vec2};

    #[test]
    fn scrolling_clips_the_picture_without_resizing_the_video_framebuffer() {
        for scale in [1.0, 1.25, 2.0] {
            for top in [100.0, 0.0, -150.0, -400.0] {
                let info = egui::PaintCallbackInfo {
                    viewport: Rect::from_min_size(pos2(100.0, top), vec2(640.0, 360.0)),
                    clip_rect: Rect::from_min_max(pos2(80.0, 80.0), pos2(900.0, 700.0)),
                    pixels_per_point: scale,
                    screen_size_px: [(1000.0 * scale) as u32, (800.0 * scale) as u32],
                };
                let viewport = video_viewport(&info);
                assert_eq!(viewport.width_px, (640.0 * scale) as i32);
                assert_eq!(viewport.height_px, (360.0 * scale) as i32);
                assert_eq!(viewport.top_px, (top * scale).round() as i32);
                assert_eq!(
                    viewport.from_bottom_px,
                    ((800.0 - top - 360.0) * scale).round() as i32
                );
                // The toolbar remains outside the scissor, even if the video
                // itself has a negative top or is completely offscreen.
                assert_eq!(info.clip_rect_in_pixels().top_px, (80.0 * scale) as i32);
            }
        }
    }

    #[test]
    fn only_a_frame_rendered_after_the_new_file_loads_is_shown() {
        let mut frame = Frame::default();
        frame.allow();
        frame.rendered(true);
        assert!(frame.visible());

        frame.forget();
        assert!(frame.discard_target());
        assert!(!frame.discard_target());
        // mpv can still deliver a frame of the previous file after loadfile.
        frame.rendered(true);
        assert!(
            !frame.visible(),
            "a frame from the old file must stay hidden"
        );

        // The new file loaded, but only its own frame may be shown: a resize
        // render before that frame does not count.
        frame.allow();
        frame.rendered(false);
        assert!(!frame.visible());
        frame.rendered(true);
        assert!(frame.visible());

        // A resize while the old picture is still in the target must not
        // revive it.
        frame.forget();
        frame.rendered(false);
        assert!(!frame.visible());
    }

    #[test]
    fn a_redraw_request_does_not_count_as_the_new_videos_first_frame() {
        use ffi::{
            MPV_RENDER_FRAME_INFO_PRESENT, MPV_RENDER_FRAME_INFO_REDRAW,
            MPV_RENDER_FRAME_INFO_REPEAT,
        };
        assert!(new_picture(MPV_RENDER_FRAME_INFO_PRESENT));
        assert!(new_picture(
            MPV_RENDER_FRAME_INFO_PRESENT | MPV_RENDER_FRAME_INFO_REPEAT
        ));
        assert!(!new_picture(
            MPV_RENDER_FRAME_INFO_PRESENT | MPV_RENDER_FRAME_INFO_REDRAW
        ));
        assert!(!new_picture(0));
    }
}
