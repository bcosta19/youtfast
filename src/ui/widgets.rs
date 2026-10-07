//! Pieces shared by the views: the pane every screen is drawn in, the
//! bracket buttons, the spinner, the selectable row.

use egui::text::LayoutJob;
use egui::{
    Color32, CornerRadius, CursorIcon, FontId, Image, Margin, Rect, Response, RichText, Sense,
    Stroke, Ui, pos2, vec2,
};

use crate::icons::Icon;
use crate::model::Video;
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
