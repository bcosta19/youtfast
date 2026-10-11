//! The Notifications page: uploads newer than the last visit, in the
//! usual rows grouped by day, with Check now and Mark all as seen.

use egui::{RichText, Ui};

use super::{results, widgets};
use crate::app::State;
use crate::model::Action;
use crate::theme;
use crate::util;

pub fn show(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    results::panel(ui, |ui| {
        results::title(ui, state, "Notifications", Some(state.alerts_visit.len()));
        if !state.settings.alerts {
            results::centered(ui, |ui| {
                ui.label(
                    RichText::new("Alerts are off")
                        .font(theme::semibold(16.0))
                        .color(state.palette.text),
                );
                ui.add_space(8.0);
                if widgets::button(ui, &state.palette, "Turn on", widgets::ButtonStyle::Normal)
                    .clicked()
                {
                    actions.push(Action::SetAlerts(true));
                }
            });
            return;
        }
        ui.horizontal(|ui| {
            if state.alerts_checking {
                widgets::loading(ui, &state.palette, "new uploads");
            } else if widgets::button(
                ui,
                &state.palette,
                "Check now",
                widgets::ButtonStyle::Normal,
            )
            .clicked()
            {
                actions.push(Action::CheckAlertsNow);
            }
            if !state.alerts_visit.is_empty()
                && widgets::button(
                    ui,
                    &state.palette,
                    "Mark all as seen",
                    widgets::ButtonStyle::Normal,
                )
                .clicked()
            {
                actions.push(Action::MarkAlertsSeen);
            }
        });
        ui.add_space(4.0);
        ui.label(
            RichText::new(checked_line(state))
                .font(theme::regular(13.0))
                .color(state.palette.secondary),
        );
        ui.add_space(12.0);
        if state.alerts_visit.is_empty() {
            results::empty(
                ui,
                state,
                "Nothing new",
                "New uploads from your subscriptions appear here",
            );
            return;
        }
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| rows(ui, state, actions));
    });
}

/// `Checked 14:32, next check in 9 min`.
fn checked_line(state: &State) -> String {
    let mut line = match state.last_checked {
        Some(at) => {
            let clock = jiff::Timestamp::from_second(at)
                .map(|at| format!("{}", at.to_zoned(jiff::tz::TimeZone::system()).time()))
                .unwrap_or_else(|_| "?".to_owned());
            format!("Checked {}", clock.get(..5).unwrap_or(&clock))
        }
        None => "Not checked yet".to_owned(),
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

/// Rows grouped by day under section headings; the cursor opens them.
fn rows(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let now = util::now();
    let videos = &state.alerts_visit;
    let mut start = 0;
    while start < videos.len() {
        let day = util::age_bucket(videos[start].published_at, now);
        let end = videos[start..]
            .iter()
            .position(|video| util::age_bucket(video.published_at, now) != day)
            .map_or(videos.len(), |offset| start + offset);
        widgets::section(ui, &state.palette, &capitalized(day), None, |ui| {
            for (index, video) in videos.iter().enumerate().take(end).skip(start) {
                let selected = index == state.cursor;
                let watched = widgets::watched(&state.library, video);
                let response =
                    widgets::video_row(ui, &state.palette, video, None, watched, selected);
                if selected && state.cursor_moved {
                    response.clone().scroll_to_me(None);
                }
                if response.clicked() {
                    actions.push(Action::Open(video.clone()));
                }
                results::video_menu(&response, state, video, actions);
            }
        });
        ui.add_space(16.0);
        start = end;
    }
}

/// `today` → `Today`.
pub(super) fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}
