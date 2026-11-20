//! Pieces shared by the views, in the siblings' look (Spotifast, ZapFast):
//! sections under a heading, rounded cards, pill buttons and chips, the
//! sidebar's items, round avatars, rounded thumbnails, the video as a row
//! or a card, the playlist as a row, the search field, and rich text.

mod controls;
mod playlist;
mod rich_text;
mod video;

pub use controls::{
    ButtonStyle, button, chip, icon_button, icon_text_button, nav_item, paint_switch, search_field,
    subscribe,
};
pub use playlist::{playlist_card, playlist_card_height, playlist_row};
pub use rich_text::{rich_text, spans_truncated};
pub(crate) use video::{meta, thumbnail, thumbnail_hairline};
pub use video::{
    short_uv, video_card, video_card_height, video_interactions, video_row, video_row_large,
    watched,
};

use youtfast_i18n::{Locale, format as trf, gettext as tr};

use egui::text::LayoutJob;
use egui::{Color32, CornerRadius, CursorIcon, Rect, Response, Sense, Stroke, Ui, pos2, vec2};

use youtfast_app::Action;
use youtfast_appearance::theme::{self, Palette, RADIUS};
use youtfast_core::model::{Channel, Fetch};
use youtfast_core::util;

/// The height of one line of body text.
pub fn line(ui: &Ui) -> f32 {
    ui.painter()
        .layout_no_wrap("0".to_owned(), theme::regular(14.0), Color32::WHITE)
        .size()
        .y
}

/// Makes keyboard focus visible on custom controls.
pub fn focus_outline(ui: &Ui, response: &Response, radius: CornerRadius) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect,
            radius,
            Stroke::new(1.5, ui.visuals().selection.stroke.color),
            egui::StrokeKind::Inside,
        );
    }
}

/// How far the pointer's hover over `response` has faded in (0 to 1). A
/// control's hover tint follows this instead of switching in one frame; the
/// window is only redrawn while the fade runs.
pub fn hover_fade(ui: &Ui, response: &Response) -> f32 {
    ui.ctx().animate_bool(
        response.id.with("hover"),
        response.hovered() && ui.is_enabled(),
    )
}

/// How far a press on `response` has sunk in (0 to 1), the way
/// [`hover_fade`] follows the pointer.
pub fn press_fade(ui: &Ui, response: &Response) -> f32 {
    ui.ctx().animate_bool(
        response.id.with("press"),
        response.is_pointer_button_down_on() && ui.is_enabled(),
    )
}

/// The colour `t` of the way from `from` to `to`.
pub fn mix(from: Color32, to: Color32, t: f32) -> Color32 {
    if t <= 0.0 {
        from
    } else if t >= 1.0 {
        to
    } else {
        Color32::from(egui::lerp(egui::Rgba::from(from)..=egui::Rgba::from(to), t))
    }
}

/// egui's spinner, `size` across.
pub fn spinner(ui: &mut Ui, size: f32) {
    ui.add(egui::Spinner::new().size(size));
}

/// A spinner and `Loading <what>…` in the secondary colour.
pub fn loading(ui: &mut Ui, palette: &Palette, what: &'static str, locale: Locale) {
    ui.horizontal(|ui| {
        spinner(ui, 16.0);
        ui.label(
            egui::RichText::new(trf(
                locale,
                "Loading {what}…",
                &[("what", tr(locale, what).into_owned())],
            ))
            .font(theme::regular(13.0))
            .color(palette.secondary),
        );
    });
}

/// A part of a page: its heading (and a count in the secondary colour), then its
/// contents. No box: the siblings' sections sit on the page.
pub fn section<R>(
    ui: &mut Ui,
    palette: &Palette,
    title: &str,
    count: Option<usize>,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<R> {
    ui.vertical(|ui| {
        if !title.is_empty() {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(title)
                        .font(theme::bold(18.0))
                        .color(palette.text),
                );
                if let Some(count) = count {
                    ui.label(
                        egui::RichText::new(util::thousands(count as u64))
                            .font(theme::regular(14.0))
                            .color(palette.secondary),
                    );
                }
            });
            ui.add_space(4.0);
        }
        add_contents(ui)
    })
}

