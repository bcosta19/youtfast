//! libmpv: the player behind every video.
//!
//! [`Mpv`] owns the player and its state as the interface sees it; the
//! interface thread is the only one that reads its events. mpv resolves
//! YouTube links itself through its ytdl hook (yt-dlp), picks DASH streams,
//! decodes them (in hardware where it can) and keeps sound and picture in
//! step. [`VideoSurface`] draws its picture inside the egui window.

pub mod ffi;
mod render;

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::fmt;
use std::sync::Arc;

pub use render::{NativeDisplay, VideoSurface};

/// The streams mpv asks yt-dlp for: the best video with the best sound, or
/// the best single file when a video has no separate streams. "Best" is
/// bounded by the format sort [`Mpv::set_max_resolution`] sets.
const FORMAT: &str = "bv*+ba/b";

/// Observed properties, by the id mpv hands back with each change.
const TIME_POS: u64 = 1;
const DURATION: u64 = 2;
const PAUSE: u64 = 3;
const BUFFERING: u64 = 4;
const MEDIA_TITLE: u64 = 5;
const VOLUME: u64 = 6;
const IDLE: u64 = 7;
const CACHED_UNTIL: u64 = 8;
const VIDEO_WIDTH: u64 = 9;
const VIDEO_HEIGHT: u64 = 10;
const FPS: u64 = 11;
const CODEC: u64 = 12;
const HWDEC: u64 = 13;

#[derive(Debug)]
pub struct Error {
    code: c_int,
    during: &'static str,
}

impl Error {
    fn new(code: c_int, during: &'static str) -> Self {
        Self { code, during }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.code == 0 {
            // Not an mpv error code: the failure has no further detail.
            return write!(f, "{}", self.during);
        }
        // SAFETY: mpv_error_string returns a static string for any code.
        let text = unsafe { CStr::from_ptr(ffi::mpv_error_string(self.code)) };
        write!(f, "{}: {}", self.during, text.to_string_lossy())
    }
}

impl std::error::Error for Error {}

fn check(code: c_int, during: &'static str) -> Result<(), Error> {
    if code < 0 {
        Err(Error::new(code, during))
    } else {
        Ok(())
    }
}

/// The raw handle, shared with the render context so mpv outlives it.
pub struct Handle(*mut ffi::mpv_handle);

// SAFETY: the libmpv client API is thread-safe; the handle is an opaque
// pointer that is only freed in Drop, after every other owner is gone.
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

impl Handle {
    fn ptr(&self) -> *mut ffi::mpv_handle {
        self.0
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle came from mpv_create and is destroyed once.
        unsafe { ffi::mpv_terminate_destroy(self.0) };
    }
}

/// What the interface shows about the current video.
#[derive(Clone, Debug, Default)]
pub struct Status {
    /// Seconds played.
    pub position: f64,
    /// Length in seconds, or zero until mpv knows it (and for live streams).
    pub duration: f64,
    /// Seconds of the video already downloaded ahead.
    pub cached_until: f64,
    pub paused: bool,
    /// Waiting for the network, at the start or after a seek.
    pub buffering: bool,
    /// Nothing is loaded.
    pub idle: bool,
    pub title: Option<String>,
    /// 0 to 100.
    pub volume: f64,
    /// The decoded picture's size in pixels, or zero before the first frame.
    pub video_width: i64,
    pub video_height: i64,
    /// Frames per second the file declares, or zero when unknown.
    pub fps: f64,
    /// The video codec, as mpv names it: "vp9", "av1", "h264".
    pub codec: Option<String>,
    /// The hardware decoder in use ("vaapi", "nvdec"), or `None` while the
    /// CPU decodes.
    pub hwdec: Option<String>,
}

/// Something the app acts on after [`Mpv::poll`].
#[derive(Debug, PartialEq)]
pub enum Event {
    /// A video started playing.
    Loaded,
    /// The video reached its end.
    Finished,
    /// The video could not be opened or stopped with an error.
    Failed(Failure),
}

/// Why a video could not play, as far as yt-dlp's error tells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// YouTube asked to sign in to prove this is not a bot: it has
    /// rate-limited this address.
    BotCheck,
    /// The video is age-restricted and needs a signed-in session.
    AgeRestricted,
    /// Private, removed, or not available here.
    Unavailable,
    /// Anything else, an outdated yt-dlp included.
    Other,
}

