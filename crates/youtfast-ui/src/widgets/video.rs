//! Video thumbnails, list rows, grid cards, and card hover/menu interactions.

use egui::text::LayoutJob;
use egui::{
    Color32, CornerRadius, CursorIcon, FontId, Id, Pos2, Rect, Response, Sense, Stroke, Ui, pos2,
    vec2,
};
use youtfast_i18n::{Locale, gettext as tr};

use super::{avatar, focus_outline, hover_fade, line, mix, press_fade, row_fill_radius};
use youtfast_app::{Action, State};
use youtfast_appearance::icons::Icon;
use youtfast_appearance::theme::{self, Palette, RADIUS, RADIUS_SMALL};
use youtfast_core::model::{Channel, Video};
use youtfast_core::util;
use youtfast_storage::library::Library;

/// A compact overlay icon button over a thumbnail (Watch later, Add to queue).
fn overlay_icon_button(
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    icon: Icon,
    tint: Color32,
    hint: &str,
) -> Response {
    let response = ui
        .interact(rect, id, Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(hint);
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), hint));
    let hover = hover_fade(ui, &response);
    let press = press_fade(ui, &response);
    let radius = CornerRadius::same(RADIUS);
    let bg = Color32::from_black_alpha((195.0 + 40.0 * hover - 15.0 * press) as u8);
    ui.painter().rect_filled(rect, radius, bg);
    ui.painter().rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, Color32::from_white_alpha((32.0 + 36.0 * hover) as u8)),
        egui::StrokeKind::Inside,
    );
    focus_outline(ui, &response, radius);
    let icon_size = 18.0;
    icon.image(tint, icon_size).paint_at(
        ui,
        Rect::from_center_size(rect.center(), vec2(icon_size, icon_size)),
    );
    response
}

/// The duration (or LIVE) in the thumbnail's corner, YouTube's way.
fn badge(ui: &Ui, thumbnail: Rect, video: &Video, palette: &Palette, locale: Locale) {
    let (text, fill, border) = if video.live {
        (tr(locale, "LIVE").into_owned(), palette.red, false)
    } else if let Some(seconds) = video.duration {
        (
            util::duration(seconds),
            Color32::from_black_alpha(210),
            true,
        )
    } else {
        return;
    };
    let painter = ui.painter();
    let galley = painter.layout_no_wrap(text, theme::semibold(12.0), Color32::WHITE);
    let size = galley.size() + vec2(10.0, 4.0);
    let rect = Rect::from_min_size(thumbnail.right_bottom() - size - vec2(6.0, 6.0), size);
    let radius = CornerRadius::same(RADIUS_SMALL + 1);
    painter.rect_filled(rect, radius, fill);
    if border {
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, Color32::from_white_alpha(22)),
            egui::StrokeKind::Inside,
        );
    }
    painter.galley(rect.min + vec2(5.0, 2.0), galley, Color32::WHITE);
}

/// The thumbnail, rounded, with its corner badge, and the part watched
/// (0..1) as YouTube's red bar along its bottom edge.
pub(crate) fn thumbnail(
    ui: &mut Ui,
    palette: &Palette,
    video: &Video,
    rect: Rect,
    watched: Option<f32>,
    locale: Locale,
) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let radius = CornerRadius::same(RADIUS);
    ui.painter().rect_filled(rect, radius, palette.surface);
    youtfast_app::images::remote(ui, &video.thumbnail())
        .fit_to_exact_size(rect.size())
        .corner_radius(radius)
        .paint_at(ui, rect);
    let hairline = thumbnail_hairline(palette);
    ui.painter().rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, hairline),
        egui::StrokeKind::Inside,
    );
    badge(ui, rect, video, palette, locale);
    if let Some(watched) = watched.filter(|fraction| *fraction > 0.0) {
        let track = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 4.0), rect.right_bottom());
        ui.painter().rect_filled(
            track,
            CornerRadius {
                nw: 0,
                ne: 0,
                sw: RADIUS,
                se: RADIUS,
            },
            Color32::from_black_alpha(165),
        );
        let fraction = watched.clamp(0.0, 1.0);
        let played = Rect::from_min_size(track.min, vec2(track.width() * fraction, track.height()));
        ui.painter().rect_filled(
            played,
            CornerRadius {
                nw: 0,
                ne: 0,
                sw: RADIUS,
                se: if fraction >= 0.99 { RADIUS } else { 0 },
            },
            palette.accent,
        );
    }
}

