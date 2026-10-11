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
const SEEKABLE: u64 = 14;
const SPEED: u64 = 15;
const MUTE: u64 = 16;
const TRACKS: u64 = 17;
const EOF_REACHED: u64 = 18;

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
    /// A seek can land: false while nothing is loaded and while a video
    /// is still opening.
    pub seekable: bool,
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
    /// The playback speed, 1 at normal speed.
    pub speed: f64,
    pub muted: bool,
    /// The captions the video offers, as yt-dlp handed them to mpv.
    pub subtitles: Vec<Subtitle>,
}

/// A captions track of the video playing.
#[derive(Clone, Debug, PartialEq)]
pub struct Subtitle {
    /// mpv's track id, for `sid`.
    pub id: i64,
    /// The language code yt-dlp gave ("en", "pt-BR", "en-orig").
    pub lang: String,
    /// The name to show ("English", "English (auto-generated)").
    pub title: String,
    pub selected: bool,
}

impl Subtitle {
    /// The captions tracks in mpv's `track-list`, which it hands over as
    /// JSON.
    fn from_track_list(json: &str) -> Vec<Self> {
        let Ok(serde_json::Value::Array(tracks)) = serde_json::from_str(json) else {
            return Vec::new();
        };
        tracks
            .iter()
            .filter(|track| track["type"] == "sub")
            .filter_map(|track| {
                let id = track["id"].as_i64()?;
                let lang = track["lang"].as_str().unwrap_or_default().to_owned();
                let title = track["title"]
                    .as_str()
                    .filter(|title| !title.is_empty())
                    .map_or_else(|| lang.clone(), str::to_owned);
                Some(Self {
                    id,
                    lang,
                    title,
                    selected: track["selected"].as_bool().unwrap_or(false),
                })
            })
            .collect()
    }
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
    /// The captions' languages yt-dlp is asked for (see [`raw_options`]).
    captions: Vec<String>,
    /// yt-dlp's `--cookies-from-browser`, when an account's browser is
    /// known: a video YouTube refuses signed out is tried again with it.
    cookies_from_browser: Option<String>,
    /// The video loaded last and where it started, for that retry.
    current: Option<(String, Option<f64>)>,
    /// The video loaded last went through the browser's session.
    signed_in: bool,
    /// mpv sits at the end of the video (`eof-reached`).
    at_end: bool,
}

