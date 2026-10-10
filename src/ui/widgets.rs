//! Pieces shared by the views: the pane every screen is drawn in, the
//! bracket buttons and subscribe, the chips, the spinner, the selectable
//! row, the video as a row or a card, and the playlist as a row.

use egui::text::LayoutJob;
use egui::{
    Color32, CornerRadius, CursorIcon, FontId, Galley, Margin, Pos2, Rect, Response, RichText,
    Sense, Stroke, Ui, pos2, vec2,
};

use crate::app::State;
use crate::icons::Icon;
use crate::library::Library;
use crate::model::{Action, Channel, Fetch, PlaylistSummary, Span, Video};
use crate::theme::Palette;
use crate::util;

/// The size of one character of body text.
fn glyph(ui: &Ui) -> egui::Vec2 {
    ui.painter()
        .layout_no_wrap("0".to_owned(), FontId::monospace(14.0), Color32::WHITE)
        .size()
}

/// The width of one character cell, the spacing grid's unit.
pub fn cell(ui: &Ui) -> f32 {
    glyph(ui).x
}

/// The height of one line of body text.
pub fn line(ui: &Ui) -> f32 {
    glyph(ui).y
}

/// A frameless button that is just the icon.
pub fn icon_button(ui: &mut Ui, icon: Icon, size: f32, hint: &str) -> Response {
    ui.add(egui::Button::image(icon.image(ui.visuals().text_color(), size)).frame(false))
        .on_hover_text(hint)
}

/// The braille spinner's frames.
const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// The spinner's frame for now: call while work is in flight; it repaints
/// the window itself until the work is done.
pub fn spinner_frame(ui: &Ui) -> char {
    let frame = (ui.input(|input| input.time) * 12.0) as usize % SPINNER.len();
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(80));
    SPINNER[frame]
}

/// A 1 px pane whose title sits in its top rule, the way lazygit draws
/// it: `─ title ──── 12 ─`. The focused pane's rule is the accent; the
/// title and count are masked by the fill behind.
pub fn pane<R>(
    ui: &mut Ui,
    palette: &Palette,
    title: &str,
    count: Option<usize>,
    focused: bool,
    // What fills the pane (and masks the rule behind the title); `None`
    // leaves it transparent, over `panel_fill`.
    fill: Option<Color32>,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<R> {
    let font = FontId::monospace(14.0);
    let cell = cell(ui);
    let half_line = line(ui) / 2.0;
    let margin = Margin {
        left: cell as i8,
        right: cell as i8,
        top: (half_line + cell) as i8,
        bottom: cell as i8,
    };
    let border = if focused {
        palette.accent
    } else {
        palette.outline
    };
    let fill = fill.unwrap_or_else(|| ui.visuals().panel_fill);
    let inner = egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, border))
        .inner_margin(margin)
        .show(ui, add_contents);
    let rect = inner.response.rect;
    let painter = ui.painter();
    // The title sits on the top rule at the left.
    let mut title = RichText::new(format!(" {title} ")).font(font.clone());
    if focused {
        title = title.strong();
    }
    let color = if focused {
        palette.text
    } else {
        palette.secondary
    };
    let galley = painter.layout_no_wrap(title.text().to_owned(), font.clone(), color);
    let text_rect = Rect::from_center_size(
        pos2(rect.left() + cell + galley.size().x / 2.0, rect.top()),
        galley.size(),
    );
    painter.rect_filled(text_rect, CornerRadius::ZERO, fill);
    painter.galley(text_rect.min, galley, color);
    // The count on the same rule, at the right.
    if let Some(count) = count {
        let galley = painter.layout_no_wrap(
            format!(" {count} "),
            font,
            if focused {
                palette.text
            } else {
                palette.secondary
            },
        );
        let rect = Rect::from_center_size(
            pos2(rect.right() - cell - galley.size().x / 2.0, rect.top()),
            galley.size(),
        );
        painter.rect_filled(rect, CornerRadius::ZERO, fill);
        painter.galley(rect.min, galley, color);
    }
    inner
}

/// How a bracket button reads.
#[derive(Clone, Copy, PartialEq)]
pub enum ButtonStyle {
    /// `[ subscribe ]`: quiet until hovered.
    Normal,
    /// `[ subscribed ]`: on, accent and bold.
    On,
    /// Inverted: accent fill, window text.
    Primary,
}

