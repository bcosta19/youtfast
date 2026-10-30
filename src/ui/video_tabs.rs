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
                    });
                });
        });
}
