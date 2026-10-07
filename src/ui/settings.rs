//! The settings page: the theme, and where palette files live. Rows use
//! the selected-row style like everywhere else.

use egui::{Align2, RichText, Ui, vec2};

use super::widgets;
use crate::app::State;
use crate::model::Action;
use crate::settings::ThemeChoice;

pub fn show(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ui, |ui| {
        widgets::pane(ui, &state.palette, "Appearance", None, true, None, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    let auto_detail = if state.follows_omarchy {
                        "follows your Omarchy theme"
                    } else {
                        "dark"
                    };
                    choice(
                        ui,
                        state,
                        state.settings.theme == ThemeChoice::Auto,
                        "automatic",
                        Some(auto_detail),
                        actions,
                        ThemeChoice::Auto,
                    );
                    choice(
                        ui,
                        state,
                        state.settings.theme == ThemeChoice::Dark,
                        "dark",
                        None,
                        actions,
                        ThemeChoice::Dark,
                    );
                    choice(
                        ui,
                        state,
                        state.settings.theme == ThemeChoice::Light,
                        "light",
                        None,
                        actions,
                        ThemeChoice::Light,
                    );
                    // The desktop's own palette: "automatic" covers it.
                    for (filename, name) in &state.themes {
                        if filename == fastframe_theme::omarchy::FILENAME {
                            continue;
                        }
                        let picked = ThemeChoice::Custom(filename.clone());
                        choice(
                            ui,
                            state,
                            state.settings.theme == picked,
                            name,
                            None,
                            actions,
                            picked,
                        );
                    }
                    if !state.theme_status.is_empty() {
                        ui.add_space(widgets::line(ui) / 2.0);
                        ui.label(
                            RichText::new(&state.theme_status)
                                .monospace()
                                .color(state.palette.secondary),
                        );
                    }
                    ui.add_space(widgets::line(ui) / 2.0);
                    ui.label(
                        RichText::new(format!("palettes are JSON files in {}", state.themes_dir))
                            .monospace()
                            .small()
                            .color(state.palette.dim),
                    );
                });
        });
        ui.add_space(widgets::line(ui));
        notifications(ui, state, actions);
    });
}

/// The `notifications` pane: the check, desktop popups, the interval.
fn notifications(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    widgets::pane(
        ui,
        &state.palette,
        "notifications",
        None,
        false,
        None,
        |ui| {
            // `[x] check for new uploads`
            let on = state.settings.alerts;
            let mark = if on { "[x]" } else { "[ ]" };
            if widgets::bracket_button(
                ui,
                &state.palette,
                &format!("{mark} check for new uploads"),
                widgets::ButtonStyle::Normal,
            )
            .clicked()
            {
                actions.push(Action::SetAlerts(!on));
            }
            let desktop = state.settings.desktop_notifications;
            let color = if on {
                state.palette.text
            } else {
                state.palette.dim
            };
            let mark = if desktop { "[x]" } else { "[ ]" };
            let response = widgets::bracket_button(
                ui,
                &state.palette,
                &format!("{mark} desktop notifications"),
                widgets::ButtonStyle::Normal,
            );
            if !on {
                // Greyed: it only matters while the checks run.
                ui.painter().rect_stroke(
                    response.rect,
                    egui::CornerRadius::ZERO,
                    egui::Stroke::new(1.0, color),
                    egui::StrokeKind::Inside,
                );
            } else if response.clicked() {
                actions.push(Action::SetDesktopNotifications(!desktop));
            }
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("every:")
                        .monospace()
                        .color(state.palette.secondary),
                );
                for minutes in [5_u32, 15, 30, 60] {
                    let style = if state.settings.alerts_every_minutes == minutes {
                        widgets::ButtonStyle::On
                    } else {
                        widgets::ButtonStyle::Normal
                    };
                    if widgets::bracket_button(ui, &state.palette, &minutes.to_string(), style)
                        .clicked()
                    {
                        actions.push(Action::SetAlertsEvery(minutes));
                    }
                }
                ui.label(
                    RichText::new("min")
                        .monospace()
                        .color(state.palette.secondary),
                );
            });
            ui.label(
            RichText::new(
                "Checks the same sources as Home. YouTube's own notifications                  have no API, so this watches subscriptions instead.",
            )
            .monospace()
            .small()
            .color(state.palette.dim),
        );
        },
    );
}

/// A theme choice as a selectable row, with its detail at the right.
fn choice(
    ui: &mut Ui,
    state: &State,
    selected: bool,
    name: &str,
    detail: Option<&str>,
    actions: &mut Vec<Action>,
    theme: ThemeChoice,
) {
    let parts = [(name.to_owned(), state.palette.text)];
    let response = widgets::row(ui, &state.palette, selected, &parts);
    if let Some(detail) = detail {
        ui.painter().text(
            response.rect.right_center() - vec2(widgets::cell(ui), 0.0),
            Align2::RIGHT_CENTER,
            detail,
            egui::FontId::monospace(13.0),
            if selected {
                state.palette.window
            } else {
                state.palette.dim
            },
        );
    }
    if response.clicked() {
        actions.push(Action::SetTheme(theme));
    }
}