/// A button that is bracketed text: `[ subscribe ]`.
pub fn bracket_button(ui: &mut Ui, palette: &Palette, label: &str, style: ButtonStyle) -> Response {
    let mut text = RichText::new(format!("[ {label} ]")).monospace();
    text = match style {
        ButtonStyle::Normal => text.color(palette.secondary),
        ButtonStyle::On => text.color(palette.accent).strong(),
        ButtonStyle::Primary => text.color(palette.window),
    };
    let mut button = egui::Button::new(text).frame(false);
    if style == ButtonStyle::Primary {
        // A frameless button draws no fill: keep the frame, without its
        // rule or corners.
        button = button
            .frame(true)
            .fill(palette.accent)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::ZERO);
    }
    ui.add(button)
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
    if state.subscribed_in_account(&channel.id) {
        ui.label(
            RichText::new("[ subscribed ]")
                .monospace()
                .color(state.palette.accent)
                .strong(),
        )
        .on_hover_text("Your account is subscribed. Manage it on youtube.com.");
        return;
    }
    let subscribed = state.library.is_subscribed(&channel.id);
    let (label, style) = if subscribed {
        ("subscribed", ButtonStyle::On)
    } else {
        ("subscribe", unsubscribed)
    };
    let hint = if subscribed {
        "Unsubscribe on this computer"
    } else {
        "Follow this channel on this computer: its videos join your feed"
    };
    if bracket_button(ui, &state.palette, label, style)
        .on_hover_text(hint)
        .clicked()
    {
        actions.push(Action::ToggleSubscription(channel.clone()));
    }
}

/// One selectable row in a list or menu, TUI style: the cursor is the
/// accent inverted. `parts` are `(text, colour)` runs; the colour is the
/// unselected one (the row inverts it). Returns the click response.
pub fn row(
    ui: &mut Ui,
    palette: &Palette,
    selected: bool,
    parts: &[(String, Color32)],
) -> Response {
    let font = FontId::monospace(14.0);
    let mut job = LayoutJob::default();
    for (text, color) in parts {
        job.append(text, 0.0, egui::TextFormat::simple(font.clone(), *color));
    }
    let galley = ui.painter().layout_job(job);
    let height = galley.rect.height().max(line(ui)) + 2.0;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    row_fill(ui, palette, rect, &response, selected);
    let text_pos = pos2(
        rect.left() + cell(ui),
        rect.center().y - galley.rect.height() / 2.0,
    );
    if selected {
        ui.painter()
            .galley_with_override_text_color(text_pos, galley, palette.window);
    } else {
        ui.painter().galley(text_pos, galley, palette.text);
    }
    response
}

/// A channel in the sidebar: a square avatar a line high, the name in
/// blue on one line. `selected` draws the TUI cursor.
pub fn channel_item(ui: &mut Ui, palette: &Palette, channel: &Channel, selected: bool) -> Response {
    let size = line(ui);
    let pad = cell(ui);
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), size + 4.0), Sense::click());
    row_fill(ui, palette, rect, &response, selected);
    let avatar = Rect::from_min_size(
        pos2(rect.left() + pad / 2.0, rect.top() + 2.0),
        vec2(size, size),
    );
    match &channel.avatar {
        Some(url) => super::images::remote(ui, url)
            .fit_to_exact_size(avatar.size())
            .corner_radius(CornerRadius::ZERO)
            .paint_at(ui, avatar),
        None => {
            ui.painter()
                .rect_filled(avatar, CornerRadius::ZERO, palette.surface);
        }
    }
    let color = if selected {
        palette.window
    } else {
        palette.blue
    };
    let mut job = LayoutJob::default();
    job.append(
        &channel.name,
        0.0,
        egui::TextFormat::simple(FontId::monospace(13.0), color),
    );
    job.wrap.max_width = (rect.right() - avatar.right() - pad).max(10.0);
    job.wrap.max_rows = 1;
    job.wrap.overflow_character = Some('…');
    let galley = ui.painter().layout_job(job);
    let at = pos2(
        avatar.right() + pad / 2.0,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(at, galley, color);
    response
}

fn badge(ui: &Ui, thumbnail: Rect, video: &Video, palette: &Palette) {
    let (text, color) = if video.live {
        ("LIVE".to_owned(), palette.red)
    } else if let Some(seconds) = video.duration {
        (util::duration(seconds), palette.yellow)
    } else {
        return;
    };
    let painter = ui.painter();
    let galley = painter.layout_no_wrap(text, FontId::monospace(12.0), color);
    let size = galley.size() + vec2(6.0, 2.0);
    let rect = Rect::from_min_size(thumbnail.right_bottom() - size - vec2(4.0, 4.0), size);
    painter.rect_filled(rect, CornerRadius::ZERO, Color32::BLACK);
    painter.galley(rect.min + vec2(3.0, 1.0), galley, color);
}

