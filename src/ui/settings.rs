//! The settings page, the siblings' way: the page's title, then each group
//! as a heading over a card of rows — the option's name and what it does
//! at the left, its control at the right.

use egui::{Align, Color32, CornerRadius, Layout, Response, RichText, Sense, Ui, pos2, vec2};

use super::{results, widgets};
use crate::app::State;
use crate::i18n::{Locale, format as trf, gettext as tr};
use crate::model::Action;
use crate::settings::ThemeChoice;
use crate::theme::{self, Palette};

/// The widest the settings read well.
const WIDTH: f32 = 760.0;
/// The room a row keeps at its right for a usual control.
const CONTROL_WIDTH: f32 = 260.0;
/// How often the checks may run, in minutes.
const INTERVALS: [u32; 4] = [5, 15, 30, 60];

pub fn show(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    results::panel(ui, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                // set_max_width also widens: a narrow window would cut
                // the text off at its edge instead of wrapping it.
                ui.set_max_width(WIDTH.min(ui.available_width()));
                results::title(ui, state, &tr(locale, "Settings"), None);
                appearance(ui, state, actions);
                ui.add_space(24.0);
                playback(ui, state, actions);
                ui.add_space(24.0);
                notifications(ui, state, actions);
                ui.add_space(24.0);
            });
    });
}

/// The Appearance group: the theme, its status, where palettes live.
fn appearance(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;
    widgets::section(ui, palette, &tr(locale, "Appearance"), None, |ui| {
        widgets::card(ui, palette, |ui| {
            let description = if state.follows_omarchy {
                tr(locale, "Automatic follows your Omarchy theme.")
            } else {
                tr(locale, "Automatic uses YouTFast's dark theme.")
            };
            setting_row(
                ui,
                palette,
                &tr(locale, "Language"),
                &tr(locale, "Choose the language used by the interface."),
                CONTROL_WIDTH,
                |ui| {
                    language_picker(ui, state, actions);
                },
            );
            ui.add_space(12.0);
            setting_row(
                ui,
                palette,
                &tr(locale, "Theme"),
                &description,
                CONTROL_WIDTH,
                |ui| {
                    theme_picker(ui, state, actions);
                },
            );
            if !state.theme_status.is_empty() {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(crate::i18n::message(locale, &state.theme_status))
                        .font(theme::regular(13.0))
                        .color(palette.secondary),
                );
            }
            ui.add_space(8.0);
            ui.label(
                RichText::new(trf(
                    locale,
                    "Palettes are JSON files in {path}",
                    &[("path", state.themes_dir.clone())],
                ))
                .font(theme::regular(13.0))
                .color(palette.secondary),
            );
        });
    });
}

fn language_picker(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::ComboBox::from_id_salt("interface-language")
        .width(200.0)
        .selected_text(state.settings.language.native_name())
        .show_ui(ui, |ui| {
            for &locale in Locale::ALL {
                if ui
                    .selectable_label(state.settings.language == locale, locale.native_name())
                    .clicked()
                {
                    actions.push(Action::SetLanguage(locale));
                    ui.close();
                }
            }
        });
}

/// The theme's menu: Automatic, Dark, Light, then the palette files.
fn theme_picker(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    // The desktop's own palette: "Automatic" covers it.
    let mut choices = vec![
        (ThemeChoice::Auto, tr(locale, "Automatic").into_owned()),
        (ThemeChoice::Dark, tr(locale, "Dark").into_owned()),
        (ThemeChoice::Light, tr(locale, "Light").into_owned()),
    ];
    choices.extend(
        state
            .themes
            .iter()
            .filter(|(filename, _)| filename != fastframe_theme::omarchy::FILENAME)
            .map(|(filename, name)| (ThemeChoice::Custom(filename.clone()), name.clone())),
    );
    let current = choices
        .iter()
        .find(|(choice, _)| *choice == state.settings.theme)
        .map(|(_, name)| name.clone())
        .unwrap_or_else(|| match &state.settings.theme {
            ThemeChoice::Custom(filename) => filename.clone(),
            _ => String::new(),
        });
    egui::ComboBox::from_id_salt("theme")
        .width(200.0)
        .selected_text(RichText::new(current).font(theme::regular(14.0)))
        .show_ui(ui, |ui| {
            for (choice, name) in choices {
                let selected = state.settings.theme == choice;
                if ui
                    .selectable_label(selected, RichText::new(name).font(theme::regular(14.0)))
                    .clicked()
                {
                    actions.push(Action::SetTheme(choice));
                }
            }
        });
}

/// The Playback group: autoplay, SponsorBlock.
fn playback(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;
    widgets::section(ui, palette, &tr(locale, "Playback"), None, |ui| {
        widgets::card(ui, palette, |ui| {
            let autoplay = state.settings.autoplay;
            setting_row(
                ui,
                palette,
                &tr(locale, "Autoplay"),
                &tr(
                    locale,
                    "At the end of a video, the next one starts after a countdown: first the queue, then the source list, then related videos.",
                ),
                CONTROL_WIDTH,
                |ui| {
                    if switch(ui, palette, autoplay, &tr(locale, "Autoplay")).clicked() {
                        actions.push(Action::SetAutoplay(!autoplay));
                    }
                },
            );
            ui.add_space(12.0);
            let sponsorblock = state.settings.sponsorblock;
            setting_row(
                ui,
                palette,
                &tr(locale, "Skip sponsors (SponsorBlock)"),
                &tr(
                    locale,
                    "Skips sponsors and offers to skip intros, credits and other segments marked by the SponsorBlock community. Requests to sponsor.ajay.app use a hash prefix matching thousands of videos.",
                ),
                CONTROL_WIDTH,
                |ui| {
                    if switch(ui, palette, sponsorblock, "SponsorBlock").clicked() {
                        actions.push(Action::SetSponsorBlock(!sponsorblock));
                    }
                },
            );
        });
    });
}

