//! Optional Google Cast adapter. `catt` owns discovery and the YouTube TV
//! protocol; no account cookies, raw helper output or TV addresses are saved.
//! A lazy worker serializes commands outside the UI and YouTube session runtime.

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::path::Path;
use std::process::Stdio;
use std::sync::mpsc;
use std::time::Duration;

use serde::Deserialize;
use tokio::io::AsyncReadExt;

const OUTPUT_LIMIT: u64 = 256 * 1024;
const DEADLINE: Duration = Duration::from_secs(75);
/// A send also waits for ads and the TV's account picker before playback.
const START_DEADLINE: Duration = Duration::from_secs(150);
const CONFIRM_WAIT: Duration = Duration::from_secs(60);

mod dial;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Protocol {
    GoogleCast,
    YouTube {
        endpoint: url::Url,
    },
    /// Linked with the code the TV's YouTube app shows. Reached through
    /// YouTube's servers, so it works across networks; `host` is unused.
    Linked {
        screen: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub host: IpAddr,
    pub name: String,
    pub protocol: Protocol,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Open,
    Scan,
    FindYouTube(String),
    Link(String),
    Start(Device),
    Control(Control),
    Disconnect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Play,
    Pause,
    Rewind,
    Forward,
    VolumeDown,
    VolumeUp,
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    MissingHelper,
    Unavailable,
    Failed,
    TimedOut,
    InvalidResponse,
    Unconfirmed,
    InvalidRequest,
    NoYouTube,
    ConnectionExpired,
    Unsupported,
    InvalidCode,
    VolumeUnavailable,
}

impl Error {
    pub fn message(self) -> &'static str {
        match self {
            Self::MissingHelper => "Install catt to use Google Cast, then search again.",
            Self::Unavailable => "Could not start the TV connection. Try again.",
            Self::Failed => "The TV command failed. Check the TV and your network, then retry.",
            Self::TimedOut => "The TV did not respond in time. Check the TV before retrying.",
            Self::InvalidResponse => "Could not read the TV response. Try again.",
            Self::Unconfirmed => "Playback on TV was not confirmed. Check the TV before retrying.",
            Self::InvalidRequest => "This video or device cannot be sent to the TV.",
            Self::NoYouTube => "No compatible YouTube app was found at this TV address.",
            Self::ConnectionExpired => {
                "The TV connection expired. Disconnect and send the video again."
            }
            Self::Unsupported => "This TV does not accept that command. Use the TV remote.",
            Self::InvalidCode => {
                "That TV code was not accepted. Check the code shown on the TV and try again."
            }
            Self::VolumeUnavailable => {
                "This TV does not report its volume. Use the TV remote for volume."
            }
        }
    }
}

pub enum Request {
    CheckSponsor {
        enabled: std::sync::Arc<std::sync::atomic::AtomicBool>,
        device: Device,
        video: String,
        segments: Vec<crate::model::Segment>,
    },
    Scan,
    Disconnect,
    FindYouTube(IpAddr),
    Link(String),
    Start {
        device: Device,
        video: String,
        seconds: u32,
    },
    Control {
        device: Device,
        control: Control,
    },
}