/// The thumbnail with its corner badge, and the part watched (0..1) as a
/// 2 px green bar under it.
fn thumbnail(ui: &Ui, palette: &Palette, video: &Video, rect: Rect, watched: Option<f32>) {
    super::images::remote(ui, &video.thumbnail())
        .fit_to_exact_size(rect.size())
        .corner_radius(CornerRadius::ZERO)
        .paint_at(ui, rect);
    badge(ui, rect, video, palette);
    if let Some(watched) = watched {
        let bar = Rect::from_min_size(
            rect.left_bottom() + vec2(0.0, 1.0),
            vec2(rect.width() * watched.clamp(0.0, 1.0), 2.0),
        );
        ui.painter()
            .rect_filled(bar, CornerRadius::ZERO, palette.green);
    }
}

/// The text YouTube puts beside or under a thumbnail: the title on up to
/// two lines, the channel in blue, then `views · age` (and the note).
/// Drawn from `at`, `width` wide; `inverted` is the selected row's colours.
/// Returns the height used.
fn video_text(
    ui: &Ui,
    palette: &Palette,
    video: &Video,
    note: Option<&str>,
    at: Pos2,
    width: f32,
    inverted: bool,
) -> f32 {
    let painter = ui.painter();
    let small = FontId::monospace(13.0);
    let (text, muted, channel_color) = if inverted {
        (palette.window, palette.window, palette.window)
    } else {
        (palette.text, palette.secondary, palette.blue)
    };
    let lines = |text: &str, font: FontId, color: Color32, rows: usize| {
        let mut job = LayoutJob::default();
        job.append(text, 0.0, egui::TextFormat::simple(font, color));
        job.wrap.max_width = width;
        job.wrap.max_rows = rows;
        job.wrap.overflow_character = Some('…');
        painter.layout_job(job)
    };
    let mut y = at.y;
    let title = lines(&video.title, FontId::monospace(14.0), text, 2);
    y += title.rect.height();
    painter.galley(at, title, text);
    if let Some(channel) = video.channel.as_deref().filter(|name| !name.is_empty()) {
        let galley = lines(channel, small.clone(), channel_color, 1);
        let height = galley.rect.height();
        painter.galley(pos2(at.x, y), galley, channel_color);
        y += height;
    }
    let rest = [Some(meta(video)), note.map(str::to_owned)]
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    if !rest.is_empty() {
        let galley = lines(&rest, small, muted, 1);
        let height = galley.rect.height();
        painter.galley(pos2(at.x, y), galley, muted);
        y += height;
    }
    y - at.y
}

/// The fill behind a row or card: the TUI cursor when selected, the
/// hover colour under the pointer.
pub fn row_fill(ui: &Ui, palette: &Palette, rect: Rect, response: &Response, selected: bool) {
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let fill = if selected {
        palette.accent
    } else if response.hovered() {
        palette.surface_hover
    } else {
        return;
    };
    ui.painter().rect_filled(rect, CornerRadius::ZERO, fill);
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
/// 120x68 thumbnail, the title on up to two lines, the channel, then
/// `views · age` and the note when there is one. `watched` is the part
/// watched (0..1). `selected` draws the TUI cursor.
pub fn video_row(
    ui: &mut Ui,
    palette: &Palette,
    video: &Video,
    note: Option<&str>,
    watched: Option<f32>,
    selected: bool,
) -> Response {
    video_row_sized(
        ui,
        palette,
        video,
        note,
        watched,
        selected,
        vec2(120.0, 68.0),
    )
}

/// [`video_row`] with a larger thumbnail, as search results show them.
pub fn video_row_large(
    ui: &mut Ui,
    palette: &Palette,
    video: &Video,
    watched: Option<f32>,
    selected: bool,
) -> Response {
    // Up to 320x180, and never more than half the row.
    let width = (ui.available_width() / 2.0).clamp(120.0, 320.0);
    let thumb = vec2(width, width * 9.0 / 16.0);
    video_row_sized(ui, palette, video, None, watched, selected, thumb)
}

fn video_row_sized(
    ui: &mut Ui,
    palette: &Palette,
    video: &Video,
    note: Option<&str>,
    watched: Option<f32>,
    selected: bool,
    thumb: egui::Vec2,
) -> Response {
    let pad = cell(ui);
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), thumb.y + pad), Sense::click());
    row_fill(ui, palette, rect, &response, selected);
    let thumb_rect = Rect::from_min_size(rect.min + vec2(pad, pad / 2.0), thumb);
    thumbnail(ui, palette, video, thumb_rect, watched);
    let text_x = thumb_rect.right() + pad;
    let width = (rect.right() - text_x - pad).max(20.0);
    video_text(
        ui,
        palette,
        video,
        note,
        pos2(text_x, thumb_rect.top()),
        width,
        selected,
    );
    response
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
) -> Response {
    let pad = cell(ui) / 2.0;
    let thumb = vec2(width - 2.0 * pad, (width - 2.0 * pad) * 9.0 / 16.0);
    // Room for two title lines, the channel and the meta line.
    let text = 2.0 * line(ui) + 2.0 * (line(ui) - 1.0);
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, thumb.y + text + 3.0 * pad), Sense::click());
    row_fill(ui, palette, rect, &response, selected);
    let thumb_rect = Rect::from_min_size(rect.min + vec2(pad, pad), thumb);
    thumbnail(ui, palette, video, thumb_rect, watched);
    video_text(
        ui,
        palette,
        video,
        None,
        pos2(thumb_rect.left(), thumb_rect.bottom() + pad),
        thumb.x,
        selected,
    );
    response
}

