//! The player's in-video settings menu: speed, quality, captions, and appearance.

use egui::{
    Align, Align2, Color32, CornerRadius, CursorIcon, Layout, Rect, Sense, Slider, Ui, UiBuilder,
    pos2, vec2,
};
use youtfast_i18n::{Locale, format as trf, gettext as tr};

use super::STRIP;
use super::surface::on_video_style;
use youtfast_app::player::{SPEEDS, speed_label};
use youtfast_app::{Action, PlayerMenu, State};
use youtfast_appearance::icons::Icon;
use youtfast_appearance::theme;
use youtfast_core::model::{Quality, Resolution};
use youtfast_mpv::Status;

/// The settings menu's width and its rows' height.
const MENU_WIDTH: f32 = 280.0;
const MENU_ROW: f32 = 36.0;

/// The settings menu over the video's bottom right: speed, quality,
/// captions and autoplay, each opening its list.
pub(super) fn menu(
    ui: &mut Ui,
    video: Rect,
    state: &mut State,
    status: &Status,
    live: bool,
    actions: &mut Vec<Action>,
) {
    let locale: Locale = state.settings.language;
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
                            if back_row(ui, &tr(locale, "Playback speed")) {
                                state.player_menu = PlayerMenu::Main;
                            }
                            for speed in SPEEDS {
                                let label = if speed == 1.0 {
                                    tr(locale, "Normal").into_owned()
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
                            if back_row(ui, &tr(locale, "Quality")) {
                                state.player_menu = PlayerMenu::Main;
                            }
                            let auto = trf(
                                locale,
                                "Auto (up to {resolution}p)",
                                &[("resolution", Quality::AUTO.to_string())],
                            );
                            if pick_row(
                                ui,
                                &auto,
                                state.settings.quality == Quality::Auto && !state.audio_only,
                            ) {
                                actions.push(Action::SetQuality(Quality::Auto));
                                state.player_menu = PlayerMenu::Closed;
                            }
                            for resolution in offered(state) {
                                let quality = Quality::Max(resolution.lines);
                                if pick_row(
                                    ui,
                                    &resolution.label(),
                                    state.settings.quality == quality && !state.audio_only,
                                ) {
                                    actions.push(Action::SetQuality(quality));
                                    state.player_menu = PlayerMenu::Closed;
                                }
                            }
                        }
                        PlayerMenu::Captions => {
                            if back_row(ui, &tr(locale, "Captions")) {
                                state.player_menu = PlayerMenu::Main;
                            }
                            let none = !status.subtitles.iter().any(|subtitle| subtitle.selected);
                            if pick_row(ui, &tr(locale, "Off"), none) {
                                actions.push(Action::SetSubtitle(None));
                                state.player_menu = PlayerMenu::Closed;
                            }
                            for subtitle in &status.subtitles {
                                if pick_row(ui, &subtitle.title, subtitle.selected) {
                                    actions.push(Action::SetSubtitle(Some(subtitle.id)));
                                    state.player_menu = PlayerMenu::Closed;
                                }
                            }
                            if value_row(ui, &tr(locale, "Caption appearance"), "").clicked() {
                                state.player_menu = PlayerMenu::CaptionStyle;
                            }
                        }
                        PlayerMenu::CaptionStyle => {
                            if back_row(ui, &tr(locale, "Caption appearance")) {
                                state.player_menu = PlayerMenu::Captions;
                            }
                            ui.horizontal(|ui| {
                                ui.label(tr(locale, "Size"));
                                let mut scale = state.settings.caption_scale;
                                if ui
                                    .add(Slider::new(&mut scale, 0.5..=2.0).suffix("×"))
                                    .changed()
                                {
                                    actions.push(Action::SetCaptionScale(scale));
                                }
                            });
                            let mut background = state.settings.caption_background;
                            if ui
                                .checkbox(&mut background, tr(locale, "Dark background"))
                                .changed()
                            {
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
    let locale: Locale = state.settings.language;
    if value_row(
        ui,
        &if state.fullscreen {
            tr(locale, "Exit full screen")
        } else {
            tr(locale, "Full screen")
        },
        "F",
    )
    .clicked()
    {
        actions.push(Action::ToggleFullscreen);
        state.player_menu = PlayerMenu::Closed;
    }
    if value_row(
        ui,
        &tr(locale, "Sound"),
        &if status.muted {
            tr(locale, "Unmute")
        } else {
            tr(locale, "Mute")
        },
    )
    .clicked()
    {
        actions.push(Action::ToggleMute);
    }
    let on = state.settings.autoplay;
    if value_row(
        ui,
        &tr(locale, "Autoplay"),
        &if on {
            tr(locale, "On")
        } else {
            tr(locale, "Off")
        },
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
            .map_or_else(
                || tr(locale, "Off").into_owned(),
                |subtitle| subtitle.title.clone(),
            );
        if value_row(ui, &tr(locale, "Captions"), &current).clicked() {
            state.player_menu = PlayerMenu::Captions;
        }
    }
    if !live {
        let speed = if status.speed == 1.0 {
            tr(locale, "Normal").into_owned()
        } else {
            speed_label(status.speed)
        };
        if value_row(ui, &tr(locale, "Playback speed"), &speed).clicked() {
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
    let quality = if state.audio_only {
        tr(locale, "Audio only").into_owned()
    } else {
        match (state.settings.quality, playing) {
            (Quality::Auto, Some(playing)) => trf(
                locale,
                "Auto · {resolution}",
                &[("resolution", playing.label())],
            ),
            (Quality::Auto, None) => tr(locale, "Auto").into_owned(),
            (_, Some(playing)) => playing.label(),
            (Quality::Max(lines), None) => format!("{lines}p"),
        }
    };
    if value_row(ui, &tr(locale, "Quality"), &quality)
        .on_hover_text(playback_details(status, locale))
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
fn playback_details(status: &Status, locale: Locale) -> String {
    if status.video_width <= 0 || status.video_height <= 0 {
        return tr(locale, "Quality").into_owned();
    }
    let mut parts = vec![format!("{}×{}", status.video_width, status.video_height)];
    if status.fps > 0.0 {
        parts.push(format!("{:.0} fps", status.fps));
    }
    if let Some(codec) = &status.codec {
        parts.push(codec.clone());
    }
    parts.push(match &status.hwdec {
        Some(decoder) => trf(
            locale,
            "hardware decoding ({decoder})",
            &[("decoder", decoder.clone())],
        ),
        None => tr(locale, "software decoding").into_owned(),
    });
    parts.join(" · ")
}
