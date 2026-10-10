//! Lists of videos as rows in a pane: search results, and every other
//! page that shows videos (the feed, Watch later, liked videos, a
//! playlist, a channel). Each pane carries its loading, empty and
//! failed states inside it.

use egui::{Align, Layout, RichText, Ui};

use super::widgets;
use crate::app::State;
use crate::model::{Action, Listing, Video};

pub fn show(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::CentralPanel::default()
        .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 4)))
        .show(ui, |ui| {
            let listing = &state.results;
            widgets::pane(
                ui,
                &state.palette,
                "Results",
                Some(listing.items().len()),
                true,
                None,
                |ui| match listing {
                    Listing::Idle => {}
                    Listing::Loading => loading(ui, state, "results"),
                    Listing::Failed(error) => failed(ui, state, error, actions),
                    Listing::Ready(videos) if videos.is_empty() => {
                        nothing(ui, state, "search for something")
                    }
                    Listing::Ready(videos) => rows(ui, state, videos, true, actions),
                },
            );
        });
}

/// A pane of video rows, or what stands in for it: the spinner, the
/// error with `[r] retry`, or `-- nothing here --` with `empty` under
/// it as the hint.
pub fn videos(
    ui: &mut Ui,
    state: &State,
    title: &str,
    listing: &Listing<Video>,
    empty: &str,
    actions: &mut Vec<Action>,
) {
    let count = listing.items().len();
    widgets::pane(
        ui,
        &state.palette,
        title,
        Some(count),
        true,
        None,
        |ui| match listing {
            Listing::Idle => {}
            Listing::Loading => loading(ui, state, title),
            Listing::Failed(error) => failed(ui, state, error, actions),
            Listing::Ready(videos) if videos.is_empty() => nothing(ui, state, empty),
            Listing::Ready(videos) => video_rows(ui, state, videos, actions),
        },
    );
}

/// The rows themselves, in a scroll area.
pub fn video_rows(ui: &mut Ui, state: &State, videos: &[Video], actions: &mut Vec<Action>) {
    rows(ui, state, videos, false, actions);
}

/// [`video_rows`]; `large` draws search's big thumbnails.
fn rows(ui: &mut Ui, state: &State, videos: &[Video], large: bool, actions: &mut Vec<Action>) {
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            for (index, video) in videos.iter().enumerate() {
                let selected = index == state.cursor;
                let watched = widgets::watched(&state.library, video);
                let response = if large {
                    widgets::video_row_large(ui, &state.palette, video, watched, selected)
                } else {
                    widgets::video_row(ui, &state.palette, video, None, watched, selected)
                };
                if selected {
                    response.clone().scroll_to_me(None);
                }
                if response.clicked() {
                    actions.push(Action::Open(video.clone()));
                }
            }
        });
}

/// The spinner and `loading <what>`, centred in the pane.
pub fn loading(ui: &mut Ui, state: &State, what: &str) {
    centered(ui, |ui| {
        ui.label(
            RichText::new(format!("{} loading {what}", widgets::spinner_frame(ui)))
                .monospace()
                .color(state.palette.secondary),
        );
    });
}

/// `-- nothing here --`, then what would fill the list.
pub fn nothing(ui: &mut Ui, state: &State, hint: &str) {
    centered(ui, |ui| {
        ui.label(
            RichText::new("-- nothing here --")
                .monospace()
                .color(state.palette.dim),
        );
        ui.label(
            RichText::new(hint)
                .monospace()
                .color(state.palette.secondary),
        );
    });
}

/// The error in red, and `[r] retry`.
pub fn failed(ui: &mut Ui, state: &State, error: &str, actions: &mut Vec<Action>) {
    centered(ui, |ui| {
        ui.label(RichText::new(error).monospace().color(state.palette.red));
        if widgets::bracket_button(ui, &state.palette, "r retry", widgets::ButtonStyle::Normal)
            .clicked()
        {
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
