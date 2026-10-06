//! Lists of videos as a grid of cards: search results, and every other page
//! that shows videos (the feed, Watch later, liked videos, a playlist, a
//! channel).

use egui::{Align, Layout, RichText, Ui, vec2};

use super::widgets;
use crate::app::State;
use crate::model::{Action, Listing, Video};

/// The narrowest a card gets before the grid drops a column.
const CARD_MIN_WIDTH: f32 = 260.0;
const GAP: f32 = 16.0;

pub fn show(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ui, |ui| {
        videos(ui, &state.results, "No results", actions);
    });
}

/// A loaded list of videos as a grid, or what stands in for it: a spinner,
/// the error, or `empty` when there are none.
pub fn videos(ui: &mut Ui, listing: &Listing<Video>, empty: &str, actions: &mut Vec<Action>) {
    match listing {
        Listing::Idle => {}
        Listing::Loading => centered(ui, |ui| {
            ui.spinner();
        }),
        Listing::Failed(error) => failed(ui, error),
        Listing::Ready(videos) if videos.is_empty() => centered(ui, |ui| {
            ui.label(RichText::new(empty).weak());
        }),
        Listing::Ready(videos) => grid(ui, videos, actions),
    }
}

pub fn failed(ui: &mut Ui, error: &str) {
    centered(ui, |ui| {
        ui.heading("This could not be loaded");
        ui.label(RichText::new(error).weak());
    });
}

pub fn centered(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.add_space(ui.available_height() * 0.25);
        add_contents(ui);
    });
}

pub fn grid(ui: &mut Ui, videos: &[Video], actions: &mut Vec<Action>) {
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            let available = ui.available_width();
            let columns = (((available + GAP) / (CARD_MIN_WIDTH + GAP)).floor() as usize).max(1);
            let width = (available - GAP * (columns as f32 - 1.0)) / columns as f32;
            for row in videos.chunks(columns) {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = GAP;
                    for video in row {
                        card(ui, video, width, actions);
                    }
                });
                ui.add_space(GAP);
            }
        });
}

fn card(ui: &mut Ui, video: &Video, width: f32, actions: &mut Vec<Action>) {
    let thumbnail = vec2(width, width * 9.0 / 16.0);
    let card = ui.allocate_ui_with_layout(
        vec2(width, thumbnail.y + 84.0),
        Layout::top_down(Align::Min),
        |ui| {
            ui.set_width(width);
            widgets::thumbnail(ui, video, thumbnail);
            ui.add_space(4.0);
            widgets::title(ui, &video.title, 15.0);
            if let Some(channel) = &video.channel {
                ui.label(RichText::new(channel).weak());
            }
            ui.label(RichText::new(widgets::meta(video)).weak().small());
        },
    );
    widgets::open_on_click(ui, card.response.rect, video, actions);
}
