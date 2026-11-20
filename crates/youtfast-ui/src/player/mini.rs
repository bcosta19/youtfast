//! The floating corner miniplayer shown when navigating away from the watch page.

use egui::{Color32, CornerRadius, Rect, Sense, Stroke, UiBuilder, pos2, vec2};
use youtfast_i18n::{Locale, gettext as tr};

use super::surface::{on_video_style, picture_rounded, video_context_menu};
use super::{focused_control, keyboard_video_focus};
use youtfast_app::player::{next_video, previous_video};
use youtfast_app::{Action, State};
use youtfast_appearance::icons::Icon;
use youtfast_appearance::theme;
use youtfast_mpv::{Status, VideoSurface};

/// The mini player's widest, and the most of the window it takes.
const MINI_WIDTH: f32 = 360.0;
const MINI_SHARE: f32 = 0.4;

fn mini_control(response: egui::Response, id: egui::Id) -> bool {
    response.ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<Vec<egui::Id>>(id)
            .push(response.id);
    });
    response.clicked()
}

/// The corner player other pages show while a video is open: a rounded
/// video box with play, close and back-to-the-page over it on hover, and a
/// thin progress line along its bottom edge. A click on the picture goes
/// back to the watch page.
pub fn mini(
    ctx: &egui::Context,
    state: &State,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
    actions: &mut Vec<Action>,
) {
    let locale: Locale = state.settings.language;
    let (Some(_watching), Some(status)) = (&state.watching, status) else {
        return;
    };
    if status.idle {
        return;
    }
    let palette = &state.palette;
    let width = (ctx.content_rect().width() * MINI_SHARE).clamp(220.0, MINI_WIDTH);
    let height = width * 9.0 / 16.0;
    let position = ctx.content_rect().right_bottom() - vec2(width + 20.0, height + 20.0);
    let frame = egui::Frame::new()
        .fill(Color32::BLACK)
        .corner_radius(CornerRadius::same(theme::RADIUS + 1))
        .inner_margin(egui::Margin::same(3))
        .shadow(egui::Shadow {
            offset: [0, 6],
            blur: 20,
            spread: 0,
            color: Color32::from_black_alpha(140),
        });
    egui::Window::new(tr(locale, "Miniplayer"))
        .id(egui::Id::new("mini-player"))
        .title_bar(false)
        .frame(frame)
        .default_pos(position)
        .default_width(width)
        .min_width(200.0)
        .max_width((ctx.content_rect().width() - 32.0).max(200.0))
        .resizable([true, false])
        .collapsible(false)
        .show(ctx, |ui| {
            let width = ui.available_width().max(180.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            let height = width * 9.0 / 16.0;
            let radius = CornerRadius::same(theme::RADIUS - 1);
            let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
            ctx.data_mut(|data| {
                data.insert_temp(egui::Id::new("mini-video-keyboard-focus"), response.id)
            });
            picture_rounded(ui, rect, radius, state, Some(status), surface);
            video_context_menu(&response, state, status, actions);
            // The part played, along the picture's bottom edge.
            if status.duration > 0.0 && !state.watching_live() {
                let fraction = (status.position / status.duration).clamp(0.0, 1.0) as f32;
                let track = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 3.0), rect.max);
                let track_radius = CornerRadius {
                    nw: 0,
                    ne: 0,
                    sw: theme::RADIUS - 1,
                    se: theme::RADIUS - 1,
                };
                ui.painter()
                    .rect_filled(track, track_radius, Color32::from_white_alpha(45));
                let line = Rect::from_min_max(
                    pos2(rect.left(), rect.bottom() - 3.0),
                    pos2(rect.left() + rect.width() * fraction, rect.bottom()),
                );
                ui.painter().rect_filled(
                    line,
                    CornerRadius {
                        nw: 0,
                        ne: 0,
                        sw: theme::RADIUS - 1,
                        se: if fraction >= 0.99 {
                            theme::RADIUS - 1
                        } else {
                            0
                        },
                    },
                    palette.accent,
                );
            }
            let mut control_clicked = false;
            let mini_controls = egui::Id::new("mini-player-control-focus");
            let keyboard_focus = keyboard_video_focus(&response);
            let controls_visible = ui.rect_contains_pointer(rect)
                || status.paused
                || keyboard_focus
                || focused_control(ctx, mini_controls);
            let overlay_fade =
                ctx.animate_bool(egui::Id::new("mini-player-overlay-fade"), controls_visible);
            if overlay_fade > 0.0 {
                ui.painter().rect_filled(
                    rect,
                    radius,
                    Color32::from_black_alpha((120.0 * overlay_fade) as u8),
                );
            }
            if controls_visible {
                ctx.data_mut(|data| data.insert_temp(mini_controls, Vec::<egui::Id>::new()));
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
                let (icon, hint) = if status.paused {
                    (Icon::Play, tr(locale, "Play"))
                } else {
                    (Icon::Pause, tr(locale, "Pause"))
                };
                if over
                    .scope_builder(place(32.0, rect.center()), |ui| {
                        mini_control(
                            crate::widgets::icon_button(ui, icon, 32.0, &hint),
                            mini_controls,
                        )
                    })
                    .inner
                {
                    control_clicked = true;
                    actions.push(Action::TogglePause);
                }
                let previous = previous_video(state.watching.as_ref(), &state.up_next);
                let related = state
                    .details
                    .ready()
                    .map_or(&[][..], |details| details.related.as_slice());
                let next = next_video(
                    state.watching.as_ref(),
                    &state.queue,
                    &state.up_next,
                    related,
                );
                for (offset, available, icon, hint, action) in [
                    (
                        -64.0,
                        previous.is_some(),
                        Icon::ChevronLeft,
                        tr(locale, "Previous video"),
                        Action::PlayPrevious,
                    ),
                    (
                        64.0,
                        next.is_some() || state.playing_more.is_some(),
                        Icon::SkipForward,
                        tr(locale, "Next video"),
                        Action::PlayNext,
                    ),
                ] {
                    if available
                        && over
                            .scope_builder(place(22.0, rect.center() + vec2(offset, 0.0)), |ui| {
                                mini_control(
                                    crate::widgets::icon_button(ui, icon, 22.0, &hint),
                                    mini_controls,
                                )
                            })
                            .inner
                    {
                        control_clicked = true;
                        actions.push(action);
                    }
                }
                let corner = rect.right_top() + vec2(-20.0, 20.0);
                if over
                    .scope_builder(place(18.0, corner), |ui| {
                        mini_control(
                            crate::widgets::icon_button(
                                ui,
                                Icon::Close,
                                18.0,
                                &tr(locale, "Close and pause"),
                            ),
                            mini_controls,
                        )
                    })
                    .inner
                {
                    control_clicked = true;
                    actions.push(Action::CloseMiniPlayer);
                }
                let corner = rect.left_top() + vec2(20.0, 20.0);
                if over
                    .scope_builder(place(18.0, corner), |ui| {
                        mini_control(
                            crate::widgets::icon_button(
                                ui,
                                Icon::MiniPlayer,
                                18.0,
                                &tr(locale, "Back to the video"),
                            ),
                            mini_controls,
                        )
                    })
                    .inner
                {
                    control_clicked = true;
                    actions.push(Action::ReturnToWatch);
                }
            }
            ui.painter().rect_stroke(
                rect.expand(1.5),
                CornerRadius::same(theme::RADIUS),
                Stroke::new(3.5, Color32::BLACK),
                egui::StrokeKind::Middle,
            );
            ui.painter().rect_stroke(
                rect.expand(3.0),
                CornerRadius::same(theme::RADIUS + 1),
                Stroke::new(1.0, palette.outline),
                egui::StrokeKind::Inside,
            );
            if !control_clicked && response.clicked() {
                actions.push(Action::ReturnToWatch);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use youtfast_core::model::Video;

    #[test]
    fn keyboard_focus_reveals_and_keeps_miniplayer_controls() {
        let ctx = egui::Context::default();
        let state = State {
            view: youtfast_core::model::View::Home,
            watching: Some(Video::from_id("aaaaaaaaaaa".into())),
            ..State::default()
        };
        let status = Status {
            idle: false,
            paused: false,
            ..Status::default()
        };
        let mut actions = Vec::new();
        let mut draw = |events: Vec<egui::Event>| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 600.0))),
                    events,
                    ..Default::default()
                },
                |ui| mini(ui.ctx(), &state, Some(&status), None, &mut actions),
            );
            output.textures_delta.clear();
        };
        draw(vec![]);
        let tab = || egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        draw(vec![tab()]);
        draw(vec![]);
        let video = ctx
            .data(|data| data.get_temp::<egui::Id>(egui::Id::new("mini-video-keyboard-focus")))
            .unwrap();
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(video));
        let controls = ctx
            .data(|data| data.get_temp::<Vec<egui::Id>>(egui::Id::new("mini-player-control-focus")))
            .expect("miniplayer controls are visible");
        assert!(!controls.is_empty());
        draw(vec![egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        draw(vec![tab()]);
        draw(vec![]);
        assert!(focused_control(
            &ctx,
            egui::Id::new("mini-player-control-focus")
        ));
        draw(vec![]);
        assert!(focused_control(
            &ctx,
            egui::Id::new("mini-player-control-focus")
        ));
        draw(vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert_eq!(actions, vec![Action::TogglePause]);
    }

    #[test]
    fn mini_player_is_a_frameless_rounded_video_box() {
        fn has_text(shape: &egui::Shape) -> bool {
            match shape {
                egui::Shape::Text(_) => true,
                egui::Shape::Vec(shapes) => shapes.iter().any(has_text),
                _ => false,
            }
        }
        fn has_rounded_rect(shape: &egui::Shape) -> bool {
            match shape {
                egui::Shape::Rect(rect) => rect.corner_radius != CornerRadius::ZERO,
                egui::Shape::Vec(shapes) => shapes.iter().any(has_rounded_rect),
                _ => false,
            }
        }
        let mut video = Video::from_id("aaaaaaaaaaa".to_owned());
        video.title = "Hidden Title".to_owned();
        video.channel = Some("Hidden Channel".to_owned());
        let state = State {
            watching: Some(video),
            ..State::default()
        };
        let status = Status {
            duration: 120.0,
            position: 30.0,
            ..Status::default()
        };
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0));
        let mut mini_shapes = Vec::new();
        for _ in 0..2 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |ui| {
                    let mut actions = Vec::new();
                    mini(ui.ctx(), &state, Some(&status), None, &mut actions);
                },
            );
            mini_shapes = output.shapes.into_iter().map(|shape| shape.shape).collect();
            output.textures_delta.clear();
        }
        assert!(!mini_shapes.is_empty());
        assert!(mini_shapes.iter().all(|shape| !has_text(shape)));
        assert!(mini_shapes.iter().any(has_rounded_rect));
    }
}
