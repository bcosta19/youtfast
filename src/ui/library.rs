//! The pages of what the user follows and keeps: the home feed,
//! subscriptions, history, Watch later, liked videos, playlists, and one
//! playlist's videos. Pages that exist both ways switch between the
//! signed-in account and this device with chips under their title.

use egui::{Align2, CornerRadius, Rect, RichText, Sense, Ui, pos2, vec2};

use super::notifications::capitalized;
use super::{results, widgets};
use crate::app::{AccountState, Page, State};
use crate::model::{Action, Channel, FeedFilter, Fetch, Listing, Source, Video, View};
use crate::theme;
use crate::util;

/// The Account / This device chips (when signed in) and a page's extra
/// buttons, on one line under the page title.
fn controls(
    ui: &mut Ui,
    state: &State,
    actions: &mut Vec<Action>,
    extra: impl FnOnce(&mut Ui, &mut Vec<Action>),
) {
    ui.horizontal(|ui| {
        if state.signed_in() {
            let source = state.source();
            if widgets::chip(ui, &state.palette, "Account", source == Source::Account).clicked() {
                actions.push(Action::SetSource(Source::Account));
            }
            if widgets::chip(ui, &state.palette, "This device", source == Source::Device).clicked()
            {
                actions.push(Action::SetSource(Source::Device));
            }
            ui.add_space(8.0);
        }
        extra(ui, actions);
    });
    ui.add_space(12.0);
}

pub fn home(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    results::panel(ui, |ui| {
        // With a history, For you has something to offer already.
        let follows_nothing = state.library.subscriptions.is_empty()
            && state.library.history.is_empty()
            && !state.signed_in()
            && state.account != AccountState::SigningIn;
        if follows_nothing {
            results::centered(ui, |ui| {
                ui.label(
                    RichText::new("Your feed is empty")
                        .font(theme::semibold(16.0))
                        .color(state.palette.text),
                );
                ui.label(
                    RichText::new("Subscribe to channels to fill your feed")
                        .font(theme::regular(14.0))
                        .color(state.palette.secondary),
                );
                ui.add_space(12.0);
                if widgets::button(
                    ui,
                    &state.palette,
                    "Import from Google Takeout",
                    widgets::ButtonStyle::Normal,
                )
                .clicked()
                {
                    actions.push(Action::ImportTakeout);
                }
                ui.add_space(8.0);
                if widgets::button(ui, &state.palette, "Sign in", widgets::ButtonStyle::Primary)
                    .clicked()
                {
                    actions.push(Action::Navigate(View::Account));
                }
            });
            return;
        }
        feed_grid(ui, state, actions);
    });
}

/// The smallest a home card gets before the grid drops a column.
const CARD_WIDTH: f32 = 280.0;

/// Home's chips: All, Today while the feed has dates, Unwatched, and Live
/// while it has a live stream.
fn feed_chips(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let live = state.feed.items().iter().any(|video| video.live);
    let dated = state
        .feed
        .items()
        .iter()
        .any(|video| video.published_at.is_some());
    ui.horizontal_wrapped(|ui| {
        for filter in FeedFilter::ALL {
            let hidden = match filter {
                FeedFilter::ForYou => false,
                FeedFilter::Live => !live,
                FeedFilter::Today => !dated,
                _ => false,
            };
            if hidden && state.feed_filter != filter {
                continue;
            }
            if widgets::chip(
                ui,
                &state.palette,
                &capitalized(filter.label()),
                state.feed_filter == filter,
            )
            .clicked()
            {
                actions.push(Action::SetFeedFilter(filter));
            }
        }
    });
    ui.add_space(16.0);
}