pub enum Reply {
    /// The segment end skipped to, and seconds until the next segment starts.
    SponsorChecked(Result<Sponsor, Error>),
    Disconnected,
    Devices(Result<Vec<Device>, Error>),
    Started {
        device: Device,
        video: String,
        result: Result<(), Error>,
    },
    Controlled {
        control: Control,
        result: Result<(), Error>,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sponsor {
    pub skipped: Option<f64>,
    pub next_in: Option<f64>,
}

pub struct Worker {
    commands: tokio::sync::mpsc::Sender<Request>,
    replies: mpsc::Receiver<Reply>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Worker {
    pub fn start(ctx: egui::Context) -> Result<Self, Error> {
        Self::start_program(ctx, "catt".into(), true)
    }

    fn start_program(
        ctx: egui::Context,
        program: std::path::PathBuf,
        native_discovery: bool,
    ) -> Result<Self, Error> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Error::Unavailable)?;
        let (commands, mut incoming) = tokio::sync::mpsc::channel(2);
        let (outgoing, replies) = mpsc::channel();
        let (shutdown, stopping) = tokio::sync::oneshot::channel();
        let thread = std::thread::Builder::new()
            .name("youtfast-cast".into())
            .spawn(move || {
                runtime.block_on(async move {
                    tokio::select! {
                        _ = stopping => {},
                        _ = async {
                            let mut lounge = None;
                            let mut next = None;
                            loop {
                                let request = match next.take() {
                                    Some(request) => request,
                                    None => match incoming.recv().await {
                                        Some(request) => request,
                                        None => break,
                                    },
                                };
                                let monitoring = matches!(request, Request::CheckSponsor { .. });
                                let reply = if monitoring {
                                    tokio::select! {
                                        biased;
                                        request = incoming.recv() => {
                                            let _ = outgoing.send(Reply::SponsorChecked(Ok(Sponsor::default())));
                                            ctx.request_repaint();
                                            let Some(request) = request else { break; };
                                            next = Some(request);
                                            continue;
                                        }
                                        reply = execute_with_session(&program, request, &mut lounge, native_discovery) => reply,
                                    }
                                } else {
                                    execute_with_session(&program, request, &mut lounge, native_discovery).await
                                };
                                if outgoing.send(reply).is_err() {
                                    break;
                                }
                                ctx.request_repaint();
                            }
                        } => {},
                    }
                });
            })
            .map_err(|_| Error::Unavailable)?;
        Ok(Self {
            commands,
            replies,
            shutdown: Some(shutdown),
            thread: Some(thread),
        })
    }

    pub fn send(&self, request: Request) -> Result<(), Error> {
        self.commands
            .try_send(request)
            .map_err(|_| Error::Unavailable)
    }

    pub fn poll(&self) -> impl Iterator<Item = Reply> + '_ {
        self.replies.try_iter()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        // Do not let process exit race the runtime's cancellation of a helper.
        // This cancels pending commands; it never stops playback on the TV.
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Each operation has one deadline, including send plus confirmation. Dropping
/// a timed-out/cancelled future kills its helper through `kill_on_drop`.
#[cfg(test)]
async fn execute(program: &Path, request: Request) -> Reply {
    execute_with_session(program, request, &mut None, false).await
}

async fn execute_with_session(
    program: &Path,
    request: Request,
    lounge: &mut Option<(Device, crate::youtube::lounge::Session)>,
    native_discovery: bool,
) -> Reply {
    match request {
        Request::CheckSponsor {
            enabled,
            device,
            video,
            segments,
        } => {
            let result = tokio::time::timeout(Duration::from_secs(15), async {
                if !reachable(&device) || start_args(&device, &video, 0).is_err() {
                    return Err(Error::InvalidRequest);
                }
                if !matches!(device.protocol, Protocol::GoogleCast) {
                    let (_, session) = lounge
                        .as_mut()
                        .filter(|(target, _)| target == &device)
                        .ok_or(Error::ConnectionExpired)?;
                    return session.skip_sponsor(&video, &segments, &enabled).await;
                }
                let info = run(program, &target_args(&device, &["info", "--json-output"])).await?;
                let next_in = cast_position(&info, &video)?
                    .and_then(|position| next_segment_in(position, &segments));
                let Some(end) = cast_sponsor_target(&info, &video, &segments)? else {
                    return Ok(Sponsor {
                        skipped: None,
                        next_in,
                    });
                };
                if !enabled.load(std::sync::atomic::Ordering::Relaxed) {
                    return Ok(Sponsor::default());
                }
                let mut args = target_args(&device, &["seek"]);
                args.push(end.ceil().to_string());
                run(program, &args).await?;
                Ok(Sponsor {
                    skipped: Some(end),
                    next_in: next_segment_in(end, &segments),
                })
            })
            .await
            .unwrap_or(Err(Error::TimedOut));
            Reply::SponsorChecked(result)
        }
        Request::Disconnect => {
            *lounge = None;
            Reply::Disconnected
        }
        Request::Scan => Reply::Devices(
            tokio::time::timeout(DEADLINE, async {
                let (cast, native) = tokio::join!(
                    async {
                        tokio::time::timeout(Duration::from_secs(20), async {
                            devices(&run(program, &["scan".into(), "--json-output".into()]).await?)
                        })
                        .await
                        .unwrap_or(Err(Error::TimedOut))
                    },
                    async {
                        if native_discovery {
                            dial::discover().await
                        } else {
                            Ok(Vec::new())
                        }
                    },
                );
                merge_discovery(cast, native)
            })
            .await
            .unwrap_or(Err(Error::TimedOut)),
        ),
        Request::FindYouTube(host) => {
            Reply::Devices(dial::find(host).await.map(|device| vec![device]))
        }
        Request::Link(code) => Reply::Devices(
            tokio::time::timeout(DEADLINE, crate::youtube::lounge::link(&code))
                .await
                .unwrap_or(Err(Error::TimedOut))
                .map(|(screen, name)| {
                    vec![Device {
                        host: IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
                        name,
                        protocol: Protocol::Linked { screen },
                    }]
                }),
        ),
        Request::Start {
            device,
            video,
            seconds,
        } => {
            let result = tokio::time::timeout(START_DEADLINE, async {
                let args = start_args(&device, &video, seconds)?;
                if !matches!(device.protocol, Protocol::GoogleCast) {
                    *lounge = None;
                    let session = lounge_session(&device).await?;
                    *lounge = Some((device.clone(), session));
                    return lounge.as_mut().unwrap().1.play(&video, seconds).await;
                }
                run(program, &args).await?;
                // A successful `catt cast` alone is not evidence of playback.
                // Confirm the requested video, never an older TV session. Ads
                // and the TV's account picker can come first.
                let deadline = tokio::time::Instant::now() + CONFIRM_WAIT;
                while tokio::time::Instant::now() < deadline {
                    let info =
                        run(program, &target_args(&device, &["info", "--json-output"])).await?;
                    if confirmed(&info, &video)? {
                        return Ok(());
                    }
                    tokio::time::sleep(Duration::from_millis(750)).await;
                }
                Err(Error::Unconfirmed)
            })
            .await
            .unwrap_or(Err(Error::TimedOut));
            Reply::Started {
                device,
                video,
                result,
            }
        }
        Request::Control { device, control } => {
            let result = tokio::time::timeout(DEADLINE, async {
                if !reachable(&device) {
                    return Err(Error::InvalidRequest);
                }
                if !matches!(device.protocol, Protocol::GoogleCast) {
                    if control == Control::Stop {
                        // DIAL closes the app; without a stop link, or for a
                        // linked TV, the Lounge stops the video instead.
                        if let Protocol::YouTube { endpoint } = &device.protocol
                            && dial::stop(endpoint, device.host).await.is_ok()
                        {
                            *lounge = None;
                            return Ok(());
                        }
                        let (_, session) = lounge
                            .as_mut()
                            .filter(|(target, _)| target == &device)
                            .ok_or(Error::ConnectionExpired)?;
                        session.control(Control::Stop).await?;
                        *lounge = None;
                        return Ok(());
                    }
                    let (_, session) = lounge
                        .as_mut()
                        .filter(|(target, _)| target == &device)
                        .ok_or(Error::ConnectionExpired)?;
                    return session.control(control).await;
                }
                run(program, &control_args(&device, control)).await?;
                Ok(())
            })
            .await
            .unwrap_or(Err(Error::TimedOut));
            Reply::Controlled { control, result }
        }
    }
}

/// Joins a YouTube app already on screen, so the TV keeps what it shows;
/// otherwise (or when that screen refuses a session) launches it fresh.
async fn lounge_session(device: &Device) -> Result<crate::youtube::lounge::Session, Error> {
    let endpoint = match &device.protocol {
        Protocol::Linked { screen } => {
            return crate::youtube::lounge::Session::connect(screen).await;
        }
        Protocol::YouTube { endpoint } => endpoint,
        Protocol::GoogleCast => return Err(Error::InvalidRequest),
    };
    if let Some(screen) = dial::running_screen(endpoint, device.host).await?
        && let Ok(session) = crate::youtube::lounge::Session::connect(&screen).await
    {
        return Ok(session);
    }
    let screen = dial::launch(endpoint, device.host).await?;
    crate::youtube::lounge::Session::connect(&screen).await
}

/// The playing position of the requested video, if the TV reports one.
fn cast_position(json: &[u8], video: &str) -> Result<Option<f64>, Error> {
    let info: serde_json::Value =
        serde_json::from_slice(json).map_err(|_| Error::InvalidResponse)?;
    Ok(confirmed(json, video)?
        .then(|| info["current_time"].as_f64())
        .flatten())
}

/// Seconds until the next skippable segment starts, to space out checks.
pub(crate) fn next_segment_in(position: f64, segments: &[crate::model::Segment]) -> Option<f64> {
    if !position.is_finite() || position < 0.0 {
        return None;
    }
    segments
        .iter()
        .filter(|segment| {
            segment.skipped_at_once() && segment.start.is_finite() && segment.start > position
        })
        .map(|segment| segment.start - position)
        .min_by(f64::total_cmp)
}

/// Fail closed on another video, live streams, pause or missing seek telemetry.
/// catt does not expose a reliable separate advertisement flag.
fn cast_sponsor_target(
    json: &[u8],
    video: &str,
    segments: &[crate::model::Segment],
) -> Result<Option<f64>, Error> {
    let info: serde_json::Value =
        serde_json::from_slice(json).map_err(|_| Error::InvalidResponse)?;
    if !confirmed(json, video)?
        || info["stream_type"] != "BUFFERED"
        || info["supported_media_commands"]
            .as_u64()
            .is_none_or(|commands| commands & 2 == 0)
    {
        return Ok(None);
    }
    Ok(info["current_time"]
        .as_f64()
        .and_then(|position| sponsor_target(position, segments)))
}

pub(crate) fn sponsor_target(position: f64, segments: &[crate::model::Segment]) -> Option<f64> {
    if !position.is_finite() || position < 0.0 {
        return None;
    }
    segments
        .iter()
        .filter(|segment| {
            segment.skipped_at_once()
                && segment.start.is_finite()
                && segment.end.is_finite()
                && segment.start >= 0.0
                && segment.end > segment.start
                && position >= segment.start
                && position + 2.0 < segment.end
        })
        .map(|segment| segment.end)
        .max_by(f64::total_cmp)
}

/// Keep both routes when a TV advertises DIAL and Cast: either service can
/// fail independently, and the user must be able to try the other one.
fn merge_discovery(
    cast: Result<Vec<Device>, Error>,
    native: Result<Vec<Device>, Error>,
) -> Result<Vec<Device>, Error> {
    let cast_error = cast.as_ref().err().copied();
    let native_error = native.as_ref().err().copied();
    let mut found = native.unwrap_or_default();
    for device in cast.unwrap_or_default() {
        if !found
            .iter()
            .any(|known| known.host == device.host && known.protocol == device.protocol)
        {
            found.push(device);
        }
    }
    let shared_hosts: Vec<_> = found
        .iter()
        .filter(|device| {
            found
                .iter()
                .any(|other| other.host == device.host && other.protocol != device.protocol)
        })
        .map(|device| device.host)
        .collect();
    for device in &mut found {
        if shared_hosts.contains(&device.host) {
            device.name.push_str(match device.protocol {
                Protocol::GoogleCast => " (Google Cast)",
                Protocol::YouTube { .. } => " (YouTube TV)",
                Protocol::Linked { .. } => " (TV code)",
            });
        }
    }
    found.sort_by(|a, b| a.name.cmp(&b.name).then(a.host.cmp(&b.host)));
    if found.is_empty()
        && let Some(error) = cast_error.or(native_error)
    {
        return Err(error);
    }
    Ok(found)
}

async fn run(program: &Path, args: &[String]) -> Result<Vec<u8>, Error> {
    let mut command = tokio::process::Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        // Helper errors can contain tokens, URLs and platform data. Never
        // inherit stderr, even with --verbose, and never log stdout.
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    let mut child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::MissingHelper
        } else {
            Error::Unavailable
        }
    })?;
    let stdout = child.stdout.take().ok_or(Error::Failed)?;
    let mut output = Vec::new();
    stdout
        .take(OUTPUT_LIMIT + 1)
        .read_to_end(&mut output)
        .await
        .map_err(|_| Error::Failed)?;
    if output.len() as u64 > OUTPUT_LIMIT {
        let _ = child.kill().await;
        return Err(Error::InvalidResponse);
    }
    if !child.wait().await.map_err(|_| Error::Failed)?.success() {
        return Err(Error::Failed);
    }
    Ok(output)
}