/// A rounded box filled with `surface`, the siblings' card: the watch
/// page's description, a settings group, a modal's body. egui's own
/// controls inside it (menus, sliders) sit a step brighter; pass
/// [`on_card`]'s palette to this module's pills inside it.
pub fn card<R>(
    ui: &mut Ui,
    palette: &Palette,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<R> {
    egui::Frame::new()
        .fill(palette.surface)
        .corner_radius(CornerRadius::same(RADIUS))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let widgets = &mut ui.visuals_mut().widgets;
            for (visuals, fill) in [
                (&mut widgets.inactive, palette.surface_hover),
                (&mut widgets.hovered, palette.surface_active),
                (&mut widgets.open, palette.surface_active),
            ] {
                visuals.bg_fill = fill;
                visuals.weak_bg_fill = fill;
            }
            add_contents(ui)
        })
}

/// The palette for pills and chips on a [`card`]: the card is `surface`,
/// so they sit a step brighter.
pub fn on_card(palette: &Palette) -> Palette {
    let mut raised = *palette;
    raised.surface = palette.surface_hover;
    raised.surface_hover = palette.surface_active;
    raised
}

/// One selectable row in a list or menu: a rounded fill on hover, and
/// `surface_active` when selected (the keyboard's cursor). `parts` are
/// `(text, colour)` runs.
pub fn row(
    ui: &mut Ui,
    palette: &Palette,
    selected: bool,
    parts: &[(String, Color32)],
) -> Response {
    let font = theme::regular(14.0);
    let mut job = LayoutJob::default();
    for (text, color) in parts {
        job.append(text, 0.0, egui::TextFormat::simple(font.clone(), *color));
    }
    let galley = ui.painter().layout_job(job);
    let height = galley.rect.height().max(line(ui)) + 12.0;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    row_fill(ui, palette, rect, &response, selected);
    let text_pos = pos2(
        rect.left() + 10.0,
        rect.center().y - galley.rect.height() / 2.0,
    );
    ui.painter().galley(text_pos, galley, palette.text);
    response
}

/// A round avatar in `rect`: the picture, or the name's initial on
/// `secondary` with a `window` letter while there is none.
pub fn avatar(ui: &Ui, palette: &Palette, url: Option<&str>, name: &str, rect: Rect) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let radius = CornerRadius::same((rect.width() / 2.0).min(255.0) as u8);
    match url {
        Some(url) => youtfast_app::images::remote(ui, url)
            .fit_to_exact_size(rect.size())
            .corner_radius(radius)
            .paint_at(ui, rect),
        None => {
            ui.painter()
                .circle_filled(rect.center(), rect.width() / 2.0, palette.secondary);
            let initial: String = name
                .trim_start_matches('@')
                .chars()
                .take(1)
                .collect::<String>()
                .to_uppercase();
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                initial,
                theme::semibold(rect.height() * 0.45),
                palette.window,
            );
        }
    }
}

/// A channel in the sidebar: its round avatar and its name on one line.
/// `selected` marks the channel whose page is open.
pub fn channel_item(ui: &mut Ui, palette: &Palette, channel: &Channel, selected: bool) -> Response {
    let size = 24.0;
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    row_fill(ui, palette, rect, &response, selected);
    let picture = Rect::from_min_size(
        pos2(rect.left() + 10.0, rect.center().y - size / 2.0),
        vec2(size, size),
    );
    avatar(
        ui,
        palette,
        channel.avatar.as_deref(),
        &channel.name,
        picture,
    );
    let mut job = LayoutJob::default();
    job.append(
        &channel.name,
        0.0,
        egui::TextFormat::simple(theme::regular(14.0), palette.text),
    );
    job.wrap.max_width = (rect.right() - picture.right() - 20.0).max(10.0);
    job.wrap.max_rows = 1;
    job.wrap.overflow_character = Some('…');
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(
        pos2(
            picture.right() + 12.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        palette.text,
    );
    response
}

/// A channel in the collapsed sidebar: only its round avatar, centred.
pub fn channel_avatar(
    ui: &mut Ui,
    palette: &Palette,
    channel: &Channel,
    selected: bool,
) -> Response {
    let size = 28.0;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), size + 10.0), Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    row_fill(ui, palette, rect, &response, selected);
    let picture = Rect::from_center_size(rect.center(), vec2(size, size));
    avatar(
        ui,
        palette,
        channel.avatar.as_deref(),
        &channel.name,
        picture,
    );
    response
}

