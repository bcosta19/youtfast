//! Playlist list rows and stacked-cover grid cards.

use egui::text::LayoutJob;
use egui::{Color32, CornerRadius, Rect, Response, Sense, Stroke, Ui, pos2, vec2};
use youtfast_i18n::Locale;

use super::{hover_fade, line, mix, row_fill_radius, thumbnail_hairline, video_count_label};
use youtfast_appearance::icons::Icon;
use youtfast_appearance::theme::{self, Palette, RADIUS};
use youtfast_core::model::PlaylistSummary;

/// A playlist as a list row: its thumbnail, the name, then `N videos`
/// (and the playlist's channel when `with_channel`). `selected` marks the
/// keyboard's cursor.
pub fn playlist_row(
    ui: &mut Ui,
    palette: &Palette,
    playlist: &PlaylistSummary,
    selected: bool,
    with_channel: bool,
    locale: Locale,
) -> Response {
    let pad = 6.0;
    let size = vec2(168.0, 94.0);
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), size.y + 2.0 * pad),
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
    let thumb_rect = Rect::from_min_size(rect.min + vec2(pad, pad), size);
    let radius = CornerRadius::same(RADIUS);
    ui.painter()
        .rect_filled(thumb_rect, radius, palette.surface);
    if let Some(url) = &playlist.thumbnail {
        youtfast_app::images::remote(ui, url)
            .fit_to_exact_size(size)
            .corner_radius(radius)
            .paint_at(ui, thumb_rect);
    }
    let text_x = thumb_rect.right() + 12.0;
    let width = (rect.right() - text_x - pad - 34.0).max(20.0);
    let painter = ui.painter();
    let mut job = LayoutJob::default();
    job.append(
        &playlist.name,
        0.0,
        egui::TextFormat::simple(theme::semibold(14.0), palette.text),
    );
    job.wrap.max_width = width;
    job.wrap.max_rows = 2;
    job.wrap.overflow_character = Some('…');
    let name = painter.layout_job(job);
    let name_height = name.rect.height();
    painter.galley(pos2(text_x, thumb_rect.top() + 2.0), name, palette.text);
    let details: Vec<String> = [
        playlist
            .videos
            .map(|count| video_count_label(count, locale)),
        playlist.channel.clone().filter(|_| with_channel),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !details.is_empty() {
        let galley =
            painter.layout_no_wrap(details.join(" · "), theme::regular(13.0), palette.secondary);
        painter.galley(
            pos2(text_x, thumb_rect.top() + 6.0 + name_height),
            galley,
            palette.secondary,
        );
    }
    response
}

pub fn playlist_card_height(ui: &Ui, width: f32) -> f32 {
    let pad = 10.0;
    let stack = 9.0;
    let thumb_h = ((width - 2.0 * pad) * 9.0 / 16.0).round();
    // Title (up to 2 lines) + channel/details line + vertical spacing.
    pad + stack + thumb_h + 3.0 * line(ui) + 18.0 + pad
}

