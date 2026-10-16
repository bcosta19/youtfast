//! Adaptive comment animation: one decoder worker, small downscaled caches,
//! sequential playback for long clips, and one reused GPU texture per image.

use std::collections::{HashMap, VecDeque};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use egui::{ColorImage, Context, Id, Rect, TextureHandle, Ui};
use image::metadata::LoopCount;
use image::{AnimationDecoder, ImageDecoder};

const GLOBAL_BYTES: usize = 64 * 1024 * 1024;
const CACHE_BYTES: usize = 8 * 1024 * 1024;
const MAX_VISIBLE: usize = 2;
const MAX_PREPARE_FRAMES: usize = 512;
const ID: &str = "youtfast-comment-animation";

#[derive(Default)]
struct Budget(AtomicUsize);
struct Lease {
    budget: Arc<Budget>,
    bytes: usize,
}
impl Budget {
    fn reserve(self: &Arc<Self>, bytes: usize) -> Result<Lease, &'static str> {
        self.0
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                used.checked_add(bytes)
                    .filter(|total| *total <= GLOBAL_BYTES)
            })
            .map_err(|_| "animation: memory budget reached")?;
        Ok(Lease {
            budget: self.clone(),
            bytes,
        })
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.budget.0.fetch_sub(self.bytes, Ordering::Relaxed);
    }
}

#[derive(Clone)]
struct Frame {
    image: Arc<ColorImage>,
    delay: Duration,
    _lease: Arc<Lease>,
}
struct Clip {
    frames: VecDeque<Frame>,
    streaming: bool,
    loops: LoopCount,
    preview_error: Option<&'static str>,
}
#[derive(Default)]
struct Slot {
    prepared: Option<Result<Clip, &'static str>>,
    next: Option<Result<Option<Frame>, &'static str>>,
}
struct Stream {
    frames: image::Frames<'static>,
    bytes: Arc<[u8]>,
    loops: LoopCount,
    iteration: u32,
    cancel: Arc<AtomicBool>,
    budget: Arc<Budget>,
    _workspace: Lease,
    slow: bool,
}
impl Stream {
    fn new(
        bytes: Arc<[u8]>,
        cancel: Arc<AtomicBool>,
        budget: Arc<Budget>,
    ) -> Result<Self, &'static str> {
        if cancel.load(Ordering::Relaxed) {
            return Err("animation: worker unavailable");
        }
        if bytes.len() > 4 * 1024 * 1024 {
            return Err("image: response too large");
        }
        let (width, height) = image::ImageReader::new(Cursor::new(&bytes))
            .with_guessed_format()
            .map_err(|_| "animation: invalid data")?
            .into_dimensions()
            .map_err(|_| "animation: invalid data")?;
        let pixels = width as usize * height as usize;
        if width == 0 || height == 0 || width > 4096 || height > 4096 || pixels > 4_000_000 {
            return Err("animation: preview only");
        }
        // Reserve original canvas/composition scratch before constructing a
        // decoder, in addition to every retained RGBA frame and GPU texture.
        let workspace = budget.reserve(pixels * 4 * 3 + bytes.len())?;
        let (frames, loops) = decoder(bytes.clone())?;
        Ok(Self {
            frames,
            bytes,
            loops,
            iteration: 1,
            cancel,
            budget,
            _workspace: workspace,
            slow: false,
        })
    }
    fn frame(&mut self) -> Result<Option<Frame>, &'static str> {
        if self.cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        if self.slow {
            return Err("animation: decode budget reached");
        }
        let started = Instant::now();
        let result = self
            .frames
            .next()
            .transpose()
            .map_err(|_| "animation: invalid data")?
            .map(|frame| downscale(frame, &self.budget))
            .transpose();
        // A very expensive source retains a preview instead of monopolizing
        // the single worker indefinitely. Ordinary frame timing is unchanged.
        self.slow = started.elapsed() > Duration::from_millis(200);
        result
    }
    fn next(&mut self) -> Result<Option<Frame>, &'static str> {
        if let Some(frame) = self.frame()? {
            return Ok(Some(frame));
        }
        if self.cancel.load(Ordering::Relaxed)
            || matches!(self.loops, LoopCount::Finite(n) if self.iteration >= n.get())
        {
            return Ok(None);
        }
        self.iteration = self.iteration.saturating_add(1);
        self.frames = image::Frames::new(Box::new(std::iter::empty()));
        self.frames = decoder(self.bytes.clone())?.0;
        self.frame()
    }
}

