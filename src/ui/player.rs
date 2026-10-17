//! The player as YouTube draws it: the video, and over its bottom edge the
//! controls — the seek bar with the chapters and SponsorBlock's segments,
//! then play, next, volume and the time (or the live badge) at the left,
//! captions, the settings menu, theater and full screen at the right. They
//! show while the pointer moves over the video and stay while it is
//! paused. Notes ("+10 s"), the skip button and the countdown to the next
//! video go over the picture too; [`mini`] is the corner player other
//! pages show.

use std::time::{Duration, Instant};

use egui::{
    Align, Align2, Color32, CornerRadius, CursorIcon, Layout, Mesh, Rect, RichText, Sense, Shape,
    Slider, Stroke, Ui, UiBuilder, pos2, vec2,
};

use super::widgets;
use crate::app::player::{LIVE_BEHIND, SPEEDS, next_video, previous_video, speed_label};
use crate::app::{PlayerMenu, State};
use crate::icons::Icon;
use crate::model::{Action, Fetch, Quality, Resolution, Segment, Video};
use crate::mpv::{Status, VideoSurface};
use crate::theme;
use crate::util;

/// How long the controls stay after the pointer stops moving.
const CONTROLS_FOR: Duration = Duration::from_millis(2500);
/// The controls' strip: the seek bar over a row of buttons.
const STRIP: f32 = 64.0;
/// The gradient behind them, taller than the strip.
const SHADE: f32 = 140.0;
/// The settings menu's width and its rows' height.
const MENU_WIDTH: f32 = 280.0;
const MENU_ROW: f32 = 36.0;
/// The mini player's widest, and the most of the window it takes.
const MINI_WIDTH: f32 = 360.0;
const MINI_SHARE: f32 = 0.4;
const HOLD_AFTER: f64 = 0.4;

#[derive(Clone)]
struct SpeedHold {
    video: String,
    started: f64,
    original: f64,
    active: bool,
}

fn hold_id() -> egui::Id {
    egui::Id::new("player-speed-hold")
}

fn remember_control_focus(response: &egui::Response) {
    response.ctx.data_mut(|data| {
        let controls =
            data.get_temp_mut_or_default::<Vec<egui::Id>>(egui::Id::new("player-control-focus"));
        if !controls.contains(&response.id) {
            controls.push(response.id);
        }
    });
}

fn controls_have_focus(ctx: &egui::Context) -> bool {
    let focused = ctx.memory(|memory| memory.focused());
    focused.is_some_and(|focused| {
        ctx.data(|data| data.get_temp::<Vec<egui::Id>>(egui::Id::new("player-control-focus")))
            .is_some_and(|controls| controls.contains(&focused))
    })
}

fn control_button(ui: &mut Ui, icon: Icon, size: f32, hint: &str) -> egui::Response {
    let response = widgets::icon_button(ui, icon, size, hint);
    remember_control_focus(&response);
    response
}

