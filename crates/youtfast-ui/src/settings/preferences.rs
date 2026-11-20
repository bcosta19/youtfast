//! Playback, History sync, and Notifications settings groups.

use egui::{RichText, Ui};
use youtfast_i18n::{format as trf, gettext as tr, interpolate, ngettext};

use super::{CONTROL_WIDTH, combo_width, setting_row, switch, switch_row};
use crate::widgets;
use youtfast_app::{Action, State};
use youtfast_appearance::theme;
use youtfast_core::model::{SponsorBehavior, SponsorCategory};
use youtfast_storage::settings::{HistorySync, PlaybackBuffer};

/// How often the checks may run, in minutes.
const INTERVALS: [u32; 4] = [5, 15, 30, 60];

/// The Playback group: autoplay, scrolling mini player, video buffer, SponsorBlock.
pub(super) fn playback(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;
    widgets::section(ui, palette, &tr(locale, "Playback"), None, |ui| {
        widgets::card(ui, palette, |ui| {
            let autoplay = state.settings.autoplay;
            if switch_row(
                ui,
                palette,
                &tr(locale, "Autoplay"),
                &tr(
                    locale,
                    "At the end of a video, the next one starts after a countdown: first the queue, then the source list, then related videos.",
                ),
                autoplay,
                &tr(locale, "Autoplay"),
            ) {
                actions.push(Action::SetAutoplay(!autoplay));
            }
            ui.add_space(12.0);
            let scroll_miniplayer = state.settings.scroll_miniplayer;
            if switch_row(
                ui,
                palette,
                &tr(locale, "Miniplayer while scrolling"),
                &tr(
                    locale,
                    "Keep the video visible when scrolling down to the comments.",
                ),
                scroll_miniplayer,
                &tr(locale, "Miniplayer while scrolling"),
            ) {
                actions.push(Action::SetScrollMiniplayer(!scroll_miniplayer));
            }
            ui.add_space(12.0);
            let buffer_choices = [
                (
                    PlaybackBuffer::Standard,
                    "Large — 150 MiB / 50 MiB (mpv default)",
                ),
                (
                    PlaybackBuffer::Compact,
                    "Small — 50 MiB / 20 MiB (less RAM)",
                ),
            ];
            let current_buffer = state.settings.playback_buffer;
            let buffer_description = match current_buffer {
                PlaybackBuffer::Standard => {
                    "Large (150 MiB ahead, 50 MiB back) buffers several minutes ahead for unstable connections and instant seeking, using more RAM (~300–650 MB) and bandwidth if you skip videos early. Small (50 MiB / 20 MiB) uses less RAM (~200 MB) and data, but holds only ~15–90s ahead."
                }
                PlaybackBuffer::Compact => {
                    "Small (50 MiB ahead, 20 MiB back) keeps RAM usage low (~200 MB) and wastes less data when skipping videos, but holds only ~15–90s ahead and re-downloads when seeking far back. Large (150 MiB / 50 MiB) buffers several minutes ahead using more RAM (~300–650 MB)."
                }
            };
            let buffer_width = combo_width(
                ui,
                buffer_choices.iter().map(|(_, label)| tr(locale, label)),
            );
            setting_row(
                ui,
                palette,
                &tr(locale, "Video buffer"),
                &tr(locale, buffer_description),
                buffer_width.max(CONTROL_WIDTH),
                |ui| {
                    egui::ComboBox::from_id_salt("playback-buffer")
                        .width(buffer_width)
                        .selected_text(tr(
                            locale,
                            buffer_choices
                                .iter()
                                .find(|(choice, _)| *choice == current_buffer)
                                .unwrap()
                                .1,
                        ))
                        .show_ui(ui, |ui| {
                            for (choice, label) in buffer_choices {
                                if ui
                                    .selectable_label(current_buffer == choice, tr(locale, label))
                                    .clicked()
                                {
                                    actions.push(Action::SetPlaybackBuffer(choice));
                                    ui.close();
                                }
                            }
                        });
                },
            );
            ui.add_space(12.0);
            let sponsorblock = state.settings.sponsorblock;
            if switch_row(
                ui,
                palette,
                &tr(locale, "Skip sponsors (SponsorBlock)"),
                &tr(
                    locale,
                    "Skips sponsors and offers to skip intros, credits and other segments marked by the SponsorBlock community. Requests to sponsor.ajay.app use a hash prefix matching thousands of videos.",
                ),
                sponsorblock,
                "SponsorBlock",
            ) {
                actions.push(Action::SetSponsorBlock(!sponsorblock));
            }
            if sponsorblock {
                let behavior_width = combo_width(
                    ui,
                    SponsorBehavior::ALL
                        .iter()
                        .map(|behavior| tr(locale, behavior.label())),
                );
                for category in SponsorCategory::ALL {
                    ui.add_space(8.0);
                    let current = state.settings.sponsorblock_categories.get(category);
                    setting_row(
                        ui,
                        palette,
                        &tr(locale, category.display_name()),
                        "",
                        behavior_width.max(CONTROL_WIDTH),
                        |ui| {
                            egui::ComboBox::from_id_salt(("sponsorblock-category", category))
                                .width(behavior_width)
                                .selected_text(tr(locale, current.label()))
                                .show_ui(ui, |ui| {
                                    for behavior in SponsorBehavior::ALL {
                                        if ui
                                            .selectable_label(
                                                current == behavior,
                                                tr(locale, behavior.label()),
                                            )
                                            .clicked()
                                        {
                                            actions.push(Action::SetSponsorCategory(
                                                category, behavior,
                                            ));
                                            ui.close();
                                        }
                                    }
                                });
                        },
                    );
                }
            }
            ui.add_space(12.0);
            let channel_count = state.settings.channel_speeds.len();
            let card = widgets::on_card(palette);
            let count_text = if channel_count == 0 {
                tr(locale, "No channel speeds saved.").into_owned()
            } else {
                let template = ngettext(
                    locale,
                    "{count} channel saved.",
                    "{count} channels saved.",
                    channel_count.min(u32::MAX as usize) as u32,
                );
                interpolate(&template, &[("count", channel_count.to_string())])
            };
            setting_row(
                ui,
                palette,
                &tr(locale, "Per-channel playback speeds"),
                &count_text,
                CONTROL_WIDTH,
                |ui| {
                    ui.add_enabled_ui(channel_count > 0, |ui| {
                        if widgets::button(
                            ui,
                            &card,
                            &tr(locale, "Clear"),
                            widgets::ButtonStyle::Normal,
                        )
                        .clicked()
                        {
                            actions.push(Action::ClearChannelSpeeds);
                        }
                    });
                },
            );
        });
    });
}