/// A playlist as a grid card: stacked silhouette above its 16:9 thumbnail,
/// video-count badge, two-line name and channel attribution.
pub fn playlist_card(
    ui: &mut Ui,
    palette: &Palette,
    playlist: &PlaylistSummary,
    selected: bool,
    width: f32,
    locale: Locale,
) -> Response {
    let pad = 10.0;
    let stack = 9.0;
    let height = playlist_card_height(ui, width);
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }

    let card_radius = CornerRadius::same(RADIUS + 4);
    ui.painter().rect_filled(rect, card_radius, palette.panel);
    row_fill_radius(ui, palette, rect, &response, selected, card_radius);
    let hover = hover_fade(ui, &response);
    let border = mix(palette.outline, palette.secondary, hover * 0.45);
    ui.painter().rect_stroke(
        rect,
        card_radius,
        Stroke::new(1.0, border),
        egui::StrokeKind::Inside,
    );

    let thumb_w = (width - 2.0 * pad).max(40.0);
    let thumb_h = (thumb_w * 9.0 / 16.0).round();
    let thumb_rect = Rect::from_min_size(rect.min + vec2(pad, pad + stack), vec2(thumb_w, thumb_h));

    // Subtle stacked tabs above the cover thumbnail to distinguish playlists from single videos.
    let stack_radius = CornerRadius {
        nw: 4,
        ne: 4,
        sw: 0,
        se: 0,
    };
    let top_strip = Rect::from_min_max(
        pos2(thumb_rect.left() + 14.0, thumb_rect.top() - 8.0),
        pos2(thumb_rect.right() - 14.0, thumb_rect.top() - 4.5),
    );
    let mid_strip = Rect::from_min_max(
        pos2(thumb_rect.left() + 7.0, thumb_rect.top() - 4.0),
        pos2(thumb_rect.right() - 7.0, thumb_rect.top() - 0.5),
    );
    ui.painter().rect_filled(
        top_strip,
        stack_radius,
        mix(palette.panel, palette.secondary, 0.14),
    );
    ui.painter().rect_filled(
        mid_strip,
        stack_radius,
        mix(palette.panel, palette.secondary, 0.26),
    );

    let thumb_radius = CornerRadius::same(RADIUS);
    ui.painter()
        .rect_filled(thumb_rect, thumb_radius, palette.surface);
    if let Some(url) = &playlist.thumbnail {
        youtfast_app::images::remote(ui, url)
            .fit_to_exact_size(thumb_rect.size())
            .corner_radius(thumb_radius)
            .paint_at(ui, thumb_rect);
    } else {
        Icon::Playlists.image(palette.secondary, 28.0).paint_at(
            ui,
            Rect::from_center_size(thumb_rect.center(), vec2(28.0, 28.0)),
        );
    }
    ui.painter().rect_stroke(
        thumb_rect,
        thumb_radius,
        Stroke::new(1.0, thumbnail_hairline(palette)),
        egui::StrokeKind::Inside,
    );

    if let Some(count) = playlist.videos {
        let count_text = video_count_label(count, locale);
        let painter = ui.painter();
        let galley = painter.layout_no_wrap(count_text, theme::tabular(11.5), Color32::WHITE);
        let pill_w = 12.0 + 5.0 + galley.size().x + 14.0;
        let pill_h = 22.0;
        let pill = Rect::from_min_size(
            pos2(
                thumb_rect.right() - pill_w - 8.0,
                thumb_rect.bottom() - pill_h - 8.0,
            ),
            vec2(pill_w, pill_h),
        );
        painter.rect_filled(
            pill,
            CornerRadius::same(theme::RADIUS_SMALL),
            Color32::from_black_alpha(195),
        );
        Icon::Playlists.image(Color32::WHITE, 12.0).paint_at(
            ui,
            Rect::from_center_size(pos2(pill.left() + 13.0, pill.center().y), vec2(12.0, 12.0)),
        );
        painter.galley(
            pos2(pill.left() + 24.0, pill.center().y - galley.size().y / 2.0),
            galley,
            Color32::WHITE,
        );
    }

    let text_w = thumb_w;
    let painter = ui.painter();
    let mut job = LayoutJob::default();
    job.append(
        &playlist.name,
        0.0,
        egui::TextFormat::simple(theme::semibold(14.5), palette.text),
    );
    job.wrap.max_width = text_w;
    job.wrap.max_rows = 2;
    job.wrap.overflow_character = Some('…');
    let title = painter.layout_job(job);
    let title_pos = pos2(thumb_rect.left(), thumb_rect.bottom() + 10.0);
    let title_h = title.size().y;
    painter.galley(title_pos, title, palette.text);

    let subtitle = playlist
        .channel
        .as_deref()
        .filter(|c| !c.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            playlist
                .videos
                .map(|count| video_count_label(count, locale))
        });
    if let Some(sub) = subtitle {
        let mut sub_job = LayoutJob::default();
        sub_job.append(
            &sub,
            0.0,
            egui::TextFormat::simple(theme::regular(12.5), palette.secondary),
        );
        sub_job.wrap.max_width = text_w;
        sub_job.wrap.max_rows = 1;
        sub_job.wrap.overflow_character = Some('…');
        let sub_galley = painter.layout_job(sub_job);
        painter.galley(
            pos2(thumb_rect.left(), title_pos.y + title_h + 4.0),
            sub_galley,
            palette.secondary,
        );
    }

    response
}