/// Called after drawing the interface, including frames where the full
/// player is no longer drawn. A release, lost window focus or navigation
/// always restores the speed that was active before the long press.
pub fn release_speed_hold(ctx: &egui::Context, state: &State, actions: &mut Vec<Action>) {
    let held = ctx.input(|input| input.focused && input.pointer.primary_down());
    let watching = state.watching.as_ref().map(|video| video.id.as_str());
    let stop = ctx
        .data(|data| data.get_temp::<SpeedHold>(hold_id()))
        .filter(|hold| {
            !held
                || watching != Some(hold.video.as_str())
                || state.view != crate::model::View::Watch
        });
    if let Some(hold) = stop {
        ctx.data_mut(|data| data.remove::<SpeedHold>(hold_id()));
        if hold.active {
            actions.push(Action::SetSpeed(hold.original));
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
    let id = hold_id();
    let now = ui.input(|input| input.time);
    let held = ui.input(|input| input.pointer.primary_down());
    let hold = ui.ctx().data(|data| data.get_temp::<SpeedHold>(id));
    let mut consumed = hold.as_ref().is_some_and(|hold| hold.active);
    if let Some(mut hold) = hold {
        if held && !hold.active && now - hold.started >= HOLD_AFTER {
            hold.active = true;
            actions.push(Action::SetSpeed(2.0));
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
        && !state.watching_live()
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
        bubble(
            ui,
            pos2(response.rect.center().x, response.rect.top() + 58.0),
            "Velocidade 2×",
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
    let response = ui.interact(rect, ui.id().with("video"), Sense::click_and_drag());
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new("video-keyboard-focus"), response.id));
    picture(ui, rect, state, status, surface);
    let Some(status) = status else {
        return;
    };
    let held_speed = speed_hold(ui, &response, state, status, actions);
    video_context_menu(&response, state, status, actions);
    let now = Instant::now();
    let hovered = ui.rect_contains_pointer(rect);
    let moved =
        ui.input(|input| input.pointer.delta() != egui::Vec2::ZERO || input.pointer.any_pressed());
    if hovered && moved {
        state.controls_until = Some(now + CONTROLS_FOR);
    }
    let recent = state.controls_until.filter(|until| *until > now);
    let menu_open = state.player_menu != PlayerMenu::Closed;
    let shown = menu_open
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
    if response.double_clicked() && !held_speed {
        actions.push(Action::ToggleFullscreen);
    } else if response.clicked() && !held_speed {
        if menu_open {
            state.player_menu = PlayerMenu::Closed;
        } else {
            actions.push(Action::TogglePause);
        }
    }

    if state.player_error.is_some() {
        let button = Rect::from_center_size(rect.center() + vec2(0.0, 52.0), vec2(180.0, 36.0));
        ui.scope_builder(UiBuilder::new().max_rect(button), |ui| {
            if ui
                .add(egui::Button::new("Tentar novamente").min_size(button.size()))
                .clicked()
            {
                actions.push(Action::RetryPlayback);
            }
        });
    }

    osd(ui, rect, state);
    if state.autoplay.is_some() {
        countdown(ui, rect, state, actions);
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
    skip_button(ui, rect, strip, state, status, shown, actions);
    if !shown {
        return;
    }
    shade(ui, rect);
    let inner = strip.shrink2(vec2(12.0, 0.0));
    let bar = Rect::from_min_max(inner.min, pos2(inner.right(), inner.top() + 16.0));
    if live {
        // A live stream has no timeline to seek in: the bar is full.
        let track = Rect::from_center_size(bar.center(), vec2(bar.width(), 3.0));
        ui.painter()
            .rect_filled(track, CornerRadius::same(1), state.palette.accent);
    } else {
        seek_bar(ui, bar, state, status, actions);
    }
    let row = Rect::from_min_max(pos2(inner.left(), bar.bottom()), inner.max);
    let mut controls = ui.new_child(
        UiBuilder::new()
            .max_rect(row)
            .layout(Layout::left_to_right(Align::Center)),
    );
    on_video_style(&mut controls);
    buttons(&mut controls, state, status, mode, live, actions);
    if state.player_menu != PlayerMenu::Closed {
        menu(ui, rect, state, status, live, actions);
    }
}

fn video_context_menu(
    response: &egui::Response,
    state: &State,
    status: &Status,
    actions: &mut Vec<Action>,
) {
    let Some(video) = &state.watching else {
        return;
    };
    response.context_menu(|ui| {
        if ui.button("Copiar link do vídeo").clicked() {
            actions.push(Action::CopyLink(video.url()));
            ui.close();
        }
        if ui.button("Copiar link neste instante").clicked() {
            actions.push(Action::CopyLink(util::video_link(
                &video.id,
                Some(status.position.max(0.0) as u64),
            )));
            ui.close();
        }
        let mut looping = state.playback_loop || video.short;
        if ui
            .add_enabled(
                !video.short,
                egui::Checkbox::new(&mut looping, "Repetir vídeo"),
            )
            .changed()
        {
            actions.push(Action::ToggleLoop);
        }
        if ui.button("Abrir no navegador").clicked() {
            actions.push(Action::OpenUrl(video.url()));
            ui.close();
        }
    });
}

/// The picture, or the error over black, and the spinner while it opens or
/// waits for the network.
fn picture(
    ui: &mut Ui,
    rect: Rect,
    state: &State,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
) {
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, Color32::BLACK);
    if let Some(surface) = surface {
        surface.paint(ui, rect);
    }
    #[cfg(feature = "demo")]
    if surface.is_none()
        && ui
            .ctx()
            .data(|d| d.get_temp::<bool>(egui::Id::new("synthetic-images")))
            .unwrap_or(false)
    {
        super::images::remote(ui, "demo")
            .fit_to_exact_size(rect.size())
            .paint_at(ui, rect);
    }
    if let Some(error) = &state.player_error {
        let galley = ui.painter().layout(
            error.clone(),
            theme::regular(16.0),
            Color32::WHITE,
            (rect.width() - 32.0).max(1.0),
        );
        ui.painter()
            .galley(rect.center() - galley.size() / 2.0, galley, Color32::WHITE);
    } else if status.is_some_and(|status| {
        !status.idle && (status.buffering || (status.position <= 0.0 && !status.paused))
    }) {
        let size = (rect.height() / 6.0).clamp(20.0, 40.0);
        egui::Spinner::new()
            .size(size)
            .color(Color32::WHITE)
            .paint_at(ui, Rect::from_center_size(rect.center(), vec2(size, size)));
        ui.ctx().request_repaint();
    }
}

/// White text and icons, and hovers that read over any picture, whatever
/// the theme.
fn on_video_style(ui: &mut Ui) {
    let visuals = ui.visuals_mut();
    visuals.override_text_color = Some(Color32::WHITE);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_white_alpha(36);
    visuals.widgets.inactive.bg_fill = Color32::from_white_alpha(70);
    visuals.widgets.hovered.bg_fill = Color32::WHITE;
    visuals.widgets.active.bg_fill = Color32::WHITE;
    visuals.selection.bg_fill = Color32::WHITE;
    visuals.slider_trailing_fill = true;
}

/// The gradient that keeps the controls readable over a bright picture.
fn shade(ui: &Ui, rect: Rect) {
    let top = (rect.bottom() - SHADE).max(rect.top());
    let shade = Rect::from_min_max(pos2(rect.left(), top), rect.max);
    let mut mesh = Mesh::default();
    let clear = Color32::from_black_alpha(0);
    let dark = Color32::from_black_alpha(210);
    mesh.colored_vertex(shade.left_top(), clear);
    mesh.colored_vertex(shade.right_top(), clear);
    mesh.colored_vertex(shade.right_bottom(), dark);
    mesh.colored_vertex(shade.left_bottom(), dark);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    ui.painter().add(Shape::mesh(mesh));
}

/// The row under the seek bar.
fn buttons(
    ui: &mut Ui,
    state: &mut State,
    status: &Status,
    mode: Mode,
    live: bool,
    actions: &mut Vec<Action>,
) -> (Rect, Rect) {
    // Reserve the entire right-hand group before laying out optional
    // volume/time/chapter content. A long title must never push a control
    // beyond the edge of the video.
    let row = ui.available_rect_before_wrap();
    let spacing = 4.0;
    let page_controls = mode == Mode::Page && !state.watching_short();
    let fullscreen_button = mode != Mode::Short || row.width() >= 152.0;
    let captions_button =
        !status.subtitles.is_empty() && (mode != Mode::Short || row.width() >= 188.0);
    let mute_button = mode != Mode::Short || row.width() >= 112.0;
    let right_count = 1
        + usize::from(fullscreen_button)
        + usize::from(page_controls) * 2
        + usize::from(captions_button);
    let speed =
        (!live && status.speed != 1.0 && row.width() >= 900.0).then(|| speed_label(status.speed));
    let speed_width = speed.as_ref().map_or(0.0, |label| {
        ui.painter()
            .layout_no_wrap(label.clone(), theme::medium(13.0), Color32::WHITE)
            .size()
            .x
            + spacing
    });
    let right_width = right_count as f32 * 32.0 + (right_count - 1) as f32 * spacing + speed_width;
    let right_rect = Rect::from_min_max(
        pos2((row.right() - right_width).max(row.left()), row.top()),
        row.max,
    );
    let left_rect = Rect::from_min_max(
        row.min,
        pos2((right_rect.left() - 8.0).max(row.left()), row.bottom()),
    );
    let mut right = ui.new_child(
        UiBuilder::new()
            .id_salt("right-controls")
            .max_rect(right_rect),
    );
    right.spacing_mut().item_spacing.x = spacing;
    let mut left = ui.new_child(
        UiBuilder::new()
            .id_salt("left-controls")
            .max_rect(left_rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    left.spacing_mut().item_spacing.x = spacing;
    let ui = &mut left;
    let (icon, hint) = if status.paused {
        (Icon::Play, "Reproduzir (k)")
    } else {
        (Icon::Pause, "Pausar (k)")
    };
    if control_button(ui, icon, 20.0, hint).clicked() {
        actions.push(Action::TogglePause);
    }
    let related = state
        .details
        .ready()
        .map_or(&[][..], |details| details.related.as_slice());
    let next = next_video(
        state.watching.as_ref(),
        &state.queue,
        &state.up_next,
        related,
    );
    if !live && mode != Mode::Short && (next.is_some() || state.playing_more.is_some()) {
        let hint = next
            .map(next_hint)
            .unwrap_or_else(|| "Carregar próximos vídeos / tentar novamente (Shift+N)".into());
        if control_button(ui, Icon::SkipForward, 18.0, &hint).clicked() {
            actions.push(Action::PlayNext);
        }
    }
    let (icon, hint) = if status.muted || status.volume <= 0.0 {
        (Icon::VolumeMuted, "Ativar som (m)")
    } else {
        (Icon::Volume, "Silenciar (m)")
    };
    if mute_button && control_button(ui, icon, 18.0, hint).clicked() {
        actions.push(Action::ToggleMute);
    }
    if mode != Mode::Short && left_rect.width() >= 300.0 {
        ui.spacing_mut().slider_width = 80.0;
        let mut volume = if status.muted { 0.0 } else { status.volume };
        let response = ui.add(Slider::new(&mut volume, 0.0..=100.0).show_value(false));
        remember_control_focus(&response);
        if response.changed() {
            actions.push(Action::SetVolume(volume));
        }
    }
    ui.add_space(4.0);
    if live && ui.available_width() >= 24.0 {
        live_badge(ui, state, status, actions);
    } else {
        time_label(
            ui,
            state,
            status,
            mode != Mode::Short && row.width() >= 720.0,
            actions,
        );
    }

    right.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let (icon, hint) = if mode == Mode::Fullscreen {
            (Icon::ExitFullscreen, "Sair da tela cheia (f)")
        } else {
            (Icon::Fullscreen, "Tela cheia (f)")
        };
        if fullscreen_button && control_button(ui, icon, 18.0, hint).clicked() {
            actions.push(Action::ToggleFullscreen);
        }
        // Full screen has no page layout to change.
        if page_controls {
            let hint = if state.settings.theater {
                "Visualização padrão (t)"
            } else {
                "Modo cinema (t)"
            };
            if control_button(ui, Icon::Theater, 18.0, hint).clicked() {
                actions.push(Action::ToggleTheater);
            }
            if control_button(ui, Icon::MiniPlayer, 18.0, "Miniplayer (i)").clicked() {
                actions.push(Action::ToggleMiniPlayer);
            }
        }
        if control_button(ui, Icon::Settings, 18.0, "Configurações").clicked() {
            state.player_menu = if state.player_menu == PlayerMenu::Closed {
                PlayerMenu::Main
            } else {
                PlayerMenu::Closed
            };
        }
        if captions_button {
            let on = status.subtitles.iter().any(|subtitle| subtitle.selected);
            let response = control_button(ui, Icon::Captions, 18.0, "Legendas (c)");
            if on {
                // YouTube's red line under the button while they show.
                let line = Rect::from_center_size(
                    pos2(response.rect.center().x, response.rect.bottom() - 4.0),
                    vec2(18.0, 2.5),
                );
                ui.painter()
                    .rect_filled(line, CornerRadius::same(1), state.palette.accent);
            }
            if response.clicked() {
                actions.push(Action::ToggleCaptions);
            }
        }
        if let Some(speed) = speed {
            ui.label(
                RichText::new(speed)
                    .font(theme::medium(13.0))
                    .color(Color32::WHITE),
            );
        }
    });
    (left.min_rect(), right.min_rect())
}

/// `Next: <title> (N)` for the next button's hover.
fn next_hint(video: &Video) -> String {
    if video.title.is_empty() {
        "Próximo (Shift+N)".to_owned()
    } else {
        format!("Próximo: {} (Shift+N)", video.title)
    }
}

/// `1:23 / 12:04 • Chapter`, the chapter when the video has them.
fn time_label(
    ui: &mut Ui,
    state: &State,
    status: &Status,
    show_chapter: bool,
    actions: &mut Vec<Action>,
) {
    let position = state.seek_preview.unwrap_or(status.position);
    let time = if status.duration > 0.0 {
        format!(
            "{} / {}",
            util::duration(position),
            util::duration(status.duration)
        )
    } else {
        util::duration(position)
    };
    let available = ui.available_width().max(0.0);
    if available < 28.0 {
        return;
    }
    let font = theme::regular(13.0);
    let full_width = ui
        .painter()
        .layout_no_wrap(time.clone(), font.clone(), Color32::WHITE)
        .size()
        .x
        + 4.0;
    let text = if full_width <= available {
        time.clone()
    } else {
        util::duration(position)
    };
    ui.add_sized(
        vec2(full_width.min(available), 32.0),
        egui::Label::new(RichText::new(text).font(font).color(Color32::WHITE)).truncate(),
    )
    .on_hover_text(time);
    if show_chapter
        && ui.available_width() >= 80.0
        && let Some(chapter) = chapter_at(state, position)
    {
        let response = ui
            .add_sized(
                vec2(ui.available_width(), 32.0),
                egui::Label::new(
                    RichText::new(format!("• {chapter}"))
                        .font(theme::regular(13.0))
                        .color(Color32::from_white_alpha(200)),
                )
                .sense(Sense::click())
                .truncate(),
            )
            .on_hover_text("Abrir capítulos");
        remember_control_focus(&response);
        egui::Popup::menu(&response).show(|ui| {
            if let Some(details) = state.details.ready() {
                for chapter in &details.chapters {
                    let label = format!(
                        "{}  {}",
                        util::duration(f64::from(chapter.seconds)),
                        chapter.name
                    );
                    if ui.button(label).clicked() {
                        actions.push(Action::SeekTo {
                            seconds: f64::from(chapter.seconds),
                            exact: true,
                        });
                        ui.close();
                    }
                }
            }
        });
    }
}

/// The chapter `seconds` falls in: the last one started by then.
fn chapter_at(state: &State, seconds: f64) -> Option<&str> {
    let details = state.details.ready()?;
    details
        .chapters
        .iter()
        .rev()
        .find(|chapter| f64::from(chapter.seconds) <= seconds)
        .map(|chapter| chapter.name.as_str())
}

/// `● LIVE`: red at the live edge; grey and a button back to it when
/// behind.
fn live_badge(ui: &mut Ui, state: &State, status: &Status, actions: &mut Vec<Action>) {
    let edge = status.duration.max(status.cached_until);
    let behind = edge > 0.0 && edge - status.position > LIVE_BEHIND;
    let width = ui.available_width().clamp(24.0, 86.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 24.0), Sense::click());
    remember_control_focus(&response);
    let dot = if behind {
        Color32::from_gray(170)
    } else {
        state.palette.accent
    };
    if response.hovered() && behind {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        ui.painter()
            .rect_filled(rect, CornerRadius::same(12), Color32::from_white_alpha(36));
    }
    let painter = ui.painter();
    painter.circle_filled(pos2(rect.left() + 10.0, rect.center().y), 4.0, dot);
    if width >= 76.0 {
        painter.text(
            pos2(rect.left() + 20.0, rect.center().y),
            Align2::LEFT_CENTER,
            "AO VIVO",
            theme::semibold(13.0),
            Color32::WHITE,
        );
    }
    let response = response.on_hover_text(if behind {
        "Voltar ao vivo"
    } else {
        "Assistindo ao vivo"
    });
    if behind && response.clicked() {
        actions.push(Action::GoLive);
    }
}

/// The seek bar: a thin rounded track, played in `accent`, buffered in
/// white, cut at each chapter and marked with SponsorBlock's segments; it
/// thickens under the pointer, and a bubble over it names the time and the
/// chapter there.
fn seek_bar(
    ui: &mut Ui,
    rect: Rect,
    state: &mut State,
    status: &Status,
    actions: &mut Vec<Action>,
) {
    let response = ui.interact(rect, ui.id().with("seek-bar"), Sense::click_and_drag());
    remember_control_focus(&response);
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new("timeline-keyboard-focus"), response.id));
    let duration = status.duration;
    let at_pointer = |pointer: egui::Pos2| {
        f64::from(((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0)) * duration
    };
    if duration > 0.0 {
        let pointer = response.interact_pointer_pos().map(at_pointer);
        if response.drag_stopped() || response.clicked() {
            if let Some(seconds) = pointer.or(state.seek_preview) {
                actions.push(Action::SeekTo {
                    seconds,
                    exact: true,
                });
            }
            state.seek_preview = None;
        } else if response.dragged()
            && let Some(seconds) = pointer
        {
            if state
                .seek_preview
                .is_none_or(|shown| (shown - seconds).abs() >= 0.5)
            {
                actions.push(Action::SeekTo {
                    seconds,
                    exact: false,
                });
            }
            state.seek_preview = Some(seconds);
        }
    }
    let active = (response.hovered() || response.dragged()) && duration > 0.0;
    if active {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let thickness = if active { 5.0 } else { 3.0 };
    let track = Rect::from_center_size(rect.center(), vec2(rect.width(), thickness));
    let x_at = |seconds: f64| {
        let fraction = if duration > 0.0 {
            (seconds / duration).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        track.left() + track.width() * fraction
    };
    let position = state.seek_preview.unwrap_or(status.position);
    let painter = ui.painter();
    let span = |from: f64, to: f64| {
        Rect::from_min_max(
            pos2(x_at(from), track.top()),
            pos2(x_at(to), track.bottom()),
        )
    };
    painter.rect_filled(track, CornerRadius::ZERO, Color32::from_white_alpha(60));
    painter.rect_filled(
        span(0.0, status.cached_until),
        CornerRadius::ZERO,
        Color32::from_white_alpha(110),
    );
    painter.rect_filled(
        span(0.0, position),
        CornerRadius::ZERO,
        state.palette.accent,
    );
    for segment in &state.segments {
        painter.rect_filled(
            span(segment.start, segment.end),
            CornerRadius::ZERO,
            segment_color(segment),
        );
    }
    // The chapters: a gap in the track where each one starts.
    if let Fetch::Ready(details) = &state.details {
        for chapter in details
            .chapters
            .iter()
            .filter(|chapter| chapter.seconds > 0)
        {
            let x = x_at(f64::from(chapter.seconds));
            let gap = Rect::from_center_size(pos2(x, track.center().y), vec2(2.0, thickness + 1.0));
            painter.rect_filled(gap, CornerRadius::ZERO, Color32::from_black_alpha(200));
        }
    }
    if active {
        painter.circle_filled(
            pos2(x_at(position), track.center().y),
            7.0,
            state.palette.accent,
        );
        if let Some(pointer) = response.hover_pos().or(response.interact_pointer_pos()) {
            let seconds = at_pointer(pointer);
            let mut text = util::duration(seconds);
            if let Some(chapter) = chapter_at(state, seconds) {
                text = format!("{chapter}\n{text}");
            }
            super::extras::seek_preview(ui, rect, state, seconds, actions);
            bubble(ui, pos2(pointer.x, rect.top() - 6.0), &text);
        }
    }
}

/// SponsorBlock's colours, by category.
fn segment_color(segment: &Segment) -> Color32 {
    match segment.category.as_str() {
        "sponsor" => Color32::from_rgb(0x00, 0xd4, 0x00),
        "selfpromo" => Color32::from_rgb(0xff, 0xff, 0x00),
        "interaction" => Color32::from_rgb(0xcc, 0x00, 0xff),
        "intro" => Color32::from_rgb(0x00, 0xff, 0xff),
        "outro" => Color32::from_rgb(0x02, 0x02, 0xed),
        "preview" => Color32::from_rgb(0x00, 0x8f, 0xd6),
        "music_offtopic" => Color32::from_rgb(0xff, 0x99, 0x00),
        _ => Color32::from_rgb(0x73, 0x00, 0xff),
    }
}

/// A dark rounded label, centred over `bottom`.
fn bubble(ui: &Ui, bottom: egui::Pos2, text: &str) {
    let galley = ui
        .painter()
        .layout(text.to_owned(), theme::medium(13.0), Color32::WHITE, 320.0);
    let size = galley.size() + vec2(16.0, 8.0);
    let clip = ui.clip_rect();
    let left =
        (bottom.x - size.x / 2.0).clamp(clip.left(), (clip.right() - size.x).max(clip.left()));
    let rect = Rect::from_min_size(pos2(left, bottom.y - size.y), size);
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(6), Color32::from_black_alpha(210));
    painter.galley(rect.min + vec2(8.0, 4.0), galley, Color32::WHITE);
}

/// The note over the video's top ("+10 s", "Captions on").
fn osd(ui: &Ui, rect: Rect, state: &State) {
    if let Some((text, _)) = &state.osd {
        let top = pos2(rect.center().x, rect.top() + 24.0);
        let galley =
            ui.painter()
                .layout_no_wrap(text.clone(), theme::semibold(15.0), Color32::WHITE);
        let size = galley.size() + vec2(24.0, 12.0);
        let pill = Rect::from_center_size(top + vec2(0.0, size.y / 2.0), size);
        ui.painter().rect_filled(
            pill,
            CornerRadius::same((size.y / 2.0) as u8),
            Color32::from_black_alpha(170),
        );
        ui.painter()
            .galley(pill.min + vec2(12.0, 6.0), galley, Color32::WHITE);
    }
}

/// "Skip intro" while a segment the user skips by hand plays; also shown
/// for one skipped at once that the user went back into.
fn skip_button(
    ui: &mut Ui,
    rect: Rect,
    strip: Rect,
    state: &State,
    status: &Status,
    shown: bool,
    actions: &mut Vec<Action>,
) {
    let Some(segment) = state
        .segments
        .iter()
        .find(|segment| status.position >= segment.start && status.position < segment.end - 0.3)
    else {
        return;
    };
    let label = segment.label();
    let text = format!("Pular {label}");
    let galley = ui
        .painter()
        .layout_no_wrap(text, theme::semibold(14.0), Color32::WHITE);
    let size = galley.size() + vec2(28.0, 16.0);
    let bottom = if shown {
        strip.top() - 12.0
    } else {
        rect.bottom() - 16.0
    };
    let button = Rect::from_min_size(pos2(rect.right() - 16.0 - size.x, bottom - size.y), size);
    let response = ui.interact(button, ui.id().with("skip-segment"), Sense::click());
    remember_control_focus(&response);
    let fill = if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        Color32::from_black_alpha(220)
    } else {
        Color32::from_black_alpha(170)
    };
    ui.painter().rect(
        button,
        CornerRadius::same(4),
        fill,
        Stroke::new(1.0, Color32::from_white_alpha(90)),
        egui::StrokeKind::Inside,
    );
    ui.painter()
        .galley(button.min + vec2(14.0, 8.0), galley, Color32::WHITE);
    if response.clicked() {
        actions.push(Action::SkipSegment);
    }
}

/// The end of the video: the next one, a countdown, Cancel and Play now.
fn countdown(ui: &mut Ui, rect: Rect, state: &State, actions: &mut Vec<Action>) {
    let Some((video, at)) = &state.autoplay else {
        return;
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, Color32::from_black_alpha(200));
    let seconds = at.saturating_duration_since(Instant::now()).as_secs() + 1;
    let card = Rect::from_center_size(rect.center(), vec2(rect.width().min(420.0) - 24.0, 200.0));
    let mut ui = ui.new_child(
        UiBuilder::new()
            .max_rect(card)
            .layout(Layout::top_down(Align::Center)),
    );
    on_video_style(&mut ui);
    ui.label(
        RichText::new(format!("Próximo vídeo em {seconds}"))
            .font(theme::regular(14.0))
            .color(Color32::from_white_alpha(200)),
    );
    ui.add_space(6.0);
    let title = if video.title.is_empty() {
        "Próximo vídeo"
    } else {
        &video.title
    };
    ui.add(
        egui::Label::new(
            RichText::new(title)
                .font(theme::bold(18.0))
                .color(Color32::WHITE),
        )
        .truncate(),
    );
    if let Some(channel) = &video.channel {
        ui.label(
            RichText::new(channel)
                .font(theme::regular(13.0))
                .color(Color32::from_white_alpha(200)),
        );
    }
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        let width = 2.0 * 110.0 + 12.0;
        ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
        let cancel = egui::Button::new(RichText::new("Cancelar").font(theme::medium(14.0)))
            .fill(Color32::from_white_alpha(40))
            .min_size(vec2(110.0, 36.0))
            .corner_radius(CornerRadius::same(18));
        if ui.add(cancel).clicked() {
            actions.push(Action::CancelAutoplay);
        }
        ui.add_space(12.0);
        let play = egui::Button::new(
            RichText::new("Reproduzir agora")
                .font(theme::medium(14.0))
                .color(Color32::BLACK),
        )
        .fill(Color32::WHITE)
        .min_size(vec2(110.0, 36.0))
        .corner_radius(CornerRadius::same(18));
        if ui.add(play).clicked() {
            actions.push(Action::PlayNext);
        }
    });
}

