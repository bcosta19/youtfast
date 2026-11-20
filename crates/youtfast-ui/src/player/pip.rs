//! Pop-out picture-in-picture OS window, rendered via a deferred egui viewport.

use egui::viewport::ResizeDirection;
use egui::{Color32, CornerRadius, CursorIcon, Frame, Key, Rect, Sense, Stroke, UiBuilder, vec2};
use youtfast_i18n::gettext as tr;

use super::surface::{PictureContent, on_video_style, picture_content, progress_strip};
use super::{focused_control, keyboard_video_focus};
use youtfast_app::player::{PipBridge, pip_viewport_id, pip_window_size};
use youtfast_app::{Action, State};
use youtfast_appearance::icons::Icon;
use youtfast_mpv::VideoSurface;

const EDGE_RESIZE: f32 = 6.0;

fn pip_control(response: egui::Response, id: egui::Id) -> bool {
    response.ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<Vec<egui::Id>>(id)
            .push(response.id);
    });
    response.clicked()
}

/// Shows the separate, always-on-top, frameless OS picture-in-picture window.
pub fn pip(ctx: &egui::Context, state: &State, surface: Option<&VideoSurface>) {
    let locale = state.settings.language;
    let initial_size = if state.pip_size[0] > 0.0 && state.pip_size[1] > 0.0 {
        state.pip_size
    } else {
        pip_window_size(16.0 / 9.0)
    };
    let builder = egui::ViewportBuilder::default()
        .with_title(tr(locale, "Picture-in-picture").into_owned())
        .with_app_id("youtfast")
        .with_decorations(false)
        .with_always_on_top()
        .with_resizable(true)
        .with_taskbar(false)
        .with_inner_size(initial_size)
        .with_min_inner_size(vec2(240.0, 135.0))
        .with_max_inner_size(vec2(1280.0, 720.0));
    let bridge = state.pip_bridge.clone();
    let surface = surface.cloned();
    ctx.show_viewport_deferred(pip_viewport_id(), builder, move |ui, class| {
        if class == egui::ViewportClass::EmbeddedWindow {
            return;
        }
        let snapshot = bridge.snapshot();
        let frame = Frame::new()
            .fill(Color32::BLACK)
            .stroke(Stroke::new(1.0, snapshot.palette.outline));
        egui::CentralPanel::default().frame(frame).show(ui, |ui| {
            show_pip_contents(ui, &bridge, surface.as_ref());
        });
    });
}

