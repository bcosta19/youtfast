//! The watch page: the video, its controls, and what to play next.

use egui::{
    Align, Align2, Button, Color32, CornerRadius, FontId, Frame, Layout, Rect, RichText, Sense,
    Slider, Ui, UiBuilder, pos2, vec2,
};

use super::{ACCENT, widgets};
use crate::app::State;
use crate::model::{Action, Channel, Quality, Resolution};
use crate::mpv::{Status, VideoSurface};
use crate::util;

/// The "Up next" column beside the video in a wide window.
const SIDEBAR_WIDTH: f32 = 400.0;
/// Narrower than this, the column moves under the video.
const WIDE: f32 = 1000.0;
const GAP: f32 = 24.0;

pub fn show(
    ui: &mut Ui,
    state: &mut State,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
    actions: &mut Vec<Action>,
) {
    egui::CentralPanel::default().show(ui, |ui| {
        // The video never takes more than most of the window's height, so
        // its controls stay in view.
        let max_video_height = ui.available_height() * 0.75;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                if ui.available_width() >= WIDE {
                    ui.horizontal_top(|ui| {
                        let main_width = ui.available_width() - SIDEBAR_WIDTH - GAP;
                        ui.vertical(|ui| {
                            ui.set_width(main_width);
                            main_column(ui, state, status, surface, max_video_height, actions);
                        });
                        ui.add_space(GAP);
                        ui.vertical(|ui| {
                            ui.set_width(SIDEBAR_WIDTH);
                            up_next(ui, state, actions);
                        });
                    });
                } else {
                    main_column(ui, state, status, surface, max_video_height, actions);
                    ui.add_space(GAP);
                    up_next(ui, state, actions);
                }
            });
    });
}

/// The video over the whole window, with the controls over its bottom edge
/// while the pointer is there.
pub fn fullscreen(
    ui: &mut Ui,
    state: &mut State,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
    actions: &mut Vec<Action>,
) {
    egui::CentralPanel::default()
        .frame(Frame::new().fill(Color32::BLACK))
        .show(ui, |ui| {
            let rect = ui.max_rect();
            video(ui, rect, state, status, surface, actions);
            let strip = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 96.0), rect.max);
            // The quality menu opens outside the strip; keep the strip while
            // it is open.
            let pinned = state.seek_preview.is_some() || ui.ctx().any_popup_open();
            if ui.rect_contains_pointer(strip) || pinned {
                ui.painter()
                    .rect_filled(strip, CornerRadius::ZERO, Color32::from_black_alpha(170));
                let mut bar = ui.new_child(
                    UiBuilder::new()
                        .max_rect(strip.shrink2(vec2(24.0, 16.0)))
                        .layout(Layout::top_down(Align::Min)),
                );
                controls(&mut bar, state, status, actions);
            }
        });
}

fn main_column(
    ui: &mut Ui,
    state: &mut State,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
    max_video_height: f32,
    actions: &mut Vec<Action>,
) {
    let width = ui.available_width();
    let height = (width * 9.0 / 16.0).min(max_video_height);
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    video(ui, rect, state, status, surface, actions);
    ui.add_space(8.0);
    controls(ui, state, status, actions);
    ui.add_space(12.0);

    let Some(watching) = &state.watching else {
        return;
    };
    // A video opened from a link has no title until mpv reads it.
    let title = Some(watching.title.as_str())
        .filter(|title| !title.is_empty())
        .or_else(|| status.and_then(|status| status.title.as_deref()))
        .unwrap_or("Loading…");
    ui.label(RichText::new(title).strong().size(20.0));
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let channel = watching.channel_id.as_ref().map(|id| Channel {
            id: id.clone(),
            name: watching.channel.clone().unwrap_or_else(|| id.clone()),
            avatar: None,
        });
        match (&channel, &watching.channel) {
            (Some(channel), _) => {
                let link = ui
                    .link(RichText::new(&channel.name).strong())
                    .on_hover_text("Open the channel");
                if link.clicked() {
                    actions.push(Action::OpenChannel(channel.clone()));
                }
                widgets::subscribe_button(ui, state, channel, actions);
            }
            (None, Some(name)) => {
                ui.label(RichText::new(name).strong());
            }
            (None, None) => {}
        }
        let meta = widgets::meta(watching);
        if !meta.is_empty() {
            ui.label(RichText::new(meta).weak());
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let saved = state.library.in_watch_later(&watching.id);
            let label = if saved {
                "✔ Saved for later"
            } else {
                "Watch later"
            };
            if ui
                .selectable_label(saved, label)
                .on_hover_text("Watch later on this computer")
                .clicked()
            {
                actions.push(Action::ToggleWatchLater(watching.clone()));
            }
        });
    });
}

