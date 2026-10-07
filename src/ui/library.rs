//! The pages of what the user follows and keeps: the home feed,
//! subscriptions, history, Watch later, liked videos, playlists, and one
//! channel's or playlist's videos. Pages that exist both ways switch
//! between the signed-in account and this device; each is one pane of
//! rows.

use egui::{Image, RichText, Sense, Ui, vec2};

use super::{results, widgets};
use crate::app::{AccountState, Page, State};
use crate::model::{Action, Channel, Listing, PlaylistSummary, Source, Video, View};

/// The account/device switch and a page's extra controls, as bracket
/// buttons on one line inside the pane.
fn controls(
    ui: &mut Ui,
    state: &State,
    both_ways: bool,
    actions: &mut Vec<Action>,
    extra: impl FnOnce(&mut Ui, &mut Vec<Action>),
) {
    let mut used = false;
    if both_ways && state.signed_in() {
        let source = state.source();
        let style = |on: bool| {
            if on {
                widgets::ButtonStyle::Primary
            } else {
                widgets::ButtonStyle::Normal
            }
        };
        if widgets::bracket_button(
            ui,
            &state.palette,
            "account",
            style(source == Source::Account),
        )
        .clicked()
        {
            actions.push(Action::SetSource(Source::Account));
        }
        if widgets::bracket_button(
            ui,
            &state.palette,
            "device",
            style(source == Source::Device),
        )
        .clicked()
        {
            actions.push(Action::SetSource(Source::Device));
        }
        used = true;
    }
    extra(ui, actions);
    let _ = used;
    ui.add_space(widgets::line(ui) / 2.0);
}

pub fn home(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ui, |ui| {
        let follows_nothing = state.library.subscriptions.is_empty()
            && !state.signed_in()
            && state.account != AccountState::SigningIn;
        if follows_nothing {
            widgets::pane(ui, &state.palette, "Home", None, true, None, |ui| {
                results::centered(ui, |ui| {
                    ui.label(
                        RichText::new("-- nothing here --")
                            .monospace()
                            .color(state.palette.dim),
                    );
                    ui.label(
                        RichText::new("subscribe to channels to fill your feed")
                            .monospace()
                            .color(state.palette.secondary),
                    );
                    ui.add_space(widgets::line(ui));
                    if widgets::bracket_button(
                        ui,
                        &state.palette,
                        "import from Google Takeout",
                        widgets::ButtonStyle::Normal,
                    )
                    .clicked()
                    {
                        actions.push(Action::ImportTakeout);
                    }
                    if widgets::bracket_button(
                        ui,
                        &state.palette,
                        "sign in",
                        widgets::ButtonStyle::Normal,
                    )
                    .clicked()
                    {
                        actions.push(Action::Navigate(View::Account));
                    }
                });
            });
            return;
        }
        results::videos(
            ui,
            state,
            "Home",
            &state.feed,
            "nothing from your subscriptions yet",
            actions,
        );
    });
}

pub fn subscriptions(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ui, |ui| {
        let count = match state.source() {
            Source::Account => state.account_subscriptions.items().len(),
            Source::Device => state.library.subscriptions.len(),
        };
        widgets::pane(
            ui,
            &state.palette,
            "Subscriptions",
            Some(count),
            true,
            None,
            |ui| {
                ui.horizontal(|ui| {
                    controls(ui, state, true, actions, |ui, actions| {
                        if widgets::bracket_button(
                            ui,
                            &state.palette,
                            "import takeout",
                            widgets::ButtonStyle::Normal,
                        )
                        .on_hover_text(
                            "Adds the channels in a Takeout subscriptions.csv to this device",
                        )
                        .clicked()
                        {
                            actions.push(Action::ImportTakeout);
                        }
                    });
                });
                match state.source() {
                    Source::Account => channel_listing(
                        ui,
                        state,
                        &state.account_subscriptions,
                        "the account's subscriptions",
                        false,
                        actions,
                    ),
                    Source::Device if state.library.subscriptions.is_empty() => results::nothing(
                        ui,
                        state,
                        "subscribe from a video's page, or import a Takeout",
                    ),
                    Source::Device => {
                        channel_rows(ui, state, &state.library.subscriptions, true, actions)
                    }
                }
            },
        );
    });
}