/// Draws the contents of the pop-out picture-in-picture window.
pub(crate) fn show_pip_contents(
    ui: &mut egui::Ui,
    bridge: &PipBridge,
    surface: Option<&VideoSurface>,
) {
    let snapshot = bridge.snapshot();
    let locale = snapshot.language;
    let status = snapshot.status.as_ref();

    let ctx = ui.ctx().clone();
    let push_action = |action: Action| {
        bridge.push_action(action);
        ctx.request_repaint_of(egui::ViewportId::ROOT);
    };

    if ui.ctx().input(|i| i.viewport().close_requested()) {
        push_action(Action::ClosePip);
    }

    let rect = ui.max_rect();
    if !rect.is_positive() {
        return;
    }

    let aspect = status
        .map(|s| s.video_aspect)
        .filter(|aspect| aspect.is_finite() && *aspect > 0.25 && *aspect <= 4.0)
        .unwrap_or(16.0 / 9.0) as f32;
    let video_rect = fit_aspect(rect, aspect);

    let response = ui.interact(rect, ui.id().with("pip-surface"), Sense::click_and_drag());
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new("pip-video-keyboard-focus"), response.id));

    picture_content(
        ui,
        video_rect,
        CornerRadius::ZERO,
        PictureContent {
            locale,
            audio_only: snapshot.audio_only,
            thumbnail: snapshot.thumbnail.as_deref(),
            player_error: snapshot.player_error.as_deref(),
            demo_synthetic_images: snapshot.demo_synthetic_images,
        },
        status,
        surface,
    );

    if let Some(status) = status {
        progress_strip(
            ui,
            rect,
            0,
            status,
            snapshot.watching_live,
            snapshot.palette.accent,
        );
    }

    let pip_controls = egui::Id::new("pip-player-control-focus");
    let keyboard_focus = keyboard_video_focus(&response);
    let paused = status.is_some_and(|s| s.paused);
    let controls_visible = ui.rect_contains_pointer(rect)
        || paused
        || keyboard_focus
        || focused_control(ui.ctx(), pip_controls);

    if !ui.ctx().memory(|m| m.focused().is_some())
        || keyboard_focus
        || focused_control(ui.ctx(), pip_controls)
    {
        let toggle_pause = ui.ctx().input_mut(|input| {
            input.consume_key(egui::Modifiers::NONE, Key::Space) || input.key_pressed(Key::K)
        });
        if toggle_pause {
            push_action(Action::TogglePause);
        }
    }

    let mut control_clicked = false;
    let overlay_fade = ui
        .ctx()
        .animate_bool(egui::Id::new("pip-player-overlay-fade"), controls_visible);
    if overlay_fade > 0.0 {
        ui.painter().rect_filled(
            rect,
            CornerRadius::ZERO,
            Color32::from_black_alpha((120.0 * overlay_fade) as u8),
        );
    }

    if controls_visible {
        ui.ctx()
            .data_mut(|data| data.insert_temp(pip_controls, Vec::<egui::Id>::new()));
        let mut over = ui.new_child(UiBuilder::new().max_rect(rect));
        on_video_style(&mut over);
        let place = |size: f32, center: egui::Pos2| {
            UiBuilder::new().max_rect(Rect::from_center_size(
                center,
                vec2(size + 12.0, size + 12.0),
            ))
        };

        over.painter()
            .circle_filled(rect.center(), 24.0, Color32::from_black_alpha(145));
        let (icon, hint) = if paused {
            (Icon::Play, tr(locale, "Play"))
        } else {
            (Icon::Pause, tr(locale, "Pause"))
        };
        if over
            .scope_builder(place(32.0, rect.center()), |ui| {
                pip_control(
                    crate::widgets::icon_button(ui, icon, 32.0, &hint),
                    pip_controls,
                )
            })
            .inner
        {
            control_clicked = true;
            push_action(Action::TogglePause);
        }

        let top_left = rect.left_top() + vec2(20.0, 20.0);
        if over
            .scope_builder(place(18.0, top_left), |ui| {
                pip_control(
                    crate::widgets::icon_button(
                        ui,
                        Icon::MiniPlayer,
                        18.0,
                        &tr(locale, "Return to app"),
                    ),
                    pip_controls,
                )
            })
            .inner
        {
            control_clicked = true;
            push_action(Action::ReturnFromPip);
        }

        let top_right = rect.right_top() + vec2(-20.0, 20.0);
        if over
            .scope_builder(place(18.0, top_right), |ui| {
                pip_control(
                    crate::widgets::icon_button(
                        ui,
                        Icon::Close,
                        18.0,
                        &tr(locale, "Close and pause"),
                    ),
                    pip_controls,
                )
            })
            .inner
        {
            control_clicked = true;
            push_action(Action::ClosePip);
        }
    }

    if !control_clicked {
        if let Some(dir) = edge_resize_direction(ui, rect) {
            ui.ctx().set_cursor_icon(resize_cursor(dir));
            if ui
                .ctx()
                .input(|i| i.pointer.button_pressed(egui::PointerButton::Primary))
            {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
            }
        } else if response.drag_started_by(egui::PointerButton::Primary) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
    }
}

fn fit_aspect(bounds: Rect, aspect: f32) -> Rect {
    let available = bounds.size();
    if available.x <= 0.0 || available.y <= 0.0 {
        return bounds;
    }
    let size = if available.x / available.y > aspect {
        vec2(available.y * aspect, available.y)
    } else {
        vec2(available.x, available.x / aspect)
    };
    Rect::from_center_size(bounds.center(), size)
}

fn edge_resize_direction(ui: &egui::Ui, rect: Rect) -> Option<ResizeDirection> {
    let pos = ui.ctx().pointer_hover_pos()?;
    if !rect.contains(pos) {
        return None;
    }
    let left = pos.x - rect.left() <= EDGE_RESIZE;
    let right = rect.right() - pos.x <= EDGE_RESIZE;
    let top = pos.y - rect.top() <= EDGE_RESIZE;
    let bottom = rect.bottom() - pos.y <= EDGE_RESIZE;
    match (left, right, top, bottom) {
        (true, _, true, _) => Some(ResizeDirection::NorthWest),
        (_, true, true, _) => Some(ResizeDirection::NorthEast),
        (true, _, _, true) => Some(ResizeDirection::SouthWest),
        (_, true, _, true) => Some(ResizeDirection::SouthEast),
        (true, _, _, _) => Some(ResizeDirection::West),
        (_, true, _, _) => Some(ResizeDirection::East),
        (_, _, true, _) => Some(ResizeDirection::North),
        (_, _, _, true) => Some(ResizeDirection::South),
        _ => None,
    }
}

