//! The Notifications page: uploads newer than the last visit, in the
//! usual rows grouped by day, with Check now and Mark all as seen.

use crate::appearance::i18n::{Locale, format as trf, gettext as tr};
use egui::{RichText, Ui};

use super::{results, widgets};
use crate::app::State;
use crate::appearance::theme;
use crate::model::Action;
use crate::util;

pub fn show(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    results::panel(ui, |ui| {
        results::title(
            ui,
            state,
            tr(locale, "Notifications").as_ref(),
            Some(state.alerts_visit.len()),
        );
        if !state.settings.alerts {
            results::centered(ui, |ui| {
                ui.label(
                    RichText::new(tr(locale, "Alerts are off").as_ref())
                        .font(theme::semibold(16.0))
                        .color(state.palette.text),
                );
                ui.add_space(8.0);
                if widgets::button(
                    ui,
                    &state.palette,
                    tr(locale, "Turn on").as_ref(),
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
                widgets::loading(ui, &state.palette, "new uploads", locale);
            } else if widgets::button(
                ui,
                &state.palette,
                tr(locale, "Check now").as_ref(),
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
                    tr(locale, "Mark all as seen").as_ref(),
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
                tr(locale, "Nothing new").as_ref(),
                tr(locale, "New videos from subscribed channels appear here").as_ref(),
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
    let locale = state.settings.language;

    let mut line = match state.last_checked {
        Some(at) => {
            let clock = jiff::Timestamp::from_second(at)
                .map(|at| format!("{}", at.to_zoned(jiff::tz::TimeZone::system()).time()))
                .unwrap_or_else(|_| "?".to_owned());
            trf(
                locale,
                "Checked at {time}",
                &[("time", clock.get(..5).unwrap_or(&clock).to_owned())],
            )
        }
        None => tr(locale, "Not checked yet").into_owned(),
    };
    let next = u64::from(state.settings.alerts_every_minutes).saturating_sub(
        state
            .last_checked
            .map(|at| (util::now() - at) as u64 / 60)
            .unwrap_or(0),
    );
    line.push_str(&trf(
        locale,
        ", next check in {minutes} min",
        &[("minutes", next.max(1).to_string())],
    ));
    line
}

/// Rows grouped by day under section headings; the cursor opens them.
fn rows(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    let now = util::now();
    let videos = &state.alerts_visit;
    let mut start = 0;
    while start < videos.len() {
        let day = util::age_bucket(videos[start].published_at, now);
        let end = videos[start..]
            .iter()
            .position(|video| util::age_bucket(video.published_at, now) != day)
            .map_or(videos.len(), |offset| start + offset);
        widgets::section(ui, &state.palette, &capitalized(day, locale), None, |ui| {
            for (index, video) in videos.iter().enumerate().take(end).skip(start) {
                let selected = index == state.cursor;
                let watched = widgets::watched(&state.library, video);
                let response =
                    widgets::video_row(ui, &state.palette, video, None, watched, selected, locale);
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
pub(super) fn capitalized(text: &str, locale: Locale) -> String {
    let source = match text {
        "today" => "Today",
        "yesterday" => "Yesterday",
        "this week" => "This week",
        "this month" => "This month",
        "undated" => "",
        _ => "Earlier",
    };
    tr(locale, source).into_owned()
}