/// The settings menu over the video's bottom right: speed, quality,
/// captions and autoplay, each opening its list.
fn menu(
    ui: &mut Ui,
    video: Rect,
    state: &mut State,
    status: &Status,
    live: bool,
    actions: &mut Vec<Action>,
) {
    let rows = menu_rows(state, status, live);
    let room = (video.height() - STRIP - 24.0).max(MENU_ROW * 2.0);
    let height = (rows as f32 * MENU_ROW + 16.0).min(room);
    // A portrait video can be narrower than a usable settings menu. Let
    // the foreground menu use the surrounding page while staying inside
    // the window; otherwise hidden Short controls would be inaccessible.
    let bounds = ui.clip_rect().intersect(ui.ctx().content_rect());
    let width = MENU_WIDTH.min(bounds.width().max(1.0));
    let rect = Rect::from_min_size(
        pos2(
            (video.right() - 12.0 - width)
                .clamp(bounds.left(), (bounds.right() - width).max(bounds.left())),
            (video.bottom() - STRIP - 8.0 - height)
                .clamp(bounds.top(), (bounds.bottom() - height).max(bounds.top())),
        ),
        vec2(width, height),
    );
    egui::Area::new(ui.id().with("player-settings-menu"))
        .fixed_pos(rect.min)
        .order(egui::Order::Foreground)
        .movable(false)
        .constrain_to(bounds)
        .show(ui.ctx(), |ui| {
            ui.set_width(width);
            ui.set_height(height);
            let rect = ui.max_rect();
            ui.painter()
                .rect_filled(rect, CornerRadius::same(12), Color32::from_black_alpha(225));
            // The menu takes its clicks before the video under it.
            ui.interact(rect, ui.id().with("player-menu-back"), Sense::click());
            let mut ui = ui.new_child(
                UiBuilder::new()
                    .max_rect(rect.shrink2(vec2(0.0, 8.0)))
                    .layout(Layout::top_down(Align::Min)),
            );
            on_video_style(&mut ui);
            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .show(&mut ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    match state.player_menu {
                        PlayerMenu::Closed => {}
                        PlayerMenu::Main => main_menu(ui, state, status, live, actions),
                        PlayerMenu::Speed => {
                            if back_row(ui, "Velocidade de reprodução") {
                                state.player_menu = PlayerMenu::Main;
                            }
                            for speed in SPEEDS {
                                let label = if speed == 1.0 {
                                    "Normal".to_owned()
                                } else {
                                    speed_label(speed)
                                };
                                if pick_row(ui, &label, (status.speed - speed).abs() < 0.01) {
                                    actions.push(Action::SetSpeed(speed));
                                    state.player_menu = PlayerMenu::Closed;
                                }
                            }
                        }
                        PlayerMenu::Quality => {
                            if back_row(ui, "Qualidade") {
                                state.player_menu = PlayerMenu::Main;
                            }
                            let auto = format!("Automática (até {}p)", Quality::AUTO);
                            if pick_row(ui, &auto, state.settings.quality == Quality::Auto) {
                                actions.push(Action::SetQuality(Quality::Auto));
                                state.player_menu = PlayerMenu::Closed;
                            }
                            for resolution in offered(state) {
                                let quality = Quality::Max(resolution.lines);
                                if pick_row(
                                    ui,
                                    &resolution.label(),
                                    state.settings.quality == quality,
                                ) {
                                    actions.push(Action::SetQuality(quality));
                                    state.player_menu = PlayerMenu::Closed;
                                }
                            }
                        }
                        PlayerMenu::Captions => {
                            if back_row(ui, "Legendas") {
                                state.player_menu = PlayerMenu::Main;
                            }
                            let none = !status.subtitles.iter().any(|subtitle| subtitle.selected);
                            if pick_row(ui, "Desativadas", none) {
                                actions.push(Action::SetSubtitle(None));
                                state.player_menu = PlayerMenu::Closed;
                            }
                            for subtitle in &status.subtitles {
                                if pick_row(ui, &subtitle.title, subtitle.selected) {
                                    actions.push(Action::SetSubtitle(Some(subtitle.id)));
                                    state.player_menu = PlayerMenu::Closed;
                                }
                            }
                            if value_row(ui, "Aparência das legendas", "").clicked() {
                                state.player_menu = PlayerMenu::CaptionStyle;
                            }
                        }
                        PlayerMenu::CaptionStyle => {
                            if back_row(ui, "Aparência das legendas") {
                                state.player_menu = PlayerMenu::Captions;
                            }
                            ui.horizontal(|ui| {
                                ui.label("Tamanho");
                                let mut scale = state.settings.caption_scale;
                                if ui
                                    .add(Slider::new(&mut scale, 0.5..=2.0).suffix("×"))
                                    .changed()
                                {
                                    actions.push(Action::SetCaptionScale(scale));
                                }
                            });
                            let mut background = state.settings.caption_background;
                            if ui.checkbox(&mut background, "Fundo escuro").changed() {
                                actions.push(Action::SetCaptionBackground(background));
                            }
                        }
                    }
                });
        });
}

