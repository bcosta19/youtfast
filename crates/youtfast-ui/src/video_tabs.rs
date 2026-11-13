//! Browser-style video tabs, available while browsing as well as watching.
use egui::{CornerRadius, CursorIcon, Frame, Margin, Rect, Sense, Stroke, pos2, vec2};

use super::*;

pub(super) fn show(ui: &mut egui::Ui, state: &State, actions: &mut Vec<Action>) {
    if state.video_tabs.videos().next().is_none() {
        return;
    }
    let palette = &state.palette;
    egui::Panel::top("video-tabs")
        .exact_size(40.0)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(palette.panel)
                .inner_margin(Margin::symmetric(16, 4)),
        )
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            egui::ScrollArea::horizontal()
                .id_salt("video-tabs-scroll")
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        for video in state.video_tabs.videos() {
                            ui.push_id(&video.id, |ui| {
                                let title = if video.title.is_empty() {
                                    &video.id
                                } else {
                                    &video.title
                                };
                                let selected = state.view == View::Watch
                                    && state.video_tabs.active() == Some(video.id.as_str());
                                video_tab(
                                    ui,
                                    palette,
                                    &video.id,
                                    title,
                                    selected,
                                    state.settings.language,
                                    actions,
                                );
                            });
                        }
                        new_tab_button(ui, state.settings.language, actions);
                    });
                });
        });
}

fn video_tab(
    ui: &mut egui::Ui,
    palette: &youtfast_appearance::theme::Palette,
    id: &str,
    title: &str,
    selected: bool,
    locale: youtfast_i18n::Locale,
    actions: &mut Vec<Action>,
) {
    let tab_size = vec2(204.0, 30.0);
    let (rect, response) = ui.allocate_exact_size(tab_size, Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, title)
    });
    let close_rect =
        Rect::from_center_size(pos2(rect.right() - 16.0, rect.center().y), vec2(22.0, 22.0));
    let close_hint = tr(locale, "Close tab");
    let close_response = ui
        .interact(close_rect, ui.id().with("close-tab"), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(close_hint.as_ref());
    close_response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, close_hint.as_ref())
    });
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let hovered = response.hovered() || close_response.hovered();
    let hover_t = ui
        .ctx()
        .animate_bool(response.id.with("tab-hover"), hovered);
    let selected_t = ui
        .ctx()
        .animate_bool(response.id.with("tab-selected"), selected);
    let radius = CornerRadius::same(RADIUS);
    let base_fill = palette.surface.gamma_multiply(0.55);
    let hover_fill = widgets::mix(base_fill, palette.surface_hover, hover_t);
    let fill = widgets::mix(hover_fill, palette.surface_active, selected_t * 0.85);
    let painter = ui.painter();
    painter.rect_filled(rect, radius, fill);
    if selected_t > 0.01 {
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, palette.outline.gamma_multiply(selected_t)),
            egui::StrokeKind::Inside,
        );
        let indicator = Rect::from_center_size(
            pos2(rect.left() + 8.0, rect.center().y),
            vec2(3.0, 14.0 * selected_t),
        );
        painter.rect_filled(
            indicator,
            CornerRadius::same(2),
            palette.accent.gamma_multiply(selected_t),
        );
    }
    widgets::focus_outline(ui, &response, radius);

    let text_left = rect.left() + if selected_t > 0.01 { 16.0 } else { 12.0 };
    let text_color = widgets::mix(palette.secondary, palette.text, hover_t.max(selected_t));
    let font = if selected {
        theme::semibold(13.0)
    } else {
        theme::medium(13.0)
    };
    let mut job = egui::text::LayoutJob::default();
    job.append(title, 0.0, egui::TextFormat::simple(font, text_color));
    job.wrap.max_width = (close_rect.left() - text_left - 6.0).max(10.0);
    job.wrap.max_rows = 1;
    job.wrap.overflow_character = Some('…');
    let galley = painter.layout_job(job);
    painter.galley(
        pos2(text_left, rect.center().y - galley.size().y / 2.0),
        galley,
        text_color,
    );

    let close_hover = widgets::hover_fade(ui, &close_response);
    if close_hover > 0.0 {
        painter.circle_filled(
            close_rect.center(),
            10.0,
            palette.surface_hover.gamma_multiply(close_hover),
        );
    }
    widgets::focus_outline(ui, &close_response, CornerRadius::same(10));
    let close_tint = widgets::mix(palette.secondary, palette.text, close_hover);
    Icon::Close.image(close_tint, 13.0).paint_at(
        ui,
        Rect::from_center_size(close_rect.center(), vec2(13.0, 13.0)),
    );

    let closed = response.middle_clicked() || close_response.clicked();
    if closed {
        actions.push(Action::CloseVideoTab(id.to_owned()));
    } else if response.clicked() && !close_response.hovered() {
        actions.push(Action::SelectVideoTab(id.to_owned()));
    }
    response.on_hover_text(title);
}

