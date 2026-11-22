//! Native TV picker and remote. These controls explicitly target the TV;
//! libmpv's controls and media keys continue to target this computer.

use crate::widgets;
use egui::RichText;
use youtfast_app::Action;
use youtfast_app::State;
use youtfast_appearance::theme;
use youtfast_cast::{Action as CastAction, Control};
use youtfast_i18n::{gettext as tr, message};

pub fn show(ctx: &egui::Context, state: &mut State, actions: &mut Vec<Action>) {
    let mut open = state.cast.open;
    let locale = state.settings.language;
    let width = (ctx.content_rect().width() - 48.0).clamp(240.0, 420.0);
    // egui pins a non-resizable window at its remembered height, a
    // ScrollArea absorbs overflow instead of growing it, and the app
    // restores that height from its saved state, so a default-sized
    // window silently clipped the TV-code section even after an update.
    // Floor the picker at the height its full content needs in every
    // language instead; short viewports clamp the window and scroll.
    const PICKER_HEIGHT: f32 = 620.0;
    let height = (ctx.content_rect().height() - 100.0).max(120.0);
    egui::Window::new(tr(locale, "Cast to TV"))
        .id(egui::Id::new("cast-to-tv"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(width)
        .max_width(width)
        .default_height(PICKER_HEIGHT)
        .min_height(PICKER_HEIGHT)
        .max_height(height)
        .default_pos(ctx.content_rect().center() - egui::vec2(width / 2.0, 150.0))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .max_height(height)
                .show(ui, |ui| body(ui, state, actions));
        });
    state.cast.open = open;
}