/// The text YouTube puts beside or under a thumbnail: the title on up to
/// two lines, the channel, then `views · age` (and the note). Drawn from
/// `at`, `width` wide. Returns the height used.
#[allow(
    clippy::too_many_arguments,
    reason = "Explicit language alongside drawing geometry, identity and presentation"
)]
fn video_text(
    ui: &Ui,
    palette: &Palette,
    video: &Video,
    note: Option<&str>,
    id: Id,
    at: Pos2,
    width: f32,
    locale: Locale,
) -> f32 {
    let painter = ui.painter();
    let lines = |text: &str, font: FontId, color: Color32, rows: usize| {
        let mut job = LayoutJob::default();
        job.append(text, 0.0, egui::TextFormat::simple(font, color));
        job.wrap.max_width = width;
        job.wrap.max_rows = rows;
        job.wrap.overflow_character = Some('…');
        painter.layout_job(job)
    };
    let mut y = at.y;
    let title = lines(&video.title, theme::semibold(14.0), palette.text, 2);
    y += title.rect.height() + 5.0;
    painter.galley(at, title, palette.text);
    if let Some(channel) = video.channel.as_deref().filter(|name| !name.is_empty()) {
        let galley = lines(channel, theme::regular(13.0), palette.secondary, 1);
        let height = galley.rect.height();
        let rect = Rect::from_min_size(pos2(at.x, y), vec2(galley.size().x, height));
        ui.ctx()
            .data_mut(|d| d.insert_temp(id.with("channel-rect"), rect));
        painter.galley(pos2(at.x, y), galley, palette.secondary);
        y += height + 2.0;
    }
    let mut rest = meta(video, locale);
    if let Some(note) = note.filter(|note| !note.is_empty()) {
        if !rest.is_empty() {
            rest.push_str(" · ");
        }
        rest.push_str(note);
    }
    if !rest.is_empty() {
        let galley = lines(&rest, theme::regular(13.0), palette.secondary, 1);
        let height = galley.rect.height();
        painter.galley(pos2(at.x, y), galley, palette.secondary);
        y += height;
    }
    y - at.y
}

/// The part of `video` watched (0..1), from its resume position.
pub fn watched(library: &Library, video: &Video) -> Option<f32> {
    video
        .duration
        .and_then(|duration| {
            library
                .resume_position(&video.id)
                .map(|left| left / duration)
        })
        .map(|fraction| fraction as f32)
}

/// A video as a list row, the way YouTube's Up next column draws it: a
/// 168x94 thumbnail, the title on up to two lines, the channel, then
/// `views · age` and the note when there is one. `watched` is the part
/// watched (0..1). `selected` marks the keyboard's cursor.
pub fn video_row(
    ui: &mut Ui,
    palette: &Palette,
    video: &Video,
    note: Option<&str>,
    watched: Option<f32>,
    selected: bool,
    locale: Locale,
) -> Response {
    video_row_sized(
        ui,
        palette,
        video,
        note,
        watched,
        selected,
        vec2(168.0, 94.0),
        locale,
    )
}

/// [`video_row`] with a larger thumbnail, as search results show them.
pub fn video_row_large(
    ui: &mut Ui,
    palette: &Palette,
    video: &Video,
    watched: Option<f32>,
    selected: bool,
    locale: Locale,
) -> Response {
    // Up to 360x202, and never more than half the row.
    let width = (ui.available_width() / 2.0).clamp(168.0, 360.0);
    let thumb = vec2(width, width * 9.0 / 16.0);
    video_row_sized(ui, palette, video, None, watched, selected, thumb, locale)
}

