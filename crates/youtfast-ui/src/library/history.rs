//! The watch History page: day-grouped rows, search, local removal, and sync controls.

use egui::{CornerRadius, RichText, Sense, Stroke, Ui, vec2};
use youtfast_i18n::{format as trf, gettext as tr};

use super::{controls, filtered_empty_state, toolbar_summary};
use crate::{results, widgets};
use youtfast_app::{Action, State};
use youtfast_appearance::theme::{self, RADIUS};
use youtfast_core::model::{Listing, Source};
use youtfast_core::util;

pub fn history(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;
    results::panel(ui, |ui| {
        let source = state.history_source();
        let account = source != Some(Source::Device);
        let cache = state.history_cache();
        results::title(
            ui,
            state,
            tr(locale, "History").as_ref(),
            Some(cache.entries.len()),
        );
        let in_progress = cache
            .entries
            .iter()
            .filter(|entry| {
                entry
                    .device
                    .and_then(|idx| state.library.history.get(idx))
                    .is_some_and(|w| w.position > 0.0)
            })
            .count();
        controls(ui, state, actions, |ui, actions| {
            let mut query = state.history_query.clone();
            let hint = if state.merged_history() || account {
                "Search history"
            } else {
                "Search this device's history"
            };
            widgets::search_field(
                ui,
                palette,
                &mut query,
                tr(locale, hint).as_ref(),
                locale,
                egui::Id::new("history search"),
                220.0_f32.min(ui.available_width()),
            );
            if query != state.history_query {
                actions.push(Action::SetHistoryQuery(query));
            }
            if source != Some(Source::Account)
                && !state.library.history.is_empty()
                && widgets::button(
                    ui,
                    palette,
                    tr(locale, "Clear history").as_ref(),
                    widgets::ButtonStyle::Normal,
                )
                .on_hover_text(
                    tr(locale, "Clear this device's history and resume positions").as_ref(),
                )
                .clicked()
            {
                actions.push(Action::ClearHistory);
            }
            if in_progress > 0 {
                toolbar_summary(
                    ui,
                    palette,
                    &widgets::compact_count_label(
                        in_progress as u64,
                        "{count} in progress",
                        "{count} in progress",
                        locale,
                    ),
                );
            }
        });
        if account {
            match &state.account_history {
                Listing::Loading => results::loading(ui, state, "history"),
                Listing::Failed(error) => results::failed(ui, state, error, actions),
                _ => {}
            }
        }
        if cache.entries.is_empty()
            && (!account || matches!(state.account_history, Listing::Ready(_)))
        {
            if !state.history_query.trim().is_empty() {
                filtered_empty_state(
                    ui,
                    palette,
                    tr(locale, "No videos found in history").as_ref(),
                    tr(locale, "Clear filter").as_ref(),
                    || actions.push(Action::SetHistoryQuery(String::new())),
                );
            } else {
                let text = if source == Some(Source::Account) {
                    "The account's watch history"
                } else {
                    "Videos you watch appear here"
                };
                results::nothing(ui, state, tr(locale, text).as_ref());
            }
            return;
        }
        let now = util::now();
        // Rows are one height, so each day's run is drawn only where it shows.
        let show_remove = source != Some(Source::Account) || state.merged_history();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for (run_idx, run) in cache.runs.iter().enumerate() {
                    if run_idx > 0 {
                        ui.add_space(12.0);
                    }
                    egui::Frame::new()
                        .fill(palette.panel)
                        .corner_radius(CornerRadius::same(RADIUS + 4))
                        .stroke(Stroke::new(1.0, palette.outline))
                        .inner_margin(egui::Margin::symmetric(12, 10))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            if let Some(heading) = &run.heading {
                                ui.horizontal(|ui| {
                                    let (pip, _) =
                                        ui.allocate_exact_size(vec2(3.5, 16.0), Sense::hover());
                                    ui.painter().rect_filled(
                                        pip,
                                        CornerRadius::same(2),
                                        palette.accent,
                                    );
                                    ui.label(
                                        RichText::new(heading)
                                            .font(theme::bold(16.0))
                                            .color(palette.text),
                                    );
                                    ui.label(
                                        RichText::new(util::thousands(
                                            (run.end - run.start) as u64,
                                        ))
                                        .font(theme::regular(13.0))
                                        .color(palette.secondary),
                                    );
                                });
                                ui.add_space(8.0);
                            }
                            let selected = (state.cursor_moved
                                && (run.start..run.end).contains(&state.cursor))
                            .then(|| state.cursor - run.start);
                            results::virtual_rows(
                                ui,
                                HISTORY_ROW,
                                run.end - run.start,
                                selected,
                                |ui, row| {
                                    let index = run.start + row;
                                    let row = state.history_row(&cache.entries[index]);
                                    history_row(ui, state, &row, index, show_remove, now, actions);
                                },
                            );
                        });
                }
                if account {
                    ui.add_space(12.0);
                    widgets::more_row(
                        ui,
                        palette,
                        &state.history_more_state,
                        state.history_more.is_some(),
                        actions,
                        locale,
                    );
                }
            });
    });
}

