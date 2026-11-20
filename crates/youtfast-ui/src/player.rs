//! The player as YouTube draws it: the video, and over its bottom edge the
//! controls — the seek bar with the chapters and SponsorBlock's segments,
//! then play, next, volume and the time (or the live badge) at the left,
//! captions, the settings menu, theater and full screen at the right. They
//! show while the pointer moves over the video and stay while it is
//! paused. Notes ("+10 s"), the skip button and the countdown to the next
//! video go over the picture too; [`mini`] is the corner player other
//! pages show.

mod controls;
mod menu;
mod mini;
mod overlays;
mod surface;

pub use mini::mini;

use std::time::{Duration, Instant};

use egui::{Align, CornerRadius, CursorIcon, Layout, Rect, Sense, Ui, UiBuilder, pos2, vec2};

use youtfast_app::Action;
use youtfast_app::{PlayerMenu, State};
use youtfast_appearance::icons::Icon;
use youtfast_i18n::{Locale, gettext as tr};
use youtfast_mpv::{Status, VideoSurface};

/// How long the controls stay after the pointer stops moving.
const CONTROLS_FOR: Duration = Duration::from_millis(2500);
/// The controls' strip: the seek bar over a row of buttons.
pub(super) const STRIP: f32 = 64.0;
const HOLD_AFTER: f64 = 0.4;

#[derive(Clone)]
struct SpeedHold {
    video: String,
    started: f64,
    original: f64,
    active: bool,
}

#[derive(Clone)]
struct PendingClick {
    video: String,
    deadline: f64,
}

fn hold_id() -> egui::Id {
    egui::Id::new("player-speed-hold")
}

fn click_id() -> egui::Id {
    egui::Id::new("player-pending-click")
}

pub(super) fn focused_control(ctx: &egui::Context, id: egui::Id) -> bool {
    let focused = ctx.memory(|memory| memory.focused());
    focused.is_some_and(|focused| {
        ctx.data(|data| data.get_temp::<Vec<egui::Id>>(id))
            .is_some_and(|controls| controls.contains(&focused))
    })
}

pub(super) fn keyboard_video_focus(response: &egui::Response) -> bool {
    let ctx = &response.ctx;
    let id = egui::Id::new("player-keyboard-navigation");
    let (tab, pointer) = ctx.input(|input| {
        (
            input.key_pressed(egui::Key::Tab),
            input.pointer.any_pressed(),
        )
    });
    if tab || pointer {
        ctx.data_mut(|data| data.insert_temp(id, tab && !pointer));
    }
    // Tab can move focus away while the video response is registered. Keep
    // the controls alive for that pass so egui can give focus to the next one.
    (response.has_focus() || response.lost_focus())
        && ctx.data(|data| data.get_temp::<bool>(id).unwrap_or(false))
}

pub(super) fn remember_control_focus(response: &egui::Response) {
    response.ctx.data_mut(|data| {
        let controls =
            data.get_temp_mut_or_default::<Vec<egui::Id>>(egui::Id::new("player-control-focus"));
        if !controls.contains(&response.id) {
            controls.push(response.id);
        }
    });
}

fn controls_have_focus(ctx: &egui::Context) -> bool {
    focused_control(ctx, egui::Id::new("player-control-focus"))
}

pub(super) fn control_button(ui: &mut Ui, icon: Icon, size: f32, hint: &str) -> egui::Response {
    let response = crate::widgets::icon_button(ui, icon, size, hint);
    remember_control_focus(&response);
    response
}

