//! Sidebar settings: visibility switches, ordering arrows, and restoring defaults.

use egui::{Align, Layout, RichText, Ui, vec2};
use youtfast_i18n::{Locale, format as trf, gettext as tr};

use super::switch;
use crate::widgets;
use youtfast_app::{Action, State};
use youtfast_appearance::icons::Icon;
use youtfast_appearance::theme;
use youtfast_storage::settings::{SidebarButton, default_sidebar};

/// The Sidebar group: each button with a switch to show it and arrows to
/// move it. Account and Settings stay shown. Number keys follow this order.
pub(super) fn sidebar(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;
    widgets::section(ui, palette, &tr(locale, "Sidebar"), None, |ui| {
        widgets::card(ui, palette, |ui| {
            let card = widgets::on_card(palette);
            ui.add(
                egui::Label::new(
                    RichText::new(tr(
                        locale,
                        "Choose which buttons the sidebar shows and in what order. The number keys 1 to 9 follow the buttons as shown. Account and Settings always stay.",
                    ))
                    .font(theme::regular(13.0))
                    .color(palette.secondary),
                )
                .wrap(),
            );
            ui.add_space(8.0);
            let entries = &state.settings.sidebar;
            let last = entries.len() - 1;
            // A row is as tall as its arrows.
            let step = 32.0 + ui.spacing().item_spacing.y;
            for (index, entry) in entries.iter().enumerate() {
                let name = sidebar_button_name(locale, entry.button);
                // A global id keyed by button (`push_id`'s widget ids still
                // follow the row's position), so focus and the switch's
                // animation travel with the row when it moves.
                let row = egui::UiBuilder::new().id(egui::Id::new(("sidebar-row", entry.button)));
                // A moved row slides from the place it had: the picture
                // only, the row itself is already where it goes.
                let place = ui.ctx().animate_value_with_time(
                    egui::Id::new(("sidebar-place", entry.button)),
                    index as f32,
                    ui.style().animation_time * 1.5,
                );
                let slide = vec2(0.0, (place - index as f32) * step);
                ui.with_visual_transform(egui::emath::TSTransform::from_translation(slide), |ui| {
                    ui.scope_builder(row, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            ui.add_enabled_ui(entry.button.can_hide(), |ui| {
                                if switch(ui, palette, entry.visible, &name).clicked() {
                                    let mut next = entries.clone();
                                    next[index].visible = !entry.visible;
                                    actions.push(Action::SetSidebar(next));
                                }
                            });
                            ui.label(
                                RichText::new(&name)
                                    .font(theme::medium(14.0))
                                    .color(palette.text),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                for (icon, up, key, enabled) in [
                                    (Icon::ChevronDown, false, "Move {name} down", index < last),
                                    (Icon::ChevronUp, true, "Move {name} up", index > 0),
                                ] {
                                    let hint = trf(locale, key, &[("name", name.clone())]);
                                    let moved = ui
                                        .add_enabled_ui(enabled, |ui| {
                                            widgets::icon_button(ui, icon, 18.0, &hint)
                                        })
                                        .inner;
                                    if moved.clicked() {
                                        let to = if up {
                                            index.checked_sub(1)
                                        } else {
                                            Some(index + 1).filter(|to| *to <= last)
                                        };
                                        if let Some(to) = to {
                                            let mut next = entries.clone();
                                            next.swap(index, to);
                                            actions.push(Action::SetSidebar(next));
                                        }
                                    }
                                }
                            });
                        })
                    })
                });
            }
            ui.add_space(8.0);
            ui.add_enabled_ui(*entries != default_sidebar(), |ui| {
                if widgets::button(
                    ui,
                    &card,
                    &tr(locale, "Restore default"),
                    widgets::ButtonStyle::Normal,
                )
                .clicked()
                {
                    actions.push(Action::SetSidebar(default_sidebar()));
                }
            });
        });
    });
}