/// The picture, with the spinner or the error over it. A click pauses; a
/// double click goes full screen.
fn video(
    ui: &mut Ui,
    rect: Rect,
    state: &State,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
    actions: &mut Vec<Action>,
) {
    let response = ui.interact(rect, ui.id().with("video"), Sense::click());
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, Color32::BLACK);
    if let Some(surface) = surface {
        surface.paint(ui, rect);
    }
    if let Some(error) = &state.player_error {
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            error,
            FontId::proportional(16.0),
            Color32::WHITE,
        );
    } else if status.is_some_and(|status| {
        !status.idle && (status.buffering || (status.position <= 0.0 && !status.paused))
    }) {
        egui::Spinner::new()
            .size(40.0)
            .color(Color32::WHITE)
            .paint_at(ui, Rect::from_center_size(rect.center(), vec2(40.0, 40.0)));
        ui.ctx().request_repaint();
    }
    if response.double_clicked() {
        actions.push(Action::ToggleFullscreen);
    } else if response.clicked() {
        actions.push(Action::TogglePause);
    }
}

fn controls(ui: &mut Ui, state: &mut State, status: Option<&Status>, actions: &mut Vec<Action>) {
    let Some(status) = status else {
        return;
    };
    seek_bar(ui, state, status, actions);
    ui.horizontal(|ui| {
        let icon = if status.paused { "▶" } else { "⏸" };
        if ui
            .add(Button::new(RichText::new(icon).size(20.0)).frame(false))
            .on_hover_text("Play or pause (Space)")
            .clicked()
        {
            actions.push(Action::TogglePause);
        }
        ui.label("🔊");
        ui.spacing_mut().slider_width = 90.0;
        let mut volume = status.volume;
        if ui
            .add(Slider::new(&mut volume, 0.0..=100.0).show_value(false))
            .changed()
        {
            actions.push(Action::SetVolume(volume));
        }
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
        ui.label(RichText::new(time).monospace());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let label = if state.fullscreen {
                "Exit full screen (F)"
            } else {
                "Full screen (F)"
            };
            if ui
                .add(Button::new(RichText::new("⛶").size(18.0)).frame(false))
                .on_hover_text(label)
                .clicked()
            {
                actions.push(Action::ToggleFullscreen);
            }
            quality_menu(ui, state, status, actions);
        });
    });
}