#[allow(
    clippy::too_many_arguments,
    reason = "Explicit language alongside thumbnail geometry and row presentation"
)]
fn video_row_sized(
    ui: &mut Ui,
    palette: &Palette,
    video: &Video,
    note: Option<&str>,
    watched: Option<f32>,
    selected: bool,
    thumb: egui::Vec2,
    locale: Locale,
) -> Response {
    let pad = 6.0;
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), thumb.y + 2.0 * pad),
        Sense::click(),
    );
    if !ui.is_rect_visible(rect) {
        return response;
    }
    row_fill_radius(
        ui,
        palette,
        rect,
        &response,
        selected,
        CornerRadius::same(RADIUS + 4),
    );
    let thumb_rect = Rect::from_min_size(rect.min + vec2(pad, pad), thumb);
    thumbnail(ui, palette, video, thumb_rect, watched, locale);
    let text_x = thumb_rect.right() + 12.0;
    let width = (rect.right() - text_x - pad - 34.0).max(20.0);
    video_text(
        ui,
        palette,
        video,
        note,
        response.id,
        pos2(text_x, thumb_rect.top() + 2.0),
        width,
        locale,
    );
    response
}

pub fn video_card_height(ui: &Ui, width: f32) -> f32 {
    (width - 12.0) * 9.0 / 16.0 + 4.0 * line(ui) + 28.0
}

