//! Lists of videos as rows on a page: search results, and every other
//! page that shows videos (the feed, Watch later, liked videos, a
//! playlist, a channel). Each list carries its loading, empty and failed
//! states in its place. Also the page frame the list pages share: the
//! page padding and the bold page title.

use egui::{Align, Layout, RichText, Ui};

use super::widgets;
use crate::app::State;
use crate::model::{Action, Listing, Video};
use crate::theme;

/// A page's central panel with DESIGN.md's padding: 24 px at the sides,
/// 16 px at the top.
pub fn panel(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    let frame = egui::Frame::central_panel(ui.style()).inner_margin(egui::Margin {
        left: 24,
        right: 24,
        top: 16,
        bottom: 0,
    });
    egui::CentralPanel::default()
        .frame(frame)
        .show(ui, |ui| add_contents(ui));
}

/// A page's own title in bold 24, with a dim count after it.
pub fn title(ui: &mut Ui, state: &State, title: &str, count: Option<usize>) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(title)
                .font(theme::bold(24.0))
                .color(state.palette.text),
        );
        if let Some(count) = count {
            ui.label(
                RichText::new(crate::util::thousands(count as u64))
                    .font(theme::regular(14.0))
                    .color(state.palette.dim),
            );
        }
    });
    ui.add_space(12.0);
}

pub fn show(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    panel(ui, |ui| {
        let listing = &state.results;
        title(ui, state, "Search results", Some(listing.items().len()));
        match listing {
            Listing::Idle => {}
            Listing::Loading => loading(ui, state, "results"),
            Listing::Failed(error) => failed(ui, state, error, actions),
            Listing::Ready(videos) if videos.is_empty() => {
                nothing(ui, state, "Search for something")
            }
            Listing::Ready(videos) => rows(ui, state, videos, true, true, actions),
        }
    });
}

/// A titled list of video rows, or what stands in for it: the spinner,
/// the error with Retry, or the empty state with `empty` as its hint.
pub fn videos(
    ui: &mut Ui,
    state: &State,
    title: &str,
    listing: &Listing<Video>,
    empty: &str,
    actions: &mut Vec<Action>,
) {
    self::title(ui, state, title, Some(listing.items().len()));
    match listing {
        Listing::Idle => {}
        Listing::Loading => loading(ui, state, "videos"),
        Listing::Failed(error) => failed(ui, state, error, actions),
        Listing::Ready(videos) if videos.is_empty() => nothing(ui, state, empty),
        Listing::Ready(videos) => video_rows(ui, state, videos, actions),
    }
}

/// The rows themselves, in a scroll area.
pub fn video_rows<'a>(
    ui: &mut Ui,
    state: &State,
    videos: impl IntoIterator<Item = &'a Video>,
    actions: &mut Vec<Action>,
) {
    rows(ui, state, videos, false, false, actions);
}

/// [`video_rows`]; `large` draws search's big thumbnails, and `more` ends
/// the list with the continuation's row.
fn rows<'a>(
    ui: &mut Ui,
    state: &State,
    videos: impl IntoIterator<Item = &'a Video>,
    large: bool,
    more: bool,
    actions: &mut Vec<Action>,
) {
    let videos: Vec<&Video> = videos.into_iter().collect();
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            let height = if large {
                (ui.available_width() / 2.0).clamp(168.0, 360.0) * 9.0 / 16.0 + 12.0
            } else {
                106.0
            };
            virtual_rows(
                ui,
                height,
                videos.len(),
                state.cursor_moved.then_some(state.cursor),
                |ui, index| {
                    let video = videos[index];
                    let selected = index == state.cursor;
                    let watched = widgets::watched(&state.library, video);
                    let response = if large {
                        widgets::video_row_large(ui, &state.palette, video, watched, selected)
                    } else {
                        widgets::video_row(ui, &state.palette, video, None, watched, selected)
                    };
                    if selected && state.cursor_moved {
                        response.clone().scroll_to_me(None);
                    }
                    if response.clicked() {
                        actions.push(Action::Open(video.clone()));
                    }
                    video_menu(&response, state, video, actions);
                },
            );
            if more {
                widgets::more_row(
                    ui,
                    &state.palette,
                    &state.results_more_state,
                    state.results_more.is_some(),
                    actions,
                );
            }
        });
}

pub fn virtual_rows(
    ui: &mut Ui,
    height: f32,
    count: usize,
    selected: Option<usize>,
    mut row: impl FnMut(&mut Ui, usize),
) {
    if count == 0 {
        return;
    }
    let stride = height + ui.spacing().item_spacing.y;
    let top = ui.next_widget_position().y;
    let range = visible_rows(
        ui.clip_rect().top() - top,
        ui.clip_rect().bottom() - top,
        stride,
        count,
    );
    let total = egui::Rect::from_min_size(
        egui::pos2(ui.next_widget_position().x, top),
        egui::vec2(
            ui.available_width(),
            stride * count as f32 - ui.spacing().item_spacing.y,
        ),
    );
    if let Some(index) = selected.filter(|index| *index < count) {
        let rect = egui::Rect::from_min_size(
            total.min + egui::vec2(0.0, index as f32 * stride),
            egui::vec2(total.width(), height),
        );
        ui.scroll_to_rect(rect, None);
    }
    let rect = egui::Rect::from_min_max(
        total.min + egui::vec2(0.0, range.start as f32 * stride),
        egui::pos2(total.right(), top + range.end as f32 * stride),
    );
    let mut rows = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    rows.skip_ahead_auto_ids(range.start);
    for index in range {
        let rect = egui::Rect::from_min_size(
            total.min + egui::vec2(0.0, index as f32 * stride),
            egui::vec2(total.width(), height),
        );
        let mut item = rows.new_child(egui::UiBuilder::new().id_salt(index).max_rect(rect));
        row(&mut item, index);
    }
    ui.advance_cursor_after_rect(total);
}

