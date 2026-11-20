//! Rich-text spans with cached layout, clickable timestamps/links, and clipboard copy.

use std::sync::Arc;

use egui::text::LayoutJob;
use egui::{CursorIcon, Galley, Id, Pos2, Sense, Stroke, Ui};
use youtfast_i18n::{Locale, gettext as tr};

use youtfast_app::Action;
use youtfast_appearance::theme::{self, Palette};
use youtfast_core::model::{Channel, Span, Video};

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

/// The clickable spans' byte ranges in a job's text, with the span each came
/// from (its action is built only when it is clicked).
type SpanLinks = Arc<[(std::ops::Range<usize>, usize)]>;

/// The spans laid out: each span one section, clickable ones remembered
/// by their byte range in the job's text.
fn span_job(
    spans: &[Span],
    palette: &Palette,
    width: f32,
    max_rows: Option<usize>,
) -> (LayoutJob, Vec<(std::ops::Range<usize>, usize)>) {
    let mut job = LayoutJob::default();
    let mut links = Vec::new();
    for (index, span) in spans.iter().enumerate() {
        let clickable = !matches!(span, Span::Text(_));
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
        if clickable {
            links.push((start..job.text.len(), index));
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
    if let Some((cached, truncated)) = ui.ctx().data_mut(|d| d.get_temp::<(f32, bool)>(id))
        && cached == width
    {
        return truncated;
    }
    let (job, _) = span_job(spans, &Palette::dark(), width, None);
    let truncated = ui.painter().layout_job(job).rows.len() > max_rows;
    ui.ctx().data_mut(|d| d.insert_temp(id, (width, truncated)));
    truncated
}

/// The galley position of the span under `pos`, if it is clickable.
fn span_at<'a>(
    galley: &Galley,
    at: Pos2,
    links: &'a [(std::ops::Range<usize>, usize)],
) -> Option<&'a (std::ops::Range<usize>, usize)> {
    for row in &galley.rows {
        for glyph in &row.row.glyphs {
            let rect = glyph.logical_rect().translate(row.pos.to_vec2());
            if rect.contains(at) {
                let byte = glyph.cluster as usize;
                return links.iter().find(|(range, _)| range.contains(&byte));
            }
        }
    }
    None
}

/// A laid-out `rich_text`, kept while its spans, width, colours, row limit
/// and the fonts stay the same.
#[derive(Clone)]
struct RichLayout {
    signature: u64,
    galley: Arc<Galley>,
    links: SpanLinks,
}

