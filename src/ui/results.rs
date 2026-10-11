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
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            for (index, video) in videos.into_iter().enumerate() {
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
            }
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
