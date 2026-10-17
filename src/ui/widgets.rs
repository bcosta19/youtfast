//! Pieces shared by the views, in the siblings' look (Spotifast, ZapFast):
//! sections under a heading, rounded cards, pill buttons and chips, the
//! sidebar's items, round avatars, rounded thumbnails, the video as a row
//! or a card, the playlist as a row, the search field, and rich text.

use crate::i18n::{Locale, format as trf, gettext as tr};
use egui::text::LayoutJob;
use egui::{
    Color32, CornerRadius, CursorIcon, FontId, Galley, Id, Pos2, Rect, Response, Sense, Stroke, Ui,
    pos2, vec2,
};

use crate::app::State;
use crate::icons::Icon;
use crate::library::Library;
use crate::model::{Action, Channel, Fetch, PlaylistSummary, Span, Video};
use crate::theme::{self, Palette, RADIUS, RADIUS_SMALL};
use crate::util;

/// The height of one line of body text.
pub fn line(ui: &Ui) -> f32 {
    ui.painter()
        .layout_no_wrap("0".to_owned(), theme::regular(14.0), Color32::WHITE)
        .size()
        .y
}

/// A frameless button that is just the icon, a round tint under it on
/// hover.
pub fn icon_button(ui: &mut Ui, icon: Icon, size: f32, hint: &str) -> Response {
    let edge = (size + 12.0).max(32.0);
    let (rect, response) = ui.allocate_exact_size(vec2(edge, edge), Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), hint));
    if response.has_focus() {
        ui.painter().circle_stroke(
            rect.center(),
            edge / 2.0,
            Stroke::new(1.5, ui.visuals().selection.stroke.color),
        );
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        ui.painter().circle_filled(
            rect.center(),
            edge / 2.0,
            ui.visuals().widgets.hovered.weak_bg_fill,
        );
    }
    let tint = ui.visuals().text_color();
    icon.image(tint, size)
        .paint_at(ui, Rect::from_center_size(rect.center(), vec2(size, size)));
    response.on_hover_text(hint)
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

/// A part of a page: its heading (and a count in the dim colour), then its
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
                            .color(palette.dim),
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
    let mut raised = palette.clone();
    raised.surface = palette.surface_hover;
    raised.surface_hover = palette.surface_active;
    raised
}

/// How a pill button reads.
#[derive(Clone, Copy, PartialEq)]
pub enum ButtonStyle {
    /// A soft pill: `surface`, brighter on hover.
    Normal,
    /// On (subscribed, saved): the soft pill with the label in bold.
    On,
    /// The page's main action, YouTube's way: `text` filled, `window`
    /// label (white on dark, black on light).
    Primary,
}

/// A pill-shaped button with a label.
pub fn button(ui: &mut Ui, palette: &Palette, label: &str, style: ButtonStyle) -> Response {
    icon_text_button(ui, palette, None, label, style)
}

/// A pill-shaped button with an icon before its label (the label may be
/// empty: then the pill is round around the icon).
pub fn icon_text_button(
    ui: &mut Ui,
    palette: &Palette,
    icon: Option<Icon>,
    label: &str,
    style: ButtonStyle,
) -> Response {
    let font = match style {
        ButtonStyle::Normal => theme::medium(14.0),
        ButtonStyle::On | ButtonStyle::Primary => theme::semibold(14.0),
    };
    let color = match style {
        ButtonStyle::Primary => palette.window,
        _ => palette.text,
    };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, color);
    let icon_size = 18.0;
    let gap = if icon.is_some() && !label.is_empty() {
        6.0
    } else {
        0.0
    };
    let icon_width = if icon.is_some() { icon_size } else { 0.0 };
    let height = 34.0;
    let padding = if label.is_empty() { 8.0 } else { 14.0 };
    let width = (icon_width + gap + galley.size().x + 2.0 * padding).max(height);
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let enabled = ui.is_enabled();
    let hovered = response.hovered() && enabled;
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let fill = match style {
        ButtonStyle::Primary if hovered => palette.text.gamma_multiply(0.85),
        ButtonStyle::Primary => palette.text,
        _ if hovered => palette.surface_hover,
        _ => palette.surface,
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same((height / 2.0) as u8), fill);
    let content = icon_width + gap + galley.size().x;
    let mut x = rect.center().x - content / 2.0;
    let tint = if enabled { color } else { palette.dim };
    if let Some(icon) = icon {
        icon.image(tint, icon_size).paint_at(
            ui,
            Rect::from_min_size(
                pos2(x, rect.center().y - icon_size / 2.0),
                vec2(icon_size, icon_size),
            ),
        );
        x += icon_size + gap;
    }
    let size = galley.size();
    ui.painter().galley_with_override_text_color(
        pos2(x, rect.center().y - size.y / 2.0),
        galley,
        tint,
    );
    response
}