/// Shared card/row actions. Child hit targets never also open the video.
pub fn video_interactions(
    ui: &mut Ui,
    response: &Response,
    state: &State,
    video: &Video,
    actions: &mut Vec<Action>,
) {
    let locale = state.settings.language;

    let mut handled = false;
    if !ui.is_rect_visible(response.rect) {
        return;
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &video.title)
    });
    // `put` advances its Ui's layout cursor. These controls overlay a card
    // already allocated by the parent, so keep their layout and automatic
    // widget IDs in a stable child instead of moving the next card/row.
    let mut overlay = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(response.id.with("video-actions"))
            .max_rect(response.rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    let ui = &mut overlay;
    for part in ["channel-rect", "avatar-rect"] {
        if let (Some(id), Some(rect)) = (
            &video.channel_id,
            ui.ctx()
                .data(|d| d.get_temp::<Rect>(response.id.with(part))),
        ) {
            let link = ui
                .interact(
                    rect,
                    response.id.with(("open-channel", part)),
                    Sense::click(),
                )
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(tr(locale, "Open channel").as_ref());
            if link.clicked() {
                handled = true;
                actions.push(Action::OpenChannel(Channel {
                    id: id.clone(),
                    name: video.channel.clone().unwrap_or_default(),
                    avatar: video.channel_avatar.clone(),
                }));
            }
        }
    }
    let menu_rect = Rect::from_center_size(
        pos2(response.rect.right() - 16.0, response.rect.bottom() - 48.0),
        vec2(30.0, 30.0),
    );
    let menu = ui
        .interact(menu_rect, response.id.with("more-menu"), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(tr(locale, "More actions").as_ref());
    menu.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            tr(locale, "More actions").as_ref(),
        )
    });
    let menu_hover = hover_fade(ui, &menu);
    if menu_hover > 0.0 {
        let circle_fill = mix(
            state.palette.surface_hover,
            state.palette.surface_active,
            press_fade(ui, &menu),
        );
        ui.painter().circle_filled(
            menu_rect.center(),
            15.0,
            circle_fill.gamma_multiply(menu_hover),
        );
    }
    focus_outline(ui, &menu, CornerRadius::same(15));
    let menu_tint = mix(state.palette.secondary, state.palette.text, menu_hover);
    Icon::More.image(menu_tint, 18.0).paint_at(
        ui,
        Rect::from_center_size(menu_rect.center(), vec2(18.0, 18.0)),
    );
    handled |= menu.clicked();
    egui::Popup::menu(&menu).show(|ui| {
        for (label, action) in [
            (
                "Play next",
                Action::Enqueue {
                    video: video.clone(),
                    next: true,
                },
            ),
            (
                "Add to queue",
                Action::Enqueue {
                    video: video.clone(),
                    next: false,
                },
            ),
            (
                if state.library.in_watch_later(&video.id) {
                    "Remove from Watch later"
                } else {
                    "Watch later on this device"
                },
                Action::ToggleWatchLater(video.clone()),
            ),
            ("Open in new tab", Action::OpenVideoTab(video.clone())),
            ("Copy link", Action::CopyLink(video.url())),
            ("Open in browser", Action::OpenUrl(video.url())),
        ] {
            if ui.button(tr(locale, label).as_ref()).clicked() {
                actions.push(action);
                ui.close();
            }
        }
    });
    if response.hovered() || response.has_focus() || ui.rect_contains_pointer(response.rect) {
        let clock = Rect::from_min_size(
            response.rect.left_top() + vec2(10.0, 10.0),
            vec2(32.0, 32.0),
        );
        let add = Rect::from_min_size(clock.min + vec2(36.0, 0.0), clock.size());
        let in_later = state.library.in_watch_later(&video.id);
        let later_hint = if in_later {
            tr(locale, "Remove from Watch later")
        } else {
            tr(locale, "Watch later on this device")
        };
        if overlay_icon_button(
            ui,
            clock,
            response.id.with("watch-later"),
            Icon::Clock,
            if in_later {
                state.palette.accent
            } else {
                Color32::WHITE
            },
            later_hint.as_ref(),
        )
        .clicked()
        {
            handled = true;
            actions.push(Action::ToggleWatchLater(video.clone()));
        }
        if overlay_icon_button(
            ui,
            add,
            response.id.with("add-queue"),
            Icon::Queue,
            Color32::WHITE,
            tr(locale, "Add to queue").as_ref(),
        )
        .clicked()
        {
            handled = true;
            actions.push(Action::Enqueue {
                video: video.clone(),
                next: false,
            });
        }
    }
    crate::results::video_menu(response, state, video, actions);
    if response.clicked() && !handled {
        actions.push(if ui.input(|i| i.modifiers.command) {
            Action::OpenVideoTab(video.clone())
        } else {
            Action::Open(video.clone())
        });
    }
}

/// A video as a card in a grid, the way YouTube's home draws it: the
/// thumbnail across the card's width, the text under it. `width` is the
/// card's.
pub fn video_card(
    ui: &mut Ui,
    palette: &Palette,
    video: &Video,
    watched: Option<f32>,
    selected: bool,
    width: f32,
    locale: Locale,
) -> Response {
    let pad = 6.0;
    let thumb = vec2(width - 2.0 * pad, (width - 2.0 * pad) * 9.0 / 16.0);
    // Room for two title lines, the channel and the meta line.
    let text = 2.0 * line(ui) + 2.0 * line(ui) + 10.0;
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, thumb.y + text + 3.0 * pad), Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    row_fill_radius(
        ui,
        palette,
        rect,
        &response,
        selected,
        CornerRadius::same(RADIUS + 4),
    );
    let thumb_rect = Rect::from_min_size(rect.min + vec2(pad, pad), thumb);
    thumbnail(ui, palette, video, thumb_rect, watched, locale);
    let avatar_rect = Rect::from_min_size(
        pos2(thumb_rect.left(), thumb_rect.bottom() + 10.0),
        vec2(36.0, 36.0),
    );
    avatar(
        ui,
        palette,
        video.channel_avatar.as_deref(),
        video.channel.as_deref().unwrap_or(""),
        avatar_rect,
    );
    ui.ctx()
        .data_mut(|d| d.insert_temp(response.id.with("avatar-rect"), avatar_rect));
    video_text(
        ui,
        palette,
        video,
        None,
        response.id,
        pos2(thumb_rect.left() + 46.0, thumb_rect.bottom() + 10.0),
        (thumb.x - 78.0).max(20.0),
        locale,
    );
    response
}

