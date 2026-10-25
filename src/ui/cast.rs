//! Native TV picker and remote. These controls explicitly target the TV;
//! libmpv's controls and media keys continue to target this computer.

use crate::app::State;
use crate::appearance::i18n::{gettext as tr, message};
use crate::desktop::cast::{Action as CastAction, Control};
use crate::model::Action;

pub fn show(ctx: &egui::Context, state: &mut State, actions: &mut Vec<Action>) {
    let mut open = state.cast.open;
    let locale = state.settings.language;
    let width = (ctx.content_rect().width() - 48.0).clamp(240.0, 420.0);
    egui::Window::new(tr(locale, "Cast to TV"))
        .id(egui::Id::new("cast-to-tv"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(width)
        .max_width(width)
        .default_pos(ctx.content_rect().center() - egui::vec2(width / 2.0, 150.0))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .max_height((ctx.content_rect().height() - 100.0).max(120.0))
                .show(ui, |ui| body(ui, state, actions));
        });
    state.cast.open = open;
}

fn body(ui: &mut egui::Ui, state: &mut State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let cast = &mut state.cast;
    ui.set_max_width((ui.ctx().content_rect().width() - 72.0).clamp(216.0, 396.0));
    ui.label(tr(locale, "Google Cast and YouTube TVs on your network."));
    ui.add_space(8.0);
    if let Some(error) = cast.error {
        ui.colored_label(state.palette.accent, message(locale, error.message()));
        if error == crate::desktop::cast::Error::MissingHelper {
            ui.hyperlink_to(
                tr(locale, "Install catt"),
                "https://github.com/skorokithakis/catt#installation",
            );
        }
        ui.add_space(8.0);
    }
    if cast.busy() {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(tr(
                locale,
                if cast.scanning() {
                    "Searching for TVs…"
                } else {
                    "Waiting for the TV…"
                },
            ));
        });
    }
    ui.add_enabled_ui(!cast.busy(), |ui| {
        if let Some(session) = &cast.session {
            ui.strong(&session.device.name);
            ui.label(&session.video.title);
            ui.label(tr(
                locale,
                if session.confirmed {
                    "Playback confirmed on TV."
                } else {
                    "Video requested; playback is not confirmed."
                },
            ));
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                for (label, control) in [
                    ("Play", Control::Play),
                    ("Pause", Control::Pause),
                    ("−10 s on TV", Control::Rewind),
                    ("+10 s on TV", Control::Forward),
                    ("TV volume −", Control::VolumeDown),
                    ("TV volume +", Control::VolumeUp),
                ] {
                    if ui.button(tr(locale, label)).clicked() {
                        actions.push(Action::Cast(CastAction::Control(control)));
                    }
                }
            });
            if cast.command_sent {
                ui.label(tr(locale, "Command sent to TV."));
            }
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                if ui.button(tr(locale, "Stop on TV")).clicked() {
                    actions.push(Action::Cast(CastAction::Control(Control::Stop)));
                }
                if ui.button(tr(locale, "Disconnect; keep TV playing")).clicked() {
                    actions.push(Action::Cast(CastAction::Disconnect));
                }
            });
            ui.add_space(8.0);
            ui.label(tr(locale, "Use these controls for the TV. Player controls and media keys control this computer."));
        } else {
            if let Some(video) = &state.watching {
                ui.label(tr(locale, "Send the current video:"));
                ui.strong(&video.title);
            } else {
                ui.label(tr(locale, "Open a video to send it to a TV."));
            }
            ui.add_space(8.0);
            ui.add_enabled_ui(state.watching.is_some(), |ui| {
                for device in &cast.devices {
                    // Long names wrap instead of expanding the floating window.
                    if ui
                        .add(egui::Button::new(&device.name).wrap())
                        .clicked()
                    {
                        actions.push(Action::Cast(CastAction::Start(device.clone())));
                    }
                }
            });
            if cast.searched && cast.devices.is_empty() && cast.error.is_none() {
                ui.label(tr(locale, "No TVs found. Open YouTube on your TV, check the network, or enter its IP address below."));
            }
            if ui.button(tr(locale, "Search for TVs")).clicked() {
                actions.push(Action::Cast(CastAction::Scan));
            }
            ui.add_space(8.0);
            ui.label(tr(locale, "Find a YouTube TV by IP address"));
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut cast.address).hint_text("192.168.1.10").desired_width(160.0));
                if ui.button(tr(locale, "Find TV")).clicked() {
                    actions.push(Action::Cast(CastAction::FindYouTube(cast.address.clone())));
                }
            });
            ui.add_space(8.0);
            ui.label(tr(locale, "The TV uses its YouTube app and account. Local playback pauses only after confirmation."));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::i18n::Locale;
    use crate::desktop::cast::{Device, Error, Reply, Request};
    use crate::model::Video;

    fn device() -> Device {
        Device {
            protocol: crate::desktop::cast::Protocol::GoogleCast,
            host: "192.168.1.20".parse().unwrap(),
            name: "Living room TV".into(),
        }
    }

    fn fixture() -> State {
        let mut state = State {
            watching: Some(Video::from_id("abcdefghijk".into())),
            ..Default::default()
        };
        state.cast.prepare(CastAction::Open, None, 0.0, 0);
        state.cast.finish(Reply::Devices(Ok(vec![device()])));
        state
    }

    fn render(
        ctx: &egui::Context,
        state: &mut State,
        size: egui::Vec2,
        events: Vec<egui::Event>,
        actions: &mut Vec<Action>,
    ) -> Vec<(String, egui::Pos2)> {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                events,
                ..Default::default()
            },
            |ui| show(ui.ctx(), state, actions),
        );
        let text = output
            .shapes
            .iter()
            .filter_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    Some((
                        text.galley.text().to_owned(),
                        text.pos + text.galley.size() / 2.0,
                    ))
                } else {
                    None
                }
            })
            .collect();
        output.textures_delta.clear();
        text
    }

    fn click(ctx: &egui::Context, state: &mut State, at: egui::Pos2, actions: &mut Vec<Action>) {
        for pressed in [true, false] {
            render(
                ctx,
                state,
                egui::vec2(900.0, 800.0),
                vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                actions,
            );
        }
    }

    #[test]
    fn selecting_a_rendered_tv_sends_the_current_video_and_disables_duplicate_commands() {
        let ctx = egui::Context::default();
        let mut state = fixture();
        let mut actions = Vec::new();
        render(
            &ctx,
            &mut state,
            egui::vec2(900.0, 800.0),
            vec![],
            &mut actions,
        );
        let labels = render(
            &ctx,
            &mut state,
            egui::vec2(900.0, 800.0),
            vec![],
            &mut actions,
        );
        let at = labels
            .iter()
            .find(|(text, _)| text == "Living room TV")
            .unwrap()
            .1;
        click(&ctx, &mut state, at, &mut actions);
        assert_eq!(actions, vec![Action::Cast(CastAction::Start(device()))]);
        let Action::Cast(action) = actions.remove(0) else {
            unreachable!()
        };
        assert!(matches!(
            state.cast.prepare(action, state.watching.as_ref(), 84.0, 7),
            Some(Request::Start { seconds: 84, .. })
        ));
        click(&ctx, &mut state, at, &mut actions);
        assert!(actions.is_empty());
    }

    #[test]
    fn rendered_remote_controls_target_the_tv_and_distinguish_stop_from_disconnect() {
        for (label, expected) in [
            ("Pause", CastAction::Control(Control::Pause)),
            ("+10 s on TV", CastAction::Control(Control::Forward)),
            ("TV volume +", CastAction::Control(Control::VolumeUp)),
            ("Stop on TV", CastAction::Control(Control::Stop)),
            ("Disconnect; keep TV playing", CastAction::Disconnect),
        ] {
            let ctx = egui::Context::default();
            let mut state = fixture();
            state
                .cast
                .prepare(CastAction::Start(device()), state.watching.as_ref(), 0.0, 1);
            state.cast.finish(Reply::Started {
                device: device(),
                video: "abcdefghijk".into(),
                result: Ok(()),
            });
            let mut actions = Vec::new();
            render(
                &ctx,
                &mut state,
                egui::vec2(900.0, 800.0),
                vec![],
                &mut actions,
            );
            let labels = render(
                &ctx,
                &mut state,
                egui::vec2(900.0, 800.0),
                vec![],
                &mut actions,
            );
            let at = labels
                .iter()
                .find(|(text, _)| text == label)
                .unwrap_or_else(|| panic!("missing {label}"))
                .1;
            click(&ctx, &mut state, at, &mut actions);
            assert_eq!(actions, vec![Action::Cast(expected)]);
        }
    }

    #[test]
    fn missing_helper_and_empty_discovery_render_recovery_in_both_themes_and_languages() {
        for locale in [Locale::English, Locale::PortugueseBrazil] {
            for light in [false, true] {
                for error in [None, Some(Error::MissingHelper)] {
                    let ctx = egui::Context::default();
                    let mut state = fixture();
                    state.settings.language = locale;
                    state.palette = if light {
                        crate::appearance::theme::Palette::light()
                    } else {
                        crate::appearance::theme::Palette::dark()
                    };
                    crate::appearance::theme::apply(&ctx, &state.palette);
                    state.cast.devices.clear();
                    state.cast.error = error;
                    let mut actions = Vec::new();
                    render(
                        &ctx,
                        &mut state,
                        egui::vec2(480.0, 650.0),
                        vec![],
                        &mut actions,
                    );
                    let labels = render(
                        &ctx,
                        &mut state,
                        egui::vec2(480.0, 650.0),
                        vec![],
                        &mut actions,
                    );
                    assert!(
                        labels
                            .iter()
                            .any(|(text, _)| text == tr(locale, "Search for TVs").as_ref())
                    );
                    let expected = if error.is_some() {
                        "Install catt"
                    } else {
                        "No TVs found. Open YouTube on your TV, check the network, or enter its IP address below."
                    };
                    assert!(
                        labels
                            .iter()
                            .any(|(text, _)| text == tr(locale, expected).as_ref())
                    );
                    assert!(actions.is_empty());
                }
            }
        }
    }

    #[test]
    fn stop_on_tv_is_clickable_without_scrolling_at_the_minimum_window_size() {
        let ctx = egui::Context::default();
        let mut state = fixture();
        state.settings.language = Locale::PortugueseBrazil;
        state.watching.as_mut().unwrap().title =
            "Travel around the world — a native experience for watching and discovering".into();
        state
            .cast
            .prepare(CastAction::Start(device()), state.watching.as_ref(), 0.0, 1);
        state.cast.finish(Reply::Started {
            device: device(),
            video: "abcdefghijk".into(),
            result: Ok(()),
        });
        let size = egui::vec2(480.0, 360.0);
        let mut actions = Vec::new();
        render(&ctx, &mut state, size, vec![], &mut actions);
        let labels = render(&ctx, &mut state, size, vec![], &mut actions);
        let at = labels
            .iter()
            .find(|(text, _)| text == "Parar na TV")
            .unwrap()
            .1;
        assert!(at.y < 340.0);
        for pressed in [true, false] {
            render(
                &ctx,
                &mut state,
                size,
                vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                &mut actions,
            );
        }
        assert_eq!(
            actions,
            vec![Action::Cast(CastAction::Control(Control::Stop))]
        );
    }
}
