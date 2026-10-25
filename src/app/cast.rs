//! TV controls are separate from libmpv, the queue and account history sync.
//! A pending handoff never pauses a different or newly opened local playback.

use crate::desktop::cast::{Action, Control, Device, Error, Reply, Request, Worker};
use crate::model::{Segment, Video};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct State {
    pub open: bool,
    pub devices: Vec<Device>,
    pub searched: bool,
    pub address: String,
    pub error: Option<Error>,
    pub session: Option<Session>,
    pub command_sent: bool,
    pending: Option<Pending>,
    local_handoff: Option<u64>,
    pub(super) sponsor_enabled: Arc<AtomicBool>,
    sponsor_pending: bool,
    sponsor_check_at: Option<Instant>,
    sponsor_segments: Vec<Segment>,
    sponsor_skipped: Vec<f64>,
}

pub struct Session {
    pub device: Device,
    pub video: Video,
    pub confirmed: bool,
}

enum Pending {
    Scan,
    Find,
    Disconnect,
    Start { video: Box<Video>, generation: u64 },
    Control,
}

impl State {
    pub(super) fn receive_segments(&mut self, id: &str, segments: &[Segment]) {
        if self
            .session
            .as_ref()
            .is_some_and(|session| session.video.id == id)
        {
            self.sponsor_segments = segments.to_vec();
        }
    }

    fn sponsor_request(&mut self, enabled: bool, now: Instant) -> Option<Request> {
        if !enabled
            || self.busy()
            || self.sponsor_pending
            || self.sponsor_check_at.is_some_and(|at| now < at)
        {
            return None;
        }
        let session = self
            .session
            .as_ref()
            .filter(|session| session.confirmed && !session.video.live)?;
        let segments: Vec<_> = self
            .sponsor_segments
            .iter()
            .filter(|segment| {
                segment.skipped_at_once() && !self.sponsor_skipped.contains(&segment.end)
            })
            .cloned()
            .collect();
        if segments.is_empty() {
            return None;
        }
        self.sponsor_pending = true;
        self.sponsor_check_at = Some(now + Duration::from_secs(2));
        Some(Request::CheckSponsor {
            enabled: self.sponsor_enabled.clone(),
            device: session.device.clone(),
            video: session.video.id.clone(),
            segments,
        })
    }

    pub fn allows_local_autoplay(&self, generation: u64) -> bool {
        self.local_handoff != Some(generation)
    }

    pub fn resume_local_playback(&mut self) {
        self.local_handoff = None;
    }

    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }

    pub fn scanning(&self) -> bool {
        matches!(self.pending, Some(Pending::Scan | Pending::Find))
    }

    /// Pure transition shared by the app and the isolated UI harness.
    pub fn prepare(
        &mut self,
        action: Action,
        watching: Option<&Video>,
        position: f64,
        generation: u64,
    ) -> Option<Request> {
        if matches!(
            action,
            Action::Disconnect | Action::Control(_) | Action::Start(_)
        ) {
            self.sponsor_enabled.store(false, Ordering::Relaxed);
        }
        if action == Action::Open {
            self.open = true;
            if self.searched || self.busy() || self.session.is_some() {
                return None;
            }
        } else if self.busy() {
            return None;
        }
        let (request, pending) = match action {
            Action::Open | Action::Scan => (Request::Scan, Pending::Scan),
            Action::FindYouTube(address) if self.session.is_none() => {
                let Ok(host) = address.trim().parse() else {
                    self.error = Some(Error::InvalidRequest);
                    return None;
                };
                (Request::FindYouTube(host), Pending::Find)
            }
            Action::Start(device) if self.session.is_none() => {
                self.sponsor_segments.clear();
                self.sponsor_skipped.clear();
                let video = watching?.clone();
                // Only targets returned by this run's explicit discovery.
                if !self.devices.contains(&device) {
                    return None;
                }
                let seconds = if video.live || !position.is_finite() {
                    0
                } else {
                    position.max(0.0).floor() as u32
                };
                (
                    Request::Start {
                        device,
                        video: video.id.clone(),
                        seconds,
                    },
                    Pending::Start {
                        video: Box::new(video),
                        generation,
                    },
                )
            }
            Action::Control(control) => (
                Request::Control {
                    device: self.session.as_ref()?.device.clone(),
                    control,
                },
                Pending::Control,
            ),
            Action::Disconnect => {
                self.session = None;
                self.error = None;
                self.command_sent = false;
                (Request::Disconnect, Pending::Disconnect)
            }
            Action::Start(_) | Action::FindYouTube(_) => return None,
        };
        self.pending = Some(pending);
        self.error = None;
        self.command_sent = false;
        Some(request)
    }

    pub fn failed_to_send(&mut self, error: Error) {
        self.pending = None;
        self.error = Some(error);
    }

    /// Returns the local playback to pause only after the TV confirms it.
    pub fn finish(&mut self, reply: Reply) -> Option<(String, u64)> {
        if let Reply::SponsorChecked(result) = reply {
            self.sponsor_pending = false;
            match result {
                Ok(Some(end)) => self.sponsor_skipped.push(end),
                Ok(None) => {}
                Err(error) => self.error = Some(error),
            }
            return None;
        }
        let pending = self.pending.take();
        match (pending, reply) {
            (Some(Pending::Disconnect), Reply::Disconnected) => {}
            (Some(Pending::Find), Reply::Devices(result)) => match result {
                Ok(devices) => {
                    for device in devices {
                        if !self.devices.contains(&device) {
                            self.devices.push(device);
                        }
                    }
                    self.searched = true;
                }
                Err(error) => self.error = Some(error),
            },
            (Some(Pending::Scan), Reply::Devices(result)) => {
                self.searched = true;
                match result {
                    Ok(devices) => self.devices = devices,
                    Err(error) => {
                        self.devices.clear();
                        self.error = Some(error);
                    }
                }
            }
            (
                Some(Pending::Start { video, generation }),
                Reply::Started {
                    device,
                    video: id,
                    result,
                },
            ) if id == video.id => {
                let confirmed = result.is_ok();
                self.error = result.err();
                // A timeout/error may happen after the TV starts. Retain the
                // target so Stop is available; never claim confirmed playback.
                if !matches!(
                    self.error,
                    Some(Error::MissingHelper | Error::Unavailable | Error::InvalidRequest)
                ) {
                    self.session = Some(Session {
                        device,
                        video: *video,
                        confirmed,
                    });
                }
                if confirmed {
                    return Some((id, generation));
                }
            }
            (Some(Pending::Control), Reply::Controlled { control, result }) => match result {
                Ok(()) => {
                    self.command_sent = true;
                    if control == Control::Stop {
                        self.session = None;
                    }
                }
                Err(error) => self.error = Some(error),
            },
            _ => {}
        }
        None
    }
}

