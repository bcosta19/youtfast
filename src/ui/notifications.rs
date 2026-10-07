//! The Notifications page: uploads newer than the last visit, in the
//! usual rows, with `[ check now ]` and `[ mark all seen ]`.

use egui::{RichText, Ui};

use super::{results, widgets};
use crate::app::State;
use crate::model::{Action, Video};
use crate::util;

pub fn show(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ui, |ui| {
        widgets::pane(
            ui,
            &state.palette,
            "new uploads",
            Some(state.alerts_visit.len()),
            true,
            None,
            |ui| {
                if !state.settings.alerts {
                    results::centered(ui, |ui| {
                        ui.label(
                            RichText::new("alerts are off")
                                .monospace()
                                .color(state.palette.dim),
                        );
                        if widgets::bracket_button(
                            ui,
                            &state.palette,
                            "turn on",
                            widgets::ButtonStyle::Normal,
                        )
                        .clicked()
                        {
                            actions.push(Action::SetAlerts(true));
                        }
                    });
                    return;
                }
                ui.horizontal(|ui| {
                    if state.alerts_checking {
                        ui.label(
                            RichText::new(format!("{} checking", widgets::spinner_frame(ui)))
                                .monospace()
                                .color(state.palette.secondary),
                        );
                    } else if widgets::bracket_button(
                        ui,
                        &state.palette,
                        "check now",
                        widgets::ButtonStyle::Normal,
                    )
                    .clicked()
                    {
                        actions.push(Action::CheckAlertsNow);
                    }
                    if !state.alerts_visit.is_empty()
                        && widgets::bracket_button(
                            ui,
                            &state.palette,
                            "mark all seen",
                            widgets::ButtonStyle::Normal,
                        )
                        .clicked()
                    {
                        actions.push(Action::MarkAlertsSeen);
                    }
                });
                ui.add_space(widgets::line(ui) / 2.0);
                if state.alerts_visit.is_empty() {
                    results::centered(ui, |ui| {
                        ui.label(
                            RichText::new("-- nothing new --")
                                .monospace()
                                .color(state.palette.dim),
                        );
                        ui.label(
                            RichText::new(checked_line(state))
                                .monospace()
                                .color(state.palette.secondary),
                        );
                    });
                    return;
                }
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| rows(ui, state, actions));
            },
        );
    });
}

/// `checked 14:32, next check in 9 min`.
fn checked_line(state: &State) -> String {
    let mut line = match state.last_checked {
        Some(at) => {
            let clock = jiff::Timestamp::from_second(at)
                .map(|at| format!("{}", at.to_zoned(jiff::tz::TimeZone::system()).time()))
                .unwrap_or_else(|_| "?".to_owned());
            format!("checked {}", clock.get(..5).unwrap_or(&clock))
        }
        None => "not checked yet".to_owned(),
    };
    let next = u64::from(state.settings.alerts_every_minutes).saturating_sub(
        state
            .last_checked
            .map(|at| (util::now() - at) as u64 / 60)
            .unwrap_or(0),
    );
    line.push_str(&format!(", next check in {} min", next.max(1)));
    line
}

/// Rows with the home feed's day separators; the cursor opens them.
fn rows(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let now = util::now();
    let mut bucket = "";
    for (index, video) in state.alerts_visit.clone().into_iter().enumerate() {
        let day = util::age_bucket(video.published_at, now);
        if day != bucket {
            bucket = day;
            ui.label(
                RichText::new(format!("── {day} ──"))
                    .monospace()
                    .color(state.palette.secondary),
            );
        }
        let selected = index == state.cursor;
        let response = video_row(ui, state, &video, selected);
        if selected {
            response.clone().scroll_to_me(None);
        }
        if response.clicked() {
            actions.push(Action::Open(video.clone()));
        }
    }
}

fn video_row(ui: &mut Ui, state: &State, video: &Video, selected: bool) -> egui::Response {
    let watched = video
        .duration
        .and_then(|duration| {
            state
                .library
                .resume_position(&video.id)
                .map(|left| left / duration)
        })
        .map(|fraction| fraction as f32);
    widgets::video_row(ui, &state.palette, video, None, watched, selected)
}
