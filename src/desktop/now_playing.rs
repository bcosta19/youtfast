//! The desktop's media controls (MPRIS on Linux, where the panel's player
//! widget, the lock screen and `playerctl` read them): what is playing,
//! and the commands media keys and those panels send back.

use std::time::{Duration, Instant};

use fastframe_now_playing as np;

use crate::app::State;
use crate::app::player::{next_video, previous_video};
use crate::model::Action;
use crate::mpv::Status;
use crate::youtube::ytdlp;

/// A desktop command mapped onto what the app does with it.
#[derive(Debug, PartialEq)]
pub enum Effect {
    Action(Box<Action>),
    /// Brings the window forward.
    Raise,
    /// Closes the app.
    Quit,
}

impl Effect {
    fn action(action: Action) -> Self {
        Self::Action(Box::new(action))
    }
}

/// The controls' handle; the controls run on a thread of their own.
pub struct MediaControls {
    controls: np::NowPlaying,
    /// What the last state sent was built from, and when.
    sent: Option<(Inputs, Instant)>,
}

/// How long a state built from the same inputs is trusted: the position
/// moves on its own, and what the next and previous buttons offer depends
/// on lists that are costly to compare every frame.
const REFRESH: Duration = Duration::from_millis(500);

/// What decides the media state besides the position and the queue.
#[derive(Debug, PartialEq)]
struct Inputs {
    video: Option<(String, String)>,
    status: Option<(Option<String>, bool, bool, u64, u64)>,
}

impl Inputs {
    fn of(state: &State, status: Option<&Status>) -> Self {
        Self {
            video: state
                .watching
                .as_ref()
                .map(|video| (video.id.clone(), video.title.clone())),
            status: status.map(|status| {
                (
                    status.title.clone(),
                    status.paused,
                    status.idle,
                    status.duration.to_bits(),
                    status.volume.to_bits(),
                )
            }),
        }
    }

    /// [`Inputs::of`] equals `self`, without building it.
    fn same(&self, state: &State, status: Option<&Status>) -> bool {
        let video = match (&self.video, &state.watching) {
            (None, None) => true,
            (Some((id, title)), Some(video)) => *id == video.id && *title == video.title,
            _ => false,
        };
        let status = match (&self.status, status) {
            (None, None) => true,
            (Some((title, paused, idle, duration, volume)), Some(status)) => {
                *title == status.title
                    && *paused == status.paused
                    && *idle == status.idle
                    && *duration == status.duration.to_bits()
                    && *volume == status.volume.to_bits()
            }
            _ => false,
        };
        video && status
    }
}

impl MediaControls {
    /// Starts the controls. On the main thread: macOS delivers the events
    /// there.
    pub fn start(ctx: &egui::Context) -> Self {
        let waker = ctx.clone();
        Self {
            controls: np::NowPlaying::start(np::App::new("youtfast", "YouTFast"), move || {
                waker.request_repaint()
            }),
            sent: None,
        }
    }

    /// What the desktop asked for since the last call.
    pub fn commands(&self, state: &State) -> Vec<Effect> {
        self.controls
            .commands()
            .iter()
            .filter_map(|command| effect(command, state))
            .collect()
    }

    /// Tells the desktop what is playing now.
    pub fn update(&mut self, state: &State, status: Option<&Status>) {
        // Building the state allocates its strings: while the inputs are
        // the same, only the position moves, which the controls take at most
        // once a second.
        let now = Instant::now();
        if let Some((inputs, at)) = &self.sent
            && now.duration_since(*at) < REFRESH
            && inputs.same(state, status)
        {
            return;
        }
        self.controls.update(media_state(state, status));
        self.sent = Some((Inputs::of(state, status), now));
    }

    /// Tells clients where playback is after a seek, so they jump.
    pub fn seeked(&self, position: f64) {
        self.controls
            .seeked(Duration::from_secs_f64(position.max(0.0)));
    }
}