/// The quality chosen and the resolution playing, with the decoding details
/// on hover. Lists what the video offers once it has loaded, and the usual
/// resolutions before that.
fn quality_menu(ui: &mut Ui, state: &State, status: &Status, actions: &mut Vec<Action>) {
    let playing = (status.video_width > 0 && status.video_height > 0).then(|| {
        Resolution::from_size(
            status.video_width as u32,
            status.video_height as u32,
            status.fps.round() as u32,
        )
    });
    let text = match (state.settings.quality, playing) {
        (Quality::Auto, Some(playing)) => format!("Auto · {}", playing.label()),
        (Quality::Auto, None) => "Auto".to_owned(),
        (_, Some(playing)) => playing.label(),
        (Quality::Max(lines), None) => format!("{lines}p"),
    };
    let offered: Vec<Resolution> = if state.resolutions.is_empty() {
        [2160, 1440, 1080, 720, 480, 360]
            .into_iter()
            .map(|lines| Resolution { lines, fps: 0 })
            .collect()
    } else {
        state.resolutions.clone()
    };
    let menu = egui::ComboBox::from_id_salt("quality")
        .selected_text(text)
        .width(130.0)
        .show_ui(ui, |ui| {
            let auto = format!("Auto (up to {}p)", Quality::AUTO);
            if ui
                .selectable_label(state.settings.quality == Quality::Auto, auto)
                .clicked()
            {
                actions.push(Action::SetQuality(Quality::Auto));
            }
            for resolution in offered {
                let quality = Quality::Max(resolution.lines);
                if ui
                    .selectable_label(state.settings.quality == quality, resolution.label())
                    .clicked()
                {
                    actions.push(Action::SetQuality(quality));
                }
            }
        });
    menu.response.on_hover_text(playback_details(status));
}

/// "1920×1080 · 60 fps · vp9 · hardware decoding (vaapi)".
fn playback_details(status: &Status) -> String {
    if status.video_width <= 0 || status.video_height <= 0 {
        return "Quality".to_owned();
    }
    let mut parts = vec![format!("{}×{}", status.video_width, status.video_height)];
    if status.fps > 0.0 {
        parts.push(format!("{:.0} fps", status.fps));
    }
    if let Some(codec) = &status.codec {
        parts.push(codec.clone());
    }
    parts.push(match &status.hwdec {
        Some(decoder) => format!("hardware decoding ({decoder})"),
        None => "software decoding".to_owned(),
    });
    parts.join(" · ")
}

/// YouTube's red bar: played, buffered, and the rest. Dragging previews
/// the position and seeks to keyframes; releasing seeks exactly.
fn seek_bar(ui: &mut Ui, state: &mut State, status: &Status, actions: &mut Vec<Action>) {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::click_and_drag());
    let duration = status.duration;
    if duration > 0.0 {
        let pointer = response.interact_pointer_pos().map(|pointer| {
            f64::from(((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0)) * duration
        });
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

    let active = response.hovered() || response.dragged();
    let thickness = if active { 6.0 } else { 4.0 };
    let track = Rect::from_center_size(rect.center(), vec2(rect.width(), thickness));
    let fraction = |seconds: f64| {
        if duration > 0.0 {
            (seconds / duration).clamp(0.0, 1.0) as f32
        } else {
            0.0
        }
    };
    let x_at = |seconds: f64| track.left() + track.width() * fraction(seconds);
    let position = state.seek_preview.unwrap_or(status.position);
    let painter = ui.painter();
    let radius = CornerRadius::same(3);
    painter.rect_filled(track, radius, Color32::from_white_alpha(50));
    let buffered = Rect::from_min_max(track.min, pos2(x_at(status.cached_until), track.max.y));
    painter.rect_filled(buffered, radius, Color32::from_white_alpha(90));
    let played = Rect::from_min_max(track.min, pos2(x_at(position), track.max.y));
    painter.rect_filled(played, radius, ACCENT);
    if active && duration > 0.0 {
        painter.circle_filled(pos2(x_at(position), track.center().y), 7.0, ACCENT);
    }
}

fn up_next(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let watching = state.watching.as_ref().map(|video| video.id.as_str());
    let next: Vec<_> = state
        .up_next
        .iter()
        .filter(|video| Some(video.id.as_str()) != watching)
        .collect();
    if next.is_empty() {
        return;
    }
    ui.label(RichText::new("Up next").strong().size(16.0));
    ui.add_space(8.0);
    for video in next {
        widgets::compact_card(ui, video, None, actions);
        ui.add_space(8.0);
    }
}