impl Failure {
    /// Reads a line yt-dlp wrote through mpv's ytdl hook; `None` when it is
    /// not an error.
    fn from_ytdl(line: &str) -> Option<Self> {
        if !line.contains("ERROR:") {
            return None;
        }
        Some(if line.contains("not a bot") {
            Self::BotCheck
        } else if line.contains("confirm your age") {
            Self::AgeRestricted
        } else if line.contains("Private video") || line.contains("Video unavailable") {
            Self::Unavailable
        } else {
            Self::Other
        })
    }
}

pub struct Mpv {
    handle: Arc<Handle>,
    status: Status,
    /// The egui context mpv's wakeup callback repaints; freed in Drop.
    wakeup: *mut egui::Context,
    /// yt-dlp's last error, until the load it belongs to ends.
    ytdl_failure: Option<Failure>,
    /// What [`Mpv::set_max_resolution`] last set.
    max_resolution: u32,
    /// yt-dlp's `--cookies-from-browser`, when an account's browser is
    /// known: videos then play signed in.
    cookies_from_browser: Option<String>,
}

impl Mpv {
    /// Starts mpv with its video output set to the render API, so nothing
    /// shows until a [`VideoSurface`] draws it.
    /// `max_resolution` bounds the streams picked, as
    /// [`Mpv::set_max_resolution`] does; `volume` is where it starts.
    pub fn new(repaint: &egui::Context, max_resolution: u32, volume: f64) -> Result<Self, Error> {
        // SAFETY: plain constructor. It returns null when the locale's
        // LC_NUMERIC is not "C"; Rust programs keep "C" unless something
        // calls setlocale, which YouTFast does not.
        let ptr = unsafe { ffi::mpv_create() };
        if ptr.is_null() {
            return Err(Error::new(0, "creating mpv (is LC_NUMERIC \"C\"?)"));
        }
        let handle = Arc::new(Handle(ptr));
        let format_sort = format_sort(max_resolution);
        let volume = volume.clamp(0.0, 100.0);
        let initial_volume = format!("{volume:.0}");
        let options: [(&CStr, &str); 14] = [
            (c"vo", "libmpv"),
            (c"hwdec", "auto-safe"),
            (c"ytdl", "yes"),
            (c"ytdl-format", FORMAT),
            (c"ytdl-raw-options", &format_sort),
            // Stay on the last frame at the end instead of unloading.
            (c"keep-open", "yes"),
            (c"idle", "yes"),
            (c"osc", "no"),
            (c"input-default-bindings", "no"),
            (c"terminal", "no"),
            // The name PipeWire and PulseAudio show in their mixers.
            (c"audio-client-name", "YouTFast"),
            // mpv keeps 150 MiB ahead and 50 MiB behind by default; at
            // 1080p that is far more than smooth playback needs.
            (c"demuxer-max-bytes", "50MiB"),
            (c"demuxer-max-back-bytes", "20MiB"),
            (c"volume", &initial_volume),
        ];
        for (name, value) in options {
            let value = CString::new(value).expect("option values have no NUL");
            // SAFETY: valid handle and NUL-terminated strings.
            let code =
                unsafe { ffi::mpv_set_option_string(handle.ptr(), name.as_ptr(), value.as_ptr()) };
            if code < 0 {
                // Builds without Lua have no osc option, for example. None
                // of these is worth refusing to start over.
                log::warn!(
                    "mpv rejected option {}: {}",
                    name.to_string_lossy(),
                    Error::new(code, "setting an option")
                );
            }
        }
        // SAFETY: valid handle, options set, not yet initialized.
        check(unsafe { ffi::mpv_initialize(handle.ptr()) }, "starting mpv")?;

        let observed: [(u64, &CStr, ffi::mpv_format); 13] = [
            (TIME_POS, c"time-pos", ffi::MPV_FORMAT_DOUBLE),
            (DURATION, c"duration", ffi::MPV_FORMAT_DOUBLE),
            (PAUSE, c"pause", ffi::MPV_FORMAT_FLAG),
            (BUFFERING, c"paused-for-cache", ffi::MPV_FORMAT_FLAG),
            (MEDIA_TITLE, c"media-title", ffi::MPV_FORMAT_STRING),
            (VOLUME, c"volume", ffi::MPV_FORMAT_DOUBLE),
            (IDLE, c"idle-active", ffi::MPV_FORMAT_FLAG),
            (CACHED_UNTIL, c"demuxer-cache-time", ffi::MPV_FORMAT_DOUBLE),
            (VIDEO_WIDTH, c"video-params/w", ffi::MPV_FORMAT_INT64),
            (VIDEO_HEIGHT, c"video-params/h", ffi::MPV_FORMAT_INT64),
            (FPS, c"container-fps", ffi::MPV_FORMAT_DOUBLE),
            (CODEC, c"video-format", ffi::MPV_FORMAT_STRING),
            (HWDEC, c"hwdec-current", ffi::MPV_FORMAT_STRING),
        ];
        for (id, name, format) in observed {
            // SAFETY: valid handle and a static property name.
            let code =
                unsafe { ffi::mpv_observe_property(handle.ptr(), id, name.as_ptr(), format) };
            check(code, "observing a property")?;
        }
        // SAFETY: valid handle; the level is a static string.
        unsafe { ffi::mpv_request_log_messages(handle.ptr(), c"warn".as_ptr()) };

        let wakeup = Box::into_raw(Box::new(repaint.clone()));
        // SAFETY: the callback only asks egui for a repaint, and the context
        // outlives it: Drop removes the callback before freeing it.
        unsafe { ffi::mpv_set_wakeup_callback(handle.ptr(), Some(wake), wakeup.cast()) };

        Ok(Self {
            handle,
            status: Status {
                idle: true,
                volume,
                ..Status::default()
            },
            wakeup,
            ytdl_failure: None,
            max_resolution,
            cookies_from_browser: None,
        })
    }