fn decoder(bytes: Arc<[u8]>) -> Result<(image::Frames<'static>, LoopCount), &'static str> {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(48 * 1024 * 1024);
    if egui::has_gif_magic_header(&bytes) {
        let mut d = image::codecs::gif::GifDecoder::new(Cursor::new(bytes))
            .map_err(|_| "animation: invalid data")?;
        d.set_limits(limits)
            .map_err(|_| "animation: preview only")?;
        let loops = d.loop_count();
        Ok((d.into_frames(), loops))
    } else if egui::has_webp_header(&bytes) {
        let mut d = image::codecs::webp::WebPDecoder::new(Cursor::new(bytes))
            .map_err(|_| "animation: invalid data")?;
        d.set_limits(limits)
            .map_err(|_| "animation: preview only")?;
        let loops = d.loop_count();
        if d.has_animation() {
            d.set_background_color(image::Rgba([0; 4]))
                .map_err(|_| "animation: invalid data")?;
            Ok((d.into_frames(), loops))
        } else {
            let still = image::DynamicImage::from_decoder(d)
                .map_err(|_| "animation: invalid data")?
                .into_rgba8();
            Ok((
                image::Frames::new(Box::new(std::iter::once(Ok(image::Frame::new(still))))),
                LoopCount::Finite(1.try_into().unwrap()),
            ))
        }
    } else {
        Err("animation: invalid data")
    }
}
fn downscale(frame: image::Frame, budget: &Arc<Budget>) -> Result<Frame, &'static str> {
    let delay = Duration::from(frame.delay()).max(Duration::from_millis(20));
    let buffer = image::DynamicImage::ImageRgba8(frame.into_buffer())
        .thumbnail(320, 200)
        .into_rgba8();
    let lease = budget.reserve(buffer.len())?;
    let image = ColorImage::from_rgba_unmultiplied(
        [buffer.width() as usize, buffer.height() as usize],
        buffer.as_raw(),
    );
    Ok(Frame {
        image: Arc::new(image),
        delay,
        _lease: Arc::new(lease),
    })
}

fn prepare(stream: &mut Stream) -> Result<Clip, &'static str> {
    let mut frames = VecDeque::new();
    let mut bytes = 0;
    let started = Instant::now();
    loop {
        if stream.cancel.load(Ordering::Relaxed) {
            return Err("animation: worker unavailable");
        }
        let decoded = stream.frame();
        if let Err(reason) = decoded {
            if frames.is_empty() {
                return Err(reason);
            }
            frames.truncate(1);
            return Ok(Clip {
                frames,
                streaming: false,
                loops: stream.loops,
                preview_error: Some(reason),
            });
        }
        let Some(frame) = decoded.unwrap() else {
            if frames.is_empty() {
                return Err("animation: invalid data");
            }
            return Ok(Clip {
                frames,
                streaming: false,
                loops: stream.loops,
                preview_error: None,
            });
        };
        bytes += frame.image.pixels.len() * 4;
        frames.push_back(frame);
        if bytes >= CACHE_BYTES
            || frames.len() >= MAX_PREPARE_FRAMES
            || started.elapsed() >= Duration::from_millis(500)
        {
            return Ok(Clip {
                frames,
                streaming: true,
                loops: stream.loops,
                preview_error: None,
            });
        }
    }
}