pub(crate) fn thumbnail_hairline(palette: &Palette) -> Color32 {
    if palette.dark {
        Color32::from_white_alpha(18)
    } else {
        Color32::from_black_alpha(16)
    }
}

/// The uv rect that crops a 16:9 thumbnail (`/mqdefault.jpg`) to 9:16, centered.
pub fn short_uv() -> Rect {
    // A 16:9 source's width fraction for 9:16 tall is (9/16) / (16/9) = 81/256.
    let width = (9.0 / 16.0) / (16.0 / 9.0);
    let left = (1.0 - width) / 2.0;
    Rect::from_min_max(pos2(left, 0.0), pos2(left + width, 1.0))
}

/// "1.2M views · 2 days ago", or "Live now".
pub(crate) fn meta(video: &Video, locale: Locale) -> String {
    if video.live {
        return tr(locale, "Live now").into_owned();
    }
    let mut meta = String::new();
    if let Some(count) = video.views {
        meta.push_str(&util::views(count, locale));
    }
    let published = video.published.as_deref().map(std::borrow::Cow::Borrowed);
    let published = published.or_else(|| {
        video
            .published_at
            .map(|at| std::borrow::Cow::Owned(util::ago(at, util::now(), locale)))
    });
    if let Some(published) = published {
        if video.views.is_some() {
            meta.push_str(" · ");
        }
        meta.push_str(&published);
    }
    meta
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::video_count_label;

    #[test]
    fn thumbnail_draws_only_positive_watched_progress() {
        for watched in [None, Some(0.0), Some(0.5), Some(1.5)] {
            let ctx = egui::Context::default();
            let palette = Palette::dark();
            let rect = Rect::from_min_size(pos2(10.0, 10.0), vec2(168.0, 94.0));
            let video = Video::from_id("thumbnail00".into());
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                thumbnail(ui, &palette, &video, rect, watched, Locale::English);
            });
            output.textures_delta.clear();
            let bars: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| {
                    if let egui::Shape::Rect(shape) = &shape.shape {
                        assert_ne!(shape.fill, Color32::from_white_alpha(70));
                        (shape.fill == palette.accent).then_some(shape.rect)
                    } else {
                        None
                    }
                })
                .collect();
            if let Some(fraction) = watched.filter(|fraction| *fraction > 0.0) {
                assert_eq!(bars.len(), 1);
                assert_eq!(bars[0].width(), rect.width() * fraction.min(1.0));
            } else {
                assert!(bars.is_empty());
            }
        }
    }

    #[test]
    fn explicit_locale_changes_app_counts_without_translating_video_content() {
        let mut video = Video::from_id("aaaaaaaaaaa".into());
        video.title = "Settings".into();
        video.channel = Some("Home".into());
        video.published = Some("The creator's own date wording".into());
        assert_eq!(
            meta(&video, Locale::English),
            "The creator's own date wording"
        );
        assert_eq!(
            meta(&video, Locale::PortugueseBrazil),
            "The creator's own date wording"
        );
        assert_eq!(video_count_label(1, Locale::English), "1 video");
        assert_eq!(video_count_label(2, Locale::English), "2 videos");
        assert_eq!(video_count_label(1, Locale::PortugueseBrazil), "1 vídeo");
        assert_eq!(video_count_label(2, Locale::PortugueseBrazil), "2 vídeos");
        assert_eq!(video_count_label(1, Locale::Spanish), "1 vídeo");
        assert_eq!(video_count_label(2, Locale::Spanish), "2 vídeos");
        assert_eq!(video_count_label(1, Locale::French), "1 vidéo");
        assert_eq!(video_count_label(2, Locale::French), "2 vidéos");
        assert_eq!(video_count_label(1, Locale::German), "1 Video");
        assert_eq!(video_count_label(2, Locale::German), "2 Videos");
        assert_eq!(video_count_label(1, Locale::Italian), "1 video");
        assert_eq!(video_count_label(2, Locale::Italian), "2 video");
        video.live = true;
        assert_eq!(meta(&video, Locale::English), "Live now");
        assert_eq!(meta(&video, Locale::PortugueseBrazil), "Ao vivo agora");
        assert_eq!(meta(&video, Locale::Spanish), "En directo ahora");
        assert_eq!(meta(&video, Locale::French), "En direct");
        assert_eq!(meta(&video, Locale::German), "Jetzt live");
        assert_eq!(meta(&video, Locale::Italian), "In diretta ora");
        assert_eq!(video.title, "Settings");
        assert_eq!(video.channel.as_deref(), Some("Home"));
    }

    fn draw_action_cards(
        ctx: &egui::Context,
        state: &State,
        videos: &[Video],
        columns: usize,
        width: f32,
        events: Vec<egui::Event>,
    ) -> (Vec<(Id, Rect)>, Vec<Action>) {
        let locale = state.settings.language;

        let mut cards = Vec::new();
        let mut actions = Vec::new();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width, 1000.0))),
                events,
                ..Default::default()
            },
            |ui| {
                let gap = 16.0;
                let card_width =
                    (ui.available_width() - gap * (columns - 1) as f32) / columns as f32;
                for row in videos.chunks(columns) {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        for video in row {
                            let response = video_card(
                                ui,
                                &state.palette,
                                video,
                                None,
                                false,
                                card_width,
                                locale,
                            );
                            cards.push((response.id, response.rect));
                            video_interactions(ui, &response, state, video, &mut actions);
                        }
                    });
                    ui.add_space(gap);
                }
            },
        );
        output.textures_delta.clear();
        (cards, actions)
    }

    fn action_videos(count: usize) -> Vec<Video> {
        (0..count)
            .map(|index| Video {
                title: format!("Vídeo de teste {index} com título em duas linhas"),
                channel: Some(format!("Canal {index}")),
                channel_id: Some(format!("UCsynthetic{index}")),
                ..Video::from_id(format!("demo{index:07}"))
            })
            .collect()
    }

    #[test]
    fn middle_and_command_click_open_background_video_tabs() {
        for (button, modifiers) in [
            (egui::PointerButton::Middle, egui::Modifiers::NONE),
            (egui::PointerButton::Primary, egui::Modifiers::COMMAND),
        ] {
            let ctx = egui::Context::default();
            youtfast_appearance::theme::install_fonts(&ctx);
            let state = State::default();
            let videos = action_videos(1);
            let (cards, _) = draw_action_cards(&ctx, &state, &videos, 1, 480.0, Vec::new());
            let pos = cards[0].1.center();
            let mut actions = Vec::new();
            for pressed in [true, false] {
                let (_, emitted) = draw_action_cards(
                    &ctx,
                    &state,
                    &videos,
                    1,
                    480.0,
                    vec![
                        egui::Event::ModifiersChanged(modifiers),
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button,
                            pressed,
                            modifiers,
                        },
                    ],
                );
                actions.extend(emitted);
            }
            assert_eq!(actions, vec![Action::OpenVideoTab(videos[0].clone())]);
        }
    }

    #[test]
    fn hover_actions_keep_card_positions_and_ids_stable_across_the_grid() {
        for light in [false, true] {
            for (width, columns) in [(480.0, 1), (960.0, 3), (1512.0, 4)] {
                let ctx = egui::Context::default();
                youtfast_appearance::theme::install_fonts(&ctx);
                let state = State {
                    palette: if light {
                        Palette::light()
                    } else {
                        Palette::dark()
                    },
                    ..Default::default()
                };
                youtfast_appearance::theme::apply(&ctx, &state.palette);
                let videos = action_videos(columns * 2);
                let (baseline, _) =
                    draw_action_cards(&ctx, &state, &videos, columns, width, Vec::new());
                for &(_, hovered) in &baseline {
                    for _ in 0..3 {
                        let (actual, _) = draw_action_cards(
                            &ctx,
                            &state,
                            &videos,
                            columns,
                            width,
                            vec![egui::Event::PointerMoved(hovered.center())],
                        );
                        assert_eq!(
                            actual, baseline,
                            "hover changed grid geometry or IDs: width={width}, light={light}"
                        );
                        for row in actual.chunks(columns) {
                            for pair in row.windows(2) {
                                assert!(pair[0].1.right() <= pair[1].1.left());
                            }
                        }
                    }
                }
                let (after, _) = draw_action_cards(
                    &ctx,
                    &state,
                    &videos,
                    columns,
                    width,
                    vec![egui::Event::PointerGone],
                );
                assert_eq!(after, baseline);
            }
        }
    }

    #[test]
    fn video_targets_keep_their_own_actions_without_moving_siblings() {
        for target in ["later", "queue", "channel", "menu", "video"] {
            let ctx = egui::Context::default();
            youtfast_appearance::theme::install_fonts(&ctx);
            let state = State::default();
            let videos = action_videos(3);
            let (baseline, _) = draw_action_cards(&ctx, &state, &videos, 3, 960.0, Vec::new());
            let (id, rect) = baseline[0];
            let pointer = match target {
                "later" => rect.min + vec2(26.0, 26.0),
                "queue" => rect.min + vec2(62.0, 26.0),
                "channel" => ctx
                    .data(|data| data.get_temp::<Rect>(id.with("channel-rect")))
                    .expect("the channel link must have a hit target")
                    .center(),
                "menu" => rect.right_bottom() - vec2(16.0, 48.0),
                _ => rect.center(),
            };
            for _ in 0..2 {
                draw_action_cards(
                    &ctx,
                    &state,
                    &videos,
                    3,
                    960.0,
                    vec![egui::Event::PointerMoved(pointer)],
                );
            }
            let mut actions = Vec::new();
            for pressed in [true, false] {
                let (cards, emitted) = draw_action_cards(
                    &ctx,
                    &state,
                    &videos,
                    3,
                    960.0,
                    vec![
                        egui::Event::PointerMoved(pointer),
                        egui::Event::PointerButton {
                            pos: pointer,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
                assert_eq!(cards, baseline);
                actions.extend(emitted);
            }
            let expected = match target {
                "later" => vec![Action::ToggleWatchLater(videos[0].clone())],
                "queue" => vec![Action::Enqueue {
                    video: videos[0].clone(),
                    next: false,
                }],
                "channel" => vec![Action::OpenChannel(Channel {
                    id: videos[0].channel_id.clone().unwrap(),
                    name: videos[0].channel.clone().unwrap(),
                    avatar: None,
                })],
                "menu" => {
                    assert!(egui::Popup::is_any_open(&ctx));
                    Vec::new()
                }
                _ => vec![Action::Open(videos[0].clone())],
            };
            assert_eq!(actions, expected, "unexpected action for {target}");
        }
    }

    #[test]
    fn the_shorts_crop_is_centered_9_of_16() {
        let uv = super::short_uv();
        // 9:16 tall in a 16:9 source is 81/256 of the width, centered.
        assert!((uv.width() - 81.0 / 256.0).abs() < 1e-6);
        assert!((uv.left() - (1.0 - 81.0 / 256.0) / 2.0).abs() < 1e-6);
        assert_eq!(uv.top(), 0.0);
        assert_eq!(uv.bottom(), 1.0);
    }
}