/// History keeps the account reporting choice visible beside its control.
pub(super) fn history(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;
    widgets::section(ui, palette, &tr(locale, "History"), None, |ui| {
        widgets::card(ui, palette, |ui| {
            let choices = [
                (HistorySync::Separate, "Separate"),
                (HistorySync::Merged, "Merged in YouTFast"),
                (HistorySync::SendToYoutube, "Merged and sent to YouTube"),
            ];
            let current = state.settings.history_sync;
            let description = match current {
                HistorySync::Separate => {
                    "Keep account and device history separate. Nothing is sent to YouTube."
                }
                HistorySync::Merged => {
                    "Combine account and device history in YouTFast. Nothing is sent to YouTube."
                }
                HistorySync::SendToYoutube => {
                    "Combine history and send playback reports to Google from your account. Google may treat this as automated access."
                }
            };
            let sync_width = combo_width(ui, choices.iter().map(|(_, label)| tr(locale, label)));
            setting_row(
                ui,
                palette,
                &tr(locale, "History sync"),
                &tr(locale, description),
                sync_width.max(CONTROL_WIDTH),
                |ui| {
                    egui::ComboBox::from_id_salt("history-sync")
                        .width(sync_width)
                        .selected_text(tr(
                            locale,
                            choices
                                .iter()
                                .find(|(choice, _)| *choice == current)
                                .unwrap()
                                .1,
                        ))
                        .show_ui(ui, |ui| {
                            for (choice, label) in choices {
                                if ui
                                    .selectable_label(current == choice, tr(locale, label))
                                    .clicked()
                                {
                                    actions.push(Action::SetHistorySync(choice));
                                    ui.close();
                                }
                            }
                        });
                },
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(tr(locale, "Applies to signed-in accounts only."))
                    .font(theme::regular(13.0))
                    .color(palette.secondary),
            );
            ui.add_space(16.0);
            let card = widgets::on_card(palette);
            let export_json = tr(locale, "Export library (JSON)");
            let import_backup = tr(locale, "Import backup");
            let backup_width =
                button_row_width(ui, &[export_json.as_ref(), import_backup.as_ref()])
                    .max(CONTROL_WIDTH);
            setting_row(
                ui,
                palette,
                &tr(locale, "Back up library"),
                &tr(
                    locale,
                    "Export your device subscriptions, watch history with resume positions, and Watch later as JSON, or merge a backup file. Accounts and sessions are never exported.",
                ),
                backup_width,
                |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if widgets::button(ui, &card, &import_backup, widgets::ButtonStyle::Normal)
                        .clicked()
                    {
                        actions.push(Action::ImportBackup);
                    }
                    if widgets::button(ui, &card, &export_json, widgets::ButtonStyle::Normal)
                        .clicked()
                    {
                        actions.push(Action::ExportLibrary);
                    }
                },
            );
            ui.add_space(12.0);
            let export_opml = tr(locale, "Export OPML");
            let opml_width = button_row_width(ui, &[export_opml.as_ref()]).max(CONTROL_WIDTH);
            setting_row(
                ui,
                palette,
                &tr(locale, "Export subscriptions"),
                &tr(
                    locale,
                    "Export your device subscriptions as an OPML file for RSS readers and other apps.",
                ),
                opml_width,
                |ui| {
                    if widgets::button(ui, &card, &export_opml, widgets::ButtonStyle::Normal)
                        .clicked()
                    {
                        actions.push(Action::ExportOpml);
                    }
                },
            );
        });
    });
}