fn visible_rows(top: f32, bottom: f32, stride: f32, count: usize) -> std::ops::Range<usize> {
    let first = ((top / stride).floor().max(0.0) as usize)
        .saturating_sub(1)
        .min(count);
    let last = (((bottom / stride).ceil() + 1.0).max(0.0) as usize).min(count);
    first..last.max(first)
}

/// A video's right-click menu: the queue, and this device's Watch later.
pub fn video_menu(
    response: &egui::Response,
    state: &State,
    video: &Video,
    actions: &mut Vec<Action>,
) {
    response.context_menu(|ui| {
        if ui.button("Play next").clicked() {
            actions.push(Action::Enqueue {
                video: video.clone(),
                next: true,
            });
            ui.close();
        }
        if ui.button("Add to queue").clicked() {
            actions.push(Action::Enqueue {
                video: video.clone(),
                next: false,
            });
            ui.close();
        }
        let later = state.library.in_watch_later(&video.id);
        let label = if later {
            "Remove from Watch later"
        } else {
            "Save to Watch later"
        };
        if ui.button(label).clicked() {
            actions.push(Action::ToggleWatchLater(video.clone()));
            ui.close();
        }
    });
}

/// The spinner and `Loading <what>…`, centred.
pub fn loading(ui: &mut Ui, state: &State, what: &str) {
    centered(ui, |ui| {
        ui.horizontal(|ui| {
            // Centre the spinner line: egui's horizontal layout starts left.
            let width = 200.0_f32.min(ui.available_width());
            ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
            widgets::loading(ui, &state.palette, what);
        });
    });
}

/// The empty state: a centred title, then what would fill the list.
pub fn nothing(ui: &mut Ui, state: &State, hint: &str) {
    empty(ui, state, "Nothing here", hint);
}

/// An empty state with its own title.
pub fn empty(ui: &mut Ui, state: &State, title: &str, hint: &str) {
    centered(ui, |ui| {
        ui.label(
            RichText::new(title)
                .font(theme::semibold(16.0))
                .color(state.palette.text),
        );
        ui.label(
            RichText::new(hint)
                .font(theme::regular(14.0))
                .color(state.palette.secondary),
        );
    });
}

/// The error in red, and Retry.
pub fn failed(ui: &mut Ui, state: &State, error: &str, actions: &mut Vec<Action>) {
    centered(ui, |ui| {
        ui.label(
            RichText::new(error)
                .font(theme::regular(14.0))
                .color(state.palette.red),
        );
        ui.add_space(8.0);
        if widgets::button(ui, &state.palette, "Retry", widgets::ButtonStyle::Normal).clicked() {
            actions.push(Action::Refresh);
        }
    });
}

pub fn centered(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.add_space(ui.available_height() * 0.25);
        add_contents(ui);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_with_nested_controls_keep_their_identity_when_scrolled() {
        let ctx = egui::Context::default();
        let mut ids = Vec::new();
        for offset in [0.0, 200.0] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(640.0, 480.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .vertical_scroll_offset(offset)
                            .show(ui, |ui| {
                                virtual_rows(ui, 32.0, 1000, None, |ui, index| {
                                    let (rect, response) = ui.allocate_exact_size(
                                        egui::vec2(ui.available_width(), 32.0),
                                        egui::Sense::click(),
                                    );
                                    let mut inner =
                                        ui.new_child(egui::UiBuilder::new().max_rect(rect));
                                    inner.label("nested control");
                                    if index == 7 {
                                        ids.push(response.id);
                                    }
                                });
                            });
                    });
                },
            );
            output.textures_delta.clear();
        }
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], ids[1]);
    }

    #[test]
    fn only_visible_rows_and_a_small_margin_are_laid_out() {
        assert_eq!(visible_rows(0.0, 400.0, 100.0, 1000), 0..5);
        assert_eq!(visible_rows(99000.0, 100000.0, 100.0, 1000), 989..1000);
        assert_eq!(visible_rows(-500.0, -100.0, 100.0, 1000), 0..0);
        assert_eq!(visible_rows(-500.0, -10.0, 100.0, 1000), 0..1);
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let state = State::default();
        let videos: Vec<_> = (0..1000)
            .map(|index| Video::from_id(format!("{index:011}")))
            .collect();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 480.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .show(ctx, |ui| video_rows(ui, &state, &videos, &mut Vec::new()));
            },
        );
        output.textures_delta.clear();
        let drawn = super::super::images::drawn_count(&ctx);
        assert!(drawn > 0 && drawn < 10, "drawn thumbnails: {drawn}");
    }

    #[test]
    fn keyboard_selection_reaches_an_offscreen_row_and_the_footer_stays_in_the_flow() {
        let ctx = egui::Context::default();
        let mut seen = false;
        let mut footer = false;
        for frame in 0..60 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    time: Some(frame as f64 / 10.0),
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(640.0, 480.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            virtual_rows(
                                ui,
                                106.0,
                                1000,
                                (frame == 1).then_some(999),
                                |ui, index| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(ui.available_width(), 106.0),
                                        egui::Sense::hover(),
                                    );
                                    if index == 999 && ui.is_rect_visible(rect) {
                                        seen = true;
                                    }
                                },
                            );
                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 32.0),
                                egui::Sense::hover(),
                            );
                            footer |= ui.is_rect_visible(rect);
                            if frame == 30 {
                                ui.scroll_to_rect(rect, Some(egui::Align::BOTTOM));
                            }
                        });
                    });
                },
            );
            output.textures_delta.clear();
        }
        assert!(seen, "the selected row never reached the viewport");
        assert!(footer, "the continuation footer never reached the viewport");
    }
}