/// A sidebar button's name. Liked and Playlists are listed even while
/// signed out: they only show when signed in.
fn sidebar_button_name(locale: Locale, button: SidebarButton) -> String {
    match button {
        SidebarButton::Home => tr(locale, "Home"),
        SidebarButton::Subscriptions => tr(locale, "Subscriptions"),
        SidebarButton::History => tr(locale, "History"),
        SidebarButton::WatchLater => tr(locale, "Watch later"),
        SidebarButton::Liked => tr(locale, "Liked videos"),
        SidebarButton::Playlists => tr(locale, "Playlists"),
        SidebarButton::Notifications => tr(locale, "Notifications"),
        SidebarButton::Account => tr(locale, "Account"),
        SidebarButton::Settings | SidebarButton::Unknown => tr(locale, "Settings"),
    }
    .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sidebar_group_lists_every_button_and_restores_the_default() {
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
                        vec2(900.0, 900.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| sidebar(ui, state, actions),
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
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let mut state = State::default();
        let mut actions = Vec::new();
        let labels = render(&ctx, &state, Vec::new(), &mut actions);
        for name in [
            "Home",
            "Subscriptions",
            "History",
            "Watch later",
            "Liked videos",
            "Playlists",
            "Notifications",
            "Account",
            "Settings",
            "Restore default",
        ] {
            assert!(labels.iter().any(|(text, _)| text == name), "{name}");
        }
        // Nothing to restore yet: the button is disabled and silent.
        let restore = labels
            .iter()
            .find(|(t, _)| t == "Restore default")
            .unwrap()
            .1;
        for pressed in [true, false] {
            render(
                &ctx,
                &state,
                vec![
                    egui::Event::PointerMoved(restore),
                    egui::Event::PointerButton {
                        pos: restore,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                &mut actions,
            );
        }
        assert!(actions.is_empty());
        // Once changed, it asks for the default list back.
        state.settings.sidebar.reverse();
        let labels = render(&ctx, &state, Vec::new(), &mut actions);
        let restore = labels
            .iter()
            .find(|(t, _)| t == "Restore default")
            .unwrap()
            .1;
        for pressed in [true, false] {
            render(
                &ctx,
                &state,
                vec![
                    egui::Event::PointerMoved(restore),
                    egui::Event::PointerButton {
                        pos: restore,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                &mut actions,
            );
        }
        assert_eq!(actions, vec![Action::SetSidebar(default_sidebar())]);
    }

    #[test]
    fn keyboard_focus_follows_a_moved_sidebar_row() {
        fn frame(
            ctx: &egui::Context,
            state: &State,
            events: Vec<egui::Event>,
            actions: &mut Vec<Action>,
        ) {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        vec2(900.0, 900.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| sidebar(ui, state, actions),
            );
            output.textures_delta.clear();
        }
        fn key(key: egui::Key) -> Vec<egui::Event> {
            vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]
        }
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let mut state = State::default();
        let mut actions = Vec::new();
        frame(&ctx, &state, Vec::new(), &mut actions);
        // Home's switch, then Home's "move down".
        for _ in 0..2 {
            frame(&ctx, &state, key(egui::Key::Tab), &mut actions);
        }
        let focused = ctx.memory(|m| m.focused()).expect("a focused widget");
        frame(&ctx, &state, key(egui::Key::Enter), &mut actions);
        frame(&ctx, &state, Vec::new(), &mut actions);
        let Some(Action::SetSidebar(next)) = actions.pop() else {
            panic!("moving down emits the new list");
        };
        assert_eq!(next[0].button, SidebarButton::Subscriptions);
        assert_eq!(next[1].button, SidebarButton::Home);
        frame(&ctx, &state, Vec::new(), &mut actions);
        state.settings.sidebar = next;
        frame(&ctx, &state, Vec::new(), &mut actions);
        frame(&ctx, &state, Vec::new(), &mut actions);
        // The ids are keyed by button: focus stays on Home's arrow, which
        // is now in the second row, instead of the first row's.
        assert_eq!(ctx.memory(|m| m.focused()), Some(focused));
    }
}