/// Channels as rows, or the listing's spinner/error in their place.
fn channel_listing(
    ui: &mut Ui,
    state: &State,
    listing: &Listing<Channel>,
    empty: &str,
    removable: bool,
    actions: &mut Vec<Action>,
) {
    match listing {
        Listing::Idle => {}
        Listing::Loading => results::loading(ui, state, "subscriptions"),
        Listing::Failed(error) => results::failed(ui, state, error, actions),
        Listing::Ready(channels) if channels.is_empty() => results::nothing(ui, state, empty),
        Listing::Ready(channels) => channel_rows(ui, state, channels, removable, actions),
    }
}

fn channel_rows(
    ui: &mut Ui,
    state: &State,
    channels: &[Channel],
    removable: bool,
    actions: &mut Vec<Action>,
) {
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            for (index, channel) in channels.iter().enumerate() {
                channel_row(
                    ui,
                    state,
                    channel,
                    index == state.cursor,
                    removable,
                    actions,
                );
            }
        });
}

/// A channel as a row: its square avatar, its name in blue, controls at
/// the end.
fn channel_row(
    ui: &mut Ui,
    state: &State,
    channel: &Channel,
    selected: bool,
    removable: bool,
    actions: &mut Vec<Action>,
) {
    let palette = &state.palette;
    let size = vec2(40.0, 40.0);
    let height = size.y + 4.0;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let fill = if selected {
        palette.accent
    } else if response.hovered() {
        palette.surface_hover
    } else {
        egui::Color32::TRANSPARENT
    };
    if fill != egui::Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::ZERO, fill);
    }
    if selected {
        response.clone().scroll_to_me(None);
    }
    let avatar_rect = egui::Rect::from_min_size(rect.min + vec2(widgets::cell(ui), 2.0), size);
    match &channel.avatar {
        Some(avatar) => {
            Image::new(avatar.clone())
                .fit_to_exact_size(size)
                .corner_radius(egui::CornerRadius::ZERO)
                .paint_at(ui, avatar_rect);
        }
        None => {
            ui.painter()
                .rect_filled(avatar_rect, egui::CornerRadius::ZERO, palette.surface);
            let initial: String = channel.name.chars().take(1).collect();
            ui.painter().text(
                avatar_rect.center(),
                egui::Align2::CENTER_CENTER,
                initial.to_uppercase(),
                egui::FontId::monospace(18.0),
                palette.dim,
            );
        }
    }
    let name_color = if selected {
        palette.window
    } else {
        palette.blue
    };
    ui.painter().text(
        pos_at(rect, avatar_rect.right() + widgets::cell(ui), 0.0),
        egui::Align2::LEFT_CENTER,
        &channel.name,
        egui::FontId::monospace(14.0),
        name_color,
    );
    if removable {
        let button = egui::Rect::from_center_size(
            egui::pos2(rect.right() - 96.0, rect.center().y),
            vec2(92.0, 22.0),
        );
        let mut inner = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(button)
                .layout(egui::Layout::default()),
        );
        if widgets::bracket_button(
            &mut inner,
            palette,
            "unsubscribe",
            widgets::ButtonStyle::Normal,
        )
        .clicked()
        {
            actions.push(Action::ToggleSubscription(channel.clone()));
        }
    }
    if response.clicked() {
        actions.push(Action::OpenChannel(channel.clone()));
    }
}

fn pos_at(rect: egui::Rect, x: f32, _y: f32) -> egui::Pos2 {
    egui::pos2(x, rect.center().y)
}