/// The feed as YouTube's grid of cards, day-grouped, capped at
/// `feed_shown`, under the chips and the shorts shelf; the last card in
/// view asks for more.
fn feed_grid(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    if state.feed_filter == FeedFilter::ForYou {
        for_you(ui, state, actions);
        return;
    }
    if !matches!(state.feed, Listing::Ready(ref videos) if !videos.is_empty()) {
        shorts_shelf(ui, state, actions);
    }
    match &state.feed {
        Listing::Idle => {}
        Listing::Loading => results::loading(ui, state, "feed"),
        Listing::Failed(error) => results::failed(ui, state, error, actions),
        Listing::Ready(videos) if videos.is_empty() => {
            results::nothing(ui, state, "Nothing from your subscriptions yet")
        }
        Listing::Ready(_) => {
            feed_chips(ui, state, actions);
            let now = crate::util::now();
            let videos = state.visible_feed();
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    shorts_shelf(ui, state, actions);
                    if videos.is_empty() {
                        results::nothing(ui, state, "No video passes this chip");
                    }
                    let gap = 16.0;
                    let (columns, width) =
                        widgets::grid_columns(ui.available_width(), CARD_WIDTH, gap, 5);
                    // Runs of the same day, each a heading and its rows of
                    // cards.
                    let mut start = 0;
                    while start < videos.len() {
                        let day = crate::util::age_bucket(videos[start].published_at, now);
                        let end = videos[start..]
                            .iter()
                            .position(|video| {
                                crate::util::age_bucket(video.published_at, now) != day
                            })
                            .map_or(videos.len(), |offset| start + offset);
                        // The account's feed carries no dates: no heading.
                        let heading = if day == "undated" {
                            String::new()
                        } else {
                            capitalized(day)
                        };
                        widgets::section(ui, &state.palette, &heading, None, |ui| {
                            for row in (start..end).collect::<Vec<_>>().chunks(columns) {
                                ui.horizontal_top(|ui| {
                                    ui.spacing_mut().item_spacing.x = gap;
                                    for &index in row {
                                        feed_card(ui, state, &videos, index, width, actions);
                                    }
                                });
                                ui.add_space(gap);
                            }
                        });
                        ui.add_space(8.0);
                        start = end;
                    }
                    widgets::more_row(
                        ui,
                        &state.palette,
                        &state.feed_more_state,
                        state.feed_more.is_some() || state.feed_shown < state.feed.items().len(),
                        actions,
                    );
                });
        }
    }
}

/// For you as a grid of cards, under the chips and a line saying where it
/// comes from; the last card in view asks for the account's next page.
fn for_you(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    feed_chips(ui, state, actions);
    match &state.for_you {
        Listing::Idle => {}
        Listing::Loading => results::loading(ui, state, "recommendations"),
        Listing::Failed(error) => results::failed(ui, state, error, actions),
        Listing::Ready(videos) if videos.is_empty() => results::nothing(
            ui,
            state,
            "Watch a few videos and recommendations show up here",
        ),
        Listing::Ready(_) => {
            let source = if state.for_you_from_account {
                "Recommended by YouTube for your account"
            } else {
                "Picked here from the videos related to what you watched"
            };
            ui.label(
                RichText::new(source)
                    .font(theme::regular(13.0))
                    .color(state.palette.secondary),
            );
            ui.add_space(12.0);
            let videos = state.visible_feed();
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    let gap = 16.0;
                    let (columns, width) =
                        widgets::grid_columns(ui.available_width(), CARD_WIDTH, gap, 5);
                    for row in (0..videos.len()).collect::<Vec<_>>().chunks(columns) {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            for &index in row {
                                feed_card(ui, state, &videos, index, width, actions);
                            }
                        });
                        ui.add_space(gap);
                    }
                    if state.for_you_more.is_some()
                        || !matches!(state.for_you_more_state, Fetch::Idle)
                    {
                        widgets::more_row(
                            ui,
                            &state.palette,
                            &state.for_you_more_state,
                            state.for_you_more.is_some(),
                            actions,
                        );
                    }
                });
        }
    }
}

/// One card of the feed's grid.
fn feed_card(
    ui: &mut Ui,
    state: &State,
    videos: &[&Video],
    index: usize,
    width: f32,
    actions: &mut Vec<Action>,
) {
    let video = videos[index];
    let selected = index == state.cursor;
    let watched = widgets::watched(&state.library, video);
    let response = widgets::video_card(ui, &state.palette, video, watched, selected, width);
    if selected && state.cursor_moved {
        response.clone().scroll_to_me(None);
    }
    if index == videos.len() - 1 && ui.is_rect_visible(response.rect) {
        actions.push(Action::LoadMore);
    }
    if response.clicked() {
        actions.push(Action::Open(video.clone()));
    }
    results::video_menu(&response, state, video, actions);
}

/// The Shorts shelf above the feed's grid, when there are shorts.
fn shorts_shelf(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let Listing::Ready(shorts) = &state.shorts else {
        return;
    };
    if shorts.is_empty() {
        return;
    }
    widgets::section(ui, &state.palette, "Shorts", Some(shorts.len()), |ui| {
        egui::ScrollArea::horizontal()
            .id_salt("shorts")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    for video in shorts {
                        short_card(ui, state, video, actions);
                    }
                });
            });
    });
    ui.add_space(24.0);
}

