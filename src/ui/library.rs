//! The pages of what the user follows and keeps: the home feed,
//! subscriptions, history, Watch later, liked videos, playlists, and one
//! channel's or playlist's videos. Pages that exist both ways switch
//! between the signed-in account and this device.

use egui::{Align, CornerRadius, Image, Layout, RichText, Sense, Ui, vec2};

use super::{results, widgets};
use crate::app::{AccountState, Page, State};
use crate::model::{Action, Channel, Listing, PlaylistSummary, Source, View};
use crate::util;

/// A page's title row: the title, the account/device switch when signed in
/// (`both_ways`), Refresh, and any `extra` controls.
fn header(
    ui: &mut Ui,
    state: &State,
    title: &str,
    both_ways: bool,
    actions: &mut Vec<Action>,
    extra: impl FnOnce(&mut Ui, &mut Vec<Action>),
) {
    ui.horizontal(|ui| {
        ui.heading(title);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Refresh").clicked() {
                actions.push(Action::Refresh);
            }
            extra(ui, actions);
            if both_ways && state.signed_in() {
                ui.add_space(12.0);
                // Right to left: the device comes first so it shows second.
                let source = state.source();
                if ui
                    .selectable_label(source == Source::Device, "This device")
                    .clicked()
                {
                    actions.push(Action::SetSource(Source::Device));
                }
                if ui
                    .selectable_label(source == Source::Account, "Your account")
                    .clicked()
                {
                    actions.push(Action::SetSource(Source::Account));
                }
            }
        });
    });
    ui.add_space(12.0);
}

/// A loaded list, or the spinner or error standing in for it.
fn loaded<T>(ui: &mut Ui, listing: &Listing<T>, show: impl FnOnce(&mut Ui, &[T])) {
    match listing {
        Listing::Idle | Listing::Loading => results::centered(ui, |ui| {
            ui.spinner();
        }),
        Listing::Failed(error) => results::failed(ui, error),
        Listing::Ready(items) => show(ui, items.as_slice()),
    }
}

fn hint(ui: &mut Ui, text: &str) {
    results::centered(ui, |ui| {
        ui.label(RichText::new(text).weak());
    });
}

pub fn home(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ui, |ui| {
        header(ui, state, "Home", false, actions, |_, _| {});
        let follows_nothing = state.library.subscriptions.is_empty()
            && !state.signed_in()
            && state.account != AccountState::SigningIn;
        if follows_nothing {
            results::centered(ui, |ui| {
                ui.heading("Your feed is empty");
                ui.add_space(8.0);
                ui.label(
                    RichText::new(
                        "Subscribe to channels from a video's page, bring your subscriptions \
                         over from Google Takeout, or sign in with your YouTube account.",
                    )
                    .weak(),
                );
                ui.add_space(16.0);
                if ui.button("Import from Google Takeout").clicked() {
                    actions.push(Action::ImportTakeout);
                }
                if ui.button("Sign in").clicked() {
                    actions.push(Action::Navigate(View::Account));
                }
            });
            return;
        }
        results::videos(
            ui,
            &state.feed,
            "No videos from your subscriptions yet",
            actions,
        );
    });
}

pub fn subscriptions(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ui, |ui| {
        header(ui, state, "Subscriptions", true, actions, |ui, actions| {
            if ui
                .button("Import from Google Takeout")
                .on_hover_text("Adds the channels in a Takeout subscriptions.csv to this device")
                .clicked()
            {
                actions.push(Action::ImportTakeout);
            }
        });
        match state.source() {
            Source::Account => loaded(ui, &state.account_subscriptions, |ui, channels| {
                channel_list(ui, channels, false, actions);
            }),
            Source::Device if state.library.subscriptions.is_empty() => hint(
                ui,
                "No subscriptions on this device yet. Subscribe from a video's page, or \
                 import them from Google Takeout.",
            ),
            Source::Device => channel_list(ui, &state.library.subscriptions, true, actions),
        }
    });
}

fn channel_list(ui: &mut Ui, channels: &[Channel], removable: bool, actions: &mut Vec<Action>) {
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            for channel in channels {
                widgets::channel_row(ui, channel, actions, |ui, actions| {
                    if removable && ui.button("Unsubscribe").clicked() {
                        actions.push(Action::ToggleSubscription(channel.clone()));
                    }
                });
                ui.add_space(6.0);
            }
        });
}