/// How many rows the open page of the menu has.
fn menu_rows(state: &State, status: &Status, live: bool) -> usize {
    match state.player_menu {
        PlayerMenu::Closed => 0,
        PlayerMenu::Main => 4 + usize::from(!live) + usize::from(!status.subtitles.is_empty()),
        PlayerMenu::Speed => 1 + SPEEDS.len(),
        PlayerMenu::Quality => 2 + offered(state).len(),
        PlayerMenu::Captions => 3 + status.subtitles.len(),
        PlayerMenu::CaptionStyle => 4,
    }
}

/// What the video offers, highest first; the usual resolutions until it
/// has loaded.
fn offered(state: &State) -> Vec<Resolution> {
    if state.resolutions.is_empty() {
        [2160, 1440, 1080, 720, 480, 360]
            .map(|lines| Resolution { lines, fps: 0 })
            .to_vec()
    } else {
        state.resolutions.clone()
    }
}

fn main_menu(
    ui: &mut Ui,
    state: &mut State,
    status: &Status,
    live: bool,
    actions: &mut Vec<Action>,
) {
    if value_row(
        ui,
        if state.fullscreen {
            "Sair da tela cheia"
        } else {
            "Tela cheia"
        },
        "F",
    )
    .clicked()
    {
        actions.push(Action::ToggleFullscreen);
        state.player_menu = PlayerMenu::Closed;
    }
    if value_row(ui, "Som", if status.muted { "Ativar" } else { "Silenciar" }).clicked() {
        actions.push(Action::ToggleMute);
    }
    let on = state.settings.autoplay;
    if value_row(
        ui,
        "Reprodução automática",
        if on { "Ativada" } else { "Desativada" },
    )
    .clicked()
    {
        actions.push(Action::SetAutoplay(!on));
    }
    if !status.subtitles.is_empty() {
        let current = status
            .subtitles
            .iter()
            .find(|subtitle| subtitle.selected)
            .map_or("Desativadas", |subtitle| subtitle.title.as_str());
        if value_row(ui, "Legendas", current).clicked() {
            state.player_menu = PlayerMenu::Captions;
        }
    }
    if !live {
        let speed = if status.speed == 1.0 {
            "Normal".to_owned()
        } else {
            speed_label(status.speed)
        };
        if value_row(ui, "Velocidade de reprodução", &speed).clicked() {
            state.player_menu = PlayerMenu::Speed;
        }
    }
    let playing = (status.video_width > 0 && status.video_height > 0).then(|| {
        Resolution::from_size(
            status.video_width as u32,
            status.video_height as u32,
            status.fps.round() as u32,
        )
    });
    let quality = match (state.settings.quality, playing) {
        (Quality::Auto, Some(playing)) => format!("Automática · {}", playing.label()),
        (Quality::Auto, None) => "Automática".to_owned(),
        (_, Some(playing)) => playing.label(),
        (Quality::Max(lines), None) => format!("{lines}p"),
    };
    if value_row(ui, "Qualidade", &quality)
        .on_hover_text(playback_details(status))
        .clicked()
    {
        state.player_menu = PlayerMenu::Quality;
    }
}