/// The Notifications group: the check, desktop popups, the interval.
fn notifications(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;
    widgets::section(ui, palette, &tr(locale, "Notifications"), None, |ui| {
        widgets::card(ui, palette, |ui| {
            let on = state.settings.alerts;
            setting_row(
                ui,
                palette,
                &tr(locale, "Check for new uploads"),
                &tr(
                    locale,
                    "Checks the same sources as Home. YouTube's notifications have no API, so the app watches subscribed channels instead.",
                ),
                CONTROL_WIDTH,
                |ui| {
                    if switch(ui, palette, on, &tr(locale, "Check for new uploads")).clicked() {
                        actions.push(Action::SetAlerts(!on));
                    }
                },
            );
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

/// One option: its name in medium and what it does in secondary at the
/// left, the control at the right in `control_width`. In a card too
/// narrow for both, the control goes under the text.
fn setting_row(
    ui: &mut Ui,
    palette: &Palette,
    label: &str,
    description: &str,
    control_width: f32,
    control: impl FnOnce(&mut Ui),
) {
    let text = |ui: &mut Ui| {
        ui.label(
            RichText::new(label)
                .font(theme::medium(14.0))
                .color(palette.text),
        );
        if !description.is_empty() {
            ui.add(
                egui::Label::new(
                    RichText::new(description)
                        .font(theme::regular(13.0))
                        .color(palette.secondary),
                )
                .wrap(),
            );
        }
    };
    let reserved = control_width + 16.0;
    if ui.available_width() - reserved < 160.0 {
        ui.vertical(|ui| {
            text(ui);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), control);
            });
        });
    } else {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - reserved).max(0.0));
                text(ui);
            });
            ui.with_layout(Layout::right_to_left(Align::Center), control);
        });
    }
}

/// An on/off switch: a pill, `accent` when on, its round knob sliding to
/// the side that is on. Dimmed while disabled. The caller reads the click.
fn switch(ui: &mut Ui, palette: &Palette, on: bool, label: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(40.0, 22.0), Sense::click());
    let enabled = ui.is_enabled();
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, enabled, on, label));
    if enabled && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool(response.id, on);
        let off = if response.hovered() && enabled {
            palette.outline
        } else {
            palette.surface_active
        };
        let fill = Color32::from(egui::lerp(
            egui::Rgba::from(off)..=egui::Rgba::from(palette.accent),
            t,
        ));
        let (fill, knob) = if enabled {
            (fill, Color32::WHITE)
        } else {
            (fill.gamma_multiply(0.5), Color32::WHITE.gamma_multiply(0.6))
        };
        ui.painter().rect_filled(rect, CornerRadius::same(11), fill);
        let x = egui::lerp(rect.left() + 11.0..=rect.right() - 11.0, t);
        ui.painter()
            .circle_filled(pos2(x, rect.center().y), 8.0, knob);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selecting_a_language_emits_an_action_and_redraws_the_same_settings_page() {
        fn render(
            ctx: &egui::Context,
            state: &State,
            events: Vec<egui::Event>,
            actions: &mut Vec<Action>,
        ) -> Vec<(String, egui::Pos2)> {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        vec2(900.0, 800.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| show(ui, state, actions),
            );
            let labels = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some((
                        text.galley.text().to_owned(),
                        text.pos + text.galley.size() / 2.0,
                    )),
                    _ => None,
                })
                .collect();
            output.textures_delta.clear();
            labels
        }
        fn click(ctx: &egui::Context, state: &State, pos: egui::Pos2, actions: &mut Vec<Action>) {
            for pressed in [true, false] {
                render(
                    ctx,
                    state,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    actions,
                );
            }
        }
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut state = State::default();
        let mut actions = Vec::new();
        for (locale, expected_title) in [
            (Locale::PortugueseBrazil, "Configurações"),
            (Locale::Spanish, "Configuración"),
            (Locale::French, "Paramètres"),
            (Locale::German, "Einstellungen"),
            (Locale::Italian, "Impostazioni"),
            (Locale::English, "Settings"),
        ] {
            let labels = render(&ctx, &state, Vec::new(), &mut actions);
            let picker = labels
                .iter()
                .find(|(label, _)| label == state.settings.language.native_name())
                .unwrap()
                .1;
            click(&ctx, &state, picker, &mut actions);
            let labels = render(&ctx, &state, Vec::new(), &mut actions);
            let option = labels
                .iter()
                .find(|(label, _)| label == locale.native_name())
                .unwrap()
                .1;
            click(&ctx, &state, option, &mut actions);
            assert_eq!(actions, [Action::SetLanguage(locale)]);
            actions.clear();
            state.settings.language = locale;
            let labels = render(&ctx, &state, Vec::new(), &mut actions);
            assert!(labels.iter().any(|(label, _)| label == expected_title));
        }
    }
}