/// What a `rich_text` layout depends on, hashed without building it.
fn rich_signature(
    spans: &[Span],
    palette: &Palette,
    width: f32,
    max_rows: Option<usize>,
    pixels_per_point: f32,
) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (
        width.to_bits(),
        palette.text,
        palette.blue,
        max_rows,
        pixels_per_point.to_bits(),
        theme::fonts_epoch(),
    )
        .hash(&mut hasher);
    for span in spans {
        std::mem::discriminant(span).hash(&mut hasher);
        span.text().hash(&mut hasher);
    }
    hasher.finish()
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
    // The id the text's widget gets below: stable from frame to frame.
    let id = ui.next_auto_id();
    let signature = rich_signature(
        spans,
        palette,
        ui.available_width(),
        max_rows,
        ui.ctx().pixels_per_point(),
    );
    let cached = ui
        .ctx()
        .data(|data| data.get_temp::<RichLayout>(id))
        .filter(|cached| cached.signature == signature);
    let RichLayout { galley, links, .. } = cached.unwrap_or_else(|| {
        let (job, links) = span_job(spans, palette, ui.available_width(), max_rows);
        let layout = RichLayout {
            signature,
            galley: ui.painter().layout_job(job),
            links: links.into(),
        };
        ui.ctx()
            .data_mut(|data| data.insert_temp(id, layout.clone()));
        layout
    });
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
                let rect = glyph
                    .logical_rect()
                    .translate(rect.min.to_vec2() + row.pos.to_vec2());
                ui.painter().line_segment(
                    [rect.left_bottom(), rect.right_bottom()],
                    Stroke::new(1.0, palette.blue),
                );
            }
        }
    }
    if response.clicked()
        && let Some(at) = response.interact_pointer_pos()
        && let Some((_, span)) = span_at(&galley, at - rect.min.to_vec2(), &links)
    {
        return span_action(&spans[*span]);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicking_wrapped_link_glyphs_opens_the_exact_url_and_copy_still_works() {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let palette = Palette::dark();
        let url = "https://example.com/path?q=one%20two#part";
        let spans = [
            Span::Text("Plain text before ".into()),
            Span::Link {
                text: "a long formatted link that wraps across several lines".into(),
                url: url.into(),
            },
            Span::Text(" plain text after".into()),
        ];
        let draw = |ui: &mut Ui, actions: &mut Vec<Action>| {
            ui.set_width(120.0);
            actions.extend(rich_text(ui, &palette, &spans, None, Locale::English));
        };
        let mut targets = Vec::new();
        let mut plain = None;
        for _ in 0..2 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                draw(ui, &mut Vec::new());
            });
            targets.clear();
            for shape in &output.shapes {
                if let egui::Shape::Text(text) = &shape.shape {
                    for (row_index, row) in text.galley.rows.iter().enumerate() {
                        for glyph in &row.row.glyphs {
                            let byte = glyph.cluster as usize;
                            let pos = text.pos
                                + row.pos.to_vec2()
                                + glyph.logical_rect().center().to_vec2();
                            if byte < spans[0].text().len() {
                                plain = Some(pos);
                            } else if byte < spans[0].text().len() + spans[1].text().len() {
                                targets.push((row_index, pos));
                            }
                        }
                    }
                }
            }
            output.textures_delta.clear();
        }
        assert!(
            targets.iter().any(|(row, _)| *row != targets[0].0),
            "the link must wrap across rows"
        );
        for (_, target) in &targets {
            let mut actions = Vec::new();
            for pressed in [true, false] {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        events: vec![
                            egui::Event::PointerMoved(*target),
                            egui::Event::PointerButton {
                                pos: *target,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                        ..Default::default()
                    },
                    |ui| draw(ui, &mut actions),
                );
                output.textures_delta.clear();
            }
            assert_eq!(
                actions,
                [Action::OpenUrl(url.into())],
                "click at {target:?}"
            );
        }
        let plain = plain.expect("plain text must have a visible glyph");
        let mut actions = Vec::new();
        for pressed in [true, false] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(plain),
                        egui::Event::PointerButton {
                            pos: plain,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| draw(ui, &mut actions),
            );
            output.textures_delta.clear();
        }
        assert!(actions.is_empty(), "plain text must not open a link");

        let target = targets[0].1;
        for pressed in [true, false] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(target),
                        egui::Event::PointerButton {
                            pos: target,
                            button: egui::PointerButton::Secondary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| draw(ui, &mut actions),
            );
            output.textures_delta.clear();
        }
        assert!(actions.is_empty(), "right-click must not open a link");
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| draw(ui, &mut actions));
        let copy = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == "Copy" => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("right-click must show Copy");
        output.textures_delta.clear();
        let mut copied = false;
        for pressed in [true, false] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(copy),
                        egui::Event::PointerButton {
                            pos: copy,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| draw(ui, &mut actions),
            );
            copied |= output.platform_output.commands.iter().any(|command| {
                matches!(command, egui::OutputCommand::CopyText(text)
                    if text == "Plain text before a long formatted link that wraps across several lines plain text after")
            });
            output.textures_delta.clear();
        }
        assert!(copied, "Copy must copy the displayed text");
        assert!(actions.is_empty(), "copying must not open a link");
    }

    #[test]
    fn clicking_the_visible_timestamp_glyphs_seeks() {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let palette = Palette::dark();
        let spans = [
            Span::Text("Comentário de alguém\n".into()),
            Span::Seek {
                text: "0:47".into(),
                seconds: 47,
            },
            Span::Text(" ótimo momento".into()),
        ];
        let mut targets = Vec::new();
        for _ in 0..2 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                rich_text(ui, &palette, &spans, None, Locale::English);
            });
            targets.clear();
            for shape in &output.shapes {
                if let egui::Shape::Text(text) = &shape.shape {
                    for row in &text.galley.rows {
                        for glyph in &row.row.glyphs {
                            let byte = glyph.cluster as usize;
                            let start = spans[0].text().len();
                            if (start..start + spans[1].text().len()).contains(&byte) {
                                targets.push(
                                    text.pos
                                        + row.pos.to_vec2()
                                        + glyph.logical_rect().center().to_vec2(),
                                );
                            }
                        }
                    }
                }
            }
            output.textures_delta.clear();
        }
        assert_eq!(targets.len(), 4, "all timestamp glyphs must be visible");
        for target in targets {
            let mut actions = Vec::new();
            for pressed in [true, false] {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        events: vec![
                            egui::Event::PointerMoved(target),
                            egui::Event::PointerButton {
                                pos: target,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                        ..Default::default()
                    },
                    |ui| {
                        actions.extend(rich_text(ui, &palette, &spans, None, Locale::English));
                    },
                );
                output.textures_delta.clear();
            }
            assert_eq!(
                actions,
                [Action::SeekTo {
                    seconds: 47.0,
                    exact: true
                }],
                "click at {target:?}"
            );
        }
    }
}