/// What a desktop command becomes, or `None` when it does not apply now.
fn effect(command: &np::Command, state: &State) -> Option<Effect> {
    let action = match command {
        np::Command::Play => Action::SetPaused(false),
        np::Command::Pause | np::Command::Stop => Action::SetPaused(true),
        np::Command::PlayPause => Action::TogglePause,
        np::Command::Next => Action::PlayNext,
        np::Command::Previous => Action::PlayPrevious,
        np::Command::SeekBy(ms) => Action::SeekBy(*ms as f64 / 1000.0),
        np::Command::SetPosition { track_id, position } => {
            // Only the video on screen can be sought.
            if state.watching.as_ref().is_none_or(|v| &v.id != track_id) {
                return None;
            }
            Action::SeekTo {
                seconds: position.as_secs_f64(),
                exact: true,
            }
        }
        np::Command::SetVolume(volume) => Action::SetVolume(volume.clamp(0.0, 1.0) * 100.0),
        np::Command::OpenUri(uri) if ytdlp::video_id_from_link(uri).is_some() => {
            Action::Search(uri.clone())
        }
        np::Command::Raise => return Some(Effect::Raise),
        np::Command::Quit => return Some(Effect::Quit),
        _ => return None,
    };
    Some(Effect::action(action))
}

/// What the controls show, from the video being watched and mpv's status.
fn media_state(state: &State, status: Option<&Status>) -> np::State {
    let (Some(video), Some(status)) = (&state.watching, status) else {
        return np::State::default();
    };
    if status.idle {
        return np::State::default();
    }
    let title = Some(video.title.as_str())
        .filter(|title| !title.is_empty())
        .or(status.title.as_deref())
        .unwrap_or_default()
        .to_owned();
    np::State {
        playback: if status.paused {
            np::Playback::Paused
        } else {
            np::Playback::Playing
        },
        track: Some(np::Track {
            id: video.id.clone(),
            title,
            artists: video.channel.clone().into_iter().collect(),
            duration: (status.duration > 0.0).then(|| Duration::from_secs_f64(status.duration)),
            art_url: Some(video.thumbnail()),
            url: Some(video.url()),
            ..np::Track::default()
        }),
        position: Duration::from_secs_f64(status.position.max(0.0)),
        volume: Some(status.volume.clamp(0.0, 100.0) / 100.0),
        controls: np::Controls {
            next: next_video(
                state.watching.as_ref(),
                &state.queue,
                &state.up_next,
                state
                    .details
                    .ready()
                    .map_or(&[], |details| details.related.as_slice()),
            )
            .is_some()
                || state.playing_more.is_some(),
            previous: previous_video(state.watching.as_ref(), &state.up_next).is_some(),
            seek: status.duration > 0.0,
            ..np::Controls::default()
        },
        ..np::State::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Video;

    fn watching() -> State {
        State {
            watching: Some(Video {
                title: "Video".into(),
                channel: Some("Channel".into()),
                ..Video::from_id("aaaaaaaaaaa".into())
            }),
            up_next: vec![
                Video::from_id("aaaaaaaaaaa".into()),
                Video::from_id("bbbbbbbbbbb".into()),
            ],
            ..State::default()
        }
    }

    fn playing() -> Status {
        Status {
            position: 42.0,
            duration: 300.0,
            volume: 40.0,
            ..Status::default()
        }
    }

    #[test]
    fn the_controls_show_the_video_being_watched() {
        let mut state = watching();
        let shown = media_state(&state, Some(&playing()));
        assert_eq!(shown.playback, np::Playback::Playing);
        let track = shown.track.unwrap();
        assert_eq!(track.id, "aaaaaaaaaaa");
        assert_eq!(track.title, "Video");
        assert_eq!(track.artists, ["Channel"]);
        assert_eq!(track.duration, Some(Duration::from_secs(300)));
        assert_eq!(
            track.url.as_deref(),
            Some("https://www.youtube.com/watch?v=aaaaaaaaaaa")
        );
        assert_eq!(shown.position, Duration::from_secs(42));
        assert_eq!(shown.volume, Some(0.4));
        assert!(shown.controls.next);
        assert!(shown.controls.seek);
        // Paused, and Up next ran out: the next control falls back to the
        // first related video the details brought.
        let mut status = playing();
        status.paused = true;
        state.up_next.pop();
        state.details = crate::model::Fetch::Ready(crate::model::Details {
            related: vec![Video::from_id("ccccccccccc".into())],
            ..crate::model::Details::default()
        });
        let shown = media_state(&state, Some(&status));
        assert_eq!(shown.playback, np::Playback::Paused);
        assert!(shown.controls.next);
        // And with no related videos either, Next greys out.
        state.details = crate::model::Fetch::Idle;
        let shown = media_state(&state, Some(&status));
        assert!(!shown.controls.next);
    }

    #[test]
    fn nothing_playing_stops_the_controls() {
        let state = watching();
        let mut status = playing();
        status.idle = true;
        assert_eq!(media_state(&state, Some(&status)), np::State::default());
        assert_eq!(
            media_state(&State::default(), Some(&playing())),
            np::State::default()
        );
        assert_eq!(media_state(&state, None), np::State::default());
    }

    #[test]
    fn desktop_commands_become_actions() {
        let state = watching();
        let action = |command: &np::Command| effect(command, &state);
        assert_eq!(
            action(&np::Command::Play),
            Some(Effect::action(Action::SetPaused(false)))
        );
        assert_eq!(
            action(&np::Command::Pause),
            Some(Effect::action(Action::SetPaused(true)))
        );
        assert_eq!(
            action(&np::Command::Stop),
            Some(Effect::action(Action::SetPaused(true)))
        );
        assert_eq!(
            action(&np::Command::PlayPause),
            Some(Effect::action(Action::TogglePause))
        );
        assert_eq!(
            action(&np::Command::SeekBy(-10_000)),
            Some(Effect::action(Action::SeekBy(-10.0)))
        );
        assert_eq!(
            action(&np::Command::SetPosition {
                track_id: "aaaaaaaaaaa".into(),
                position: Duration::from_secs(60),
            }),
            Some(Effect::action(Action::SeekTo {
                seconds: 60.0,
                exact: true,
            }))
        );
        // A position for a track that has since changed is ignored.
        assert_eq!(
            action(&np::Command::SetPosition {
                track_id: "ccccccccccc".into(),
                position: Duration::from_secs(60),
            }),
            None
        );
        assert_eq!(
            action(&np::Command::SetVolume(0.5)),
            Some(Effect::action(Action::SetVolume(50.0)))
        );
        assert_eq!(
            action(&np::Command::Previous),
            Some(Effect::action(Action::PlayPrevious))
        );
        // A YouTube link opens; another URI does not.
        assert_eq!(
            action(&np::Command::OpenUri("https://youtu.be/ccccccccccc".into())),
            Some(Effect::action(Action::Search(
                "https://youtu.be/ccccccccccc".into()
            )))
        );
        assert_eq!(
            action(&np::Command::OpenUri("https://example.com".into())),
            None
        );
        assert_eq!(action(&np::Command::SetShuffle(true)), None);
        assert_eq!(action(&np::Command::Raise), Some(Effect::Raise));
        assert_eq!(action(&np::Command::Quit), Some(Effect::Quit));
    }

    #[test]
    fn next_and_previous_are_dispatched_to_the_shared_player_logic() {
        let mut state = watching();
        assert_eq!(
            effect(&np::Command::Next, &state),
            Some(Effect::action(Action::PlayNext))
        );
        state.up_next.pop();
        assert_eq!(
            effect(&np::Command::Next, &state),
            Some(Effect::action(Action::PlayNext))
        );
        state.watching = None;
        assert_eq!(
            effect(&np::Command::Next, &state),
            Some(Effect::action(Action::PlayNext))
        );
    }

    #[test]
    fn desktop_next_includes_the_queue_and_previous_matches_the_source() {
        let mut state = watching();
        state.up_next = vec![
            Video::from_id("before".into()),
            state.watching.clone().unwrap(),
        ];
        state.queue = vec![Video::from_id("queued".into())];
        let media = media_state(&state, Some(&playing()));
        assert!(media.controls.next);
        assert!(media.controls.previous);
        state.queue.clear();
        assert!(!media_state(&state, Some(&playing())).controls.next);
        state.up_next.remove(0);
        assert!(!media_state(&state, Some(&playing())).controls.previous);
        state.playing_more = Some(crate::youtube::innertube::More {
            ctoken: "next".into(),
            endpoint: rustypipe::model::paginator::ContinuationEndpoint::Browse,
            visitor_data: None,
            authenticated: false,
            ours: true,
        });
        assert!(media_state(&state, Some(&playing())).controls.next);
    }
}