/// A menu row: the hover tint across it.
fn menu_row(ui: &mut Ui) -> (Rect, egui::Response) {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), MENU_ROW), Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, Color32::from_white_alpha(28));
    }
    (rect, response)
}

/// `Name                value ›`.
fn value_row(ui: &mut Ui, name: &str, value: &str) -> egui::Response {
    let (rect, response) = menu_row(ui);
    let painter = ui.painter();
    painter.text(
        pos2(rect.left() + 16.0, rect.center().y),
        Align2::LEFT_CENTER,
        name,
        theme::medium(14.0),
        Color32::WHITE,
    );
    let chevron =
        Rect::from_center_size(pos2(rect.right() - 18.0, rect.center().y), vec2(14.0, 14.0));
    Icon::ChevronRight
        .image(Color32::WHITE, 14.0)
        .paint_at(ui, chevron);
    // The value, cut short to leave the name its room.
    let room = rect.width() * 0.5;
    let galley = painter.layout(
        value.to_owned(),
        theme::regular(13.0),
        Color32::from_white_alpha(200),
        room,
    );
    let galley = if galley.rows.len() > 1 {
        let short: String = value.chars().take(18).collect();
        painter.layout_no_wrap(
            format!("{short}…"),
            theme::regular(13.0),
            Color32::from_white_alpha(200),
        )
    } else {
        galley
    };
    let at = pos2(
        chevron.left() - 6.0 - galley.size().x,
        rect.center().y - galley.size().y / 2.0,
    );
    painter.galley(at, galley, Color32::WHITE);
    response
}