/// A history row's height.
const HISTORY_ROW: f32 = 106.0;

/// One History row: the video, how long ago it was watched here, and the
/// button that forgets it.
fn history_row(
    ui: &mut Ui,
    state: &State,
    row: &youtfast_logic::history::Row<'_>,
    index: usize,
    controls: bool,
    now: i64,
    actions: &mut Vec<Action>,
) {
    let locale = state.settings.language;
    let note = row.device.map(|watched| {
        let mut note = trf(
            locale,
            "Watched {age}",
            &[("age", util::ago(watched.at, now, locale))],
        );
        if watched.position > 0.0 {
            note.push_str(&trf(
                locale,
                " · left at {time}",
                &[("time", util::duration(watched.position))],
            ));
        }
        note
    });
    let video = row.video;
    let fraction = row
        .device
        .and_then(|watched| {
            video
                .duration
                .map(|duration| (watched.position / duration) as f32)
        })
        .or_else(|| widgets::watched(&state.library, video));
    let selected = state.is_cursor(index);
    ui.push_id(&video.id, |ui| {
        ui.horizontal(|ui| {
            // Reserve the same trailing control space for every merged row.
            let reserve = if controls {
                32.0 + ui.spacing().item_spacing.x
            } else {
                0.0
            };
            let width = (ui.available_width() - reserve).max(0.0);
            ui.allocate_ui_with_layout(
                vec2(width, HISTORY_ROW),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    let response = widgets::video_row(
                        ui,
                        &state.palette,
                        video,
                        note.as_deref(),
                        fraction,
                        selected,
                        locale,
                    );
                    widgets::video_interactions(ui, &response, state, video, actions);
                },
            );
            if row.device.is_some() {
                if widgets::icon_button(
                    ui,
                    youtfast_appearance::icons::Icon::Close,
                    16.0,
                    tr(locale, "Remove from this device's history").as_ref(),
                )
                .clicked()
                {
                    actions.push(Action::RemoveHistory(video.id.clone()));
                }
            } else if controls {
                ui.allocate_space(vec2(32.0, 32.0));
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Rect;
    use youtfast_app::AccountState;
    use youtfast_core::model::{Video, View};

    #[test]
    fn merged_history_controls_fit_narrow_and_wide_dark_and_light_pages() {
        use youtfast_core::model::HistoryRow;
        use youtfast_storage::library::Watched;
        use youtfast_storage::settings::HistorySync;
        for width in [360.0, 900.0] {
            for palette in [theme::Palette::dark(), theme::Palette::light()] {
                let ctx = egui::Context::default();
                ctx.enable_accesskit();
                youtfast_appearance::theme::install_fonts(&ctx);
                let mut state = State {
                    view: View::History,
                    palette,
                    account: AccountState::SignedIn(1),
                    account_history: Listing::Ready(vec![HistoryRow {
                        video: Video::from_id("account-only".into()),
                        when: Some("Today".into()),
                    }]),
                    ..Default::default()
                };
                state.settings.history_sync = HistorySync::Merged;
                state.library.history = vec![Watched {
                    video: Video::from_id("device-only".into()),
                    at: 0,
                    position: 0.0,
                }];
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            egui::Pos2::ZERO,
                            vec2(width, 900.0),
                        )),
                        ..Default::default()
                    },
                    |ui| history(ui, &state, &mut Vec::new()),
                );
                output.textures_delta.clear();
                for label in ["All", "Account", "This device"] {
                    let text = output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Text(text) if text.galley.text() == label => Some(text),
                            _ => None,
                        })
                        .unwrap_or_else(|| panic!("missing {label}"));
                    let rect =
                        Rect::from_min_size(text.pos, text.galley.size()).expand2(vec2(12.0, 0.0));
                    assert!(
                        rect.left() >= 0.0 && rect.right() <= width,
                        "{width}: {label}: {rect:?}"
                    );
                }
                let tree = output.platform_output.accesskit_update.unwrap();
                for label in ["Clear history", "Remove from this device's history"] {
                    let (_, node) = tree
                        .nodes
                        .iter()
                        .find(|(_, node)| node.label() == Some(label))
                        .unwrap_or_else(|| panic!("missing {label}"));
                    let rect = node.bounds().unwrap();
                    assert!(
                        rect.x0 >= 0.0 && rect.x1 <= f64::from(width),
                        "{width}: {label}: {rect:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn merged_history_only_offers_local_removal_and_keeps_search_and_keyboard_in_step() {
        use youtfast_core::model::HistoryRow;
        use youtfast_logic::history::HistoryFilter;
        use youtfast_storage::library::Watched;
        use youtfast_storage::settings::HistorySync;
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State {
            view: View::History,
            account: AccountState::SignedIn(1),
            account_history: Listing::Ready(vec![
                HistoryRow {
                    video: Video::from_id("shared".into()),
                    when: Some("Today".into()),
                },
                HistoryRow {
                    video: Video::from_id("account-only".into()),
                    when: Some("Today".into()),
                },
            ]),
            ..Default::default()
        };
        state.settings.history_sync = HistorySync::Merged;
        let mut local = Video::from_id("shared".into());
        local.title = "Local title".into();
        local.duration = Some(100.0);
        state.library.history = vec![Watched {
            video: local,
            at: 0,
            position: 42.0,
        }];
        assert_eq!(state.history_source(), None);
        assert_eq!(state.source(), Source::Account);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(960.0, 900.0))),
                ..Default::default()
            },
            |ui| history(ui, &state, &mut Vec::new()),
        );
        output.textures_delta.clear();
        let tree = output.platform_output.accesskit_update.unwrap();
        assert_eq!(
            tree.nodes
                .iter()
                .filter(|(_, node)| node.label() == Some("Remove from this device's history"))
                .count(),
            1
        );
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().contains("left at 0:42"))));
        state.history_query = "local".into();
        assert_eq!(crate::cursor_count(&state), 1);
        assert!(
            matches!(crate::open_at(&state, 0), Some(Action::Open(video)) if video.id == "shared")
        );
        state.history_filter = HistoryFilter::Device;
        assert_eq!(state.history_source(), Some(Source::Device));
        assert_eq!(state.source(), Source::Account);
        state.account = AccountState::SignedOut;
        state.history_filter = HistoryFilter::All;
        assert_eq!(state.history_source(), Some(Source::Device));
        assert_eq!(state.history_rows().len(), 1);
    }

    #[test]
    fn history_remove_buttons_stay_aligned_and_inside_the_page() {
        use youtfast_storage::library::Watched;
        for width in [480.0, 960.0, 1280.0] {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            youtfast_appearance::theme::install_fonts(&ctx);
            let mut state = State::default();
            state.library.history = (0..5)
                .map(|index| Watched {
                    video: Video::from_id(format!("history{index:04}")),
                    at: 0,
                    position: 0.0,
                })
                .collect();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(width, 900.0))),
                    ..Default::default()
                },
                |ui| history(ui, &state, &mut Vec::new()),
            );
            output.textures_delta.clear();
            let tree = output.platform_output.accesskit_update.unwrap();
            let buttons: Vec<_> = tree
                .nodes
                .iter()
                .filter(|(_, node)| node.label() == Some("Remove from this device's history"))
                .map(|(_, node)| node.bounds().unwrap())
                .collect();
            assert_eq!(buttons.len(), 5);
            for button in &buttons {
                assert!((button.x0 - buttons[0].x0).abs() < 0.1, "{buttons:?}");
                assert!(button.x1 <= f64::from(width), "{button:?}");
                assert_eq!(button.width(), 32.0);
            }
        }
    }

    #[test]
    fn the_history_rows_are_rebuilt_only_when_what_they_come_from_changes() {
        use youtfast_core::model::HistoryRow;
        use youtfast_logic::history::HistoryFilter;
        use youtfast_storage::library::Watched;
        use youtfast_storage::settings::HistorySync;
        let watched = |id: &str, title: &str, at: i64| {
            let mut video = Video::from_id(id.into());
            video.title = title.into();
            Watched {
                video,
                at,
                position: 0.0,
            }
        };
        let mut state = State {
            account: AccountState::SignedIn(1),
            account_history: Listing::Ready(vec![HistoryRow {
                video: Video::from_id("account-only".into()),
                when: Some("Today".into()),
            }]),
            history_requested_at: Some(1_000_000),
            ..Default::default()
        };
        state.settings.history_sync = HistorySync::Merged;
        state
            .library
            .record_watch(watched("a", "Alpha", 1).video, 1);
        let built = |state: &State| state.history_cache().entries.as_ptr();
        let first = built(&state);
        assert_eq!(state.history_rows().len(), 2);
        // Nothing changed: the same rows, not rebuilt.
        assert_eq!(built(&state), first);
        // A search rebuilds, with the same rows lowercased once.
        state.history_query = " ALPHA ".into();
        let ids = |state: &State| {
            state
                .history_rows()
                .iter()
                .map(|row| row.video.id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&state), ["a"]);
        state.history_query.clear();
        assert_eq!(state.history_rows().len(), 2);
        // The device history changing (a watch, a removal, a clear).
        state.library.record_watch(watched("b", "Beta", 2).video, 2);
        assert_eq!(state.history_rows().len(), 3);
        state.library.remove_history("a");
        assert_eq!(state.history_rows().len(), 2);
        state.library.clear_history();
        assert_eq!(state.history_rows().len(), 1);
        // A retitled video is searched by its new title.
        state
            .library
            .record_watch(watched("c", "Gamma", 3).video, 3);
        let mut renamed = Video::from_id("c".into());
        renamed.title = "Delta".into();
        state.library.update_video(&renamed);
        state.history_query = "delta".into();
        assert_eq!(ids(&state), ["c"]);
        state.history_query.clear();
        // The account's list changing, and the filter.
        state.account_history_generation += 1;
        state.account_history.items_mut().push(HistoryRow {
            video: Video::from_id("later".into()),
            when: Some("Today".into()),
        });
        assert_eq!(state.history_rows().len(), 3);
        state.history_filter = HistoryFilter::Device;
        assert_eq!(ids(&state), ["c"]);
        // The day moving on: sections of a request time are fixed, but with
        // none the key follows the clock's day.
        state.history_filter = HistoryFilter::All;
        state.history_requested_at = None;
        let key_day = state.history_cache().entries.len();
        assert_eq!(key_day, 3);
    }
}