/// Linked TVs are reached through YouTube; every other target must be local.
fn reachable(device: &Device) -> bool {
    matches!(device.protocol, Protocol::Linked { .. }) || local_address(device.host)
}

fn local_address(host: IpAddr) -> bool {
    match host {
        IpAddr::V4(host) => host.is_private() || host.is_link_local(),
        IpAddr::V6(host) => host.is_unique_local() || host.is_unicast_link_local(),
    }
}

#[derive(Deserialize)]
struct Discovered {
    host: IpAddr,
    friendly_name: String,
    port: u16,
    #[serde(default)]
    model_name: Option<String>,
}

/// `catt scan` does not report the cast type, so speakers are known by model.
fn audio_only(model: Option<&str>) -> bool {
    model.is_some_and(|model| {
        // Exact names: "Google Home Hub" (a Nest Hub) has a screen.
        [
            "google home",
            "google home mini",
            "google home max",
            "google nest mini",
            "nest audio",
            "google nest audio",
            "chromecast audio",
            "google nest wifi point",
        ]
        .contains(&model.trim().to_ascii_lowercase().as_str())
    })
}

fn devices(json: &[u8]) -> Result<Vec<Device>, Error> {
    let discovered: BTreeMap<String, Discovered> =
        serde_json::from_slice(json).map_err(|_| Error::InvalidResponse)?;
    let mut devices = Vec::new();
    for device in discovered.into_values() {
        if !local_address(device.host)
            || device.port != 8009
            || audio_only(device.model_name.as_deref())
            || devices
                .iter()
                .any(|known: &Device| known.host == device.host)
        {
            continue;
        }
        let name: String = device
            .friendly_name
            .chars()
            .filter(|c| !c.is_control())
            .take(100)
            .collect();
        let name = name.trim();
        devices.push(Device {
            host: device.host,
            protocol: Protocol::GoogleCast,
            name: if name.is_empty() {
                "Google Cast".into()
            } else {
                name.into()
            },
        });
        if devices.len() == 128 {
            break;
        }
    }
    devices.sort_by(|a, b| a.name.cmp(&b.name).then(a.host.cmp(&b.host)));
    Ok(devices)
}