pub fn history(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ui, |ui| {
        let device = state.source() == Source::Device;
        header(ui, state, "History", true, actions, |ui, actions| {
            if device
                && !state.library.history.is_empty()
                && ui
                    .button("Clear")
                    .on_hover_text("Clear this computer's history and resume positions")
                    .clicked()
            {
                actions.push(Action::ClearHistory);
            }
        });
        if !device {
            loaded(ui, &state.account_history, |ui, rows| {
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        let mut day = None;
                        for row in rows {
                            // YouTube groups its history by day: "Today",
                            // "Yesterday", "Monday".
                            if row.when.is_some() && row.when != day {
                                day.clone_from(&row.when);
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new(row.when.as_deref().unwrap_or_default()).strong(),
                                );
                                ui.add_space(4.0);
                            }
                            widgets::compact_card(ui, &row.video, None, actions);
                            ui.add_space(8.0);
                        }
                    });
            });
            return;
        }
        if state.library.history.is_empty() {
            hint(ui, "Videos you watch here appear in this list.");
            return;
        }
        let now = util::now();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for watched in &state.library.history {
                    let mut note = format!("Watched {}", util::ago(watched.at, now));
                    if watched.position > 0.0 {
                        note.push_str(&format!(" · left at {}", util::duration(watched.position)));
                    }
                    widgets::compact_card(ui, &watched.video, Some(&note), actions);
                    ui.add_space(8.0);
                }
            });
    });
}

pub fn watch_later(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ui, |ui| {
        header(ui, state, "Watch later", true, actions, |_, _| {});
        match state.source() {
            Source::Account => {
                results::videos(
                    ui,
                    &state.account_watch_later,
                    "Nothing saved for later",
                    actions,
                );
            }
            Source::Device if state.library.watch_later.is_empty() => {
                hint(ui, "Save videos from their page to watch them later.");
            }
            Source::Device => results::grid(ui, &state.library.watch_later, actions),
        }
    });
}

pub fn liked(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ui, |ui| {
        header(ui, state, "Liked videos", false, actions, |_, _| {});
        results::videos(ui, &state.liked, "No liked videos", actions);
    });
}

pub fn playlists(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ui, |ui| {
        header(ui, state, "Playlists", false, actions, |_, _| {});
        loaded(ui, &state.playlists, |ui, playlists| {
            if playlists.is_empty() {
                hint(ui, "No playlists in your library.");
                return;
            }
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    for playlist in playlists {
                        playlist_row(ui, playlist, actions);
                        ui.add_space(8.0);
                    }
                });
        });
    });
}

fn playlist_row(ui: &mut Ui, playlist: &PlaylistSummary, actions: &mut Vec<Action>) {
    let row = ui.horizontal_top(|ui| {
        let size = vec2(160.0, 90.0);
        match &playlist.thumbnail {
            Some(url) => {
                ui.add(
                    Image::new(url.clone())
                        .fit_to_exact_size(size)
                        .corner_radius(CornerRadius::same(8)),
                );
            }
            None => {
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(8), ui.visuals().faint_bg_color);
            }
        }
        ui.vertical(|ui| {
            widgets::title(ui, &playlist.name, 15.0);
            let details: Vec<String> = [
                playlist.videos.map(|count| format!("{count} videos")),
                playlist.channel.clone(),
            ]
            .into_iter()
            .flatten()
            .collect();
            ui.label(RichText::new(details.join(" · ")).weak().small());
        });
    });
    let response = ui.interact(
        row.response.rect,
        ui.id().with(("playlist", &playlist.id)),
        Sense::click(),
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if response.clicked() {
        actions.push(Action::OpenPlaylist {
            id: playlist.id.clone(),
            name: playlist.name.clone(),
        });
    }
}

/// One channel's latest videos, or one playlist's.
pub fn page(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ui, |ui| {
        match &state.page {
            Some(Page::Channel(channel)) => {
                header(ui, state, &channel.name, false, actions, |ui, actions| {
                    widgets::subscribe_button(ui, state, channel, actions);
                });
                ui.label(RichText::new("The channel's latest videos").weak().small());
                ui.add_space(8.0);
            }
            Some(Page::Playlist { name, .. }) => {
                header(ui, state, name, false, actions, |_, _| {});
            }
            None => return,
        }
        results::videos(ui, &state.page_videos, "No videos here", actions);
    });
}
