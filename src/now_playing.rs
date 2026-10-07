//! The desktop's media controls (MPRIS on Linux, where the panel's player
//! widget, the lock screen and `playerctl` read them): what is playing,
//! and the commands media keys and those panels send back.

use std::time::Duration;

use fastframe_now_playing as np;

use crate::app::State;
use crate::model::{Action, Video};
use crate::mpv::Status;
use crate::youtube;

/// A desktop command mapped onto what the app does with it.
#[derive(Debug, PartialEq)]
pub enum Effect {
    Action(Action),
    /// Brings the window forward.
    Raise,
    /// Closes the app.
    Quit,
}

/// The controls' handle; the controls run on a thread of their own.
pub struct MediaControls {
    controls: np::NowPlaying,
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
        self.controls.update(media_state(state, status));
    }

    /// Tells clients where playback is after a seek, so they jump.
    pub fn seeked(&self, position: f64) {
        self.controls
            .seeked(Duration::from_secs_f64(position.max(0.0)));
    }
}

/// The video after the one being watched in Up next, or the first related
/// video when the list ran out.
fn next_video(state: &State) -> Option<Video> {
    let watching = state.watching.as_ref()?;
    if let Some(index) = state
        .up_next
        .iter()
        .position(|video| video.id == watching.id)
        && let Some(next) = state.up_next.get(index + 1)
    {
        return Some(next.clone());
    }
    state
        .details
        .ready()
        .and_then(|details| details.related.iter().find(|video| video.id != watching.id))
        .cloned()
}

/// What a desktop command becomes, or `None` when it does not apply now.
fn effect(command: &np::Command, state: &State) -> Option<Effect> {
    let action = match command {
        np::Command::Play => Action::SetPaused(false),
        np::Command::Pause | np::Command::Stop => Action::SetPaused(true),
        np::Command::PlayPause => Action::TogglePause,
        np::Command::Next => {
            return next_video(state).map(|video| Effect::Action(Action::Open(video)));
        }
        np::Command::Previous => Action::SeekTo {
            seconds: 0.0,
            exact: true,
        },
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
        np::Command::OpenUri(uri) if youtube::video_id_from_link(uri).is_some() => {
            Action::Search(uri.clone())
        }
        np::Command::Raise => return Some(Effect::Raise),
        np::Command::Quit => return Some(Effect::Quit),
        _ => return None,
    };
    Some(Effect::Action(action))
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
            next: next_video(state).is_some(),
            seek: status.duration > 0.0,
            ..np::Controls::default()
        },
        ..np::State::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            Some(Effect::Action(Action::SetPaused(false)))
        );
        assert_eq!(
            action(&np::Command::Pause),
            Some(Effect::Action(Action::SetPaused(true)))
        );
        assert_eq!(
            action(&np::Command::Stop),
            Some(Effect::Action(Action::SetPaused(true)))
        );
        assert_eq!(
            action(&np::Command::PlayPause),
            Some(Effect::Action(Action::TogglePause))
        );
        assert_eq!(
            action(&np::Command::SeekBy(-10_000)),
            Some(Effect::Action(Action::SeekBy(-10.0)))
        );
        assert_eq!(
            action(&np::Command::SetPosition {
                track_id: "aaaaaaaaaaa".into(),
                position: Duration::from_secs(60),
            }),
            Some(Effect::Action(Action::SeekTo {
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
            Some(Effect::Action(Action::SetVolume(50.0)))
        );
        assert_eq!(
            action(&np::Command::Previous),
            Some(Effect::Action(Action::SeekTo {
                seconds: 0.0,
                exact: true
            }))
        );
        // A YouTube link opens; another URI does not.
        assert_eq!(
            action(&np::Command::OpenUri("https://youtu.be/ccccccccccc".into())),
            Some(Effect::Action(Action::Search(
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
    fn next_needs_a_next_video() {
        let mut state = watching();
        assert_eq!(
            effect(&np::Command::Next, &state),
            Some(Effect::Action(Action::Open(Video::from_id(
                "bbbbbbbbbbb".into()
            ))))
        );
        state.up_next.pop();
        assert_eq!(effect(&np::Command::Next, &state), None);
        state.watching = None;
        assert_eq!(effect(&np::Command::Next, &state), None);
    }
}