/// A 9:16 card: the rounded, cropped thumbnail, a two-line title in
/// semibold, the views in secondary.
fn short_card(ui: &mut Ui, state: &State, video: &Video, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let width = 162.0;
    let thumb = vec2(width, 288.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, thumb.y + 64.0), Sense::click());
    let thumb_rect = Rect::from_min_size(rect.min, thumb);
    widgets::row_fill(ui, palette, rect.expand(4.0), &response, false);
    super::images::remote(ui, &video.thumbnail())
        .uv(widgets::short_uv())
        .fit_to_exact_size(thumb)
        .corner_radius(CornerRadius::same(theme::RADIUS))
        .paint_at(ui, thumb_rect);
    let painter = ui.painter();
    let mut job = egui::text::LayoutJob::single_section(
        video.title.clone(),
        egui::TextFormat::simple(theme::semibold(14.0), palette.text),
    );
    job.wrap.max_width = width;
    job.wrap.max_rows = 2;
    job.wrap.overflow_character = Some('…');
    let galley = painter.layout_job(job);
    let title_bottom = thumb_rect.bottom() + 8.0 + galley.size().y;
    painter.galley(
        pos2(rect.left(), thumb_rect.bottom() + 8.0),
        galley,
        palette.text,
    );
    if let Some(views) = video.views {
        painter.text(
            pos2(rect.left(), title_bottom + 2.0),
            Align2::LEFT_TOP,
            util::views(views),
            theme::regular(13.0),
            palette.secondary,
        );
    }
    if response.clicked() {
        actions.push(Action::Open(video.clone()));
    }
    results::video_menu(&response, state, video, actions);
}

pub fn subscriptions(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    results::panel(ui, |ui| {
        let count = match state.source() {
            Source::Account => state.account_subscriptions.items().len(),
            Source::Device => state.library.subscriptions.len(),
        };
        results::title(ui, state, "Subscriptions", Some(count));
        controls(ui, state, actions, |ui, actions| {
            if widgets::button(
                ui,
                &state.palette,
                "Import from Takeout",
                widgets::ButtonStyle::Normal,
            )
            .on_hover_text("Adds the channels in a Takeout subscriptions.csv to this device")
            .clicked()
            {
                actions.push(Action::ImportTakeout);
            }
        });
        match state.source() {
            Source::Account => {
                channel_listing(
                    ui,
                    state,
                    &state.account_subscriptions,
                    "The account's subscriptions",
                    false,
                    actions,
                );
                widgets::more_row(
                    ui,
                    &state.palette,
                    &state.subscriptions_more_state,
                    state.subscriptions_more.is_some(),
                    actions,
                );
            }
            Source::Device if state.library.subscriptions.is_empty() => results::nothing(
                ui,
                state,
                "Subscribe from a video's page, or import a Takeout",
            ),
            Source::Device => channel_rows(ui, state, &state.library.subscriptions, true, actions),
        }
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

/// A channel as a rounded row: its round avatar, its name in semibold,
/// and (on this device's list) the Subscribed toggle at the end.
fn channel_row(
    ui: &mut Ui,
    state: &State,
    channel: &Channel,
    selected: bool,
    removable: bool,
    actions: &mut Vec<Action>,
) {
    let palette = &state.palette;
    let size = 48.0;
    let height = size + 16.0;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    widgets::row_fill(ui, palette, rect, &response, selected);
    if selected && state.cursor_moved {
        response.clone().scroll_to_me(None);
    }
    let avatar_rect = Rect::from_min_size(rect.min + vec2(12.0, 8.0), vec2(size, size));
    widgets::avatar(
        ui,
        palette,
        channel.avatar.as_deref(),
        &channel.name,
        avatar_rect,
    );
    ui.painter().text(
        pos2(avatar_rect.right() + 16.0, rect.center().y),
        Align2::LEFT_CENTER,
        &channel.name,
        theme::semibold(14.0),
        palette.text,
    );
    if removable {
        let button = Rect::from_min_max(
            pos2(rect.right() - 160.0, rect.top()),
            pos2(rect.right() - 12.0, rect.bottom()),
        );
        let mut inner = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(button)
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        widgets::subscribe(
            &mut inner,
            state,
            channel,
            widgets::ButtonStyle::Primary,
            actions,
        );
    }
    if response.clicked() {
        actions.push(Action::OpenChannel(channel.clone()));
    }
}

pub fn history(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    results::panel(ui, |ui| {
        let device = state.source() == Source::Device;
        let count = if device {
            state.library.history.len()
        } else {
            state.account_history.items().len()
        };
        results::title(ui, state, "History", Some(count));
        controls(ui, state, actions, |ui, actions| {
            if device
                && !state.library.history.is_empty()
                && widgets::button(
                    ui,
                    &state.palette,
                    "Clear history",
                    widgets::ButtonStyle::Normal,
                )
                .on_hover_text("Clear this computer's history and resume positions")
                .clicked()
            {
                actions.push(Action::ClearHistory);
            }
        });
        if !device {
            match &state.account_history {
                Listing::Idle => {}
                Listing::Loading => results::loading(ui, state, "history"),
                Listing::Failed(error) => results::failed(ui, state, error, actions),
                Listing::Ready(rows) if rows.is_empty() => {
                    results::nothing(ui, state, "The account's watch history")
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
                                    if index > 0 {
                                        ui.add_space(16.0);
                                    }
                                    widgets::section(
                                        ui,
                                        &state.palette,
                                        row.when.as_deref().unwrap_or_default(),
                                        None,
                                        |_| {},
                                    );
                                }
                                let selected = index == state.cursor;
                                let video = &row.video;
                                let watched = widgets::watched(&state.library, video);
                                let response = widgets::video_row(
                                    ui,
                                    &state.palette,
                                    video,
                                    None,
                                    watched,
                                    selected,
                                );
                                if selected && state.cursor_moved {
                                    response.clone().scroll_to_me(None);
                                }
                                if response.clicked() {
                                    actions.push(Action::Open(video.clone()));
                                }
                                results::video_menu(&response, state, video, actions);
                            }
                        });
                }
            }
            widgets::more_row(
                ui,
                &state.palette,
                &state.history_more_state,
                state.history_more.is_some(),
                actions,
            );
            return;
        }
        if state.library.history.is_empty() {
            results::nothing(ui, state, "Videos you watch appear here");
            return;
        }
        let now = util::now();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for (index, watched) in state.library.history.iter().enumerate() {
                    let mut note = format!("Watched {}", util::ago(watched.at, now));
                    if watched.position > 0.0 {
                        note.push_str(&format!(" · left at {}", util::duration(watched.position)));
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
                    if selected && state.cursor_moved {
                        response.clone().scroll_to_me(None);
                    }
                    if response.clicked() {
                        actions.push(Action::Open(video.clone()));
                    }
                    results::video_menu(&response, state, video, actions);
                }
            });
    });
}