fn resize_cursor(direction: ResizeDirection) -> CursorIcon {
    match direction {
        ResizeDirection::North => CursorIcon::ResizeNorth,
        ResizeDirection::South => CursorIcon::ResizeSouth,
        ResizeDirection::East => CursorIcon::ResizeEast,
        ResizeDirection::West => CursorIcon::ResizeWest,
        ResizeDirection::NorthEast => CursorIcon::ResizeNorthEast,
        ResizeDirection::NorthWest => CursorIcon::ResizeNorthWest,
        ResizeDirection::SouthEast => CursorIcon::ResizeSouthEast,
        ResizeDirection::SouthWest => CursorIcon::ResizeSouthWest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use youtfast_app::State;
    use youtfast_core::model::Video;
    use youtfast_mpv::Status;

    fn click(pos: egui::Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }

    #[test]
    fn pip_window_controls_emit_play_pause_return_to_app_and_close() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let bridge = PipBridge::default();
        let state = State {
            watching: Some(Video::from_id("aaaaaaaaaaa".into())),
            ..State::default()
        };
        let status = Status {
            paused: true,
            duration: 120.0,
            position: 30.0,
            ..Status::default()
        };
        bridge.sync(&state, Some(&status), false);

        let screen = Rect::from_min_size(egui::pos2(0.0, 0.0), vec2(384.0, 216.0));
        let run = |events: Vec<egui::Event>| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |ui| show_pip_contents(ui, &bridge, None),
            );
            output.textures_delta.clear();
        };

        // Controls are visible because paused == true.
        run(vec![]);
        run(click(screen.center()));
        assert_eq!(bridge.take_actions(), vec![Action::TogglePause]);

        run(click(screen.left_top() + vec2(20.0, 20.0)));
        assert_eq!(bridge.take_actions(), vec![Action::ReturnFromPip]);

        run(click(screen.right_top() + vec2(-20.0, 20.0)));
        assert_eq!(bridge.take_actions(), vec![Action::ClosePip]);
    }

    #[test]
    fn pip_window_handles_os_close_and_space_shortcut() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let bridge = PipBridge::default();
        let state = State {
            watching: Some(Video::from_id("aaaaaaaaaaa".into())),
            ..State::default()
        };
        bridge.sync(&state, Some(&Status::default()), false);
        let screen = Rect::from_min_size(egui::pos2(0.0, 0.0), vec2(384.0, 216.0));

        let mut input = egui::RawInput {
            screen_rect: Some(screen),
            events: vec![egui::Event::Key {
                key: Key::Space,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        };
        input.viewports.insert(
            ctx.viewport_id(),
            egui::ViewportInfo {
                events: vec![egui::ViewportEvent::Close],
                ..Default::default()
            },
        );
        let mut output = ctx.run_ui(input, |ui| show_pip_contents(ui, &bridge, None));
        output.textures_delta.clear();
        assert_eq!(
            bridge.take_actions(),
            vec![Action::ClosePip, Action::TogglePause]
        );
    }

    #[test]
    fn pip_window_draws_in_every_state_without_panics() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let bridge = PipBridge::default();
        for size in [vec2(240.0, 135.0), vec2(384.0, 216.0), vec2(800.0, 600.0)] {
            for (paused, audio_only, error) in [
                (false, false, None),
                (true, false, None),
                (false, true, None),
                (true, false, Some("Playback failed".to_owned())),
            ] {
                let mut state = State {
                    watching: Some(Video::from_id("aaaaaaaaaaa".into())),
                    audio_only,
                    player_error: error,
                    ..State::default()
                };
                let status = Status {
                    paused,
                    duration: 100.0,
                    position: 25.0,
                    video_aspect: 16.0 / 9.0,
                    ..Status::default()
                };
                bridge.sync(&state, Some(&status), false);
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(egui::pos2(0.0, 0.0), size)),
                        ..Default::default()
                    },
                    |ui| show_pip_contents(ui, &bridge, None),
                );
                output.textures_delta.clear();
                state.watching = None;
            }
        }
    }
}
