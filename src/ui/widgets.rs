//! Pieces shared by the views: the pane every screen is drawn in, the
//! bracket buttons, the spinner, the selectable row.

use egui::text::LayoutJob;
use egui::{
    Color32, CornerRadius, CursorIcon, FontId, Galley, Image, Margin, Pos2, Rect, Response,
    RichText, Sense, Stroke, Ui, pos2, vec2,
};

use crate::icons::Icon;
use crate::model::{Action, Channel, Fetch, Span, Video};
use crate::theme::Palette;
use crate::util;

/// The width of one character cell, the spacing grid's unit.
pub fn cell(ui: &Ui) -> f32 {
    ui.painter()
        .layout_no_wrap("0".to_owned(), FontId::monospace(14.0), Color32::WHITE)
        .size()
        .x
}

/// The height of one line of body text.
pub fn line(ui: &Ui) -> f32 {
    ui.painter()
        .layout_no_wrap("0".to_owned(), FontId::monospace(14.0), Color32::WHITE)
        .size()
        .y
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
    let behind = fill;
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
    painter.rect_filled(text_rect, CornerRadius::ZERO, behind);
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
        painter.rect_filled(rect, CornerRadius::ZERO, behind);
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
        button = button.fill(palette.accent);
    }
    ui.add(button)
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
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let fill = if selected {
        palette.accent
    } else if response.hovered() {
        palette.surface_hover
    } else {
        Color32::TRANSPARENT
    };
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, fill);
    }
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

/// A video as a list row: a 120x68 thumbnail, the title on one line,
/// `channel · views · age` under it, and a note line when there is one.
/// `watched` is the part watched (0..1), drawn as a green bar under the
/// thumbnail. `selected` draws the TUI cursor.
pub fn video_row(
    ui: &mut Ui,
    palette: &Palette,
    video: &Video,
    note: Option<&str>,
    watched: Option<f32>,
    selected: bool,
) -> Response {
    let font = FontId::monospace(14.0);
    let small = FontId::monospace(13.0);
    let pad = cell(ui);
    let thumb = vec2(120.0, 68.0);
    let height = thumb.y + pad;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let fill = if selected {
        palette.accent
    } else if response.hovered() {
        palette.surface_hover
    } else {
        Color32::TRANSPARENT
    };
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, fill);
    }
    let thumb_rect = Rect::from_min_size(rect.min + vec2(pad, pad / 2.0), thumb);
    Image::new(video.thumbnail())
        .fit_to_exact_size(thumb)
        .corner_radius(CornerRadius::ZERO)
        .paint_at(ui, thumb_rect);
    badge(ui, thumb_rect, video, palette);
    if let Some(watched) = watched {
        let bar = Rect::from_min_size(
            thumb_rect.left_bottom() + vec2(0.0, 1.0),
            vec2(thumb.x * watched.clamp(0.0, 1.0), 2.0),
        );
        ui.painter()
            .rect_filled(bar, CornerRadius::ZERO, palette.green);
    }
    // The text beside the thumbnail: title, then channel · views · age.
    let painter = ui.painter();
    let text_x = thumb_rect.right() + pad;
    let width = (rect.right() - text_x - pad).max(20.0);
    let text_color = if selected {
        palette.window
    } else {
        palette.text
    };
    let muted = if selected {
        palette.window
    } else {
        palette.secondary
    };
    let truncate = |text: &str, font: FontId, color: Color32| {
        let mut job = LayoutJob::default();
        job.append(text, 0.0, egui::TextFormat::simple(font, color));
        job.wrap.max_width = width;
        job.wrap.max_rows = 1;
        job.wrap.overflow_character = Some('…');
        painter.layout_job(job)
    };
    let mut y = thumb_rect.top();
    let galley = truncate(&video.title, font, text_color);
    painter.galley(pos2(text_x, y), galley.clone(), text_color);
    y += galley.rect.height();
    let channel = video.channel.as_deref().unwrap_or_default();
    let mut details = vec![meta(video)];
    if let Some(note) = note {
        details.push(note.to_owned());
    }
    let rest = details
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    let mut job = LayoutJob::default();
    if !channel.is_empty() {
        job.append(
            channel,
            0.0,
            egui::TextFormat::simple(small.clone(), palette.blue),
        );
        if !rest.is_empty() {
            job.append(" · ", 0.0, egui::TextFormat::simple(small.clone(), muted));
        }
    }
    if !rest.is_empty() {
        job.append(&rest, 0.0, egui::TextFormat::simple(small, muted));
    }
    job.wrap.max_width = width;
    job.wrap.max_rows = 1;
    job.wrap.overflow_character = Some('…');
    let galley = painter.layout_job(job);
    painter.galley(pos2(text_x, y), galley.clone(), muted);
    response
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
pub fn meta(video: &Video) -> String {
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
    fn the_shorts_crop_is_centered_9_of_16() {
        let uv = super::short_uv();
        // 9:16 tall in a 4:3 source is 27/64 of the width, centered.
        assert!((uv.width() - 27.0 / 64.0).abs() < 1e-6);
        assert!((uv.left() - (1.0 - 27.0 / 64.0) / 2.0).abs() < 1e-6);
        assert_eq!(uv.top(), 0.0);
        assert_eq!(uv.bottom(), 1.0);
    }
}