/// The fill behind a row or card: `surface_active` when selected (the
/// keyboard's cursor), `surface_hover` under the pointer; rounded.
pub fn row_fill(ui: &Ui, palette: &Palette, rect: Rect, response: &Response, selected: bool) {
    row_fill_radius(
        ui,
        palette,
        rect,
        response,
        selected,
        CornerRadius::same(RADIUS),
    );
}

pub(crate) fn row_fill_radius(
    ui: &Ui,
    palette: &Palette,
    rect: Rect,
    response: &Response,
    selected: bool,
    radius: CornerRadius,
) {
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let hover = hover_fade(ui, response);
    let press = press_fade(ui, response);
    let on = ui
        .ctx()
        .animate_bool(response.id.with("selected"), selected);
    let lit = hover.max(on);
    if lit > 0.0 {
        let hover_fill = mix(palette.surface_hover, palette.surface_active, press * 0.6);
        let fill = mix(hover_fill, palette.surface_active, on);
        ui.painter()
            .rect_filled(rect, radius, fill.gamma_multiply(lit));
    }
    focus_outline(ui, response, radius);
}

/// Count phrases use the selected catalog's plural rules, independently of content.
pub fn video_count_label(count: u64, locale: Locale) -> String {
    let label = youtfast_i18n::ngettext(
        locale,
        "{count} video",
        "{count} videos",
        count.min(u32::MAX as u64) as u32,
    );
    youtfast_i18n::interpolate(&label, &[("count", util::thousands(count))])
}

/// Compact count label using catalog plural rules (`1.2M`, `340`, `2`, …).
pub fn compact_count_label(
    count: u64,
    singular: &'static str,
    plural: &'static str,
    locale: Locale,
) -> String {
    let phrase =
        youtfast_i18n::ngettext(locale, singular, plural, count.min(u32::MAX as u64) as u32);
    youtfast_i18n::interpolate(&phrase, &[("count", util::compact(count))])
}

/// How many cards of at least `min` wide fit a row `available` wide, with
/// `gap` between them, and the width each then gets.
pub fn grid_columns(available: f32, min: f32, gap: f32, most: usize) -> (usize, f32) {
    let columns = (((available + gap) / (min + gap)).floor() as usize).clamp(1, most);
    let width = (available - gap * (columns - 1) as f32) / columns as f32;
    (columns, width)
}

/// The tail of a paged list: the page on its way, a failed page's retry,
/// or `Load more`.
pub fn more_row(
    ui: &mut Ui,
    palette: &Palette,
    state: &Fetch<()>,
    has_more: bool,
    actions: &mut Vec<Action>,
    locale: Locale,
) {
    match state {
        Fetch::Loading => loading(ui, palette, "more", locale),
        Fetch::Failed(error) => {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(youtfast_i18n::message(locale, error).as_ref())
                        .color(palette.red),
                );
                if button(
                    ui,
                    palette,
                    tr(locale, "Retry").as_ref(),
                    ButtonStyle::Normal,
                )
                .clicked()
                {
                    actions.push(Action::LoadMore);
                }
            });
        }
        _ => {
            if has_more
                && button(
                    ui,
                    palette,
                    tr(locale, "Load more").as_ref(),
                    ButtonStyle::Normal,
                )
                .clicked()
            {
                actions.push(Action::LoadMore);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_grid_fits_as_many_columns_as_it_can() {
        // 3 × 280 + 2 × 8 = 856 fits in 900; a fourth does not.
        let (columns, width) = super::grid_columns(900.0, 280.0, 8.0, 5);
        assert_eq!(columns, 3);
        assert!((width - (900.0 - 16.0) / 3.0).abs() < 1e-3);
        // Narrower than one card still draws one; wide stops at `most`.
        assert_eq!(super::grid_columns(100.0, 280.0, 8.0, 5).0, 1);
        assert_eq!(super::grid_columns(5000.0, 280.0, 8.0, 5).0, 5);
    }
}