pub fn watch_later(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    results::panel(ui, |ui| {
        let count = match state.source() {
            Source::Account => state.account_watch_later.items().len(),
            Source::Device => state.library.watch_later.len(),
        };
        results::title(ui, state, "Watch later", Some(count));
        controls(ui, state, actions, |_, _| {});
        match state.source() {
            Source::Account => {
                rows_listing(
                    ui,
                    state,
                    &state.account_watch_later,
                    "The account's Watch later",
                    actions,
                );
                widgets::more_row(
                    ui,
                    &state.palette,
                    &state.watch_later_more_state,
                    state.watch_later_more.is_some(),
                    actions,
                );
            }
            Source::Device if state.library.watch_later.is_empty() => {
                results::nothing(ui, state, "Save videos from their page")
            }
            Source::Device => results::video_rows(ui, state, &state.library.watch_later, actions),
        }
    });
}

/// A `Listing<Video>` as rows under the page's title.
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
        Listing::Ready(videos) => results::video_rows(ui, state, videos, actions),
    }
}

pub fn liked(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    results::panel(ui, |ui| {
        results::videos(
            ui,
            state,
            "Liked videos",
            &state.liked,
            "The account's liked videos",
            actions,
        );
        widgets::more_row(
            ui,
            &state.palette,
            &state.liked_more_state,
            state.liked_more.is_some(),
            actions,
        );
    });
}

pub fn playlists(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    results::panel(ui, |ui| {
        results::title(ui, state, "Playlists", Some(state.playlists.items().len()));
        match &state.playlists {
            Listing::Idle => {}
            Listing::Loading => results::loading(ui, state, "playlists"),
            Listing::Failed(error) => results::failed(ui, state, error, actions),
            Listing::Ready(playlists) if playlists.is_empty() => {
                results::nothing(ui, state, "The account's playlists")
            }
            Listing::Ready(playlists) => {
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        for (index, playlist) in playlists.iter().enumerate() {
                            let selected = index == state.cursor;
                            let response =
                                widgets::playlist_row(ui, &state.palette, playlist, selected, true);
                            if selected && state.cursor_moved {
                                response.clone().scroll_to_me(None);
                            }
                            if response.clicked() {
                                actions.push(Action::OpenPlaylist {
                                    id: playlist.id.clone(),
                                    name: playlist.name.clone(),
                                });
                            }
                        }
                    });
            }
        }
        widgets::more_row(
            ui,
            &state.palette,
            &state.playlists_more_state,
            state.playlists_more.is_some(),
            actions,
        );
    });
}

/// One playlist's videos.
pub fn page(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    results::panel(ui, |ui| {
        let Some(Page::Playlist { name, .. }) = &state.page else {
            return;
        };
        results::title(ui, state, name, Some(state.page_videos.items().len()));
        rows_listing(ui, state, &state.page_videos, "No videos here", actions);
        widgets::more_row(
            ui,
            &state.palette,
            &state.page_more_state,
            state.page_more.is_some(),
            actions,
        );
    });
}