/// Subscribe on this device, or show that the account already does.
/// `unsubscribed` is the button's style before subscribing.
pub fn subscribe(
    ui: &mut Ui,
    state: &State,
    channel: &Channel,
    unsubscribed: ButtonStyle,
    actions: &mut Vec<Action>,
) {
    let locale = state.settings.language;

    if state.subscribed_in_account(&channel.id) {
        ui.add_enabled_ui(false, |ui| {
            button(
                ui,
                &state.palette,
                tr(locale, "Subscribed").as_ref(),
                ButtonStyle::On,
            )
        })
        .inner
        .on_disabled_hover_text(
            tr(
                locale,
                "Your account is subscribed. Manage it on youtube.com.",
            )
            .as_ref(),
        );
        return;
    }
    let subscribed = state.library.is_subscribed(&channel.id);
    let (label, style) = if subscribed {
        ("Subscribed", ButtonStyle::On)
    } else {
        ("Subscribe", unsubscribed)
    };
    let hint = if subscribed {
        "Unsubscribe on this device"
    } else {
        "Follow this channel on this device: its videos join your feed"
    };
    if button(ui, &state.palette, tr(locale, label).as_ref(), style)
        .on_hover_text(tr(locale, hint).as_ref())
        .clicked()
    {
        actions.push(Action::ToggleSubscription(channel.clone()));
    }
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

/// A sidebar item, the siblings' way: the icon and the label (bold when
/// current), a rounded fill when current or hovered, and a count at the
/// end. `collapsed` draws only the icon, centred, with the count as a dot.
pub fn nav_item(
    ui: &mut Ui,
    palette: &Palette,
    icon: Icon,
    label: &str,
    count: Option<usize>,
    selected: bool,
    collapsed: bool,
) -> Response {
    let height = 40.0;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    let hovered = response.hovered();
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if selected || hovered {
        let fill = if selected {
            palette.surface
        } else {
            palette.surface_hover
        };
        ui.painter()
            .rect_filled(rect, CornerRadius::same(RADIUS), fill);
    }
    let color = if selected || hovered {
        palette.text
    } else {
        palette.secondary
    };
    let icon_size = 20.0;
    let count = count.filter(|count| *count > 0);
    if collapsed {
        let icon_rect = Rect::from_center_size(rect.center(), vec2(icon_size, icon_size));
        icon.image(color, icon_size).paint_at(ui, icon_rect);
        if count.is_some() {
            ui.painter()
                .circle_filled(icon_rect.right_top() + vec2(1.0, 1.0), 4.0, palette.accent);
        }
        return response.on_hover_text(label);
    }
    let icon_rect = Rect::from_min_size(
        pos2(rect.left() + 12.0, rect.center().y - icon_size / 2.0),
        vec2(icon_size, icon_size),
    );
    icon.image(color, icon_size).paint_at(ui, icon_rect);
    let font = if selected {
        theme::semibold(14.0)
    } else {
        theme::medium(14.0)
    };
    let mut right = rect.right() - 12.0;
    if let Some(count) = count {
        let galley = ui.painter().layout_no_wrap(
            count.to_string(),
            theme::semibold(12.0),
            palette.on_accent,
        );
        let pill = Rect::from_center_size(
            pos2(right - (galley.size().x + 12.0) / 2.0, rect.center().y),
            vec2((galley.size().x + 12.0).max(20.0), 20.0),
        );
        ui.painter()
            .rect_filled(pill, CornerRadius::same(10), palette.accent);
        ui.painter().galley(
            pill.center() - galley.size() / 2.0,
            galley,
            palette.on_accent,
        );
        right = pill.left() - 6.0;
    }
    let mut job = LayoutJob::default();
    job.append(label, 0.0, egui::TextFormat::simple(font, color));
    job.wrap.max_width = (right - icon_rect.right() - 14.0).max(10.0);
    job.wrap.max_rows = 1;
    job.wrap.overflow_character = Some('…');
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(
        pos2(
            icon_rect.right() + 14.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        color,
    );
    response
}

/// A round avatar in `rect`: the picture, or the name's initial on
/// `surface_active` while there is none.
pub fn avatar(ui: &Ui, palette: &Palette, url: Option<&str>, name: &str, rect: Rect) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let radius = CornerRadius::same((rect.width() / 2.0).min(255.0) as u8);
    match url {
        Some(url) => super::images::remote(ui, url)
            .fit_to_exact_size(rect.size())
            .corner_radius(radius)
            .paint_at(ui, rect),
        None => {
            ui.painter()
                .circle_filled(rect.center(), rect.width() / 2.0, palette.surface_active);
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
                palette.text,
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

/// The duration (or LIVE) in the thumbnail's corner, YouTube's way.
fn badge(ui: &Ui, thumbnail: Rect, video: &Video, palette: &Palette, locale: Locale) {
    let (text, fill) = if video.live {
        (tr(locale, "LIVE").into_owned(), palette.red)
    } else if let Some(seconds) = video.duration {
        (util::duration(seconds), Color32::from_black_alpha(200))
    } else {
        return;
    };
    let painter = ui.painter();
    let galley = painter.layout_no_wrap(text, theme::semibold(12.0), Color32::WHITE);
    let size = galley.size() + vec2(8.0, 4.0);
    let rect = Rect::from_min_size(thumbnail.right_bottom() - size - vec2(6.0, 6.0), size);
    painter.rect_filled(rect, CornerRadius::same(RADIUS_SMALL), fill);
    painter.galley(rect.min + vec2(4.0, 2.0), galley, Color32::WHITE);
}

/// The thumbnail, rounded, with its corner badge, and the part watched
/// (0..1) as YouTube's red bar along its bottom edge.
fn thumbnail(
    ui: &Ui,
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
    super::images::remote(ui, &video.thumbnail())
        .fit_to_exact_size(rect.size())
        .corner_radius(radius)
        .paint_at(ui, rect);
    badge(ui, rect, video, palette, locale);
    if let Some(watched) = watched {
        let track = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 4.0), rect.right_bottom());
        let bottom = CornerRadius {
            nw: 0,
            ne: 0,
            sw: RADIUS,
            se: RADIUS,
        };
        ui.painter()
            .rect_filled(track, bottom, Color32::from_white_alpha(70));
        let played = Rect::from_min_size(
            track.min,
            vec2(track.width() * watched.clamp(0.0, 1.0), track.height()),
        );
        ui.painter().rect_filled(
            played,
            CornerRadius {
                nw: 0,
                ne: 0,
                sw: RADIUS,
                se: 0,
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
    y += title.rect.height() + 4.0;
    painter.galley(at, title, palette.text);
    if let Some(channel) = video.channel.as_deref().filter(|name| !name.is_empty()) {
        let galley = lines(channel, theme::regular(13.0), palette.secondary, 1);
        let height = galley.rect.height();
        let rect = Rect::from_min_size(pos2(at.x, y), vec2(galley.size().x, height));
        ui.ctx()
            .data_mut(|d| d.insert_temp(id.with("channel-rect"), rect));
        painter.galley(pos2(at.x, y), galley, palette.secondary);
        y += height + 1.0;
    }
    let rest = [Some(meta(video, locale)), note.map(str::to_owned)]
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    if !rest.is_empty() {
        let galley = lines(&rest, theme::regular(13.0), palette.secondary, 1);
        let height = galley.rect.height();
        painter.galley(pos2(at.x, y), galley, palette.secondary);
        y += height;
    }
    y - at.y
}

/// The fill behind a row or card: `surface_active` when selected (the
/// keyboard's cursor), `surface_hover` under the pointer; rounded.
pub fn row_fill(ui: &Ui, palette: &Palette, rect: Rect, response: &Response, selected: bool) {
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let fill = if selected {
        palette.surface_active
    } else if response.hovered() {
        palette.surface_hover
    } else {
        return;
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(RADIUS), fill);
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
    row_fill(ui, palette, rect, &response, selected);
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
    let menu_rect = Rect::from_min_size(
        pos2(response.rect.right() - 30.0, response.rect.bottom() - 62.0),
        vec2(28.0, 28.0),
    );
    let menu = ui
        .put(
            menu_rect,
            egui::Button::new(Icon::More.image(state.palette.text, 18.0)).frame(false),
        )
        .on_hover_text(tr(locale, "More actions").as_ref());
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
        if ui
            .put(
                clock,
                egui::Button::new(Icon::Clock.image(state.palette.text, 18.0))
                    .fill(state.palette.overlay),
            )
            .on_hover_text(tr(locale, "Watch later on this device").as_ref())
            .clicked()
        {
            handled = true;
            actions.push(Action::ToggleWatchLater(video.clone()));
        }
        if ui
            .put(
                add,
                egui::Button::new(Icon::Queue.image(state.palette.text, 18.0))
                    .fill(state.palette.overlay),
            )
            .on_hover_text(tr(locale, "Add to queue").as_ref())
            .clicked()
        {
            handled = true;
            actions.push(Action::Enqueue {
                video: video.clone(),
                next: false,
            });
        }
    }
    super::results::video_menu(response, state, video, actions);
    if response.clicked() && !handled {
        actions.push(Action::Open(video.clone()));
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
    row_fill(ui, palette, rect, &response, selected);
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
    row_fill(ui, palette, rect, &response, selected);
    let thumb_rect = Rect::from_min_size(rect.min + vec2(pad, pad), size);
    let radius = CornerRadius::same(RADIUS);
    ui.painter()
        .rect_filled(thumb_rect, radius, palette.surface);
    if let Some(url) = &playlist.thumbnail {
        super::images::remote(ui, url)
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

/// Count phrases use the selected catalog's plural rules, independently of content.
pub fn video_count_label(count: u64, locale: Locale) -> String {
    let label = crate::i18n::ngettext(
        locale,
        "{count} video",
        "{count} videos",
        count.min(u32::MAX as u64) as u32,
    );
    crate::i18n::interpolate(&label, &[("count", util::thousands(count))])
}

/// One of the chips over a list, YouTube's way: a pill on `surface`,
/// `text` filled with the `window` colour when on.
pub fn chip(ui: &mut Ui, palette: &Palette, label: &str, on: bool) -> Response {
    let (fill, color) = if on {
        (palette.text, palette.window)
    } else {
        (palette.surface, palette.text)
    };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::medium(14.0), color);
    let size = vec2(galley.size().x + 24.0, 32.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = if !on && response.hovered() {
        palette.surface_hover
    } else {
        fill
    };
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    ui.painter()
        .rect_filled(rect, CornerRadius::same(RADIUS), fill);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
    response
}

/// The search field, the siblings' way: a rounded field with the search
/// icon at its left and a clear button at its right while it has text.
/// Returns the text edit's response (Enter is the caller's to read).
pub fn search_field(
    ui: &mut Ui,
    palette: &Palette,
    text: &mut String,
    hint: &str,
    id: egui::Id,
    width: f32,
) -> Response {
    let height = 36.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let focused = ui.memory(|memory| memory.has_focus(id));
    let fill = if focused {
        palette.surface_hover
    } else {
        palette.surface
    };
    let radius = CornerRadius::same((height / 2.0) as u8);
    ui.painter().rect_filled(rect, radius, fill);
    if focused {
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(1.5, palette.text.gamma_multiply(0.6)),
            egui::StrokeKind::Inside,
        );
    }
    let icon = Rect::from_center_size(pos2(rect.left() + 20.0, rect.center().y), vec2(16.0, 16.0));
    Icon::Search
        .image(palette.secondary, 16.0)
        .paint_at(ui, icon);
    let clear = !text.is_empty();
    let field_rect = Rect::from_min_max(
        pos2(rect.left() + 36.0, rect.top()),
        pos2(
            rect.right() - if clear { 34.0 } else { 14.0 },
            rect.bottom(),
        ),
    );
    let response = ui.put(
        field_rect,
        egui::TextEdit::singleline(text)
            .id(id)
            .frame(egui::Frame::NONE)
            .hint_text(egui::RichText::new(hint).color(palette.dim))
            .font(theme::regular(14.0))
            .vertical_align(egui::Align::Center)
            .desired_width(field_rect.width()),
    );
    if clear {
        let button =
            Rect::from_center_size(pos2(rect.right() - 20.0, rect.center().y), vec2(24.0, 24.0));
        let clear_response = ui.interact(button, id.with("clear"), Sense::click());
        if clear_response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            ui.painter()
                .circle_filled(button.center(), 12.0, palette.surface_active);
        }
        Icon::Close.image(palette.secondary, 15.0).paint_at(
            ui,
            Rect::from_center_size(button.center(), vec2(15.0, 15.0)),
        );
        if clear_response.clicked() {
            text.clear();
            ui.memory_mut(|memory| memory.request_focus(id));
        }
    }
    response
}

/// How many cards of at least `min` wide fit a row `available` wide, with
/// `gap` between them, and the width each then gets.
pub fn grid_columns(available: f32, min: f32, gap: f32, most: usize) -> (usize, f32) {
    let columns = (((available + gap) / (min + gap)).floor() as usize).clamp(1, most);
    let width = (available - gap * (columns - 1) as f32) / columns as f32;
    (columns, width)
}

/// What a span does when clicked.
fn span_action(span: &Span) -> Option<Action> {
    match span {
        Span::Text(_) => None,
        Span::Link { url, .. } => Some(Action::OpenUrl(url.clone())),
        Span::Seek { seconds, .. } => Some(Action::SeekTo {
            seconds: f64::from(*seconds),
            exact: true,
        }),
        Span::Video { id, .. } => Some(Action::Open(Video::from_id(id.clone()))),
        Span::Channel { id, text } => Some(Action::OpenChannel(Channel {
            id: id.clone(),
            name: text.clone(),
            avatar: None,
        })),
    }
}

/// The spans laid out: each span one section, clickable ones remembered
/// by their byte range in the job's text.
fn span_job(
    spans: &[Span],
    palette: &Palette,
    width: f32,
    max_rows: Option<usize>,
) -> (LayoutJob, Vec<(std::ops::Range<usize>, Action)>) {
    let mut job = LayoutJob::default();
    let mut links = Vec::new();
    for span in spans {
        let clickable = span_action(span).is_some();
        let format = egui::TextFormat::simple(
            theme::regular(14.0),
            if clickable {
                palette.blue
            } else {
                palette.text
            },
        );
        let start = job.text.len();
        job.append(span.text(), 0.0, format);
        if clickable && let Some(action) = span_action(span) {
            links.push((start..job.text.len(), action));
        }
    }
    job.wrap.max_width = width;
    if let Some(rows) = max_rows {
        job.wrap.max_rows = rows;
        job.wrap.break_anywhere = false;
        job.wrap.overflow_character = Some('…');
    }
    (job, links)
}

/// Whether `max_rows` would cut the spans at the caller's width.
/// (Colours do not change where a row wraps, so a default palette is fine.)
/// The answer is kept in `id` and only recomputed when the width changes:
/// laying the job out every frame just for this check allocates the whole
/// text again for every comment.
pub fn spans_truncated(ui: &Ui, id: Id, spans: &[Span], max_rows: usize) -> bool {
    let width = ui.available_width();
    if let Some((cached, truncated)) = ui.ctx().data_mut(|d| d.get_persisted::<(f32, bool)>(id))
        && cached == width
    {
        return truncated;
    }
    let (job, _) = span_job(spans, &Palette::dark(), width, None);
    let truncated = ui.painter().layout_job(job).rows.len() > max_rows;
    ui.ctx()
        .data_mut(|d| d.insert_persisted(id, (width, truncated)));
    truncated
}

/// The galley position of the span under `pos`, if it is clickable.
fn span_at<'a>(
    galley: &Galley,
    at: Pos2,
    links: &'a [(std::ops::Range<usize>, Action)],
) -> Option<&'a (std::ops::Range<usize>, Action)> {
    for row in &galley.rows {
        for glyph in &row.row.glyphs {
            let rect = Rect::from_min_size(
                row.pos + glyph.pos.to_vec2(),
                vec2(glyph.advance_width, glyph.line_height),
            );
            if rect.contains(at) {
                let byte = glyph.cluster as usize;
                return links.iter().find(|(range, _)| range.contains(&byte));
            }
        }
    }
    None
}

/// Descriptions and comments: spans wrapped to the width, links in blue
/// and underlined on hover. Returns the action a clicked span wants.
pub fn rich_text(
    ui: &mut Ui,
    palette: &Palette,
    spans: &[Span],
    max_rows: Option<usize>,
    locale: Locale,
) -> Option<Action> {
    let (job, links) = span_job(spans, palette, ui.available_width(), max_rows);
    let galley = ui.painter().layout_job(job);
    let size = galley.size();
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    ui.painter().galley(rect.min, galley.clone(), palette.text);
    if links.is_empty() {
        return None;
    }
    if let Some(hover) = response.hover_pos()
        && let Some((range, _)) = span_at(&galley, hover - rect.min.to_vec2(), &links)
    {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        // Underline the hovered link's glyphs.
        for row in &galley.rows {
            for glyph in &row.row.glyphs {
                if !range.contains(&(glyph.cluster as usize)) {
                    continue;
                }
                let base = rect.min + row.pos.to_vec2() + glyph.pos.to_vec2();
                ui.painter().line_segment(
                    [
                        pos2(base.x, base.y + glyph.font_height),
                        pos2(base.x + glyph.advance_width, base.y + glyph.font_height),
                    ],
                    Stroke::new(1.0, palette.blue),
                );
            }
        }
    }
    if response.clicked()
        && let Some(at) = response.interact_pointer_pos()
        && let Some((_, action)) = span_at(&galley, at - rect.min.to_vec2(), &links)
    {
        return Some(action.clone());
    }
    // Lets the description or a comment's text be copied to the
    // clipboard, since the galley is painted by hand and cannot be
    // selected the way an ordinary label can. The plain text is built
    // only when the menu is actually opened, not every frame.
    response.context_menu(|ui| {
        if ui.button(tr(locale, "Copy").as_ref()).clicked() {
            let plain: String = spans.iter().map(Span::text).collect::<Vec<_>>().join("");
            ui.ctx().copy_text(plain);
            ui.close();
        }
    });
    None
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
                    egui::RichText::new(crate::i18n::message(locale, error).as_ref())
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

/// The uv rect that crops a 4:3 thumbnail to 9:16, centered.
pub fn short_uv() -> Rect {
    // A 4:3 source's width fraction for 9:16 tall is 9/16 * 3/4.
    let width = 9.0 / 16.0 * 3.0 / 4.0;
    let left = (1.0 - width) / 2.0;
    Rect::from_min_max(pos2(left, 0.0), pos2(left + width, 1.0))
}

/// "1.2M views · 2 days ago", or "Live now".
fn meta(video: &Video, locale: Locale) -> String {
    if video.live {
        return tr(locale, "Live now").into_owned();
    }
    let parts: Vec<String> = [
        video.views.map(|count| util::views(count, locale)),
        video.published.clone().or_else(|| {
            video
                .published_at
                .map(|at| util::ago(at, util::now(), locale))
        }),
    ]
    .into_iter()
    .flatten()
    .collect();
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

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
        video.live = true;
        assert_eq!(meta(&video, Locale::English), "Live now");
        assert_eq!(meta(&video, Locale::PortugueseBrazil), "Ao vivo agora");
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
    fn hover_actions_keep_card_positions_and_ids_stable_across_the_grid() {
        for light in [false, true] {
            for (width, columns) in [(480.0, 1), (960.0, 3), (1512.0, 4)] {
                let ctx = egui::Context::default();
                crate::theme::install_fonts(&ctx);
                let state = State {
                    palette: if light {
                        Palette::light()
                    } else {
                        Palette::dark()
                    },
                    ..Default::default()
                };
                crate::theme::apply(&ctx, &state.palette);
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
            crate::theme::install_fonts(&ctx);
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
    fn the_grid_fits_as_many_columns_as_it_can() {
        // 3 × 280 + 2 × 8 = 856 fits in 900; a fourth does not.
        let (columns, width) = super::grid_columns(900.0, 280.0, 8.0, 5);
        assert_eq!(columns, 3);
        assert!((width - (900.0 - 16.0) / 3.0).abs() < 1e-3);
        // Narrower than one card still draws one; wide stops at `most`.
        assert_eq!(super::grid_columns(100.0, 280.0, 8.0, 5).0, 1);
        assert_eq!(super::grid_columns(5000.0, 280.0, 8.0, 5).0, 5);
    }

    #[test]
    fn the_shorts_crop_is_centered_9_of_16() {
        let uv = super::short_uv();
        // 9:16 tall in a 4:3 source is 27/64 of the width, centered.
        assert!((uv.width() - 27.0 / 64.0).abs() < 1e-6);
        assert!((uv.left() - (1.0 - 27.0 / 64.0) / 2.0).abs() < 1e-6);
        assert_eq!(uv.top(), 0.0);
        assert_eq!(uv.bottom(), 1.0);
    }
}