impl Mpv {
    /// Starts mpv with its video output set to the render API, so nothing
    /// shows until a [`VideoSurface`] draws it.
    /// `max_resolution` bounds the streams picked, as
    /// [`Mpv::set_max_resolution`] does; `volume` is where it starts.
    pub fn new(
        repaint: &egui::Context,
        max_resolution: u32,
        volume: f64,
        captions: Vec<String>,
    ) -> Result<Self, Error> {
        // SAFETY: plain constructor. It returns null when the locale's
        // LC_NUMERIC is not "C"; Rust programs keep "C" unless something
        // calls setlocale, which YouTFast does not.
        let ptr = unsafe { ffi::mpv_create() };
        if ptr.is_null() {
            return Err(Error::new(0, "creating mpv (is LC_NUMERIC \"C\"?)"));
        }
        let handle = Arc::new(Handle(ptr));
        let raw_options = raw_options(max_resolution, &captions);
        let volume = volume.clamp(0.0, 100.0);
        let initial_volume = volume_text(volume);
        let options: [(&CStr, &str); 14] = [
            (c"vo", "libmpv"),
            (c"hwdec", "auto-safe"),
            (c"ytdl", "yes"),
            (c"ytdl-format", FORMAT),
            (c"ytdl-raw-options", &raw_options),
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

        let observed: [(u64, &CStr, ffi::mpv_format); 18] = [
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
            (SEEKABLE, c"seekable", ffi::MPV_FORMAT_FLAG),
            (SPEED, c"speed", ffi::MPV_FORMAT_DOUBLE),
            (MUTE, c"mute", ffi::MPV_FORMAT_FLAG),
            // A node; as a string mpv writes it as JSON.
            (TRACKS, c"track-list", ffi::MPV_FORMAT_STRING),
            // keep-open holds the last frame instead of ending the file, so
            // the end is this flag turning on, not an end-file event.
            (EOF_REACHED, c"eof-reached", ffi::MPV_FORMAT_FLAG),
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
                speed: 1.0,
                ..Status::default()
            },
            wakeup,
            ytdl_failure: None,
            max_resolution,
            captions,
            cookies_from_browser: None,
            current: None,
            signed_in: false,
            at_end: false,
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
    /// It plays signed out: signed in, yt-dlp offers YouTube's web clients
    /// only, which hand out little more than one 360p stream without a PO
    /// token. [`Mpv::retry_signed_in`] covers what needs the session.
    pub fn load(&mut self, url: &str, start: Option<f64>) -> Result<(), Error> {
        self.current = Some((url.to_owned(), start));
        self.signed_in = false;
        match start {
            Some(start) => self.load_with(url, &format!("start={start:.3}"))?,
            None => self.command(&["loadfile", url, "replace"])?,
        }
        self.set_paused(false)
    }

    /// Loads the last video again through the browser's session, once,
    /// after YouTube refused it signed out (a bot check, an age limit, a
    /// private video). False when there is no session or it was tried.
    pub fn retry_signed_in(&mut self) -> Result<bool, Error> {
        let (Some(browser), Some((url, start)), false) = (
            self.cookies_from_browser.as_deref(),
            self.current.clone(),
            self.signed_in,
        ) else {
            return Ok(false);
        };
        self.signed_in = true;
        let mut options = format!("start={:.3}", start.unwrap_or(0.0));
        options.push(',');
        options.push_str(&session_option(browser));
        self.load_with(&url, &options)?;
        self.set_paused(false)?;
        Ok(true)
    }

    /// Opens `url` again at `position`, keeping it paused or playing: how a
    /// new quality takes effect. mpv asks yt-dlp for the streams again,
    /// through the session when the video needed it.
    pub fn reload(&mut self, url: &str, position: f64, paused: bool) -> Result<(), Error> {
        self.current = Some((url.to_owned(), Some(position)));
        let mut options = format!(
            "start={position:.3},pause={}",
            if paused { "yes" } else { "no" }
        );
        if self.signed_in
            && let Some(browser) = &self.cookies_from_browser
        {
            options.push(',');
            options.push_str(&session_option(browser));
        }
        self.load_with(url, &options)
    }

    /// Opens `url` in place of the current video with per-file `options`.
    fn load_with(&self, url: &str, options: &str) -> Result<(), Error> {
        // The index argument (-1, ignored by "replace") came in mpv 0.38;
        // per-file options follow it.
        self.command(&["loadfile", url, "replace", "-1", options])
    }

    /// Bounds the streams of the next video loaded to `resolution` (720 for
    /// 720p), by its smaller side, so a vertical video is judged like
    /// YouTube labels it. Below that, yt-dlp picks the best.
    pub fn set_max_resolution(&mut self, resolution: u32) -> Result<(), Error> {
        self.max_resolution = resolution;
        self.set_property("ytdl-raw-options", &raw_options(resolution, &self.captions))
    }

    /// The captions' languages yt-dlp offers from the next video loaded,
    /// besides the original language's automatic captions.
    pub fn set_caption_languages(&mut self, languages: Vec<String>) -> Result<(), Error> {
        if languages == self.captions {
            return Ok(());
        }
        self.captions = languages;
        self.set_property(
            "ytdl-raw-options",
            &raw_options(self.max_resolution, &self.captions),
        )
    }

    /// Shows the captions track `id` (from [`Status::subtitles`]), or none.
    pub fn set_subtitle(&self, id: Option<i64>) -> Result<(), Error> {
        match id {
            Some(id) => self.set_property("sid", &id.to_string()),
            None => self.set_property("sid", "no"),
        }
    }

    /// 0.25 to 2, as YouTube offers.
    pub fn set_speed(&self, speed: f64) -> Result<(), Error> {
        self.set_property("speed", &format!("{:.2}", speed.clamp(0.25, 2.0)))
    }

    pub fn set_muted(&self, muted: bool) -> Result<(), Error> {
        self.set_property("mute", if muted { "yes" } else { "no" })
    }

    /// One frame ahead or back, pausing: the `.` and `,` keys.
    pub fn frame_step(&self, forward: bool) -> Result<(), Error> {
        self.command(&[if forward {
            "frame-step"
        } else {
            "frame-back-step"
        }])
    }

    /// Plays the video over and over, as Shorts do, from the next load on.
    pub fn set_looping(&self, looping: bool) -> Result<(), Error> {
        self.set_property("loop-file", if looping { "inf" } else { "no" })
    }

    /// The browser whose YouTube session yt-dlp reads itself
    /// (`browser[+keyring][:profile]`) for a video refused signed out;
    /// `None` forgets it. The session never passes through a file.
    pub fn set_cookies_from_browser(&mut self, browser: Option<String>) {
        self.cookies_from_browser = browser;
    }

    /// yt-dlp's answer for the current video, as mpv's ytdl hook received
    /// it: the video's metadata, with every format it offers. Available
    /// from [`Event::Loaded`] until the video is replaced.
    pub fn ytdl_json(&self) -> Option<String> {
        if let Some(stdout) = self.get_property("user-data/mpv/ytdl/json-subprocess-result/stdout")
        {
            return Some(unquoted(stdout));
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
        self.set_property("volume", &volume_text(volume))
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
        let was_at_end = self.at_end;
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
                    log::warn!(target: "mpv", "mpv reported a playback diagnostic");
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
        if self.at_end && !was_at_end && !events.contains(&Event::Finished) {
            events.push(Event::Finished);
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
        if id == EOF_REACHED {
            self.at_end = flag.unwrap_or(false);
            return;
        }
        let status = &mut self.status;
        match id {
            TIME_POS => status.position = double.unwrap_or(0.0),
            DURATION => status.duration = double.unwrap_or(0.0),
            CACHED_UNTIL => status.cached_until = double.unwrap_or(0.0),
            VOLUME => status.volume = double.unwrap_or(status.volume),
            PAUSE => status.paused = flag.unwrap_or(false),
            BUFFERING => status.buffering = flag.unwrap_or(false),
            IDLE => status.idle = flag.unwrap_or(true),
            SEEKABLE => status.seekable = flag.unwrap_or(false),
            MEDIA_TITLE => status.title = string,
            VIDEO_WIDTH => status.video_width = integer.unwrap_or(0),
            VIDEO_HEIGHT => status.video_height = integer.unwrap_or(0),
            FPS => status.fps = double.unwrap_or(0.0),
            CODEC => status.codec = string,
            // "no" (or nothing) while the CPU decodes.
            HWDEC => status.hwdec = string.filter(|name| !name.is_empty() && name != "no"),
            SPEED => status.speed = double.unwrap_or(1.0),
            MUTE => status.muted = flag.unwrap_or(false),
            TRACKS => {
                status.subtitles = string
                    .as_deref()
                    .map(Subtitle::from_track_list)
                    .unwrap_or_default();
            }
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

/// yt-dlp's options for every load. The format sort bounds the streams to
/// `resolution`: `res` is a format's smaller side, and `res:1080` prefers
/// the largest at or below 1080. The captions are the uploader's and
/// YouTube's automatic ones in `captions`' languages (base codes, "pt"),
/// plus the automatic captions in the video's own language (`-orig`); mpv
/// opens a track only once it is shown. A list value goes in brackets, mpv's quoting, so its
/// commas stay inside it.
fn raw_options(resolution: u32, captions: &[String]) -> String {
    let mut languages: Vec<String> = captions
        .iter()
        .filter(|lang| !lang.is_empty() && lang.chars().all(|c| c.is_ascii_alphanumeric()))
        // The language, a region ("pt-BR") and one translation into it
        // ("pt-en"), not the translations' own regions ("pt-PT-de-DE").
        .map(|lang| format!("{lang}(-[a-zA-Z0-9]+)?"))
        .collect();
    languages.push(".*-orig".to_owned());
    // A finished live stream's chat replay is listed as captions.
    languages.push("-live_chat".to_owned());
    format!(
        "format-sort=res:{resolution},write-auto-subs=,sub-langs=[{}]",
        languages.join(",")
    )
}

/// The per-file option that adds the browser's session to yt-dlp's
/// options for one load. The value goes as `%length%value`, mpv's quoting
/// in a `loadfile` option list, so a comma in a profile's name cannot
/// split it.
fn session_option(browser: &str) -> String {
    let value = format!("cookies-from-browser={browser}");
    format!("ytdl-raw-options-append=%{}%{value}", value.len())
}

/// mpv's volume value: 0 to 100, as a whole number.
fn volume_text(volume: f64) -> String {
    format!("{:.0}", volume.clamp(0.0, 100.0))
}

/// mpv hands a user-data string back JSON-encoded, quotes and escapes
/// included; yt-dlp's own JSON is an object, so a string is unwrapped.
fn unquoted(value: String) -> String {
    serde_json::from_str::<String>(&value).unwrap_or(value)
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

    #[test]
    fn yt_dlp_is_asked_for_the_captions_languages() {
        assert_eq!(
            raw_options(1080, &["pt".to_owned(), "en".to_owned()]),
            "format-sort=res:1080,write-auto-subs=,sub-langs=[pt(-[a-zA-Z0-9]+)?,en(-[a-zA-Z0-9]+)?,.*-orig,-live_chat]"
        );
        // Nothing that could close the brackets gets in.
        assert_eq!(
            raw_options(720, &["e]n".to_owned(), String::new(), "pt-BR".to_owned()]),
            "format-sort=res:720,write-auto-subs=,sub-langs=[.*-orig,-live_chat]"
        );
    }

    #[test]
    fn captions_tracks_are_read_from_the_track_list() {
        let json = r#"[
            {"id": 1, "type": "video", "selected": true},
            {"id": 1, "type": "sub", "lang": "en", "title": "English", "selected": false},
            {"id": 2, "type": "sub", "lang": "pt-orig", "selected": true}
        ]"#;
        assert_eq!(
            Subtitle::from_track_list(json),
            vec![
                Subtitle {
                    id: 1,
                    lang: "en".to_owned(),
                    title: "English".to_owned(),
                    selected: false,
                },
                Subtitle {
                    id: 2,
                    lang: "pt-orig".to_owned(),
                    title: "pt-orig".to_owned(),
                    selected: true,
                },
            ]
        );
        assert!(Subtitle::from_track_list("not json").is_empty());
    }

    #[test]
    fn yt_dlp_output_is_read_through_mpv_quoting() {
        let json = r#"{"id": "abc", "formats": []}"#;
        let quoted = serde_json::to_string(json).unwrap();
        assert_eq!(unquoted(quoted), json);
        assert_eq!(unquoted(json.to_owned()), json);
    }
}
