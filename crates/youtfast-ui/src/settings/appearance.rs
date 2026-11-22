//! Appearance settings: language, interface font picker, and theme selection.

use egui::{RichText, Ui};
use youtfast_i18n::{Locale, format as trf, gettext as tr};

use super::{CONTROL_WIDTH, setting_row};
use crate::widgets;
use youtfast_app::{Action, State};
use youtfast_appearance::theme::{self, InterfaceFont, InterfaceStyle};
use youtfast_storage::settings::ThemeChoice;

/// How tall the list of installed families may grow.
const FONT_MENU_HEIGHT: f32 = 320.0;

/// The Appearance group: the theme, its status, where palettes live.
pub(super) fn appearance(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
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
            widgets::hairline_divider(ui, palette);
            setting_row(
                ui,
                palette,
                &tr(locale, "Interface font"),
                &tr(
                    locale,
                    "The font that draws the interface. System uses your desktop's own.",
                ),
                CONTROL_WIDTH,
                |ui| {
                    interface_font_picker(ui, state, actions);
                },
            );
            if state.interface_font_missing {
                // Inter draws instead of the face asked for. Only the
                // system's own face is worth a report: a family that was
                // removed is chosen again in the menu above.
                let notice = match &state.settings.interface_font {
                    InterfaceFont::System => Some((
                        tr(
                            locale,
                            "Your system's interface font couldn't be loaded, so YouTFast uses Inter.",
                        )
                        .into_owned(),
                        true,
                    )),
                    InterfaceFont::Family(family) => Some((
                        trf(
                            locale,
                            "{font} couldn't be loaded, so YouTFast uses Inter.",
                            &[("font", family.clone())],
                        ),
                        false,
                    )),
                    InterfaceFont::Inter => None,
                };
                if let Some((notice, report)) = notice {
                    ui.add_space(8.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(notice)
                                .font(theme::regular(13.0))
                                .color(palette.secondary),
                        )
                        .wrap(),
                    );
                    if report {
                        ui.add_space(4.0);
                        if widgets::button(
                            ui,
                            palette,
                            &tr(locale, "Report this"),
                            widgets::ButtonStyle::Normal,
                        )
                        .clicked()
                        {
                            actions.push(Action::ReportInterfaceFont);
                        }
                    }
                }
            }
            widgets::hairline_divider(ui, palette);
            setting_row(
                ui,
                palette,
                &tr(locale, "Interface style"),
                &tr(locale, state.settings.interface_style.description()),
                CONTROL_WIDTH,
                |ui| {
                    interface_style_picker(ui, state, actions);
                },
            );
            widgets::hairline_divider(ui, palette);
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
                    RichText::new(youtfast_i18n::message(locale, &state.theme_status))
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

/// The interface font's menu: the system's, Inter, then the families
/// installed on this computer, narrowed by what is typed in its field.
fn interface_font_picker(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let current = &state.settings.interface_font;
    let name = |font: &InterfaceFont| match font {
        InterfaceFont::System => tr(locale, "System (default)").into_owned(),
        InterfaceFont::Inter => "Inter".to_owned(),
        InterfaceFont::Family(family) => family.clone(),
    };
    let item = |ui: &mut Ui, font: InterfaceFont, actions: &mut Vec<Action>| {
        let label = RichText::new(name(&font)).font(theme::regular(14.0));
        if ui.selectable_label(*current == font, label).clicked() {
            actions.push(Action::SetInterfaceFont(font));
            ui.close();
        }
    };
    let search = egui::Id::new("interface-font-search");
    let combo = egui::ComboBox::from_id_salt("interface-font")
        .width(200.0)
        .selected_text(RichText::new(name(current)).font(theme::regular(14.0)));
    if state.font_families.is_empty() {
        // Nothing installed is listed (not yet, or not on this system).
        combo.show_ui(ui, |ui| {
            for font in InterfaceFont::ALL {
                item(ui, font, actions);
            }
        });
        return;
    }
    let shown = combo
        // A click in the search field must not close the menu.
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .height(FONT_MENU_HEIGHT)
        .show_ui(ui, |ui| {
            let mut typed: String = ui.data(|d| d.get_temp(search)).unwrap_or_default();
            let field = ui.add(
                egui::TextEdit::singleline(&mut typed)
                    .hint_text(tr(locale, "Search fonts"))
                    .desired_width(f32::INFINITY),
            );
            if field.changed() {
                ui.data_mut(|d| d.insert_temp(search, typed.clone()));
            }
            let wanted = typed.trim().to_lowercase();
            let fixed: Vec<InterfaceFont> = InterfaceFont::ALL
                .into_iter()
                .filter(|font| name(font).to_lowercase().contains(&wanted))
                .collect();
            let families: Vec<&String> = state
                .font_families
                .iter()
                .filter(|family| wanted.is_empty() || family.to_lowercase().contains(&wanted))
                .collect();
            let row = ui.spacing().interact_size.y;
            // Only the rows in view are laid out, however many families
            // are installed.
            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .show_rows(ui, row, fixed.len() + families.len(), |ui, rows| {
                    for index in rows {
                        let font = match fixed.get(index) {
                            Some(font) => font.clone(),
                            None => InterfaceFont::Family(families[index - fixed.len()].clone()),
                        };
                        item(ui, font, actions);
                    }
                });
        });
    if shown.inner.is_none() {
        // The next time it opens, the menu starts unfiltered.
        ui.data_mut(|d| d.remove::<String>(search));
    }
}