/// Called after drawing the interface, including frames where the full
/// player is no longer drawn. A release, lost window focus or navigation
/// always restores the speed that was active before the long press.
pub fn release_speed_hold(ctx: &egui::Context, state: &State, actions: &mut Vec<Action>) {
    let held = ctx.input(|input| input.focused && input.pointer.primary_down());
    let watching = state.watching.as_ref().map(|video| video.id.as_str());
    if ctx
        .data(|data| data.get_temp::<PendingClick>(click_id()))
        .is_some_and(|click| {
            watching != Some(click.video.as_str())
                || state.view != youtfast_core::model::View::Watch
        })
    {
        ctx.data_mut(|data| data.remove::<PendingClick>(click_id()));
    }
    let stop = ctx
        .data(|data| data.get_temp::<SpeedHold>(hold_id()))
        .filter(|hold| {
            !held
                || watching != Some(hold.video.as_str())
                || state.view != youtfast_core::model::View::Watch
        });
    if let Some(hold) = stop {
        ctx.data_mut(|data| data.remove::<SpeedHold>(hold_id()));
        if hold.active {
            actions.push(Action::SetTemporarySpeed(hold.original));
        }
    }
}

fn speed_hold(
    ui: &Ui,
    response: &egui::Response,
    state: &State,
    status: &Status,
    actions: &mut Vec<Action>,
) -> bool {
    let locale: Locale = state.settings.language;
    let id = hold_id();
    let now = ui.input(|input| input.time);
    let held = ui.input(|input| input.pointer.primary_down());
    let hold = ui.ctx().data(|data| data.get_temp::<SpeedHold>(id));
    let mut consumed = hold.as_ref().is_some_and(|hold| hold.active);
    if let Some(mut hold) = hold {
        if held && !hold.active && now - hold.started >= HOLD_AFTER {
            hold.active = true;
            actions.push(Action::SetTemporarySpeed(2.0));
            consumed = true;
            ui.ctx().data_mut(|data| data.insert_temp(id, hold));
        }
        if held {
            ui.ctx().request_repaint_after(Duration::from_millis(30));
        }
    } else if response.is_pointer_button_down_on()
        && held
        && !status.idle
        && !status.paused
        && state.player_error.is_none()
        && !youtfast_app::player::at_live_edge(state.watching_live(), status)
        && let Some(video) = &state.watching
    {
        ui.ctx().data_mut(|data| {
            data.insert_temp(
                id,
                SpeedHold {
                    video: video.id.clone(),
                    started: now,
                    original: status.speed,
                    active: false,
                },
            );
        });
        ui.ctx().request_repaint_after(Duration::from_millis(30));
    }
    if consumed && held {
        overlays::bubble(
            ui,
            pos2(response.rect.center().x, response.rect.top() + 58.0),
            &tr(locale, "Speed 2×"),
        );
    }
    consumed
}

/// Where the player is drawn.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// On the watch page, theater mode included.
    Page,
    Fullscreen,
    Short,
}