fn new_tab_button(
    ui: &mut egui::Ui,
    language: youtfast_i18n::Locale,
    actions: &mut Vec<Action>,
) -> egui::Response {
    let response = widgets::icon_button(ui, Icon::Plus, 15.0, tr(language, "New tab").as_ref());
    if response.clicked() {
        actions.push(Action::NewVideoTab);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicking_plus_emits_only_the_new_tab_action() {
        let ctx = egui::Context::default();
        let run = |events: Vec<egui::Event>| {
            let mut actions = Vec::new();
            let mut rect = egui::Rect::NOTHING;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(400.0, 100.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    rect = new_tab_button(ui, youtfast_i18n::Locale::default(), &mut actions).rect;
                },
            );
            output.textures_delta.clear();
            (rect, actions)
        };
        let (rect, _) = run(Vec::new());
        let pos = rect.center();
        let mut actions = Vec::new();
        for pressed in [true, false] {
            actions.extend(
                run(vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ])
                .1,
            );
        }
        assert_eq!(actions, vec![Action::NewVideoTab]);
    }

    #[test]
    fn clicking_tab_selects_and_clicking_close_closes_without_selecting() {
        use youtfast_core::model::Video;
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State::default();
        let first = Video {
            title: "First video".into(),
            ..Video::from_id("first000000".into())
        };
        let second = Video {
            title: "Second video".into(),
            ..Video::from_id("second00000".into())
        };
        state.video_tabs.open(first.clone());
        state.video_tabs.add(second);

        let run = |events: Vec<egui::Event>| {
            let mut actions = Vec::new();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(600.0, 200.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| show(ui, &state, &mut actions),
            );
            output.textures_delta.clear();
            (output.platform_output.accesskit_update, actions)
        };

        let (tree, _) = run(Vec::new());
        let tree = tree.expect("accessibility tree");
        let close_bounds = tree
            .nodes
            .iter()
            .filter(|(_, node)| node.label() == Some("Close tab"))
            .filter_map(|(_, node)| node.bounds())
            .min_by(|a, b| a.x0.total_cmp(&b.x0))
            .expect("close tab button must be exposed");
        let close_pos = egui::pos2(
            ((close_bounds.x0 + close_bounds.x1) / 2.0) as f32,
            ((close_bounds.y0 + close_bounds.y1) / 2.0) as f32,
        );
        let tab_pos = egui::pos2(close_bounds.x0 as f32 - 40.0, close_pos.y);

        let click = |pos: egui::Pos2| {
            let mut emitted = Vec::new();
            for pressed in [true, false] {
                emitted.extend(
                    run(vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ])
                    .1,
                );
            }
            emitted
        };

        assert_eq!(
            click(tab_pos),
            vec![Action::SelectVideoTab(first.id.clone())]
        );
        assert_eq!(click(close_pos), vec![Action::CloseVideoTab(first.id)]);
    }
}