enum Job {
    Prepare {
        id: u64,
        bytes: Arc<[u8]>,
        slot: Arc<Mutex<Slot>>,
        cancel: Arc<AtomicBool>,
        ctx: Context,
    },
    Next {
        id: u64,
        slot: Arc<Mutex<Slot>>,
        ctx: Context,
    },
    Forget(u64),
}
fn worker(rx: mpsc::Receiver<Job>, budget: Arc<Budget>) {
    // Non-Send decoders never leave this worker thread.
    let mut streams = HashMap::new();
    while let Ok(job) = rx.recv() {
        match job {
            Job::Prepare {
                id,
                bytes,
                slot,
                cancel,
                ctx,
            } => {
                let result =
                    Stream::new(bytes, cancel.clone(), budget.clone()).and_then(|mut stream| {
                        let clip = prepare(&mut stream)?;
                        if clip.streaming {
                            streams.insert(id, stream);
                        }
                        Ok(clip)
                    });
                if !cancel.load(Ordering::Relaxed) {
                    match &result {
                        Ok(clip) if clip.preview_error.is_some() => {
                            log::warn!("animation: preview only")
                        }
                        Ok(clip) if clip.streaming => log::debug!("animation: streaming"),
                        Ok(_) => log::debug!("animation: cached"),
                        Err(reason) => log::warn!("{reason}"),
                    }
                    slot.lock().unwrap().prepared = Some(result);
                    ctx.request_repaint();
                }
            }
            Job::Next { id, slot, ctx } => {
                if let Some(stream) = streams.get_mut(&id) {
                    let result = stream.next();
                    if let Err(reason) = &result {
                        log::warn!("{reason}");
                    }
                    let done = !matches!(result, Ok(Some(_)));
                    slot.lock().unwrap().next = Some(result);
                    if done {
                        streams.remove(&id);
                    }
                    ctx.request_repaint();
                }
            }
            Job::Forget(id) => {
                streams.remove(&id);
            }
        }
        streams.retain(|_, stream| !stream.cancel.load(Ordering::Relaxed));
    }
}