/// The video in `rect`, with everything that goes over it.
pub fn player(
    ui: &mut Ui,
    rect: Rect,
    state: &mut State,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
    mode: Mode,
    actions: &mut Vec<Action>,
) {
    let locale: Locale = state.settings.language;
    let response = ui.interact(rect, ui.id().with("video"), Sense::click_and_drag());
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new("video-keyboard-focus"), response.id));
    surface::picture(ui, rect, state, status, surface);
    let Some(status) = status else {
        return;
    };
    let held_speed = speed_hold(ui, &response, state, status, actions);
    surface::video_context_menu(&response, state, status, actions);
    let now = Instant::now();
    let hovered = ui.rect_contains_pointer(rect);
    let moved =
        ui.input(|input| input.pointer.delta() != egui::Vec2::ZERO || input.pointer.any_pressed());
    if hovered && moved {
        state.controls_until = Some(now + CONTROLS_FOR);
    }
    let recent = state.controls_until.filter(|until| *until > now);
    let menu_open = state.player_menu != PlayerMenu::Closed;
    let keyboard_focus = keyboard_video_focus(&response);
    let shown = menu_open
        || keyboard_focus
        || controls_have_focus(ui.ctx())
        || state.seek_preview.is_some()
        || status.paused
        || state.autoplay.is_some()
        || (hovered && recent.is_some());
    if let (Some(until), false) = (recent, status.paused) {
        ui.ctx().request_repaint_after(until - now);
    }
    if hovered && !shown {
        ui.ctx().set_cursor_icon(CursorIcon::None);
    }
    let click = ui
        .ctx()
        .data(|data| data.get_temp::<PendingClick>(click_id()));
    let time = ui.input(|input| input.time);
    if response.double_clicked() && !held_speed {
        ui.ctx()
            .data_mut(|data| data.remove::<PendingClick>(click_id()));
        actions.push(Action::ToggleFullscreen);
    } else if response.clicked() && !held_speed {
        if menu_open {
            state.player_menu = PlayerMenu::Closed;
        } else if response.clicked_by(egui::PointerButton::Primary) {
            if click
                .is_some_and(|click| state.watching.as_ref().is_some_and(|v| v.id == click.video))
            {
                actions.push(Action::TogglePause);
            }
            if let Some(video) = &state.watching {
                let delay = ui
                    .ctx()
                    .memory(|memory| memory.options.input_options.max_double_click_delay);
                ui.ctx().data_mut(|data| {
                    data.insert_temp(
                        click_id(),
                        PendingClick {
                            video: video.id.clone(),
                            deadline: time + delay,
                        },
                    )
                });
                ui.ctx()
                    .request_repaint_after(Duration::from_secs_f64(delay));
            }
        } else {
            actions.push(Action::TogglePause);
        }
    } else if let Some(click) = click {
        if state
            .watching
            .as_ref()
            .is_none_or(|video| video.id != click.video)
        {
            ui.ctx()
                .data_mut(|data| data.remove::<PendingClick>(click_id()));
        } else if time >= click.deadline {
            ui.ctx()
                .data_mut(|data| data.remove::<PendingClick>(click_id()));
            actions.push(Action::TogglePause);
        } else {
            ui.ctx()
                .request_repaint_after(Duration::from_secs_f64(click.deadline - time));
        }
    }

    let retry_button = Rect::from_center_size(rect.center() + vec2(0.0, 52.0), vec2(180.0, 36.0));
    // A child, not a scope: the overlay must not advance the page's layout
    // cursor, or the metadata is drawn inside the player. It exists whether or
    // not there is an error so the page's later auto ids stay the same.
    let mut retry = ui.new_child(UiBuilder::new().max_rect(retry_button));
    if state.player_error.is_some()
        && retry
            .add(egui::Button::new(tr(locale, "Retry")).min_size(retry_button.size()))
            .clicked()
    {
        actions.push(Action::RetryPlayback);
    }

    overlays::osd(ui, rect, state);
    if state.autoplay.is_some() {
        overlays::countdown(ui, rect, state, actions);
        return;
    }
    let live = state.watching_live();
    if shown {
        ui.ctx().data_mut(|data| {
            data.insert_temp(
                egui::Id::new("player-control-focus"),
                Vec::<egui::Id>::new(),
            )
        });
    }
    let strip = Rect::from_min_max(pos2(rect.left(), rect.bottom() - STRIP), rect.max);
    overlays::skip_button(ui, rect, strip, state, status, shown, actions);
    if !shown {
        return;
    }
    surface::shade(ui, rect);
    let inner = strip.shrink2(vec2(12.0, 0.0));
    let bar = Rect::from_min_max(inner.min, pos2(inner.right(), inner.top() + 16.0));
    if live {
        // A live stream has no timeline to seek in: the bar is full.
        let track = Rect::from_center_size(bar.center(), vec2(bar.width(), 3.0));
        ui.painter()
            .rect_filled(track, CornerRadius::same(1), state.palette.accent);
    } else {
        controls::seek_bar(ui, bar, state, status, actions);
    }
    let row = Rect::from_min_max(pos2(inner.left(), bar.bottom()), inner.max);
    let mut row_controls = ui.new_child(
        UiBuilder::new()
            .max_rect(row)
            .layout(Layout::left_to_right(Align::Center)),
    );
    surface::on_video_style(&mut row_controls);
    controls::buttons(&mut row_controls, state, status, mode, live, actions);
    if state.player_menu != PlayerMenu::Closed {
        menu::menu(ui, rect, state, status, live, actions);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use youtfast_core::model::{Chapter, Details, Fetch, Segment, SegmentAction, Video};
    use youtfast_mpv::Subtitle;

    #[test]
    fn a_real_volume_slider_keeps_keyboard_focus_after_controls_timeout() {
        fn draw(
            ctx: &egui::Context,
            state: &mut State,
            status: &Status,
            events: Vec<egui::Event>,
        ) -> Vec<Action> {
            let mut actions = Vec::new();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 400.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    player(
                        ui,
                        ui.max_rect(),
                        state,
                        Some(status),
                        None,
                        Mode::Page,
                        &mut actions,
                    );
                },
            );
            output.textures_delta.clear();
            actions
        }
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State {
            watching: Some(Video::from_id("aaaaaaaaaaa".into())),
            queue: vec![Video::from_id("bbbbbbbbbbb".into())],
            ..Default::default()
        };
        let mut status = Status {
            paused: true,
            speed: 1.0,
            volume: 65.0,
            ..Default::default()
        };
        draw(&ctx, &mut state, &status, Vec::new());
        let controls = ctx
            .data(|data| data.get_temp::<Vec<egui::Id>>(egui::Id::new("player-control-focus")))
            .unwrap();
        let slider = controls
            .into_iter()
            .find(|id| {
                ctx.read_response(*id).is_some_and(|response| {
                    response.sense.senses_drag() && (response.rect.width() - 80.0).abs() < 1.0
                })
            })
            .expect("a real volume Slider was drawn");
        ctx.memory_mut(|memory| memory.request_focus(slider));
        draw(&ctx, &mut state, &status, Vec::new());
        status.paused = false;
        state.controls_until = Some(Instant::now() - CONTROLS_FOR - Duration::from_secs(1));
        for _ in 0..2 {
            draw(&ctx, &mut state, &status, Vec::new());
            assert_eq!(ctx.memory(|memory| memory.focused()), Some(slider));
        }
        let actions = draw(
            &ctx,
            &mut state,
            &status,
            vec![egui::Event::Key {
                key: egui::Key::ArrowRight,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(
            actions
                .iter()
                .any(|action| matches!(action, Action::SetVolume(value) if *value > 65.0)),
            "focused Slider stopped receiving arrows: {actions:?}"
        );
        let outside = egui::Id::new("an-unrelated-field");
        ctx.memory_mut(|memory| memory.request_focus(outside));
        assert!(
            !controls_have_focus(&ctx),
            "focus from another area must not keep player controls visible"
        );
    }

    #[test]
    fn a_long_press_restores_the_original_speed_without_pausing_on_release() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State {
            view: youtfast_core::model::View::Watch,
            watching: Some(Video::from_id("aaaaaaaaaaa".into())),
            ..State::default()
        };
        let status = Status {
            speed: 1.25,
            ..Status::default()
        };
        let pointer = pos2(240.0, 120.0);
        let button = |pressed| egui::Event::PointerButton {
            pos: pointer,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let mut actions = Vec::new();
        for (time, events) in [
            (0.0, vec![]),
            (0.1, vec![egui::Event::PointerMoved(pointer), button(true)]),
            (0.6, vec![]),
            (0.7, vec![button(false)]),
        ] {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(640.0, 360.0))),
                time: Some(time),
                events,
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                player(
                    ui,
                    ui.max_rect(),
                    &mut state,
                    Some(&status),
                    None,
                    Mode::Page,
                    &mut actions,
                );
                release_speed_hold(ui.ctx(), &state, &mut actions);
            });
            output.textures_delta.clear();
        }
        assert_eq!(
            actions,
            [
                Action::SetTemporarySpeed(2.0),
                Action::SetTemporarySpeed(1.25)
            ]
        );
    }

    #[test]
    fn single_and_double_clicks_have_distinct_playback_effects() {
        for double in [false, true] {
            let ctx = egui::Context::default();
            youtfast_appearance::theme::install_fonts(&ctx);
            let mut state = State {
                view: youtfast_core::model::View::Watch,
                watching: Some(Video::from_id("aaaaaaaaaaa".into())),
                ..State::default()
            };
            let status = Status {
                speed: 1.0,
                ..Status::default()
            };
            let pointer = pos2(240.0, 120.0);
            let button = |pressed| egui::Event::PointerButton {
                pos: pointer,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            let mut actions = Vec::new();
            let mut frames = vec![
                (0.0, vec![]),
                (0.1, vec![egui::Event::PointerMoved(pointer), button(true)]),
                (0.12, vec![button(false)]),
            ];
            if double {
                frames.extend([(0.2, vec![button(true)]), (0.22, vec![button(false)])]);
            }
            frames.push((0.6, vec![]));
            for (time, events) in frames {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(640.0, 360.0))),
                        time: Some(time),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        player(
                            ui,
                            ui.max_rect(),
                            &mut state,
                            Some(&status),
                            None,
                            Mode::Page,
                            &mut actions,
                        );
                        release_speed_hold(ui.ctx(), &state, &mut actions);
                    },
                );
                output.textures_delta.clear();
            }
            assert_eq!(
                actions,
                if double {
                    vec![Action::ToggleFullscreen]
                } else {
                    vec![Action::TogglePause]
                }
            );
        }
    }

    #[test]
    fn keyboard_focus_reveals_playing_video_controls() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State {
            view: youtfast_core::model::View::Watch,
            watching: Some(Video::from_id("aaaaaaaaaaa".into())),
            ..State::default()
        };
        let status = Status {
            paused: false,
            speed: 1.0,
            ..Status::default()
        };
        for event in [
            None,
            Some(egui::Event::Key {
                key: egui::Key::Tab,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }),
            None,
        ] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(640.0, 360.0))),
                    events: event.into_iter().collect(),
                    ..Default::default()
                },
                |ui| {
                    player(
                        ui,
                        ui.max_rect(),
                        &mut state,
                        Some(&status),
                        None,
                        Mode::Page,
                        &mut Vec::new(),
                    )
                },
            );
            output.textures_delta.clear();
        }
        let video = ctx
            .data(|data| data.get_temp::<egui::Id>(egui::Id::new("video-keyboard-focus")))
            .unwrap();
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(video));
        assert!(
            ctx.data(|data| data.get_temp::<Vec<egui::Id>>(egui::Id::new("player-control-focus")))
                .is_some_and(|controls| !controls.is_empty())
        );
    }

    #[test]
    fn leaving_the_watch_page_releases_a_speed_hold_even_with_mouse_down() {
        let ctx = egui::Context::default();
        let mut state = State {
            view: youtfast_core::model::View::Home,
            watching: Some(Video::from_id("aaaaaaaaaaa".into())),
            ..State::default()
        };
        ctx.data_mut(|data| {
            data.insert_temp(
                hold_id(),
                SpeedHold {
                    video: "aaaaaaaaaaa".into(),
                    started: 0.0,
                    original: 0.75,
                    active: true,
                },
            )
        });
        let mut actions = Vec::new();
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::PointerButton {
                    pos: pos2(100.0, 100.0),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| release_speed_hold(ui.ctx(), &state, &mut actions),
        );
        output.textures_delta.clear();
        assert_eq!(actions, [Action::SetTemporarySpeed(0.75)]);
        state.view = youtfast_core::model::View::Watch;
        release_speed_hold(&ctx, &state, &mut actions);
        assert_eq!(actions.len(), 1);
    }

    /// Every state the player draws over the video lays out without a NaN
    /// or a negative rect (egui's debug assertions panic on either), in a
    /// roomy window and a cramped one.
    #[test]
    fn the_player_draws_in_every_state() {
        let status = Status {
            position: 30.0,
            duration: 600.0,
            cached_until: 60.0,
            paused: true,
            seekable: true,
            volume: 80.0,
            speed: 1.5,
            video_width: 1920,
            video_height: 1080,
            subtitles: vec![Subtitle {
                id: 1,
                lang: "en".to_owned(),
                title: "English".to_owned(),
                selected: true,
            }],
            ..Status::default()
        };
        let mut video = Video::from_id("aaaaaaaaaaa".to_owned());
        video.title = "A video".to_owned();
        let next = Video::from_id("bbbbbbbbbbb".to_owned());
        let menus = [
            PlayerMenu::Closed,
            PlayerMenu::Main,
            PlayerMenu::Speed,
            PlayerMenu::Quality,
            PlayerMenu::Captions,
            PlayerMenu::CaptionStyle,
        ];
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        for size in [vec2(1600.0, 900.0), vec2(320.0, 180.0)] {
            for live in [false, true] {
                for audio_only in [false, true] {
                    for (menu, autoplay) in menus
                        .iter()
                        .flat_map(|menu| [(*menu, false), (*menu, true)])
                    {
                        let mut watching = video.clone();
                        watching.live = live;
                        let mut state = State {
                            watching: Some(watching),
                            up_next: vec![video.clone(), next.clone()],
                            player_menu: menu,
                            audio_only,
                            autoplay: autoplay.then(|| (next.clone(), Instant::now())),
                            osd: Some(("+10 s".to_owned(), Instant::now())),
                            segments: vec![Segment {
                                category: "intro".to_owned(),
                                start: 20.0,
                                end: 40.0,
                                action: SegmentAction::Skip,
                            }],
                            details: Fetch::Ready(Details {
                                chapters: vec![
                                    Chapter {
                                        name: "Intro".to_owned(),
                                        seconds: 0,
                                    },
                                    Chapter {
                                        name: "Middle".to_owned(),
                                        seconds: 300,
                                    },
                                ],
                                ..Default::default()
                            }),
                            ..State::default()
                        };
                        let input = egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), size)),
                            ..Default::default()
                        };
                        for mode in [Mode::Page, Mode::Fullscreen, Mode::Short] {
                            let mut output = ctx.run_ui(input.clone(), |ui| {
                                let mut actions = Vec::new();
                                let rect = ui.max_rect();
                                player(
                                    ui,
                                    rect,
                                    &mut state,
                                    Some(&status),
                                    None,
                                    mode,
                                    &mut actions,
                                );
                                mini(ui.ctx(), &state, Some(&status), None, &mut actions);
                            });
                            output.textures_delta.clear();
                        }
                    }
                }
            }
        }
    }

    /// A Short on the watch page, and the page with its queue.
    #[test]
    fn the_watch_page_draws_a_short_and_the_queue() {
        let mut short = Video::from_id("sssssssssss".to_owned());
        short.short = true;
        let mut state = State {
            view: youtfast_core::model::View::Watch,
            watching: Some(short),
            queue: vec![Video::from_id("qqqqqqqqqqq".to_owned())],
            ..State::default()
        };
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        for width in [900.0, 1600.0] {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(width, 1000.0))),
                ..Default::default()
            };
            let status = Status {
                paused: true,
                ..Status::default()
            };
            for _ in 0..2 {
                let mut output = ctx.run_ui(input.clone(), |ui| {
                    let mut actions = Vec::new();
                    crate::show(ui, &mut state, Some(&status), None, &mut actions);
                });
                output.textures_delta.clear();
            }
        }
    }

    #[test]
    fn an_error_retry_stays_out_of_the_page_flow_and_still_retries() {
        fn draw(
            ctx: &egui::Context,
            state: &mut State,
            status: &Status,
            events: Vec<egui::Event>,
            actions: &mut Vec<Action>,
        ) -> f32 {
            let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 800.0));
            let video = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 450.0));
            let mut below = 0.0;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.allocate_exact_size(video.size(), Sense::hover());
                    player(ui, video, state, Some(status), None, Mode::Page, actions);
                    below = ui.label("below the video").rect.top();
                },
            );
            output.textures_delta.clear();
            below
        }

        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State {
            watching: Some(Video::from_id("aaaaaaaaaaa".into())),
            player_error: Some("The video could not be loaded".into()),
            ..State::default()
        };
        let status = Status::default();
        let mut actions = Vec::new();
        let below = draw(&ctx, &mut state, &status, Vec::new(), &mut actions);
        assert!(
            below >= 450.0,
            "the retry button must not move the page content into the player: {below:?} < 450"
        );

        let retry = pos2(400.0, 225.0 + 52.0);
        for pressed in [true, false] {
            draw(
                &ctx,
                &mut state,
                &status,
                vec![
                    egui::Event::PointerMoved(retry),
                    egui::Event::PointerButton {
                        pos: retry,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                &mut actions,
            );
        }
        assert!(
            actions
                .iter()
                .any(|action| matches!(action, Action::RetryPlayback)),
            "clicking the retry overlay must still retry: {actions:?}"
        );
    }

    #[test]
    fn seek_bar_and_skip_button_respect_sponsorblock_category_behaviors() {
        use youtfast_core::model::{SponsorBehavior, SponsorCategory};

        fn render(state: &mut State, position: f64) -> (Vec<String>, bool, bool) {
            let ctx = egui::Context::default();
            youtfast_appearance::theme::install_fonts(&ctx);
            let status = Status {
                duration: 100.0,
                position,
                ..Default::default()
            };
            let video = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 450.0));
            let mut actions = Vec::new();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(video),
                    ..Default::default()
                },
                |ui| {
                    controls::seek_bar(
                        ui,
                        Rect::from_min_max(pos2(16.0, 400.0), pos2(784.0, 412.0)),
                        state,
                        &status,
                        &mut actions,
                    );
                    overlays::skip_button(
                        ui,
                        video,
                        Rect::from_min_max(pos2(0.0, 386.0), pos2(800.0, 450.0)),
                        state,
                        &status,
                        true,
                        &mut actions,
                    );
                },
            );
            let texts: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                    _ => None,
                })
                .collect();
            let has_sponsor_green = output.shapes.iter().any(|shape| match &shape.shape {
                egui::Shape::Rect(rect) => rect.fill == egui::Color32::from_rgb(0x00, 0xd4, 0x00),
                _ => false,
            });
            let has_intro_cyan = output.shapes.iter().any(|shape| match &shape.shape {
                egui::Shape::Rect(rect) => rect.fill == egui::Color32::from_rgb(0x00, 0xff, 0xff),
                _ => false,
            });
            output.textures_delta.clear();
            (texts, has_sponsor_green, has_intro_cyan)
        }

        let mut state = State {
            watching: Some(Video::from_id("aaaaaaaaaaa".into())),
            segments: vec![
                Segment {
                    category: "sponsor".into(),
                    start: 10.0,
                    end: 20.0,
                    action: SegmentAction::Skip,
                },
                Segment {
                    category: "intro".into(),
                    start: 30.0,
                    end: 40.0,
                    action: SegmentAction::Mute,
                },
            ],
            ..State::default()
        };

        // Ignore hides segment from seek bar and never shows skip button.
        state
            .settings
            .sponsorblock_categories
            .set(SponsorCategory::Sponsor, SponsorBehavior::Ignore);
        // SeekBarOnly draws on seek bar without showing skip button.
        state
            .settings
            .sponsorblock_categories
            .set(SponsorCategory::Intro, SponsorBehavior::SeekBarOnly);

        let (texts, green, cyan) = render(&mut state, 15.0);
        assert!(!green);
        assert!(cyan);
        assert!(!texts.iter().any(|t| t.starts_with("Skip ")));

        let (texts, _, _) = render(&mut state, 35.0);
        assert!(!texts.iter().any(|t| t.starts_with("Skip ")));

        // ManualSkip shows skip button even when action is Mute.
        state
            .settings
            .sponsorblock_categories
            .set(SponsorCategory::Intro, SponsorBehavior::ManualSkip);
        let (texts, _, cyan) = render(&mut state, 35.0);
        assert!(cyan);
        assert!(texts.iter().any(|t| t == "Skip intro"));
    }
}