fn button_row_width(ui: &Ui, labels: &[&str]) -> f32 {
    let mut total = 0.0f32;
    for (index, label) in labels.iter().enumerate() {
        if index > 0 {
            total += 8.0;
        }
        let width = (ui
            .painter()
            .layout_no_wrap(
                (*label).to_owned(),
                theme::medium(14.0),
                egui::Color32::WHITE,
            )
            .size()
            .x
            + 28.0)
            .max(34.0);
        total += width;
    }
    total.ceil()
}

/// The Notifications group: the check, desktop popups, the interval.
pub(super) fn notifications(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;
    widgets::section(ui, palette, &tr(locale, "Notifications"), None, |ui| {
        widgets::card(ui, palette, |ui| {
            let on = state.settings.alerts;
            if switch_row(
                ui,
                palette,
                &tr(locale, "Check for new uploads"),
                &tr(
                    locale,
                    "Checks the same sources as Home. YouTube's notifications have no API, so the app watches subscribed channels instead.",
                ),
                on,
                &tr(locale, "Check for new uploads"),
            ) {
                actions.push(Action::SetAlerts(!on));
            }
            ui.add_space(12.0);
            let desktop = state.settings.desktop_notifications;
            setting_row(
                ui,
                palette,
                &tr(locale, "Desktop notifications"),
                &tr(
                    locale,
                    "Shows a desktop notification for each new video found.",
                ),
                CONTROL_WIDTH,
                |ui| {
                    // Greyed: it only matters while the checks run.
                    let response = ui
                        .add_enabled_ui(on, |ui| {
                            switch(ui, palette, desktop, &tr(locale, "Desktop notifications"))
                        })
                        .inner;
                    if on && response.clicked() {
                        actions.push(Action::SetDesktopNotifications(!desktop));
                    }
                },
            );
            ui.add_space(12.0);
            let chips = 4.0 * 72.0 + 3.0 * 8.0;
            setting_row(
                ui,
                palette,
                &tr(locale, "Check every"),
                &tr(locale, "Time between checks."),
                chips,
                |ui| {
                    let raised = widgets::on_card(palette);
                    // Right to left: the longest interval goes in first.
                    for minutes in INTERVALS.into_iter().rev() {
                        let picked = state.settings.alerts_every_minutes == minutes;
                        if widgets::chip(
                            ui,
                            &raised,
                            &trf(locale, "{minutes} min", &[("minutes", minutes.to_string())]),
                            picked,
                        )
                        .clicked()
                        {
                            actions.push(Action::SetAlertsEvery(minutes));
                        }
                    }
                },
            );
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::vec2;
    use youtfast_appearance::theme::Palette;

    #[test]
    fn history_reporting_disclosure_is_visible_in_both_themes_and_widths() {
        for light in [false, true] {
            for width in [360.0, 900.0] {
                let ctx = egui::Context::default();
                theme::install_fonts(&ctx);
                let mut state = State::default();
                state.settings.history_sync = HistorySync::SendToYoutube;
                state.palette = if light {
                    Palette::light()
                } else {
                    Palette::dark()
                };
                let mut actions = Vec::new();
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            vec2(width, 800.0),
                        )),
                        ..Default::default()
                    },
                    |ui| history(ui, &state, &mut actions),
                );
                let labels: Vec<_> = output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) => Some(text.galley.text()),
                        _ => None,
                    })
                    .collect();
                assert!(labels.contains(&"Merged and sent to YouTube"));
                assert!(labels.contains(&"Combine history and send playback reports to Google from your account. Google may treat this as automated access."));
                assert!(labels.contains(&"Applies to signed-in accounts only."));
                assert!(actions.is_empty());
                output.textures_delta.clear();
            }
        }
    }

    #[test]
    fn playback_buffer_shows_tradeoffs_and_emits_action_on_change() {
        for (buffer, expected_selected, expected_tradeoff) in [
            (
                PlaybackBuffer::Standard,
                "Large — 150 MiB / 50 MiB (mpv default)",
                "Large (150 MiB ahead, 50 MiB back) buffers several minutes ahead for unstable connections and instant seeking, using more RAM (~300–650 MB) and bandwidth if you skip videos early. Small (50 MiB / 20 MiB) uses less RAM (~200 MB) and data, but holds only ~15–90s ahead.",
            ),
            (
                PlaybackBuffer::Compact,
                "Small — 50 MiB / 20 MiB (less RAM)",
                "Small (50 MiB ahead, 20 MiB back) keeps RAM usage low (~200 MB) and wastes less data when skipping videos, but holds only ~15–90s ahead and re-downloads when seeking far back. Large (150 MiB / 50 MiB) buffers several minutes ahead using more RAM (~300–650 MB).",
            ),
        ] {
            let ctx = egui::Context::default();
            theme::install_fonts(&ctx);
            let mut state = State::default();
            state.settings.playback_buffer = buffer;
            let mut actions = Vec::new();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        vec2(900.0, 800.0),
                    )),
                    ..Default::default()
                },
                |ui| playback(ui, &state, &mut actions),
            );
            let labels: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(text.galley.text()),
                    _ => None,
                })
                .collect();
            assert!(labels.contains(&"Video buffer"));
            assert!(labels.contains(&expected_selected));
            assert!(labels.contains(&expected_tradeoff));
            assert!(actions.is_empty());
            output.textures_delta.clear();
        }
    }

    #[test]
    fn playback_channel_speeds_row_shows_count_and_clears() {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);

        let empty_state = State::default();
        let mut actions = Vec::new();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    vec2(900.0, 800.0),
                )),
                ..Default::default()
            },
            |ui| playback(ui, &empty_state, &mut actions),
        );
        let labels: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect();
        output.textures_delta.clear();
        assert!(labels.iter().any(|l| l == "Per-channel playback speeds"));
        assert!(labels.iter().any(|l| l == "No channel speeds saved."));
        assert!(actions.is_empty());

        let mut filled_state = State::default();
        filled_state
            .settings
            .set_channel_speed("UC_one".into(), 1.5);
        filled_state
            .settings
            .set_channel_speed("UC_two".into(), 1.25);
        let mut actions = Vec::new();
        let mut clear_pos = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    vec2(900.0, 800.0),
                )),
                ..Default::default()
            },
            |ui| playback(ui, &filled_state, &mut actions),
        );
        let mut labels = Vec::new();
        for shape in &output.shapes {
            if let egui::Shape::Text(text) = &shape.shape {
                labels.push(text.galley.text().to_owned());
                if text.galley.text() == "Clear" {
                    clear_pos =
                        Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center());
                }
            }
        }
        output.textures_delta.clear();
        assert!(labels.iter().any(|l| l == "2 channels saved."));
        let pos = clear_pos.expect("Clear button should be drawn");

        for pressed in [true, false] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        vec2(900.0, 800.0),
                    )),
                    events: vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| playback(ui, &filled_state, &mut actions),
            );
            output.textures_delta.clear();
        }
        assert_eq!(actions, [Action::ClearChannelSpeeds]);
    }

    #[test]
    fn history_backup_and_opml_buttons_render_and_emit_actions() {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let state = State::default();

        let render = |events: Vec<egui::Event>, actions: &mut Vec<Action>| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        vec2(900.0, 800.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| history(ui, &state, actions),
            );
            let positions: Vec<(String, egui::Pos2)> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some((
                        text.galley.text().to_owned(),
                        egui::Rect::from_min_size(text.pos, text.galley.size()).center(),
                    )),
                    _ => None,
                })
                .collect();
            output.textures_delta.clear();
            positions
        };

        let mut actions = Vec::new();
        let positions = render(Vec::new(), &mut actions);
        assert!(actions.is_empty());
        for expected in [
            "Back up library",
            "Export library (JSON)",
            "Import backup",
            "Export subscriptions",
            "Export OPML",
        ] {
            assert!(
                positions.iter().any(|(label, _)| label == expected),
                "missing label: {expected}"
            );
        }

        for (button, expected_action) in [
            ("Export library (JSON)", Action::ExportLibrary),
            ("Import backup", Action::ImportBackup),
            ("Export OPML", Action::ExportOpml),
        ] {
            let pos = positions
                .iter()
                .find(|(label, _)| label == button)
                .unwrap()
                .1;
            for pressed in [true, false] {
                render(
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    &mut actions,
                );
            }
            assert_eq!(actions, [expected_action]);
            actions.clear();
        }
    }

    #[test]
    fn playback_shows_sponsorblock_category_choices_when_enabled() {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);

        let mut state = State::default();
        let mut actions = Vec::new();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    vec2(900.0, 1200.0),
                )),
                ..Default::default()
            },
            |ui| playback(ui, &state, &mut actions),
        );
        let disabled_labels: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect();
        output.textures_delta.clear();
        assert!(!disabled_labels.iter().any(|l| l == "Non-music section"));

        state.settings.sponsorblock = true;
        state
            .settings
            .sponsorblock_categories
            .set(SponsorCategory::Filler, SponsorBehavior::Ignore);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    vec2(900.0, 1200.0),
                )),
                ..Default::default()
            },
            |ui| playback(ui, &state, &mut actions),
        );
        let labels: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect();
        output.textures_delta.clear();
        for cat in SponsorCategory::ALL {
            assert!(
                labels.iter().any(|l| l == cat.display_name()),
                "missing category {}",
                cat.display_name()
            );
        }
        assert!(labels.iter().any(|l| l == "Skip automatically"));
        assert!(labels.iter().any(|l| l == "Show skip button"));
        assert!(labels.iter().any(|l| l == "Ignore"));
    }
}
