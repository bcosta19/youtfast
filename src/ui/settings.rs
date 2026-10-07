//! The settings page: the theme, and where palette files live.

use egui::{RichText, Ui};

use crate::app::State;
use crate::model::Action;
use crate::settings::ThemeChoice;

pub fn show(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("Appearance").strong().size(20.0));
                ui.add_space(8.0);
                let auto_detail = if state.follows_omarchy {
                    "Follows your Omarchy theme"
                } else {
                    "Dark"
                };
                row(
                    ui,
                    state.settings.theme == ThemeChoice::Auto,
                    "Automatic",
                    Some(auto_detail),
                    actions,
                    ThemeChoice::Auto,
                );
                row(
                    ui,
                    state.settings.theme == ThemeChoice::Dark,
                    "Dark",
                    None,
                    actions,
                    ThemeChoice::Dark,
                );
                row(
                    ui,
                    state.settings.theme == ThemeChoice::Light,
                    "Light",
                    None,
                    actions,
                    ThemeChoice::Light,
                );
                // The desktop's own palette: "Automatic" already covers it.
                for (filename, name) in &state.themes {
                    if filename == fastframe_theme::omarchy::FILENAME {
                        continue;
                    }
                    let choice = ThemeChoice::Custom(filename.clone());
                    row(
                        ui,
                        state.settings.theme == choice,
                        name,
                        None,
                        actions,
                        choice,
                    );
                }
                if !state.theme_status.is_empty() {
                    ui.add_space(4.0);
                    ui.label(RichText::new(&state.theme_status).weak());
                }
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!("Palettes are JSON files in {}.", state.themes_dir))
                        .weak(),
                );
            });
    });
}

fn row(
    ui: &mut Ui,
    selected: bool,
    name: &str,
    detail: Option<&str>,
    actions: &mut Vec<Action>,
    choice: ThemeChoice,
) {
    let response = egui::Button::selectable(selected, RichText::new(name).size(15.0));
    let response = ui.add_sized([ui.available_width(), 34.0], response);
    if let Some(detail) = detail {
        ui.painter().text(
            response.rect.right_center() - egui::vec2(12.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            detail,
            egui::FontId::proportional(13.0),
            ui.visuals().weak_text_color(),
        );
    }
    if response.clicked() {
        actions.push(Action::SetTheme(choice));
    }
}