pub fn history(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ui, |ui| {
        let device = state.source() == Source::Device;
        let count = if device {
            state.library.history.len()
        } else {
            state.account_history.items().len()
        };
        widgets::pane(
            ui,
            &state.palette,
            "History",
            Some(count),
            true,
            None,
            |ui| {
                ui.horizontal(|ui| {
                    controls(ui, state, true, actions, |ui, actions| {
                        if device
                            && !state.library.history.is_empty()
                            && widgets::bracket_button(
                                ui,
                                &state.palette,
                                "clear",
                                widgets::ButtonStyle::Normal,
                            )
                            .on_hover_text("Clear this computer's history and resume positions")
                            .clicked()
                        {
                            actions.push(Action::ClearHistory);
                        }
                    });
                });
                if !device {
                    match &state.account_history {
                        Listing::Idle => {}
                        Listing::Loading => results::loading(ui, state, "history"),
                        Listing::Failed(error) => results::failed(ui, state, error, actions),
                        Listing::Ready(rows) if rows.is_empty() => {
                            results::nothing(ui, state, "the account's watch history")
                        }
                        Listing::Ready(rows) => {
                            egui::ScrollArea::vertical()
                                .auto_shrink(false)
                                .show(ui, |ui| {
                                    let mut day = None;
                                    for (index, row) in rows.iter().enumerate() {
                                        // YouTube groups its history by day:
                                        // "Today", "Yesterday", "Monday".
                                        if row.when.is_some() && row.when != day {
                                            day.clone_from(&row.when);
                                            ui.add_space(widgets::line(ui));
                                            ui.label(
                                                RichText::new(
                                                    row.when.as_deref().unwrap_or_default(),
                                                )
                                                .monospace()
                                                .color(state.palette.dim),
                                            );
                                        }
                                        let selected = index == state.cursor;
                                        let video = &row.video;
                                        let watched = video.duration.and_then(|duration| {
                                            state
                                                .library
                                                .resume_position(&video.id)
                                                .map(|left| left / duration)
                                        });
                                        let response = widgets::video_row(
                                            ui,
                                            &state.palette,
                                            video,
                                            None,
                                            watched.map(|f| f as f32),
                                            selected,
                                        );
                                        if selected {
                                            response.clone().scroll_to_me(None);
                                        }
                                        if response.clicked() {
                                            actions.push(Action::Open(video.clone()));
                                        }
                                    }
                                });
                        }
                    }
                    return;
                }
                if state.library.history.is_empty() {
                    results::nothing(ui, state, "videos you watch appear here");
                    return;
                }
                let now = util::now();
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        for (index, watched) in state.library.history.iter().enumerate() {
                            let mut note = format!("watched {}", util::ago(watched.at, now));
                            if watched.position > 0.0 {
                                note.push_str(&format!(
                                    " · left at {}",
                                    util::duration(watched.position)
                                ));
                            }
                            let video = &watched.video;
                            let fraction = video
                                .duration
                                .map(|duration| watched.position / duration)
                                .map(|f| f as f32);
                            let selected = index == state.cursor;
                            let response = widgets::video_row(
                                ui,
                                &state.palette,
                                video,
                                Some(&note),
                                fraction,
                                selected,
                            );
                            if selected {
                                response.clone().scroll_to_me(None);
                            }
                            if response.clicked() {
                                actions.push(Action::Open(video.clone()));
                            }
                        }
                    });
            },
        );
    });
}

pub fn watch_later(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ui, |ui| {
        let count = match state.source() {
            Source::Account => state.account_watch_later.items().len(),
            Source::Device => state.library.watch_later.len(),
        };
        widgets::pane(
            ui,
            &state.palette,
            "Watch later",
            Some(count),
            true,
            None,
            |ui| {
                ui.horizontal(|ui| {
                    controls(ui, state, true, actions, |_, _| {});
                });
                match state.source() {
                    Source::Account => rows_listing(
                        ui,
                        state,
                        &state.account_watch_later,
                        "the account's Watch later",
                        actions,
                    ),
                    Source::Device if state.library.watch_later.is_empty() => {
                        results::nothing(ui, state, "save videos from their page")
                    }
                    Source::Device => results::video_rows(
                        ui,
                        state,
                        &state.library.watch_later,
                        &|_| None,
                        actions,
                    ),
                }
            },
        );
    });
}

