//! Optional transcript and storyboard views, sharing the bounded image cache.
use super::*;
use crate::appearance::i18n::{Locale, gettext as tr};
use crate::util;

pub(super) fn transcript(
    ui: &mut egui::Ui,
    state: &mut State,
    status: Option<&Status>,
    actions: &mut Vec<Action>,
) {
    let locale: Locale = state.settings.language;
    ui.add_space(12.0);
    let label = if state.transcript_open {
        tr(locale, "Hide transcript")
    } else {
        tr(locale, "Show transcript")
    };
    if widgets::icon_text_button(
        ui,
        &state.palette,
        Some(Icon::Captions),
        &label,
        widgets::ButtonStyle::Normal,
    )
    .clicked()
    {
        actions.push(Action::ToggleTranscript);
    }
    if !state.transcript_open {
        return;
    }
    widgets::card(ui, &state.palette, |ui| match &state.extras {
        Fetch::Idle => actions.push(Action::LoadExtras),
        Fetch::Loading => widgets::loading(ui, &state.palette, "transcript", locale),
        Fetch::Failed(_) => {
            ui.label(tr(locale, "Could not load the transcript."));
            if ui.button(tr(locale, "Retry")).clicked() {
                actions.push(Action::LoadExtras);
            }
        }
        Fetch::Ready(extras) => {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(&extras.language).strong());
                ui.checkbox(&mut state.transcript_follow, tr(locale, "Follow video"));
            });
            ui.add(
                egui::TextEdit::singleline(&mut state.transcript_query)
                    .hint_text(tr(locale, "Search transcript"))
                    .desired_width(ui.available_width()),
            );
            match &extras.transcript {
                Err(error) => {
                    ui.label(crate::appearance::i18n::message(locale, error));
                    if ui.button(tr(locale, "Retry")).clicked() {
                        actions.push(Action::LoadExtras);
                    }
                }
                Ok(cues) if cues.is_empty() => {
                    ui.label(tr(locale, "This video has no transcript available."));
                }
                Ok(cues) => {
                    let position = status.map_or(0.0, |s| s.position);
                    let active = cues.iter().rposition(|c| c.seconds <= position);
                    let query = state.transcript_query.to_lowercase();
                    let area = egui::ScrollArea::vertical()
                        .id_salt("transcript")
                        .max_height(300.0)
                        .show(ui, |ui| {
                            for (index, cue) in cues.iter().enumerate().filter(|(_, c)| {
                                query.is_empty() || c.text.to_lowercase().contains(&query)
                            }) {
                                let response = ui.selectable_label(
                                    active == Some(index),
                                    format!("{}  {}", util::duration(cue.seconds), cue.text),
                                );
                                if state.transcript_follow
                                    && active == Some(index)
                                    && query.is_empty()
                                {
                                    response.scroll_to_me(Some(egui::Align::Center));
                                }
                                if response.clicked() {
                                    actions.push(Action::SeekTo {
                                        seconds: cue.seconds,
                                        exact: true,
                                    });
                                }
                            }
                        });
                    if ui.rect_contains_pointer(area.inner_rect)
                        && ui.input(|i| i.smooth_scroll_delta.y != 0.0)
                    {
                        state.transcript_follow = false;
                    }
                }
            }
        }
    });
}

pub(super) fn seek_preview(
    ui: &egui::Ui,
    bar: egui::Rect,
    state: &State,
    seconds: f64,
    actions: &mut Vec<Action>,
) {
    if matches!(state.extras, Fetch::Idle) {
        actions.push(Action::LoadExtras);
    }
    let Some(preview) = state.extras.ready().and_then(|e| e.preview.as_ref()) else {
        return;
    };
    let Some((url, uv)) = preview.frame(seconds) else {
        return;
    };
    let pointer = ui.input(|i| i.pointer.hover_pos()).unwrap_or(bar.center());
    let size = egui::vec2(
        160.0,
        160.0 * preview.height as f32 / preview.width.max(1) as f32,
    );
    let bounds = ui.ctx().content_rect();
    let left = (pointer.x - size.x / 2.0)
        .clamp(bounds.left(), (bounds.right() - size.x).max(bounds.left()));
    egui::Area::new(ui.id().with("seek-preview"))
        .order(egui::Order::Tooltip)
        .interactable(false)
        .fixed_pos(egui::pos2(
            left,
            (bar.top() - size.y - 58.0).max(bounds.top()),
        ))
        .show(ui.ctx(), |ui| {
            ui.add(
                super::images::remote(ui, &url)
                    .uv(uv)
                    .fit_to_exact_size(size)
                    .corner_radius(egui::CornerRadius::same(6)),
            );
        });
}