/// The visual style picker across all twelve [`InterfaceStyle`] presets.
fn interface_style_picker(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let current = state.settings.interface_style;
    let label = |style: InterfaceStyle| {
        if style == InterfaceStyle::Classic {
            tr(locale, style.label()).into_owned()
        } else {
            style.label().to_owned()
        }
    };
    egui::ComboBox::from_id_salt("interface-style")
        .width(200.0)
        .height(FONT_MENU_HEIGHT)
        .selected_text(RichText::new(label(current)).font(theme::regular(14.0)))
        .show_ui(ui, |ui| {
            for style in InterfaceStyle::ALL {
                let selected = current == style;
                if ui
                    .selectable_label(
                        selected,
                        RichText::new(label(style)).font(theme::regular(14.0)),
                    )
                    .clicked()
                {
                    actions.push(Action::SetInterfaceStyle(style));
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

#[cfg(test)]
mod tests {
    use super::*;
    use egui::vec2;

    /// Draws the Appearance group and returns its text, with the centre of
    /// each, after the pointer events.
    fn draw_appearance(
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
            |ui| appearance(ui, state, actions),
        );
        output.textures_delta.clear();
        output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some((
                    text.galley.text().to_owned(),
                    text.pos + text.galley.size() / 2.0,
                )),
                _ => None,
            })
            .collect()
    }

    fn click_at(ctx: &egui::Context, state: &State, pos: egui::Pos2, actions: &mut Vec<Action>) {
        for pressed in [true, false] {
            draw_appearance(
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

    fn centre_of(labels: &[(String, egui::Pos2)], text: &str) -> egui::Pos2 {
        labels
            .iter()
            .find(|(label, _)| label == text)
            .unwrap_or_else(|| panic!("{text} is not drawn"))
            .1
    }

    const NOTICE: &str = "Your system's interface font couldn't be loaded, so YouTFast uses Inter.";

    #[test]
    fn the_font_notice_shows_only_when_the_systems_face_fell_back_to_inter() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        for (font, missing, shown) in [
            (InterfaceFont::System, false, false),
            (InterfaceFont::System, true, true),
            // Choosing Inter is not a failure.
            (InterfaceFont::Inter, true, false),
            (InterfaceFont::Inter, false, false),
        ] {
            let mut state = State::default();
            state.settings.interface_font = font.clone();
            state.interface_font_missing = missing;
            let labels = draw_appearance(&ctx, &state, Vec::new(), &mut Vec::new());
            let has = |text: &str| labels.iter().any(|(label, _)| label == text);
            assert_eq!(has(NOTICE), shown, "{font:?} {missing}");
            assert_eq!(has("Report this"), shown, "{font:?} {missing}");
            assert!(has("Interface font"));
        }
    }

    #[test]
    fn a_chosen_family_that_is_gone_is_named_without_a_report() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State::default();
        state.settings.interface_font = InterfaceFont::Family("Fira Sans".into());
        let gone = "Fira Sans couldn't be loaded, so YouTFast uses Inter.";
        for missing in [false, true] {
            state.interface_font_missing = missing;
            let labels = draw_appearance(&ctx, &state, Vec::new(), &mut Vec::new());
            let has = |text: &str| labels.iter().any(|(label, _)| label == text);
            // The menu names the family whether or not it is listed.
            assert!(has("Fira Sans"));
            assert_eq!(has(gone), missing);
            assert!(!has(NOTICE));
            assert!(!has("Report this"));
        }
    }

    #[test]
    fn the_interface_font_menu_lists_installed_families_and_narrows_them() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let state = State {
            font_families: vec![
                "Adwaita Sans".to_owned(),
                "Fira Sans".to_owned(),
                "Noto Serif".to_owned(),
            ],
            ..State::default()
        };
        let mut actions = Vec::new();
        let labels = draw_appearance(&ctx, &state, Vec::new(), &mut actions);
        assert!(!labels.iter().any(|(label, _)| label == "Fira Sans"));
        click_at(
            &ctx,
            &state,
            centre_of(&labels, "System (default)"),
            &mut actions,
        );
        let open = draw_appearance(&ctx, &state, Vec::new(), &mut actions);
        for listed in [
            "Inter",
            "Adwaita Sans",
            "Fira Sans",
            "Noto Serif",
            "Search fonts",
        ] {
            assert!(open.iter().any(|(label, _)| label == listed), "{listed}");
        }
        // A click in the search field leaves the menu open; typing narrows
        // the list to the names holding the text, whatever its case.
        click_at(&ctx, &state, centre_of(&open, "Search fonts"), &mut actions);
        let narrowed = draw_appearance(
            &ctx,
            &state,
            vec![egui::Event::Text("SANS".to_owned())],
            &mut actions,
        );
        let narrowed = if narrowed.iter().any(|(label, _)| label == "Noto Serif") {
            // The frame that takes the text still drew the old list.
            draw_appearance(&ctx, &state, Vec::new(), &mut actions)
        } else {
            narrowed
        };
        let has = |text: &str| narrowed.iter().any(|(label, _)| label == text);
        assert!(has("Adwaita Sans") && has("Fira Sans"));
        assert!(!has("Noto Serif") && !has("Inter"));
        assert!(actions.is_empty());
        click_at(
            &ctx,
            &state,
            centre_of(&narrowed, "Fira Sans"),
            &mut actions,
        );
        assert_eq!(
            actions,
            [Action::SetInterfaceFont(InterfaceFont::Family(
                "Fira Sans".into()
            ))]
        );
        // Chosen: the menu is closed again.
        let closed = draw_appearance(&ctx, &state, Vec::new(), &mut Vec::new());
        assert!(!closed.iter().any(|(label, _)| label == "Adwaita Sans"));
    }

    #[test]
    fn report_this_asks_to_open_the_issue_and_nothing_else() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let state = State {
            interface_font_missing: true,
            ..State::default()
        };
        let mut actions = Vec::new();
        let labels = draw_appearance(&ctx, &state, Vec::new(), &mut actions);
        assert!(actions.is_empty());
        click_at(
            &ctx,
            &state,
            centre_of(&labels, "Report this"),
            &mut actions,
        );
        assert_eq!(actions, [Action::ReportInterfaceFont]);
    }

    #[test]
    fn the_interface_font_menu_emits_the_chosen_face() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State::default();
        let mut actions = Vec::new();
        let labels = draw_appearance(&ctx, &state, Vec::new(), &mut actions);
        click_at(
            &ctx,
            &state,
            centre_of(&labels, "System (default)"),
            &mut actions,
        );
        let labels = draw_appearance(&ctx, &state, Vec::new(), &mut actions);
        click_at(&ctx, &state, centre_of(&labels, "Inter"), &mut actions);
        assert_eq!(actions, [Action::SetInterfaceFont(InterfaceFont::Inter)]);
        actions.clear();
        // And back, from Inter.
        state.settings.interface_font = InterfaceFont::Inter;
        let labels = draw_appearance(&ctx, &state, Vec::new(), &mut actions);
        click_at(&ctx, &state, centre_of(&labels, "Inter"), &mut actions);
        let labels = draw_appearance(&ctx, &state, Vec::new(), &mut actions);
        click_at(
            &ctx,
            &state,
            centre_of(&labels, "System (default)"),
            &mut actions,
        );
        assert_eq!(actions, [Action::SetInterfaceFont(InterfaceFont::System)]);
    }

    #[test]
    fn the_interface_style_menu_emits_the_chosen_style() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let state = State::default();
        let mut actions = Vec::new();
        let labels = draw_appearance(&ctx, &state, Vec::new(), &mut actions);
        click_at(
            &ctx,
            &state,
            centre_of(&labels, InterfaceStyle::default().label()),
            &mut actions,
        );
        let open = draw_appearance(&ctx, &state, Vec::new(), &mut actions);
        for style in InterfaceStyle::ALL {
            assert!(
                open.iter().any(|(label, _)| label == style.label()),
                "missing style {}",
                style.label()
            );
        }
        click_at(&ctx, &state, centre_of(&open, "Liquid Glass"), &mut actions);
        assert_eq!(
            actions,
            [Action::SetInterfaceStyle(InterfaceStyle::LiquidGlass)]
        );
        actions.clear();
        click_at(
            &ctx,
            &state,
            centre_of(&labels, InterfaceStyle::default().label()),
            &mut actions,
        );
        let open = draw_appearance(&ctx, &state, Vec::new(), &mut actions);
        click_at(
            &ctx,
            &state,
            centre_of(&open, "Industrial (Braun)"),
            &mut actions,
        );
        assert_eq!(
            actions,
            [Action::SetInterfaceStyle(InterfaceStyle::Industrial)]
        );
    }
}