/// A playlist as a list row: its 120x68 thumbnail, the name, then
/// `N videos` (and the playlist's channel when `with_channel`).
/// `selected` draws the TUI cursor.
pub fn playlist_row(
    ui: &mut Ui,
    palette: &Palette,
    playlist: &PlaylistSummary,
    selected: bool,
    with_channel: bool,
) -> Response {
    let pad = cell(ui);
    let size = vec2(120.0, 68.0);
    let height = size.y + pad;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    row_fill(ui, palette, rect, &response, selected);
    let thumb_rect = Rect::from_min_size(rect.min + vec2(pad, pad / 2.0), size);
    match &playlist.thumbnail {
        Some(url) => {
            super::images::remote(ui, url)
                .fit_to_exact_size(size)
                .corner_radius(CornerRadius::ZERO)
                .paint_at(ui, thumb_rect);
        }
        None => {
            ui.painter()
                .rect_filled(thumb_rect, CornerRadius::ZERO, palette.surface);
        }
    }
    let name_color = if selected {
        palette.window
    } else {
        palette.text
    };
    let muted = if selected {
        palette.window
    } else {
        palette.secondary
    };
    let text_x = thumb_rect.right() + pad;
    let painter = ui.painter();
    let name = painter.layout_no_wrap(playlist.name.clone(), FontId::monospace(14.0), name_color);
    painter.galley(pos2(text_x, thumb_rect.top()), name, name_color);
    let details: Vec<String> = [
        playlist.videos.map(|count| format!("{count} videos")),
        playlist.channel.clone().filter(|_| with_channel),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !details.is_empty() {
        let galley = painter.layout_no_wrap(details.join(" · "), FontId::monospace(13.0), muted);
        painter.galley(pos2(text_x, thumb_rect.top() + line(ui)), galley, muted);
    }
    response
}

/// One of the chips over a list: ` today `, inverted when on.
pub fn chip(ui: &mut Ui, palette: &Palette, label: &str, on: bool) -> Response {
    let (fill, color) = if on {
        (palette.accent, palette.window)
    } else {
        (palette.surface, palette.text)
    };
    let button = egui::Button::new(RichText::new(format!(" {label} ")).monospace().color(color))
        .fill(fill)
        .corner_radius(CornerRadius::ZERO)
        .stroke(Stroke::NONE);
    let response = ui.add(button);
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
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
            FontId::monospace(14.0),
            if clickable {
                palette.cyan
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
pub fn spans_truncated(ui: &Ui, spans: &[Span], max_rows: usize) -> bool {
    let (job, _) = span_job(spans, &Palette::dark(), ui.available_width(), None);
    let galley = ui.painter().layout_job(job);
    galley.rows.len() > max_rows
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

/// Descriptions and comments: spans wrapped to the pane's width, links in
/// cyan and underlined on hover. Returns the action a clicked span wants.
pub fn rich_text(
    ui: &mut Ui,
    palette: &Palette,
    spans: &[Span],
    max_rows: Option<usize>,
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
                    Stroke::new(1.0, palette.cyan),
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
    None
}

/// The tail row of a paged list: the page on its way, a failed page's
/// retry, or the `[ load more ]` button.
pub fn more_row(
    ui: &mut Ui,
    palette: &Palette,
    state: &Fetch<()>,
    has_more: bool,
    actions: &mut Vec<Action>,
) {
    match state {
        Fetch::Loading => {
            ui.label(
                RichText::new(format!("{} loading", spinner_frame(ui)))
                    .monospace()
                    .color(palette.secondary),
            );
        }
        Fetch::Failed(error) => {
            ui.label(RichText::new(error.as_str()).monospace().color(palette.red));
            if bracket_button(ui, palette, "retry", ButtonStyle::Normal).clicked() {
                actions.push(Action::LoadMore);
            }
        }
        _ => {
            if has_more && bracket_button(ui, palette, "load more", ButtonStyle::Normal).clicked() {
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
fn meta(video: &Video) -> String {
    if video.live {
        return "Live now".to_owned();
    }
    let parts: Vec<String> = [video.views.map(util::views), video.published.clone()]
        .into_iter()
        .flatten()
        .collect();
    parts.join(" · ")
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