fn body(ui: &mut egui::Ui, state: &mut State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = state.palette;
    let raised = widgets::on_card(&palette);
    let cast = &mut state.cast;
    ui.set_max_width((ui.ctx().content_rect().width() - 72.0).clamp(216.0, 396.0));
    ui.label(
        RichText::new(tr(locale, "Google Cast and YouTube TVs on your network."))
            .font(theme::regular(13.0))
            .color(palette.secondary),
    );
    ui.add_space(10.0);
    if let Some(error) = cast.error {
        ui.colored_label(palette.accent, message(locale, error.message()));
        if error == youtfast_cast::Error::MissingHelper {
            ui.hyperlink_to(
                tr(locale, "Install catt"),
                "https://github.com/skorokithakis/catt#installation",
            );
        }
        ui.add_space(8.0);
    }
    // Guidance, not an error: the search itself still worked, and the
    // IP-address and TV-code routes below need no helper.
    if cast.helper_missing {
        ui.label(
            RichText::new(tr(
                locale,
                "Google Cast is optional: without the catt tool, only TVs with the YouTube app are found.",
            ))
            .font(theme::regular(12.5))
            .color(palette.secondary),
        );
        ui.hyperlink_to(
            tr(locale, "Install catt"),
            "https://github.com/skorokithakis/catt#installation",
        );
        ui.add_space(8.0);
    }
    if cast.busy() {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(
                RichText::new(tr(
                    locale,
                    if cast.scanning() {
                        "Searching for TVs…"
                    } else {
                        "Waiting for the TV…"
                    },
                ))
                .font(theme::medium(13.5))
                .color(palette.text),
            );
        });
        if cast.starting() {
            ui.label(
                RichText::new(tr(
                    locale,
                    "If the TV shows an ad or asks who is watching, answer on the TV. This can take up to a minute.",
                ))
                .font(theme::regular(12.5))
                .color(palette.secondary),
            );
        }
        ui.add_space(8.0);
    }
    ui.add_enabled_ui(!cast.busy(), |ui| {
        if let Some(session) = &cast.session {
            widgets::card(ui, &palette, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(&session.device.name)
                            .font(theme::semibold(14.5))
                            .color(palette.text),
                    );
                    widgets::status_pill(
                        ui,
                        tr(
                            locale,
                            if session.confirmed {
                                "Playback confirmed on TV."
                            } else {
                                "Video requested; playback is not confirmed."
                            },
                        )
                        .as_ref(),
                        if session.confirmed {
                            palette.green
                        } else {
                            palette.warning
                        },
                    );
                });
                ui.add(
                    egui::Label::new(
                        RichText::new(&session.video.title)
                            .font(theme::regular(12.5))
                            .color(palette.secondary),
                    )
                    .truncate(),
                )
                .on_hover_text(&session.video.title);
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(5.0, 4.0);
                    for (label, control) in [
                        ("Play", Control::Play),
                        ("Pause", Control::Pause),
                        ("−10 s on TV", Control::Rewind),
                        ("+10 s on TV", Control::Forward),
                        ("TV volume −", Control::VolumeDown),
                        ("TV volume +", Control::VolumeUp),
                    ] {
                        let btn = egui::Button::new(
                            RichText::new(tr(locale, label).as_ref())
                                .font(theme::medium(13.0))
                                .color(palette.text),
                        )
                        .fill(raised.surface)
                        .corner_radius(egui::CornerRadius::same(13))
                        .min_size(egui::vec2(0.0, 26.0));
                        if ui.add(btn).clicked() {
                            actions.push(Action::Cast(CastAction::Control(control)));
                        }
                    }
                });
                if cast.command_sent {
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(tr(locale, "Command sent to TV."))
                            .font(theme::regular(12.5))
                            .color(palette.green),
                    );
                }
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
                    let stop = egui::Button::new(
                        RichText::new(tr(locale, "Stop on TV").as_ref())
                            .font(theme::semibold(13.0))
                            .color(palette.window),
                    )
                    .fill(palette.text)
                    .corner_radius(egui::CornerRadius::same(14))
                    .min_size(egui::vec2(0.0, 28.0));
                    if ui.add(stop).clicked() {
                        actions.push(Action::Cast(CastAction::Control(Control::Stop)));
                    }
                    let disconnect = egui::Button::new(
                        RichText::new(tr(locale, "Disconnect; keep TV playing").as_ref())
                            .font(theme::medium(13.0))
                            .color(palette.text),
                    )
                    .fill(raised.surface)
                    .corner_radius(egui::CornerRadius::same(14))
                    .min_size(egui::vec2(0.0, 28.0));
                    if ui.add(disconnect).clicked() {
                        actions.push(Action::Cast(CastAction::Disconnect));
                    }
                });
            });
            ui.add_space(8.0);
            ui.label(
                RichText::new(tr(
                    locale,
                    "Use these controls for the TV. Player controls and media keys control this computer.",
                ))
                .font(theme::regular(12.5))
                .color(palette.secondary),
            );
        } else {
            widgets::card(ui, &palette, |ui| {
                if let Some(video) = &state.watching {
                    ui.label(
                        RichText::new(tr(locale, "Send the current video:"))
                            .font(theme::regular(12.5))
                            .color(palette.secondary),
                    );
                    ui.label(
                        RichText::new(&video.title)
                            .font(theme::semibold(14.0))
                            .color(palette.text),
                    );
                } else {
                    ui.label(
                        RichText::new(tr(locale, "Open a video to send it to a TV."))
                            .font(theme::regular(13.0))
                            .color(palette.secondary),
                    );
                }
                ui.add_space(10.0);
                ui.add_enabled_ui(state.watching.is_some(), |ui| {
                    for (idx, device) in cast.devices.iter().enumerate() {
                        if idx > 0 {
                            ui.add_space(6.0);
                        }
                        if widgets::button(
                            ui,
                            &raised,
                            &device.name,
                            widgets::ButtonStyle::Normal,
                        )
                        .clicked()
                        {
                            actions.push(Action::Cast(CastAction::Start(device.clone())));
                        }
                        if !matches!(device.protocol, youtfast_cast::Protocol::Linked { .. }) {
                            ui.label(
                                RichText::new(device.host.to_string())
                                    .font(theme::regular(12.0))
                                    .color(palette.secondary),
                            );
                        }
                    }
                });
                if cast.searched && cast.devices.is_empty() && cast.error.is_none() {
                    ui.label(
                        RichText::new(tr(
                            locale,
                            "No TVs found. Open YouTube on your TV, check the network, or enter its IP address below.",
                        ))
                        .font(theme::regular(13.0))
                        .color(palette.secondary),
                    );
                    ui.add_space(6.0);
                }
                ui.add_space(4.0);
                if widgets::button(
                    ui,
                    &raised,
                    tr(locale, "Search for TVs").as_ref(),
                    widgets::ButtonStyle::Primary,
                )
                .clicked()
                {
                    actions.push(Action::Cast(CastAction::Scan));
                }
            });
            ui.add_space(10.0);
            widgets::card(ui, &palette, |ui| {
                ui.label(
                    RichText::new(tr(locale, "Find a YouTube TV by IP address"))
                        .font(theme::medium(13.5))
                        .color(palette.text),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    widgets::text_input(
                        ui,
                        &palette,
                        &mut cast.address,
                        widgets::TextInputConfig {
                            id: egui::Id::new("cast-ip-input"),
                            width: 180.0,
                            hint: "192.168.1.10",
                            password: false,
                            monospace: true,
                            char_limit: None,
                        },
                    );
                    if widgets::button(
                        ui,
                        &raised,
                        tr(locale, "Find TV").as_ref(),
                        widgets::ButtonStyle::Normal,
                    )
                    .clicked()
                    {
                        actions.push(Action::Cast(CastAction::FindYouTube(cast.address.clone())));
                    }
                });
                widgets::hairline_divider(ui, &palette);
                // Works for any YouTube app (Apple TV, consoles, another network).
                ui.label(
                    RichText::new(tr(locale, "Link a TV with a code"))
                        .font(theme::medium(13.5))
                        .color(palette.text),
                );
                ui.label(
                    RichText::new(tr(
                        locale,
                        "On the TV, open YouTube, then Settings, then Link with TV code.",
                    ))
                    .font(theme::regular(12.5))
                    .color(palette.secondary),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    widgets::text_input(
                        ui,
                        &palette,
                        &mut cast.code,
                        widgets::TextInputConfig {
                            id: egui::Id::new("cast-code-input"),
                            width: 180.0,
                            hint: "123 456 789 012",
                            password: false,
                            monospace: true,
                            char_limit: None,
                        },
                    );
                    if widgets::button(
                        ui,
                        &raised,
                        tr(locale, "Link TV").as_ref(),
                        widgets::ButtonStyle::Normal,
                    )
                    .clicked()
                    {
                        actions.push(Action::Cast(CastAction::Link(std::mem::take(
                            &mut cast.code,
                        ))));
                    }
                });
            });
            ui.add_space(8.0);
            ui.label(
                RichText::new(tr(
                    locale,
                    "The TV uses its YouTube app and account. Local playback pauses only after confirmation.",
                ))
                .font(theme::regular(12.5))
                .color(palette.secondary),
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use youtfast_cast::{Device, Error, Reply, Request};
    use youtfast_core::model::Video;
    use youtfast_i18n::Locale;

    fn device() -> Device {
        Device {
            protocol: youtfast_cast::Protocol::GoogleCast,
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
        state.cast.finish(Reply::Scanned {
            devices: Ok(vec![device()]),
            helper_missing: false,
        });
        state
    }

    fn render(
        ctx: &egui::Context,
        state: &mut State,
        size: egui::Vec2,
        events: Vec<egui::Event>,
        actions: &mut Vec<Action>,
    ) -> Vec<(String, egui::Pos2)> {
        if !ctx.data(|d| {
            d.get_temp::<bool>(egui::Id::new("cast-test-fonts"))
                .unwrap_or(false)
        }) {
            youtfast_appearance::theme::install_fonts(ctx);
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("cast-test-fonts"), true));
        }
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
                for (error, helper_missing) in [
                    (None, false),
                    (None, true),
                    (Some(Error::MissingHelper), false),
                ] {
                    let ctx = egui::Context::default();
                    let mut state = fixture();
                    state.settings.language = locale;
                    state.palette = if light {
                        youtfast_appearance::theme::Palette::light()
                    } else {
                        youtfast_appearance::theme::Palette::dark()
                    };
                    youtfast_appearance::theme::apply(&ctx, &state.palette);
                    state.cast.devices.clear();
                    state.cast.error = error;
                    state.cast.helper_missing = helper_missing;
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
                    let guidance = "Google Cast is optional: without the catt tool, only TVs with the YouTube app are found.";
                    let empty = "No TVs found. Open YouTube on your TV, check the network, or enter its IP address below.";
                    let expected = if helper_missing {
                        vec![guidance, "Install catt"]
                    } else if error.is_some() {
                        vec!["Install catt"]
                    } else {
                        vec![empty]
                    };
                    for label in expected {
                        assert!(
                            labels
                                .iter()
                                .any(|(text, _)| text == tr(locale, label).as_ref()),
                            "missing {label} for error={error:?} helper_missing={helper_missing}"
                        );
                    }
                    assert!(actions.is_empty());
                }
            }
        }
    }

    #[test]
    fn link_tv_stays_clickable_without_scrolling_at_typical_sizes() {
        // The app's own faces decide the real line heights; without them
        // the test fonts are short enough to hide the clipped section.
        for locale in [
            Locale::English,
            Locale::PortugueseBrazil,
            Locale::Spanish,
            Locale::French,
            Locale::German,
            Locale::Italian,
        ] {
            let ctx = egui::Context::default();
            youtfast_appearance::theme::install_fonts(&ctx);
            let mut state = fixture();
            state.settings.language = locale;
            state.palette = youtfast_appearance::theme::Palette::dark();
            youtfast_appearance::theme::apply(&ctx, &state.palette);
            state.cast.devices.clear();
            state.cast.helper_missing = true;
            state.watching.as_mut().unwrap().title =
                "Travel around the world — a native experience for watching and discovering".into();
            let mut actions = Vec::new();
            render(
                &ctx,
                &mut state,
                egui::vec2(1280.0, 800.0),
                vec![],
                &mut actions,
            );
            let labels = render(
                &ctx,
                &mut state,
                egui::vec2(1280.0, 800.0),
                vec![],
                &mut actions,
            );
            // egui culls clipped widgets from the tessellation, so a
            // scrolled-out TV-code button has no painted label at all.
            let Some(at) = labels
                .iter()
                .find(|(text, _)| text == tr(locale, "Link TV").as_ref())
                .map(|(_, at)| *at)
            else {
                panic!("the TV-code section is clipped for {locale:?}");
            };
            // The dialog's own rect is the visible region: a label below it
            // is behind the scroll, which is what the reported dialog did.
            let rect = ctx.memory(|memory| memory.area_rect(egui::Id::new("cast-to-tv")));
            assert!(
                rect.is_some_and(|rect| at.y + 10.0 < rect.bottom() - 24.0 && at.y > rect.top()),
                "the TV-code section is clipped behind the scroll for {locale:?}"
            );
            click(&ctx, &mut state, at, &mut actions);
            assert!(
                actions
                    .iter()
                    .any(|action| matches!(action, Action::Cast(CastAction::Link(_)))),
                "the TV-code button does not respond for {locale:?}"
            );
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