impl super::State {
    /// EOF and playlist replies may already be queued when Cast confirms.
    /// Cancel existing advancement and keep that playback from scheduling more.
    pub(super) fn accept_cast_handoff(
        &mut self,
        id: &str,
        generation: u64,
        current_generation: u64,
    ) -> bool {
        if generation != current_generation
            || !self.watching.as_ref().is_some_and(|video| video.id == id)
        {
            return false;
        }
        self.autoplay = None;
        self.playing_waiting = None;
        self.cast.local_handoff = Some(generation);
        true
    }
}

impl super::App {
    pub(super) fn cast_action(&mut self, ctx: &egui::Context, action: Action) {
        let position = self.player.as_ref().map_or(0.0, |p| p.status().position);
        let Some(request) = self.state.cast.prepare(
            action,
            self.state.watching.as_ref(),
            position,
            self.local_playback_generation,
        ) else {
            return;
        };
        if self.cast_worker.is_none() {
            match Worker::start(ctx.clone()) {
                Ok(worker) => self.cast_worker = Some(worker),
                Err(error) => {
                    self.state.cast.failed_to_send(error);
                    return;
                }
            }
        }
        if let Err(error) = self.cast_worker.as_ref().unwrap().send(request) {
            self.state.cast.failed_to_send(error);
        }
    }