fn target_args(device: &Device, command: &[&str]) -> Vec<String> {
    ["--device".into(), device.host.to_string()]
        .into_iter()
        .chain(command.iter().map(|arg| (*arg).into()))
        .collect()
}

fn start_args(device: &Device, video: &str, seconds: u32) -> Result<Vec<String>, Error> {
    if !reachable(device)
        || video.len() != 11
        || !video
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
    {
        return Err(Error::InvalidRequest);
    }
    let mut args = target_args(device, &["cast", "--no-playlist", "--no-subs", "--seek-to"]);
    args.push(seconds.to_string());
    args.push(format!("https://www.youtube.com/watch?v={video}"));
    Ok(args)
}

fn control_args(device: &Device, control: Control) -> Vec<String> {
    let command: &[&str] = match control {
        Control::Play => &["play"],
        Control::Pause => &["pause"],
        Control::Rewind => &["rewind", "10"],
        Control::Forward => &["ffwd", "10"],
        Control::VolumeDown => &["volumedown", "10"],
        Control::VolumeUp => &["volumeup", "10"],
        Control::Stop => &["stop"],
    };
    target_args(device, command)
}

fn confirmed(json: &[u8], video: &str) -> Result<bool, Error> {
    #[derive(Deserialize)]
    struct Info {
        content_id: Option<String>,
        player_state: Option<String>,
    }
    let info: Info = serde_json::from_slice(json).map_err(|_| Error::InvalidResponse)?;
    let same = info.content_id.as_deref().is_some_and(|id| {
        id == video || crate::youtube::ytdlp::video_id_from_link(id).as_deref() == Some(video)
    });
    Ok(same && info.player_state.as_deref() == Some("PLAYING"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device() -> Device {
        Device {
            protocol: Protocol::GoogleCast,
            host: "192.168.1.20".parse().unwrap(),
            // Never used as a command, argument or shell expression.
            name: "TV; $(untrusted name)".into(),
        }
    }

    fn sponsor() -> crate::model::Segment {
        crate::model::Segment {
            category: "sponsor".into(),
            start: 10.0,
            end: 30.25,
        }
    }

    #[test]
    fn automatic_skips_require_a_valid_matching_segment_and_tv_telemetry() {
        let segments = [sponsor()];
        let base = serde_json::json!({"content_id":"abcdefghijk", "player_state":"PLAYING",
            "current_time":12.0, "stream_type":"BUFFERED", "supported_media_commands":2});
        let target = |info: &serde_json::Value| {
            cast_sponsor_target(&serde_json::to_vec(info).unwrap(), "abcdefghijk", &segments)
                .unwrap()
        };
        assert_eq!(target(&base), Some(30.25));
        for (key, value) in [
            ("content_id", serde_json::json!("another_one")),
            ("player_state", serde_json::json!("PAUSED")),
            ("player_state", serde_json::json!("BUFFERING")),
            ("current_time", serde_json::Value::Null),
            ("current_time", serde_json::json!(-1)),
            ("current_time", serde_json::json!(30.25)),
            ("stream_type", serde_json::json!("LIVE")),
            ("supported_media_commands", serde_json::json!(0)),
        ] {
            let mut info = base.clone();
            info[key] = value;
            assert_eq!(target(&info), None, "unexpected seek for {key}");
        }
        for key in [
            "content_id",
            "player_state",
            "current_time",
            "stream_type",
            "supported_media_commands",
        ] {
            let mut info = base.clone();
            info.as_object_mut().unwrap().remove(key);
            assert_eq!(target(&info), None);
        }
        let mut segment = sponsor();
        segment.category = "intro".into();
        assert_eq!(sponsor_target(12.0, &[segment]), None);
        for position in [f64::NAN, f64::INFINITY, -1.0, 9.0, 29.0, 30.25] {
            assert_eq!(sponsor_target(position, &segments), None);
        }
        let mut invalid = sponsor();
        invalid.end = f64::INFINITY;
        assert_eq!(sponsor_target(12.0, &[invalid]), None);
    }

    #[test]
    fn discovery_is_bounded_deduplicated_and_limited_to_local_cast_targets() {
        let json = br#"{
            "a": {"host":"192.168.1.20","port":8009,"friendly_name":"  Living\nroom  "},
            "b": {"host":"192.168.1.20","port":8009,"friendly_name":"Duplicate"},
            "c": {"host":"8.8.8.8","port":8009,"friendly_name":"Public"},
            "d": {"host":"127.0.0.1","port":8009,"friendly_name":"Loopback"},
            "e": {"host":"192.168.1.30","port":80,"friendly_name":"Other port"},
            "f": {"host":"fd00::1","port":8009,"friendly_name":"Bedroom"}
        }"#;
        let found = devices(json).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].name, "Bedroom");
        assert_eq!(found[1].name, "Livingroom");
        let entries: BTreeMap<_, _> = (1..=200)
            .map(|i| {
                (
                    i.to_string(),
                    serde_json::json!({"host":format!("192.168.1.{i}"), "port":8009, "friendly_name":"X".repeat(500)}),
                )
            })
            .collect();
        let found = devices(&serde_json::to_vec(&entries).unwrap()).unwrap();
        assert_eq!(found.len(), 128);
        assert!(found.iter().all(|d| d.name.len() == 100));
    }

    #[test]
    fn speakers_are_not_listed_as_tvs_but_screens_are() {
        let json = br#"{
            "a": {"host":"192.168.1.20","port":8009,"friendly_name":"Kitchen","model_name":"Google Nest Mini"},
            "b": {"host":"192.168.1.21","port":8009,"friendly_name":"Hub","model_name":"Google Home Hub"},
            "c": {"host":"192.168.1.22","port":8009,"friendly_name":"TV","model_name":"BRAVIA 4K"},
            "d": {"host":"192.168.1.23","port":8009,"friendly_name":"Old"}
        }"#;
        let names: Vec<_> = devices(json).unwrap().into_iter().map(|d| d.name).collect();
        assert_eq!(names, ["Hub", "Old", "TV"]);
    }

    #[test]
    fn both_routes_remain_selectable_for_a_tv_with_cast_and_dial() {
        let native = Device {
            host: "192.168.1.20".parse().unwrap(),
            name: "Living room TV".into(),
            protocol: Protocol::YouTube {
                endpoint: url::Url::parse("http://192.168.1.20:8008/apps/YouTube").unwrap(),
            },
        };
        let mut other = device();
        other.host = "192.168.1.30".parse().unwrap();
        let found =
            merge_discovery(Ok(vec![device(), other.clone()]), Ok(vec![native.clone()])).unwrap();
        assert_eq!(found.len(), 3);
        assert!(found.contains(&other));
        assert!(found.iter().any(|device| device.host == native.host
            && device.protocol == native.protocol
            && device.name == "Living room TV (YouTube TV)"));
        assert!(found.iter().any(|device| device.host == native.host
            && device.protocol == Protocol::GoogleCast
            && device.name.ends_with(" (Google Cast)")));
        assert_eq!(
            merge_discovery(Err(Error::MissingHelper), Ok(vec![native.clone()])),
            Ok(vec![native])
        );
    }

    #[test]
    fn failed_native_discovery_is_not_reported_as_an_empty_success() {
        assert_eq!(
            merge_discovery(Ok(Vec::new()), Err(Error::Unavailable)),
            Err(Error::Unavailable)
        );
        assert_eq!(
            merge_discovery(Ok(vec![device()]), Err(Error::Unavailable)),
            Ok(vec![device()])
        );
        assert_eq!(
            merge_discovery(Ok(Vec::new()), Ok(Vec::new())),
            Ok(Vec::new())
        );
    }

    #[test]
    fn linked_tvs_are_reached_without_a_local_address_and_others_still_need_one() {
        let linked = Device {
            host: std::net::Ipv4Addr::UNSPECIFIED.into(),
            name: "TV".into(),
            protocol: Protocol::Linked {
                screen: "synthetic".into(),
            },
        };
        assert!(reachable(&linked));
        assert!(start_args(&linked, "abcdefghijk", 0).is_ok());
        let mut public = device();
        public.host = "8.8.8.8".parse().unwrap();
        assert!(!reachable(&public));
    }

    #[test]
    fn checks_are_spaced_by_the_time_to_the_next_segment() {
        let mut later = sponsor();
        later.start = 100.0;
        later.end = 120.0;
        let segments = [sponsor(), later];
        assert_eq!(next_segment_in(0.0, &segments), Some(10.0));
        assert_eq!(next_segment_in(12.0, &segments), Some(88.0));
        assert_eq!(next_segment_in(130.0, &segments), None);
        assert_eq!(next_segment_in(f64::NAN, &segments), None);
    }

    #[test]
    fn discovery_never_turns_a_host_name_or_broken_json_into_a_command() {
        for json in [
            b"not json".as_slice(),
            br#"{"TV":{"host":"--help","port":8009,"friendly_name":"TV"}}"#,
            br#"{"TV":{"host":"example.com","port":8009,"friendly_name":"TV"}}"#,
        ] {
            assert_eq!(devices(json), Err(Error::InvalidResponse));
        }
        assert_eq!(devices(b"{}"), Ok(Vec::new()));
    }

    #[test]
    fn sending_builds_only_a_canonical_youtube_url_without_credentials_or_playlists() {
        let args = start_args(&device(), "abcdefghijk", 84).unwrap();
        assert_eq!(
            args,
            [
                "--device",
                "192.168.1.20",
                "cast",
                "--no-playlist",
                "--no-subs",
                "--seek-to",
                "84",
                "https://www.youtube.com/watch?v=abcdefghijk"
            ]
        );
        for id in ["--help", "abcdefghij&", "https://example.com", "../../file"] {
            assert_eq!(start_args(&device(), id, 0), Err(Error::InvalidRequest));
        }
        let mut public = device();
        public.host = "8.8.8.8".parse().unwrap();
        assert_eq!(
            start_args(&public, "abcdefghijk", 0),
            Err(Error::InvalidRequest)
        );
        assert_eq!(
            control_args(&device(), Control::Forward),
            ["--device", "192.168.1.20", "ffwd", "10"]
        );
    }

    #[test]
    fn confirmation_requires_the_requested_video_and_playing_state() {
        for content in ["abcdefghijk", "https://www.youtube.com/watch?v=abcdefghijk"] {
            let json = serde_json::json!({"content_id":content,"player_state":"PLAYING", "ignored":"private helper data"});
            assert!(confirmed(&serde_json::to_vec(&json).unwrap(), "abcdefghijk").unwrap());
        }
        for json in [
            br#"{}"#.as_slice(),
            br#"{"content_id":"abcdefghijk","player_state":"BUFFERING"}"#,
            br#"{"content_id":"abcdefghijk","player_state":"PAUSED"}"#,
            br#"{"content_id":"another_one","player_state":"PLAYING"}"#,
        ] {
            assert!(!confirmed(json, "abcdefghijk").unwrap());
        }
        assert_eq!(
            confirmed(b"private helper error", "abcdefghijk"),
            Err(Error::InvalidResponse)
        );
    }

    #[tokio::test]
    async fn missing_helper_is_an_actionable_fixed_error() {
        let result = run(Path::new("target/nonexistent-catt-test/helper"), &[]).await;
        assert_eq!(result, Err(Error::MissingHelper));
        assert!(Error::MissingHelper.message().contains("Install catt"));
    }

    #[cfg(unix)]
    mod process {
        use super::*;
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;
        use std::sync::atomic::{AtomicU64, Ordering};

        struct Fixture(PathBuf);

        impl Fixture {
            fn new(body: &str) -> Self {
                static NEXT: AtomicU64 = AtomicU64::new(0);
                let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
                    "target/cast-test-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                std::fs::create_dir_all(&dir).unwrap();
                let helper = dir.join("catt");
                std::fs::write(&helper, format!("#!/usr/bin/env python3\n{body}\n")).unwrap();
                std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
                Self(dir)
            }

            fn helper(&self) -> PathBuf {
                self.0.join("catt")
            }
        }

        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        #[tokio::test]
        async fn helper_contract_covers_discovery_send_confirmation_and_control() {
            let fixture = Fixture::new(
                r#"
import sys, json
args = sys.argv[1:]
if args == ['scan', '--json-output']:
    print(json.dumps({'TV': {'host':'192.168.1.20', 'port':8009, 'friendly_name':'TV'}}))
else:
    assert args[:2] == ['--device', '192.168.1.20']
    if args[2] == 'cast':
        assert args[3:] == ['--no-playlist', '--no-subs', '--seek-to', '84', 'https://www.youtube.com/watch?v=abcdefghijk']
    elif args[2:] == ['info', '--json-output']:
        print(json.dumps({'content_id':'abcdefghijk', 'player_state':'PLAYING'}))
    else:
        assert args[2:] == ['pause']
"#,
            );
            let Reply::Devices(Ok(found)) = execute(&fixture.helper(), Request::Scan).await else {
                panic!("discovery failed")
            };
            assert_eq!(found.len(), 1);
            let reply = execute(
                &fixture.helper(),
                Request::Start {
                    device: found[0].clone(),
                    video: "abcdefghijk".into(),
                    seconds: 84,
                },
            )
            .await;
            assert!(matches!(reply, Reply::Started { result: Ok(()), .. }));
            let reply = execute(
                &fixture.helper(),
                Request::Control {
                    device: found[0].clone(),
                    control: Control::Pause,
                },
            )
            .await;
            assert!(matches!(reply, Reply::Controlled { result: Ok(()), .. }));
        }

        #[tokio::test]
        async fn sponsor_monitor_reads_tv_position_and_uses_an_absolute_seek() {
            let fixture = Fixture::new(
                r#"
import sys, json
args = sys.argv[1:]
assert args[:2] == ['--device', '192.168.1.20']
if args[2:] == ['info', '--json-output']:
    print(json.dumps({'content_id':'abcdefghijk', 'player_state':'PLAYING', 'current_time':12,
        'stream_type':'BUFFERED', 'supported_media_commands':2}))
else:
    assert args[2:] == ['seek', '31']
    from pathlib import Path
    Path(__file__).with_name('sought').touch()
"#,
            );
            for enabled in [false, true] {
                let reply = execute(
                    &fixture.helper(),
                    Request::CheckSponsor {
                        device: device(),
                        video: "abcdefghijk".into(),
                        segments: vec![sponsor()],
                        enabled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(enabled)),
                    },
                )
                .await;
                let Reply::SponsorChecked(result) = reply else {
                    panic!("wrong reply")
                };
                assert_eq!(result.unwrap().skipped, enabled.then_some(30.25));
                assert_eq!(fixture.0.join("sought").exists(), enabled);
            }
        }

        #[test]
        fn interactive_commands_cancel_slow_sponsor_checks() {
            let fixture = Fixture::new(
                r#"
import sys, json, time
from pathlib import Path
args = sys.argv[1:]
if args[2:] == ['info', '--json-output']:
    Path(__file__).with_name('checking').touch()
    time.sleep(30)
    print('{}')
elif args[2:] == ['stop']:
    pass
else:
    raise AssertionError('unexpected command')
"#,
            );
            let worker =
                Worker::start_program(egui::Context::default(), fixture.helper(), false).unwrap();
            worker
                .send(Request::CheckSponsor {
                    device: device(),
                    video: "abcdefghijk".into(),
                    segments: vec![sponsor()],
                    enabled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
                })
                .unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while !fixture.0.join("checking").exists() {
                assert!(std::time::Instant::now() < deadline, "helper did not start");
                std::thread::sleep(Duration::from_millis(10));
            }
            worker
                .send(Request::Control {
                    device: device(),
                    control: Control::Stop,
                })
                .unwrap();
            loop {
                if worker.poll().any(|reply| {
                    matches!(
                        reply,
                        Reply::Controlled {
                            control: Control::Stop,
                            result: Ok(())
                        }
                    )
                }) {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "stop waited for background telemetry"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }

        #[tokio::test]
        async fn helper_failures_and_excess_output_never_escape_as_raw_diagnostics() {
            let fixture = Fixture::new(
                "import sys\nprint('synthetic private diagnostic', file=sys.stderr)\nsys.exit(1)",
            );
            assert_eq!(run(&fixture.helper(), &[]).await, Err(Error::Failed));
            let fixture = Fixture::new("print('x' * 300000)");
            assert_eq!(
                run(&fixture.helper(), &[]).await,
                Err(Error::InvalidResponse)
            );
        }

        #[tokio::test]
        async fn cancelling_a_stalled_helper_kills_it_before_late_side_effects() {
            let fixture = Fixture::new(
                "import pathlib, time\ntime.sleep(0.5)\npathlib.Path(__file__).with_name('late').touch()",
            );
            assert!(
                tokio::time::timeout(Duration::from_millis(50), run(&fixture.helper(), &[]))
                    .await
                    .is_err()
            );
            tokio::time::sleep(Duration::from_millis(650)).await;
            assert!(!fixture.0.join("late").exists());
        }

        #[test]
        fn closing_worker_waits_for_pending_helper_cancellation() {
            let fixture = Fixture::new(
                "import pathlib, time\npathlib.Path(__file__).with_name('started').touch()\ntime.sleep(0.5)\npathlib.Path(__file__).with_name('late').touch()",
            );
            let worker =
                Worker::start_program(egui::Context::default(), fixture.helper(), false).unwrap();
            assert!(worker.send(Request::Scan).is_ok());
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while !fixture.0.join("started").exists() {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
            drop(worker);
            std::thread::sleep(Duration::from_millis(650));
            assert!(!fixture.0.join("late").exists());
        }
    }
}