    pub fn handle(&self) -> Arc<Handle> {
        Arc::clone(&self.handle)
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    /// Opens a YouTube link (or anything else mpv plays) in place of the
    /// current video, from `start` seconds when given (where it was left).
    pub fn load(&self, url: &str, start: Option<f64>) -> Result<(), Error> {
        match start {
            // The index argument (-1, ignored by "replace") came in mpv
            // 0.38; per-file options follow it.
            Some(start) => self.command(&[
                "loadfile",
                url,
                "replace",
                "-1",
                &format!("start={start:.3}"),
            ])?,
            None => self.command(&["loadfile", url, "replace"])?,
        }
        self.set_paused(false)
    }

    /// Opens `url` again at `position`, keeping it paused or playing: how a
    /// new quality takes effect. mpv asks yt-dlp for the streams again.
    pub fn reload(&self, url: &str, position: f64, paused: bool) -> Result<(), Error> {
        let options = format!(
            "start={position:.3},pause={}",
            if paused { "yes" } else { "no" }
        );
        // The index argument (-1, ignored by "replace") came in mpv 0.38;
        // per-file options follow it.
        self.command(&["loadfile", url, "replace", "-1", &options])
    }

    /// Bounds the streams of the next video loaded to `resolution` (720 for
    /// 720p), by its smaller side, so a vertical video is judged like
    /// YouTube labels it. Below that, yt-dlp picks the best.
    pub fn set_max_resolution(&mut self, resolution: u32) -> Result<(), Error> {
        self.max_resolution = resolution;
        self.set_raw_options()
    }

    /// Plays signed in as this browser's YouTube session, read by yt-dlp
    /// itself (`browser[+keyring][:profile]`); `None` plays signed out.
    /// The session never passes through a file.
    pub fn set_cookies_from_browser(&mut self, browser: Option<String>) -> Result<(), Error> {
        self.cookies_from_browser = browser;
        self.set_raw_options()
    }

    /// yt-dlp's options, from the two above. The browser goes in alone
    /// with `append`, so a comma in a profile's name cannot split it.
    fn set_raw_options(&self) -> Result<(), Error> {
        self.set_property("ytdl-raw-options", &format_sort(self.max_resolution))?;
        if let Some(browser) = &self.cookies_from_browser {
            let option = format!("cookies-from-browser={browser}");
            self.command(&["change-list", "ytdl-raw-options", "append", &option])?;
        }
        Ok(())
    }

    /// yt-dlp's answer for the current video, as mpv's ytdl hook received
    /// it: the video's metadata, with every format it offers. Available
    /// from [`Event::Loaded`] until the video is replaced.
    pub fn ytdl_json(&self) -> Option<String> {
        if let Some(stdout) = self.get_property("user-data/mpv/ytdl/json-subprocess-result/stdout")
        {
            return Some(stdout);
        }
        // Read as a whole, the result is JSON with yt-dlp's output inside.
        let result = self.get_property("user-data/mpv/ytdl/json-subprocess-result")?;
        let result: serde_json::Value = serde_json::from_str(&result).ok()?;
        result.get("stdout")?.as_str().map(str::to_owned)
    }

    fn get_property(&self, name: &str) -> Option<String> {
        let name = CString::new(name).expect("property names have no NUL");
        // SAFETY: valid handle and name; the result is mpv's to free.
        unsafe {
            let value = ffi::mpv_get_property_string(self.handle.ptr(), name.as_ptr());
            if value.is_null() {
                return None;
            }
            let text = CStr::from_ptr(value).to_string_lossy().into_owned();
            ffi::mpv_free(value.cast());
            Some(text)
        }
    }

    pub fn set_paused(&self, paused: bool) -> Result<(), Error> {
        self.set_property("pause", if paused { "yes" } else { "no" })
    }

    /// Seeks to `seconds` from the start. `exact` is for a released seek
    /// bar; while it is dragged, keyframes answer faster.
    pub fn seek_to(&self, seconds: f64, exact: bool) -> Result<(), Error> {
        let flags = if exact {
            "absolute+exact"
        } else {
            "absolute+keyframes"
        };
        self.command(&["seek", &format!("{seconds:.3}"), flags])
    }

    pub fn seek_by(&self, seconds: f64) -> Result<(), Error> {
        self.command(&["seek", &format!("{seconds:.3}"), "relative"])
    }

    pub fn set_volume(&self, volume: f64) -> Result<(), Error> {
        self.set_property("volume", &format!("{:.0}", volume.clamp(0.0, 100.0)))
    }

    fn command(&self, args: &[&str]) -> Result<(), Error> {
        let owned: Vec<CString> = args
            .iter()
            .map(|arg| CString::new(arg.replace('\0', "")).expect("NUL removed"))
            .collect();
        let mut pointers: Vec<*const c_char> = owned.iter().map(|arg| arg.as_ptr()).collect();
        pointers.push(std::ptr::null());
        // SAFETY: a NULL-terminated array of strings that outlive the call.
        check(
            unsafe { ffi::mpv_command(self.handle.ptr(), pointers.as_mut_ptr()) },
            "running a command",
        )
    }

    fn set_property(&self, name: &str, value: &str) -> Result<(), Error> {
        let name = CString::new(name).expect("property names have no NUL");
        let value = CString::new(value.replace('\0', "")).expect("NUL removed");
        // SAFETY: valid handle and NUL-terminated strings.
        check(
            unsafe {
                ffi::mpv_set_property_string(self.handle.ptr(), name.as_ptr(), value.as_ptr())
            },
            "setting a property",
        )
    }

    /// Reads every pending event without waiting. Call once per frame on
    /// the interface thread; the wakeup callback asks for that frame.
    pub fn poll(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        loop {
            // SAFETY: valid handle; a zero timeout never blocks, and the
            // event stays valid until the next mpv_wait_event call.
            let event = unsafe { &*ffi::mpv_wait_event(self.handle.ptr(), 0.0) };
            match event.event_id {
                ffi::MPV_EVENT_NONE => break,
                ffi::MPV_EVENT_PROPERTY_CHANGE => {
                    // SAFETY: mpv documents data as mpv_event_property here.
                    let property = unsafe { &*(event.data as *const ffi::mpv_event_property) };
                    // SAFETY: data matches the format mpv reports.
                    unsafe { self.apply_property(event.reply_userdata, property) };
                }
                ffi::MPV_EVENT_FILE_LOADED => {
                    self.ytdl_failure = None;
                    events.push(Event::Loaded);
                }
                ffi::MPV_EVENT_END_FILE => {
                    // SAFETY: mpv documents data as mpv_event_end_file here.
                    let end = unsafe { &*(event.data as *const ffi::mpv_event_end_file) };
                    match end.reason {
                        ffi::MPV_END_FILE_REASON_EOF => events.push(Event::Finished),
                        ffi::MPV_END_FILE_REASON_ERROR => {
                            log::warn!("mpv could not play: {}", Error::new(end.error, "playback"));
                            let failure = self.ytdl_failure.take().unwrap_or(Failure::Other);
                            events.push(Event::Failed(failure));
                        }
                        _ => self.ytdl_failure = None,
                    }
                }
                ffi::MPV_EVENT_LOG_MESSAGE => {
                    // SAFETY: mpv documents data as mpv_event_log_message here.
                    let message = unsafe { &*(event.data as *const ffi::mpv_event_log_message) };
                    // SAFETY: both strings are valid for the event's lifetime.
                    let (prefix, text) = unsafe {
                        (
                            CStr::from_ptr(message.prefix).to_string_lossy(),
                            CStr::from_ptr(message.text).to_string_lossy(),
                        )
                    };
                    log::warn!(target: "mpv", "[{prefix}] {}", text.trim_end());
                    if prefix == "ytdl_hook"
                        && let Some(failure) = Failure::from_ytdl(&text)
                    {
                        self.ytdl_failure = Some(failure);
                    }
                }
                ffi::MPV_EVENT_SHUTDOWN => break,
                _ => {}
            }
        }
        events
    }

    /// # Safety
    ///
    /// `property.data` must point at a value of `property.format`, as mpv
    /// guarantees for a property-change event.
    unsafe fn apply_property(&mut self, id: u64, property: &ffi::mpv_event_property) {
        // SAFETY (all four): the caller guarantees data matches format, and
        // each read checks the format first. MPV_FORMAT_NONE means the
        // property is unavailable, as duration is before a file loads.
        let double = (property.format == ffi::MPV_FORMAT_DOUBLE)
            .then(|| unsafe { *(property.data as *const f64) });
        let flag = (property.format == ffi::MPV_FORMAT_FLAG)
            .then(|| unsafe { *(property.data as *const c_int) != 0 });
        let integer = (property.format == ffi::MPV_FORMAT_INT64)
            .then(|| unsafe { *(property.data as *const i64) });
        let string = (property.format == ffi::MPV_FORMAT_STRING).then(|| {
            unsafe { CStr::from_ptr(*(property.data as *const *const c_char)) }
                .to_string_lossy()
                .into_owned()
        });
        let status = &mut self.status;
        match id {
            TIME_POS => status.position = double.unwrap_or(0.0),
            DURATION => status.duration = double.unwrap_or(0.0),
            CACHED_UNTIL => status.cached_until = double.unwrap_or(0.0),
            VOLUME => status.volume = double.unwrap_or(status.volume),
            PAUSE => status.paused = flag.unwrap_or(false),
            BUFFERING => status.buffering = flag.unwrap_or(false),
            IDLE => status.idle = flag.unwrap_or(true),
            MEDIA_TITLE => status.title = string,
            VIDEO_WIDTH => status.video_width = integer.unwrap_or(0),
            VIDEO_HEIGHT => status.video_height = integer.unwrap_or(0),
            FPS => status.fps = double.unwrap_or(0.0),
            CODEC => status.codec = string,
            // "no" (or nothing) while the CPU decodes.
            HWDEC => status.hwdec = string.filter(|name| !name.is_empty() && name != "no"),
            _ => {}
        }
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        // SAFETY: removing the callback first means mpv never calls it with
        // the context freed below.
        unsafe {
            ffi::mpv_set_wakeup_callback(self.handle.ptr(), None, std::ptr::null_mut());
            drop(Box::from_raw(self.wakeup));
        }
    }
}

/// yt-dlp's format sort for streams up to `resolution`: `res` is a format's
/// smaller side, and `res:1080` prefers the largest at or below 1080.
fn format_sort(resolution: u32) -> String {
    format!("format-sort=res:{resolution}")
}

/// mpv's wakeup callback: events are waiting, so ask for a frame to read
/// them. It may run on any thread and must not call back into mpv.
unsafe extern "C" fn wake(context: *mut c_void) {
    // SAFETY: the pointer is the boxed context Mpv::new registered.
    let context = unsafe { &*(context as *const egui::Context) };
    context.request_repaint();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_why_yt_dlp_failed() {
        let failure = |line| Failure::from_ytdl(line);
        assert_eq!(
            failure(
                "ERROR: [youtube] W0jPTqIo5oE: Sign in to confirm you\u{2019}re not a bot. Use --cookies-from-browser"
            ),
            Some(Failure::BotCheck)
        );
        assert_eq!(
            failure(
                "ERROR: [youtube] abc: Sign in to confirm your age. This video may be inappropriate"
            ),
            Some(Failure::AgeRestricted)
        );
        assert_eq!(
            failure("ERROR: [youtube] abc: Private video. Sign in if you've been granted access"),
            Some(Failure::Unavailable)
        );
        assert_eq!(
            failure("ERROR: [youtube] abc: Video unavailable"),
            Some(Failure::Unavailable)
        );
        assert_eq!(
            failure("ERROR: [youtube] abc: nsig extraction failed"),
            Some(Failure::Other)
        );
        assert_eq!(
            failure("WARNING: [youtube] abc: HTTP Error 429: Too Many Requests"),
            None
        );
    }
}