struct Entry {
    id: u64,
    slot: Arc<Mutex<Slot>>,
    cancel: Arc<AtomicBool>,
    seen: u64,
    clip: Option<Clip>,
    error: Option<&'static str>,
    texture: Option<TextureHandle>,
    texture_lease: Option<Lease>,
    current: Option<Frame>,
    at: f64,
    index: usize,
    iteration: u32,
    paused: bool,
    pending: bool,
    ended: bool,
    repair_reported: bool,
}
impl Drop for Entry {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
struct Manager {
    tx: mpsc::Sender<Job>,
    budget: Arc<Budget>,
    entries: HashMap<String, Entry>,
    serial: u64,
    deferred: HashMap<String, u64>,
}

fn manager(ctx: &Context) -> Arc<Mutex<Manager>> {
    ctx.data_mut(|data| data.get_temp::<Arc<Mutex<Manager>>>(Id::new(ID)).unwrap())
}
pub fn install(ctx: &Context) {
    if ctx.data(|data| data.get_temp::<Arc<Mutex<Manager>>>(Id::new(ID)).is_some()) {
        return;
    }
    let (tx, rx) = mpsc::channel();
    let budget = Arc::new(Budget::default());
    let task_budget = budget.clone();
    let result = std::thread::Builder::new()
        .name("comment-animation".into())
        .spawn(move || worker(rx, task_budget));
    if result.is_err() {
        log::warn!("animation: worker unavailable");
    }
    ctx.data_mut(|data| {
        data.insert_temp(
            Id::new(ID),
            Arc::new(Mutex::new(Manager {
                tx,
                budget,
                entries: HashMap::new(),
                serial: 0,
                deferred: HashMap::new(),
            })),
        )
    });
}

/// Reclaim animations as soon as they leave the viewport. Sending Forget wakes
/// the idle worker too; orphaned work can never repopulate the manager.
pub fn sweep(ctx: &Context) {
    let manager = manager(ctx);
    let mut m = manager.lock().unwrap();
    let frame = ctx.cumulative_frame_nr();
    m.deferred.retain(|_, seen| *seen + 1 >= frame);
    let stale: Vec<_> = m
        .entries
        .iter()
        .filter(|(_, e)| e.seen + 1 < frame)
        .map(|(url, _)| url.clone())
        .collect();
    for url in stale {
        if let Some(e) = m.entries.remove(&url) {
            let _ = m.tx.send(Job::Forget(e.id));
        }
    }
}
pub fn forget(ctx: &Context, uri: &str) {
    let manager = manager(ctx);
    let mut m = manager.lock().unwrap();
    if let Some(e) = m.entries.remove(uri) {
        let _ = m.tx.send(Job::Forget(e.id));
    }
}

pub fn show(ui: &mut Ui, uri: &str, bytes: Arc<[u8]>, rect: Rect) -> Option<bool> {
    let manager = manager(ui.ctx());
    let mut m = manager.lock().unwrap();
    if !m.entries.contains_key(uri) {
        if m.entries.len() >= MAX_VISIBLE || m.deferred.contains_key(uri) {
            m.deferred
                .insert(uri.to_owned(), ui.ctx().cumulative_frame_nr());
            if !ui
                .put(
                    rect,
                    egui::Button::new("Play GIF · pause another animation"),
                )
                .clicked()
            {
                return None;
            }
            m.deferred.remove(uri);
            if m.entries.len() >= MAX_VISIBLE {
                let previous = m
                    .entries
                    .iter()
                    .min_by_key(|(_, e)| e.seen)
                    .map(|(uri, _)| uri.clone())
                    .unwrap();
                if let Some(e) = m.entries.remove(&previous) {
                    let _ = m.tx.send(Job::Forget(e.id));
                }
                m.deferred.insert(previous, ui.ctx().cumulative_frame_nr());
            }
        }
        m.serial += 1;
        let id = m.serial;
        let slot = Arc::new(Mutex::new(Slot::default()));
        let cancel = Arc::new(AtomicBool::new(false));
        let failed =
            m.tx.send(Job::Prepare {
                id,
                bytes,
                slot: slot.clone(),
                cancel: cancel.clone(),
                ctx: ui.ctx().clone(),
            })
            .is_err();
        m.entries.insert(
            uri.to_owned(),
            Entry {
                id,
                slot,
                cancel,
                seen: 0,
                clip: None,
                error: failed.then_some("animation: worker unavailable"),
                texture: None,
                texture_lease: None,
                current: None,
                at: 0.0,
                index: 0,
                iteration: 1,
                paused: false,
                pending: false,
                ended: false,
                repair_reported: false,
            },
        );
    }
    let Manager {
        entries,
        budget,
        tx,
        ..
    } = &mut *m;
    let e = entries.get_mut(uri).unwrap();
    e.seen = ui.ctx().cumulative_frame_nr();
    let now = ui.input(|i| i.time);
    let prepared = e.slot.lock().unwrap().prepared.take();
    if let Some(prepared) = prepared {
        match prepared {
            Ok(clip) => {
                e.clip = Some(clip);
                e.at = now;
            }
            Err(reason) => e.error = Some(reason),
        }
    }
    let mut next = None;
    if e.error.is_none()
        && (e.current.is_none() || (!e.paused && !e.ended && now >= e.at))
        && let Some(clip) = &mut e.clip
    {
        if clip.streaming {
            if let Some(frame) = clip.frames.pop_front() {
                next = Some(frame);
            } else {
                let incoming = e.slot.lock().unwrap().next.take();
                if let Some(incoming) = incoming {
                    e.pending = false;
                    match incoming {
                        Ok(Some(frame)) => next = Some(frame),
                        Ok(None) => e.ended = true,
                        Err(reason) => {
                            e.error = Some(reason);
                            e.paused = true;
                        }
                    }
                } else if !e.pending {
                    e.pending = true;
                    if tx
                        .send(Job::Next {
                            id: e.id,
                            slot: e.slot.clone(),
                            ctx: ui.ctx().clone(),
                        })
                        .is_err()
                    {
                        e.error = Some("animation: worker unavailable");
                    }
                }
            }
        } else if !clip.frames.is_empty() {
            if e.index == clip.frames.len() {
                if matches!(clip.loops, LoopCount::Finite(n) if e.iteration >= n.get())
                    || clip.frames.len() == 1
                {
                    e.ended = true;
                } else {
                    e.index = 0;
                    e.iteration = e.iteration.saturating_add(1);
                }
            }
            if !e.ended {
                next = Some(clip.frames[e.index].clone());
                e.index += 1;
            }
        }
    }
    if let Some(frame) = next {
        if let Some(texture) = &mut e.texture {
            texture.set(frame.image.clone(), egui::TextureOptions::LINEAR);
        } else {
            match budget.reserve(frame.image.pixels.len() * 4) {
                Ok(lease) => {
                    e.texture = Some(ui.ctx().load_texture(
                        ID,
                        frame.image.clone(),
                        egui::TextureOptions::LINEAR,
                    ));
                    e.texture_lease = Some(lease);
                }
                Err(reason) => {
                    e.error = Some(reason);
                    e.paused = true;
                }
            }
        }
        e.at = now + frame.delay.as_secs_f64();
        e.current = Some(frame);
        if let Some(reason) = e.clip.as_ref().and_then(|c| c.preview_error) {
            e.error = Some(reason);
            e.paused = true;
        }
    }
    let mut restart = false;
    let mut repair = false;
    if let Some(texture) = &e.texture {
        let size = texture.size_vec2();
        let area = Rect::from_min_max(rect.min, rect.max - egui::vec2(0.0, 24.0));
        let scale = (area.width() / size.x).min(area.height() / size.y);
        let target = Rect::from_center_size(area.center(), size * scale);
        ui.put(target, egui::Image::new(texture).corner_radius(8));
        let controls = Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - 24.0), rect.max);
        let label = if e.error.is_some() {
            "Preview only · Retry"
        } else if e.ended {
            "Replay GIF"
        } else if e.paused {
            "Play GIF"
        } else {
            "Pause GIF"
        };
        if ui.put(controls, egui::Button::new(label).small()).clicked() {
            if e.error.is_some() || e.ended {
                restart = true;
                repair = e.error.is_some();
            } else {
                e.paused = !e.paused;
                e.at = now;
            }
        }
        if !e.paused && !e.ended && e.error.is_none() {
            ui.ctx()
                .request_repaint_after(Duration::from_secs_f64((e.at - now).max(0.02)));
        }
    } else if e.error.is_some() {
        if ui
            .put(
                rect,
                egui::Button::new(if e.error == Some("animation: memory budget reached") {
                    "GIF paused to save memory · Play"
                } else {
                    "Image / GIF unavailable · Retry"
                }),
            )
            .clicked()
        {
            restart = true;
            repair = true;
        }
    } else {
        ui.put(rect, egui::Label::new("Loading GIF…"));
    }
    let memory_retry = restart && e.error == Some("animation: memory budget reached");
    let auto_repair = !e.repair_reported && e.error == Some("animation: invalid data");
    e.repair_reported |= auto_repair;
    if restart {
        let id = e.id;
        entries.remove(uri);
        let _ = tx.send(Job::Forget(id));
        ui.ctx().request_repaint();
    }
    let mut deferred = Vec::new();
    if memory_retry {
        for (uri, e) in entries.drain() {
            e.cancel.store(true, Ordering::Relaxed);
            let _ = tx.send(Job::Forget(e.id));
            deferred.push(uri);
        }
    }
    for uri in deferred {
        m.deferred.insert(uri, ui.ctx().cumulative_frame_nr());
    }
    if repair {
        return Some(false);
    }
    if !restart && auto_repair {
        return Some(true);
    }
    None
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn gif() -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
            for (color, delay) in [([255, 0, 0, 255], 100), ([0, 0, 255, 255], 200)] {
                encoder
                    .encode_frame(image::Frame::from_parts(
                        image::RgbaImage::from_pixel(2, 2, image::Rgba(color)),
                        0,
                        0,
                        image::Delay::from_numer_denom_ms(delay, 1),
                    ))
                    .unwrap();
            }
        }
        bytes
    }

    pub(crate) fn webp() -> Vec<u8> {
        fn chunk(bytes: &mut Vec<u8>, name: &[u8; 4], payload: &[u8]) {
            bytes.extend_from_slice(name);
            bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            bytes.extend_from_slice(payload);
            if payload.len() % 2 == 1 {
                bytes.push(0);
            }
        }
        let mut chunks = Vec::new();
        // A synthetic 2x2 animated WebP container with two lossless frames.
        chunk(&mut chunks, b"VP8X", &[2, 0, 0, 0, 1, 0, 0, 1, 0, 0]);
        chunk(&mut chunks, b"ANIM", &[0; 6]);
        for (color, delay) in [([255, 0, 0], 100), ([0, 0, 255], 200)] {
            let mut still = Vec::new();
            image::codecs::webp::WebPEncoder::new_lossless(&mut still)
                .encode(&color.repeat(4), 2, 2, image::ExtendedColorType::Rgb8)
                .unwrap();
            let mut frame = vec![0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, delay, 0, 0, 2];
            frame.extend_from_slice(&still[12..]);
            chunk(&mut chunks, b"ANMF", &frame);
        }
        let mut bytes = b"RIFF".to_vec();
        bytes.extend_from_slice(&((chunks.len() + 4) as u32).to_le_bytes());
        bytes.extend_from_slice(b"WEBP");
        bytes.extend_from_slice(&chunks);
        bytes
    }

    #[test]
    fn gif_webp_colors_delays_and_static_webp() {
        for bytes in [gif(), webp()] {
            let budget = Arc::new(Budget::default());
            let mut stream = Stream::new(
                bytes.into(),
                Arc::new(AtomicBool::new(false)),
                budget.clone(),
            )
            .unwrap();
            let clip = prepare(&mut stream).unwrap();
            assert!(!clip.streaming);
            assert_eq!(clip.frames.len(), 2);
            assert_eq!(clip.frames[0].image.pixels[0], egui::Color32::RED);
            assert_eq!(clip.frames[1].image.pixels[0], egui::Color32::BLUE);
            assert_eq!(clip.frames[0].delay, Duration::from_millis(100));
            assert_eq!(clip.frames[1].delay, Duration::from_millis(200));
            drop((stream, clip));
            assert_eq!(budget.0.load(Ordering::Relaxed), 0);
        }
        let mut bytes = Vec::new();
        image::codecs::webp::WebPEncoder::new_lossless(&mut bytes)
            .encode(&[255, 0, 0], 1, 1, image::ExtendedColorType::Rgb8)
            .unwrap();
        let mut stream = Stream::new(
            bytes.into(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(Budget::default()),
        )
        .unwrap();
        assert_eq!(prepare(&mut stream).unwrap().frames.len(), 1);
    }

    #[test]
    fn large_animation_streams_without_truncating_its_frames() {
        let budget = Arc::new(Budget::default());
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut stream = Stream::new(gif().into(), cancelled, budget.clone()).unwrap();
        // Model a 173-frame, 498x452 canvas (149 MiB if retained at source size).
        stream.frames = image::Frames::new(Box::new((0..173).map(|n| {
            Ok(image::Frame::from_parts(
                image::RgbaImage::from_pixel(498, 452, image::Rgba([n as u8, 0, 0, 255])),
                0,
                0,
                image::Delay::from_numer_denom_ms(50, 1),
            ))
        })));
        let clip = prepare(&mut stream).unwrap();
        assert!(clip.streaming);
        let mut count = clip.frames.len();
        drop(clip);
        while stream.frame().unwrap().is_some() {
            count += 1;
        }
        assert_eq!(count, 173);
        drop(stream);
        assert_eq!(budget.0.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn budget_is_global_and_cancellation_releases_memory() {
        let budget = Arc::new(Budget::default());
        let lease = budget.reserve(GLOBAL_BYTES - 16).unwrap();
        assert!(budget.reserve(17).is_err());
        drop(lease);
        let cancel = Arc::new(AtomicBool::new(false));
        let mut stream = Stream::new(gif().into(), cancel.clone(), budget.clone()).unwrap();
        cancel.store(true, Ordering::Relaxed);
        assert!(prepare(&mut stream).is_err());
        drop(stream);
        assert_eq!(budget.0.load(Ordering::Relaxed), 0);
        assert!(
            Stream::new(
                Arc::from(&b"GIF89a"[..]),
                Arc::new(AtomicBool::new(false)),
                budget
            )
            .is_err()
        );
    }
    fn draw(ctx: &Context, bytes: Arc<[u8]>, time: f64, events: Vec<egui::Event>) {
        let mut output = ctx.run_ui(
            egui::RawInput {
                time: Some(time),
                events,
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400.0, 260.0),
                )),
                ..Default::default()
            },
            |ui| {
                let rect = Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(320.0, 200.0));
                show(ui, "synthetic", bytes.clone(), rect);
            },
        );
        output.textures_delta.clear();
    }

    #[test]
    fn rendered_animation_reuses_texture_pauses_and_releases_offscreen_work() {
        let ctx = Context::default();
        install(&ctx);
        let bytes: Arc<[u8]> = webp().into();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            draw(&ctx, bytes.clone(), 0.0, Vec::new());
            if manager(&ctx).lock().unwrap().entries["synthetic"]
                .texture
                .is_some()
            {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let m = manager(&ctx);
        let texture = m.lock().unwrap().entries["synthetic"]
            .texture
            .as_ref()
            .unwrap()
            .id();
        assert_eq!(
            m.lock().unwrap().entries["synthetic"]
                .current
                .as_ref()
                .unwrap()
                .image
                .pixels[0],
            egui::Color32::RED
        );
        draw(&ctx, bytes.clone(), 0.15, Vec::new());
        assert_eq!(
            m.lock().unwrap().entries["synthetic"]
                .current
                .as_ref()
                .unwrap()
                .image
                .pixels[0],
            egui::Color32::BLUE
        );
        assert_eq!(
            m.lock().unwrap().entries["synthetic"]
                .texture
                .as_ref()
                .unwrap()
                .id(),
            texture
        );
        let control = egui::pos2(170.0, 198.0);
        let pointer = |pressed| egui::Event::PointerButton {
            pos: control,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        draw(
            &ctx,
            bytes.clone(),
            0.16,
            vec![egui::Event::PointerMoved(control), pointer(true)],
        );
        draw(&ctx, bytes.clone(), 0.17, vec![pointer(false)]);
        assert!(m.lock().unwrap().entries["synthetic"].paused);
        draw(&ctx, bytes, 1.0, Vec::new());
        assert_eq!(
            m.lock().unwrap().entries["synthetic"]
                .current
                .as_ref()
                .unwrap()
                .image
                .pixels[0],
            egui::Color32::BLUE
        );
        for _ in 0..3 {
            let mut output = ctx.run_ui(Default::default(), |_| {});
            output.textures_delta.clear();
            sweep(&ctx);
        }
        assert!(m.lock().unwrap().entries.is_empty());
        let budget = m.lock().unwrap().budget.clone();
        while budget.0.load(Ordering::Relaxed) != 0 {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn finite_loops_and_partial_corruption_keep_a_safe_preview() {
        let mut bytes = webp();
        let anim = bytes
            .windows(4)
            .position(|window| window == b"ANIM")
            .unwrap();
        bytes[anim + 12] = 2; // two complete passes
        let mut stream = Stream::new(
            bytes.into(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(Budget::default()),
        )
        .unwrap();
        assert!(matches!(stream.loops,LoopCount::Finite(n) if n.get()==2));
        assert!(stream.frame().unwrap().is_some());
        assert!(stream.frame().unwrap().is_some());
        assert!(stream.next().unwrap().is_some());
        assert!(stream.next().unwrap().is_some());
        assert!(stream.next().unwrap().is_none());
        stream.frames = image::Frames::new(Box::new(
            [
                Ok(image::Frame::new(image::RgbaImage::from_pixel(
                    2,
                    2,
                    image::Rgba([255, 0, 0, 255]),
                ))),
                Err(image::ImageError::IoError(std::io::Error::other(
                    "synthetic",
                ))),
            ]
            .into_iter(),
        ));
        let clip = prepare(&mut stream).unwrap();
        assert_eq!(clip.frames.len(), 1);
        assert_eq!(clip.preview_error, Some("animation: invalid data"));
    }
    #[test]
    fn long_tiny_animations_continue_after_the_preparation_work_limit() {
        let mut stream = Stream::new(
            gif().into(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(Budget::default()),
        )
        .unwrap();
        stream.frames = image::Frames::new(Box::new(
            (0..600).map(|_| Ok(image::Frame::new(image::RgbaImage::new(1, 1)))),
        ));
        let clip = prepare(&mut stream).unwrap();
        assert!(clip.streaming);
        let mut count = clip.frames.len();
        drop(clip);
        while stream.frame().unwrap().is_some() {
            count += 1;
        }
        assert_eq!(count, 600);
    }
}