/// `‹ Title`, back to the main page; true when clicked.
fn back_row(ui: &mut Ui, title: &str) -> bool {
    let (rect, response) = menu_row(ui);
    Icon::ChevronLeft.image(Color32::WHITE, 14.0).paint_at(
        ui,
        Rect::from_center_size(pos2(rect.left() + 18.0, rect.center().y), vec2(14.0, 14.0)),
    );
    ui.painter().text(
        pos2(rect.left() + 34.0, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        theme::semibold(14.0),
        Color32::WHITE,
    );
    response.clicked()
}

/// `✓ Choice`; true when clicked.
fn pick_row(ui: &mut Ui, label: &str, picked: bool) -> bool {
    let (rect, response) = menu_row(ui);
    if picked {
        Icon::Check.image(Color32::WHITE, 14.0).paint_at(
            ui,
            Rect::from_center_size(pos2(rect.left() + 18.0, rect.center().y), vec2(14.0, 14.0)),
        );
    }
    ui.painter().text(
        pos2(rect.left() + 34.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        theme::regular(14.0),
        Color32::WHITE,
    );
    response.clicked()
}

/// "1920×1080 · 60 fps · vp9 · hardware decoding (vaapi)".
fn playback_details(status: &Status) -> String {
    if status.video_width <= 0 || status.video_height <= 0 {
        return "Qualidade".to_owned();
    }
    let mut parts = vec![format!("{}×{}", status.video_width, status.video_height)];
    if status.fps > 0.0 {
        parts.push(format!("{:.0} fps", status.fps));
    }
    if let Some(codec) = &status.codec {
        parts.push(codec.clone());
    }
    parts.push(match &status.hwdec {
        Some(decoder) => format!("decodificação por hardware ({decoder})"),
        None => "decodificação por software".to_owned(),
    });
    parts.join(" · ")
}

/// The corner player other pages show while a video is open: the picture
/// with play, close and back-to-the-page over it on hover, a thin progress
/// line, and the title under it. A click on the picture goes back to the
/// watch page.
pub fn mini(
    ctx: &egui::Context,
    state: &State,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
    actions: &mut Vec<Action>,
) {
    let (Some(watching), Some(status)) = (&state.watching, status) else {
        return;
    };
    if status.idle {
        return;
    }
    let palette = &state.palette;
    let width = (ctx.content_rect().width() * MINI_SHARE).clamp(220.0, MINI_WIDTH);
    let position =
        ctx.content_rect().right_bottom() - vec2(width + 16.0, width * 9.0 / 16.0 + 104.0);
    egui::Window::new("Miniplayer")
        .id(egui::Id::new("mini-player"))
        .default_pos(position)
        .default_width(width)
        .min_width(200.0)
        .max_width((ctx.content_rect().width() - 32.0).max(200.0))
        .resizable([true, false])
        .collapsible(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(palette.panel)
                .corner_radius(CornerRadius::same(12))
                .shadow(egui::Shadow {
                    offset: [0, 4],
                    blur: 16,
                    spread: 0,
                    color: Color32::from_black_alpha(120),
                })
                .show(ui, |ui| {
                    let width = ui.available_width().max(180.0);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let height = width * 9.0 / 16.0;
                    let (rect, response) =
                        ui.allocate_exact_size(vec2(width, height), Sense::click());
                    picture(ui, rect, state, Some(status), surface);
                    video_context_menu(&response, state, status, actions);
                    // The part played, along the picture's bottom edge.
                    if status.duration > 0.0 && !state.watching_live() {
                        let fraction = (status.position / status.duration).clamp(0.0, 1.0) as f32;
                        let line = Rect::from_min_max(
                            pos2(rect.left(), rect.bottom() - 3.0),
                            pos2(rect.left() + rect.width() * fraction, rect.bottom()),
                        );
                        ui.painter()
                            .rect_filled(line, CornerRadius::ZERO, palette.accent);
                    }
                    if ui.rect_contains_pointer(rect) || status.paused {
                        ui.painter().rect_filled(
                            rect,
                            CornerRadius::ZERO,
                            Color32::from_black_alpha(110),
                        );
                        let mut over = ui.new_child(UiBuilder::new().max_rect(rect));
                        on_video_style(&mut over);
                        let place = |size: f32, center: egui::Pos2| {
                            UiBuilder::new().max_rect(Rect::from_center_size(
                                center,
                                vec2(size + 12.0, size + 12.0),
                            ))
                        };
                        let (icon, hint) = if status.paused {
                            (Icon::Play, "Reproduzir")
                        } else {
                            (Icon::Pause, "Pausar")
                        };
                        if over
                            .scope_builder(place(32.0, rect.center()), |ui| {
                                widgets::icon_button(ui, icon, 32.0, hint).clicked()
                            })
                            .inner
                        {
                            actions.push(Action::TogglePause);
                        }
                        let previous = previous_video(state.watching.as_ref(), &state.up_next);
                        let related = state
                            .details
                            .ready()
                            .map_or(&[][..], |details| details.related.as_slice());
                        let next = next_video(
                            state.watching.as_ref(),
                            &state.queue,
                            &state.up_next,
                            related,
                        );
                        for (offset, available, icon, hint, action) in [
                            (
                                -64.0,
                                previous.is_some(),
                                Icon::ChevronLeft,
                                "Vídeo anterior",
                                Action::PlayPrevious,
                            ),
                            (
                                64.0,
                                next.is_some() || state.playing_more.is_some(),
                                Icon::SkipForward,
                                "Próximo vídeo",
                                Action::PlayNext,
                            ),
                        ] {
                            if available
                                && over
                                    .scope_builder(
                                        place(22.0, rect.center() + vec2(offset, 0.0)),
                                        |ui| widgets::icon_button(ui, icon, 22.0, hint).clicked(),
                                    )
                                    .inner
                            {
                                actions.push(action);
                            }
                        }
                        let corner = rect.right_top() + vec2(-20.0, 20.0);
                        if over
                            .scope_builder(place(18.0, corner), |ui| {
                                widgets::icon_button(ui, Icon::Close, 18.0, "Fechar e pausar")
                                    .clicked()
                            })
                            .inner
                        {
                            actions.push(Action::CloseMiniPlayer);
                        }
                        let corner = rect.left_top() + vec2(20.0, 20.0);
                        if over
                            .scope_builder(place(18.0, corner), |ui| {
                                widgets::icon_button(
                                    ui,
                                    Icon::MiniPlayer,
                                    18.0,
                                    "Abrir página do vídeo",
                                )
                                .clicked()
                            })
                            .inner
                        {
                            actions.push(Action::ReturnToWatch);
                        }
                    }
                    if response.clicked() {
                        actions.push(Action::ReturnToWatch);
                    }
                    egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(12, 10))
                        .show(ui, |ui| {
                            ui.set_width(width - 24.0);
                            ui.spacing_mut().item_spacing.y = 2.0;
                            let title = if watching.title.is_empty() {
                                status.title.as_deref().unwrap_or("Carregando…")
                            } else {
                                &watching.title
                            };
                            let title = ui.add(
                                egui::Label::new(
                                    RichText::new(title)
                                        .font(theme::semibold(14.0))
                                        .color(palette.text),
                                )
                                .truncate()
                                .sense(Sense::click()),
                            );
                            if title.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                actions.push(Action::ReturnToWatch);
                            }
                            if let Some(channel) = &watching.channel {
                                ui.label(
                                    RichText::new(channel)
                                        .font(theme::regular(13.0))
                                        .color(palette.secondary),
                                );
                            }
                        });
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Chapter, Details};
    use crate::mpv::Subtitle;

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
        crate::theme::install_fonts(&ctx);
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
    fn narrow_controls_stay_inside_the_video_and_keep_the_groups_apart() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut state = State {
            watching: Some(Video::from_id("aaaaaaaaaaa".into())),
            queue: vec![Video::from_id("bbbbbbbbbbb".into())],
            details: Fetch::Ready(Details {
                chapters: vec![Chapter {
                    name: "Um capítulo com um nome muito longo ".repeat(20),
                    seconds: 0,
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let status = Status {
            duration: 100_000.0,
            position: 40_000.0,
            speed: 1.5,
            subtitles: vec![Subtitle {
                id: 1,
                lang: "pt".into(),
                title: "Português".into(),
                selected: true,
            }],
            ..Default::default()
        };
        for (mode, widths) in [
            (Mode::Page, &[296.0, 336.0, 420.0, 720.0, 1280.0][..]),
            (
                Mode::Short,
                &[76.0, 100.0, 115.0, 140.0, 164.0, 200.0, 296.0][..],
            ),
        ] {
            for &width in widths {
                for captions in [false, true] {
                    let mut status = status.clone();
                    if !captions {
                        status.subtitles.clear();
                    }
                    for live in [false, true] {
                        let mut output = ctx.run_ui(
                            egui::RawInput {
                                screen_rect: Some(Rect::from_min_size(
                                    pos2(0.0, 0.0),
                                    vec2(width, 48.0),
                                )),
                                ..Default::default()
                            },
                            |ui| {
                                let row = ui.max_rect();
                                let (left, right) =
                                    buttons(ui, &mut state, &status, mode, live, &mut Vec::new());
                                assert!(
                                    left.right() <= right.left(),
                                    "control groups overlap at {width}: {left:?}, {right:?}"
                                );
                                assert!(
                                    right.right() <= row.right() + 0.1,
                                    "controls escape at {width}: {right:?} vs {row:?}"
                                );
                                assert!(left.left() >= row.left() - 0.1);
                            },
                        );
                        output.textures_delta.clear();
                    }
                }
            }
        }
    }

    #[test]
    fn a_long_press_restores_the_original_speed_without_pausing_on_release() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut state = State {
            view: crate::model::View::Watch,
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
        assert_eq!(actions, [Action::SetSpeed(2.0), Action::SetSpeed(1.25)]);
    }

    #[test]
    fn leaving_the_watch_page_releases_a_speed_hold_even_with_mouse_down() {
        let ctx = egui::Context::default();
        let mut state = State {
            view: crate::model::View::Home,
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
        assert_eq!(actions, [Action::SetSpeed(0.75)]);
        state.view = crate::model::View::Watch;
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
        crate::theme::install_fonts(&ctx);
        for size in [vec2(1600.0, 900.0), vec2(320.0, 180.0)] {
            for live in [false, true] {
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
                        autoplay: autoplay.then(|| (next.clone(), Instant::now())),
                        osd: Some(("+10 s".to_owned(), Instant::now())),
                        segments: vec![Segment {
                            category: "intro".to_owned(),
                            start: 20.0,
                            end: 40.0,
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

    /// A Short on the watch page, and the page with its queue.
    #[test]
    fn the_watch_page_draws_a_short_and_the_queue() {
        let mut short = Video::from_id("sssssssssss".to_owned());
        short.short = true;
        let mut state = State {
            view: crate::model::View::Watch,
            watching: Some(short),
            queue: vec![Video::from_id("qqqqqqqqqqq".to_owned())],
            ..State::default()
        };
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
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
                    super::super::show(ui, &mut state, Some(&status), None, &mut actions);
                });
                output.textures_delta.clear();
            }
        }
    }
}
