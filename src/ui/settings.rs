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
    });
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