    pub(super) fn poll_cast(&mut self, ctx: &egui::Context) {
        let replies: Vec<_> = self
            .cast_worker
            .iter()
            .flat_map(|worker| worker.poll())
            .collect();
        for reply in replies {
            let handoff = self.state.cast.finish(reply);
            if let Some((id, _)) = &handoff
                && self.state.settings.sponsorblock
            {
                if self
                    .state
                    .watching
                    .as_ref()
                    .is_some_and(|video| video.id == *id)
                {
                    self.state.cast.receive_segments(id, &self.state.segments);
                }
                self.backend
                    .send(crate::backend::Command::LoadSegments(id.clone()));
            }
            if let Some((id, generation)) = handoff
                && self
                    .state
                    .accept_cast_handoff(&id, generation, self.local_playback_generation)
                && let Some(player) = &self.player
                && player.set_paused(true).is_err()
            {
                self.state.notice =
                    Some("The TV is playing, but local playback could not be paused.".into());
            }
        }
        let enabled = self.state.settings.sponsorblock
            && !self.state.cast.busy()
            && self
                .state
                .cast
                .session
                .as_ref()
                .is_some_and(|session| session.confirmed && !session.video.live);
        self.state
            .cast
            .sponsor_enabled
            .store(enabled, Ordering::Relaxed);
        if enabled {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
        if let Some(request) = self
            .state
            .cast
            .sponsor_request(self.state.settings.sponsorblock, Instant::now())
            && let Some(worker) = &self.cast_worker
            && let Err(error) = worker.send(request)
        {
            self.state.cast.sponsor_pending = false;
            self.state.cast.error = Some(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device() -> Device {
        Device {
            protocol: crate::desktop::cast::Protocol::GoogleCast,
            host: "192.168.1.20".parse().unwrap(),
            name: "Living room".into(),
        }
    }

    fn ready() -> State {
        State {
            devices: vec![device()],
            searched: true,
            ..Default::default()
        }
    }

    fn monitoring() -> State {
        let mut state = ready();
        let video = Video::from_id("abcdefghijk".into());
        state.prepare(Action::Start(device()), Some(&video), 0.0, 1);
        state.finish(Reply::Started {
            device: device(),
            video: video.id.clone(),
            result: Ok(()),
        });
        state.receive_segments(
            &video.id,
            &[Segment {
                category: "sponsor".into(),
                start: 10.0,
                end: 30.0,
            }],
        );
        state
    }

    #[test]
    fn sponsor_checks_are_opt_in_bounded_and_do_not_consume_remote_replies() {
        let mut state = monitoring();
        let now = Instant::now();
        assert!(state.sponsor_request(false, now).is_none());
        assert!(matches!(
            state.sponsor_request(true, now),
            Some(Request::CheckSponsor { .. })
        ));
        assert!(!state.busy());
        assert!(
            state
                .sponsor_request(true, now + Duration::from_secs(3))
                .is_none()
        );
        assert!(matches!(
            state.prepare(Action::Control(Control::Pause), None, 0.0, 1),
            Some(Request::Control { .. })
        ));
        state.finish(Reply::SponsorChecked(Ok(None)));
        assert!(state.busy());
        state.finish(Reply::Controlled {
            control: Control::Pause,
            result: Ok(()),
        });
        assert!(!state.busy());
        assert!(
            state
                .sponsor_request(true, now + Duration::from_secs(1))
                .is_none()
        );
        assert!(
            state
                .sponsor_request(true, now + Duration::from_secs(3))
                .is_some()
        );
        state.finish(Reply::SponsorChecked(Ok(Some(30.0))));
        assert!(
            state
                .sponsor_request(true, now + Duration::from_secs(10))
                .is_none()
        );
    }

    #[test]
    fn monitoring_ignores_other_videos_live_unconfirmed_and_disconnected_sessions() {
        let now = Instant::now();
        let mut state = monitoring();
        state.receive_segments("another_one", &[]);
        assert_eq!(state.sponsor_segments.len(), 1);
        state.session.as_mut().unwrap().confirmed = false;
        assert!(state.sponsor_request(true, now).is_none());
        state.session.as_mut().unwrap().confirmed = true;
        state.session.as_mut().unwrap().video.live = true;
        assert!(state.sponsor_request(true, now).is_none());
        state.session.as_mut().unwrap().video.live = false;
        state.prepare(Action::Disconnect, None, 0.0, 1);
        assert!(state.sponsor_request(true, now).is_none());
        state.finish(Reply::Disconnected);
        assert!(state.sponsor_request(true, now).is_none());
    }

    #[test]
    fn discovery_starts_only_on_request_and_cannot_be_queued_repeatedly() {
        let mut state = State::default();
        assert!(!state.busy());
        assert!(matches!(
            state.prepare(Action::Open, None, 0.0, 1),
            Some(Request::Scan)
        ));
        assert!(state.open && state.scanning());
        assert!(state.prepare(Action::Scan, None, 0.0, 1).is_none());
        state.finish(Reply::Devices(Ok(vec![device()])));
        assert!(!state.busy());
        assert!(state.prepare(Action::Open, None, 0.0, 1).is_none());
        assert!(matches!(
            state.prepare(Action::Scan, None, 0.0, 1),
            Some(Request::Scan)
        ));
    }

    #[test]
    fn sending_requires_a_video_and_a_discovered_device_and_deduplicates_clicks() {
        let mut state = ready();
        assert!(
            state
                .prepare(Action::Start(device()), None, 12.0, 2)
                .is_none()
        );
        let video = Video::from_id("abcdefghijk".into());
        let mut other = device();
        other.host = "192.168.1.21".parse().unwrap();
        assert!(
            state
                .prepare(Action::Start(other), Some(&video), 12.0, 2)
                .is_none()
        );
        assert!(matches!(
            state.prepare(Action::Start(device()), Some(&video), 84.9, 2),
            Some(Request::Start { seconds: 84, .. })
        ));
        assert!(
            state
                .prepare(Action::Start(device()), Some(&video), 12.0, 2)
                .is_none()
        );
        assert!(state.session.is_none());
    }

    #[test]
    fn only_confirmation_returns_the_original_playback_generation_for_local_pause() {
        let mut state = ready();
        let video = Video::from_id("abcdefghijk".into());
        state.prepare(Action::Start(device()), Some(&video), 84.0, 41);
        // The caller compares both id and generation, even after reopening
        // this same id. The currently open video is never substituted here.
        let pause = state.finish(Reply::Started {
            device: device(),
            video: video.id.clone(),
            result: Ok(()),
        });
        assert_eq!(pause, Some((video.id, 41)));
        assert!(state.session.as_ref().unwrap().confirmed);
    }

    #[test]
    fn failed_confirmation_keeps_stop_available_without_pausing_local_playback() {
        let mut state = ready();
        let video = Video::from_id("abcdefghijk".into());
        state.prepare(Action::Start(device()), Some(&video), 0.0, 1);
        assert!(
            state
                .finish(Reply::Started {
                    device: device(),
                    video: video.id,
                    result: Err(Error::TimedOut)
                })
                .is_none()
        );
        assert!(!state.session.as_ref().unwrap().confirmed);
        assert!(matches!(
            state.prepare(Action::Control(Control::Stop), None, 0.0, 2),
            Some(Request::Control {
                control: Control::Stop,
                ..
            })
        ));
        state.finish(Reply::Controlled {
            control: Control::Stop,
            result: Err(Error::Failed),
        });
        assert!(state.session.is_some());
        state.prepare(Action::Control(Control::Stop), None, 0.0, 2);
        state.finish(Reply::Controlled {
            control: Control::Stop,
            result: Ok(()),
        });
        assert!(state.session.is_none());
    }

    #[test]
    fn live_starts_at_the_edge_and_missing_helper_keeps_local_playback_and_retry_available() {
        let mut state = ready();
        let mut video = Video::from_id("abcdefghijk".into());
        video.live = true;
        assert!(matches!(
            state.prepare(Action::Start(device()), Some(&video), 5000.0, 1),
            Some(Request::Start { seconds: 0, .. })
        ));
        assert!(
            state
                .finish(Reply::Started {
                    device: device(),
                    video: video.id,
                    result: Err(Error::MissingHelper)
                })
                .is_none()
        );
        assert!(state.session.is_none());
        assert!(!state.busy());
        assert_eq!(state.error, Some(Error::MissingHelper));
        assert!(matches!(
            state.prepare(Action::Scan, None, 0.0, 2),
            Some(Request::Scan)
        ));
    }

    #[test]
    fn disconnect_is_local_and_never_sends_a_stop_command() {
        let mut state = ready();
        let video = Video::from_id("abcdefghijk".into());
        state.prepare(Action::Start(device()), Some(&video), 0.0, 1);
        state.finish(Reply::Started {
            device: device(),
            video: video.id,
            result: Ok(()),
        });
        assert!(matches!(
            state.prepare(Action::Disconnect, None, 0.0, 2),
            Some(Request::Disconnect)
        ));
        state.finish(Reply::Disconnected);
        assert!(!state.busy());
        assert!(state.session.is_none());
    }

    #[test]
    fn confirmed_handoff_cancels_countdown_and_blocks_same_frame_or_late_eof() {
        let video = Video::from_id("abcdefghijk".into());
        let mut state = super::super::State {
            watching: Some(video.clone()),
            autoplay: Some((
                Video::from_id("another_one".into()),
                std::time::Instant::now(),
            )),
            playing_waiting: Some(super::super::PlaylistAdvance::Autoplay),
            ..Default::default()
        };
        state.settings.autoplay = true;
        assert!(state.accept_cast_handoff(&video.id, 7, 7));
        assert!(state.autoplay.is_none());
        assert!(state.playing_waiting.is_none());
        // Both the EOF handler and start_autoplay use this gate, including
        // delayed playlist replies. Stopping/disconnecting the TV does not
        // silently resume this computer's playback or autoplay.
        assert!(!state.cast.allows_local_autoplay(7));
        state.cast.prepare(Action::Disconnect, None, 0.0, 7);
        assert!(!state.cast.allows_local_autoplay(7));
        assert!(state.settings.autoplay);
        assert!(state.cast.allows_local_autoplay(8));
        state.cast.resume_local_playback();
        assert!(state.cast.allows_local_autoplay(7));
    }

    #[test]
    fn stale_handoff_preserves_a_new_playback_and_its_countdown_even_for_the_same_id() {
        let video = Video::from_id("abcdefghijk".into());
        let mut state = super::super::State {
            watching: Some(video.clone()),
            autoplay: Some((
                Video::from_id("another_one".into()),
                std::time::Instant::now(),
            )),
            ..Default::default()
        };
        assert!(!state.accept_cast_handoff(&video.id, 7, 8));
        assert!(state.autoplay.is_some());
        assert!(state.cast.allows_local_autoplay(8));
        assert!(!state.accept_cast_handoff("differentid", 8, 8));
        assert!(state.autoplay.is_some());
    }
}