/// A `Listing<Video>` as rows inside the already-open pane.
fn rows_listing(
    ui: &mut Ui,
    state: &State,
    listing: &Listing<Video>,
    empty: &str,
    actions: &mut Vec<Action>,
) {
    match listing {
        Listing::Idle => {}
        Listing::Loading => results::loading(ui, state, "videos"),
        Listing::Failed(error) => results::failed(ui, state, error, actions),
        Listing::Ready(videos) if videos.is_empty() => results::nothing(ui, state, empty),
        Listing::Ready(videos) => results::video_rows(ui, state, videos, &|_| None, actions),
    }
}

pub fn liked(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ui, |ui| {
        results::videos(
            ui,
            state,
            "Liked",
            &state.liked,
            "the account's liked videos",
            actions,
        );
    });
}

pub fn playlists(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ui, |ui| {
        let count = state.playlists.items().len();
        widgets::pane(
            ui,
            &state.palette,
            "Playlists",
            Some(count),
            true,
            None,
            |ui| match &state.playlists {
                Listing::Idle => {}
                Listing::Loading => results::loading(ui, state, "playlists"),
                Listing::Failed(error) => results::failed(ui, state, error, actions),
                Listing::Ready(playlists) if playlists.is_empty() => {
                    results::nothing(ui, state, "the account's playlists")
                }
                Listing::Ready(playlists) => {
                    egui::ScrollArea::vertical()
                        .auto_shrink(false)
                        .show(ui, |ui| {
                            for (index, playlist) in playlists.iter().enumerate() {
                                playlist_row(ui, state, playlist, index == state.cursor, actions);
                            }
                        });
                }
            },
        );
    });
}

fn playlist_row(
    ui: &mut Ui,
    state: &State,
    playlist: &PlaylistSummary,
    selected: bool,
    actions: &mut Vec<Action>,
) {
    let palette = &state.palette;
    let pad = widgets::cell(ui);
    let size = vec2(120.0, 68.0);
    let height = size.y + pad;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let fill = if selected {
        palette.accent
    } else if response.hovered() {
        palette.surface_hover
    } else {
        egui::Color32::TRANSPARENT
    };
    if fill != egui::Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::ZERO, fill);
    }
    if selected {
        response.clone().scroll_to_me(None);
    }
    let thumb_rect = egui::Rect::from_min_size(rect.min + vec2(pad, pad / 2.0), size);
    match &playlist.thumbnail {
        Some(url) => {
            Image::new(url.clone())
                .fit_to_exact_size(size)
                .corner_radius(egui::CornerRadius::ZERO)
                .paint_at(ui, thumb_rect);
        }
        None => {
            ui.painter()
                .rect_filled(thumb_rect, egui::CornerRadius::ZERO, palette.surface);
        }
    }
    let name_color = if selected {
        palette.window
    } else {
        palette.text
    };
    let muted = if selected {
        palette.window
    } else {
        palette.secondary
    };
    let text_x = thumb_rect.right() + pad;
    let painter = ui.painter();
    let name = painter.layout_no_wrap(
        playlist.name.clone(),
        egui::FontId::monospace(14.0),
        name_color,
    );
    painter.galley(egui::pos2(text_x, thumb_rect.top()), name, name_color);
    let details: Vec<String> = [
        playlist.videos.map(|count| format!("{count} videos")),
        playlist.channel.clone(),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !details.is_empty() {
        let galley =
            painter.layout_no_wrap(details.join(" · "), egui::FontId::monospace(13.0), muted);
        painter.galley(
            egui::pos2(text_x, thumb_rect.top() + widgets::line(ui)),
            galley,
            muted,
        );
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
        let Some(Page::Playlist { name, .. }) = &state.page else {
            return;
        };
        widgets::pane(
            ui,
            &state.palette,
            name,
            Some(state.page_videos.items().len()),
            true,
            None,
            |ui| rows_listing(ui, state, &state.page_videos, "no videos here", actions),
        );
    });
}

use crate::util;
