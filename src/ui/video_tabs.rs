//! Browser-style video tabs, available while browsing as well as watching.
use super::*;

pub(super) fn show(ui: &mut egui::Ui, state: &State, actions: &mut Vec<Action>) {
    if state.video_tabs.videos().next().is_none() {
        return;
    }
    egui::Panel::top("video-tabs")
        .exact_size(40.0)
        .show(ui, |ui| {
            egui::ScrollArea::horizontal()
                .id_salt("video-tabs-scroll")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for video in state.video_tabs.videos() {
                            ui.push_id(&video.id, |ui| {
                                let title = if video.title.is_empty() {
                                    &video.id
                                } else {
                                    &video.title
                                };
                                let selected = state.view == View::Watch
                                    && state.video_tabs.active() == Some(video.id.as_str());
                                let response = ui
                                    .add_sized(
                                        [180.0, 28.0],
                                        egui::Button::new(egui::RichText::new(title))
                                            .truncate()
                                            .selected(selected),
                                    )
                                    .on_hover_text(title);
                                if response.clicked() {
                                    actions.push(Action::SelectVideoTab(video.id.clone()));
                                }
                                let close = widgets::icon_button(
                                    ui,
                                    Icon::Close,
                                    14.0,
                                    tr(state.settings.language, "Close tab").as_ref(),
                                )
                                .clicked();
                                if response.middle_clicked() || close {
                                    actions.push(Action::CloseVideoTab(video.id.clone()));
                                }
                            });
                        }
                        new_tab_button(ui, state.settings.language, actions);
                    });
                });
        });
}

fn new_tab_button(
    ui: &mut egui::Ui,
    language: crate::appearance::i18n::Locale,
    actions: &mut Vec<Action>,
) -> egui::Response {
    let response = widgets::icon_button(ui, Icon::Plus, 14.0, tr(language, "New tab").as_ref());
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
                    rect = new_tab_button(
                        ui,
                        crate::appearance::i18n::Locale::default(),
                        &mut actions,
                    )
                    .rect;
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
}
