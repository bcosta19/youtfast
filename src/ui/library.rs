//! The pages of what the user follows and keeps: the home feed,
//! subscriptions, history, Watch later, liked videos, playlists, and one
//! playlist's videos. Pages that exist both ways switch between the
//! signed-in account and this device with chips under their title.

use crate::appearance::i18n::{format as trf, gettext as tr};
use egui::{Align2, CornerRadius, Rect, RichText, Sense, Ui, pos2, vec2};

use super::{results, widgets};
use crate::app::{AccountState, Page, State};
use crate::appearance::theme;
use crate::model::{Action, Channel, FeedFilter, Fetch, Listing, Source, Video, View};
use crate::util;

/// The Account / This device chips (when signed in) and a page's extra
/// buttons, on one line under the page title.
fn controls(
    ui: &mut Ui,
    state: &State,
    actions: &mut Vec<Action>,
    extra: impl FnOnce(&mut Ui, &mut Vec<Action>),
) {
    let locale = state.settings.language;

    ui.horizontal(|ui| {
        if state.signed_in() {
            let source = state.source();
            if widgets::chip(
                ui,
                &state.palette,
                tr(locale, "Account").as_ref(),
                source == Source::Account,
            )
            .clicked()
            {
                actions.push(Action::SetSource(Source::Account));
            }
            if widgets::chip(
                ui,
                &state.palette,
                tr(locale, "This device").as_ref(),
                source == Source::Device,
            )
            .clicked()
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
    let locale = state.settings.language;

    results::panel(ui, |ui| {
        // With a history, For you has something to offer already.
        let follows_nothing = state.library.subscriptions.is_empty()
            && state.library.history.is_empty()
            && !state.signed_in()
            && state.account != AccountState::SigningIn;
        if follows_nothing {
            results::centered(ui, |ui| {
                ui.label(
                    RichText::new(tr(locale, "Your feed is empty").as_ref())
                        .font(theme::semibold(16.0))
                        .color(state.palette.text),
                );
                ui.label(
                    RichText::new(tr(locale, "Subscribe to channels to fill your feed").as_ref())
                        .font(theme::regular(14.0))
                        .color(state.palette.secondary),
                );
                ui.add_space(12.0);
                if widgets::button(
                    ui,
                    &state.palette,
                    tr(locale, "Import from Google Takeout").as_ref(),
                    widgets::ButtonStyle::Normal,
                )
                .clicked()
                {
                    actions.push(Action::ImportTakeout);
                }
                ui.add_space(8.0);
                if widgets::button(
                    ui,
                    &state.palette,
                    tr(locale, "Sign in").as_ref(),
                    widgets::ButtonStyle::Primary,
                )
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
    let locale = state.settings.language;

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
                tr(locale, filter.label()).as_ref(),
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
    let locale = state.settings.language;

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
        Listing::Ready(videos) if videos.is_empty() => results::nothing(
            ui,
            state,
            tr(locale, "Nothing from your subscriptions yet").as_ref(),
        ),
        Listing::Ready(_) => {
            feed_chips(ui, state, actions);
            let now = crate::util::now();
            let videos = state.visible_feed();
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    shorts_shelf(ui, state, actions);
                    if videos.is_empty() {
                        results::nothing(
                            ui,
                            state,
                            tr(locale, "No video matches this filter").as_ref(),
                        );
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
                        let heading = super::notifications::capitalized(day, locale);
                        widgets::section(ui, &state.palette, &heading, None, |ui| {
                            let selected = (state.cursor_moved
                                && (start..end).contains(&state.cursor))
                            .then(|| (state.cursor - start) / columns);
                            results::virtual_rows(
                                ui,
                                widgets::video_card_height(ui, width) + gap,
                                (end - start).div_ceil(columns),
                                selected,
                                |ui, row| {
                                    ui.horizontal_top(|ui| {
                                        ui.spacing_mut().item_spacing.x = gap;
                                        for index in (start + row * columns)
                                            ..(start + (row + 1) * columns).min(end)
                                        {
                                            feed_card(ui, state, &videos, index, width, actions);
                                        }
                                    });
                                    ui.add_space(gap);
                                },
                            );
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
                        locale,
                    );
                });
        }
    }
}

/// For you as a grid of cards, under the chips and a line saying where it
/// comes from; the last card in view asks for the account's next page.
fn for_you(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    feed_chips(ui, state, actions);
    match &state.for_you {
        Listing::Idle => {}
        Listing::Loading => results::loading(ui, state, "recommendations"),
        Listing::Failed(error) => results::failed(ui, state, error, actions),
        Listing::Ready(videos) if videos.is_empty() => results::nothing(
            ui,
            state,
            tr(locale, "Watch a few videos to get recommendations here").as_ref(),
        ),
        Listing::Ready(_) => {
            let source = if state.for_you_from_account {
                "Recommended by YouTube for your account"
            } else {
                "Picked from videos related to what you watched"
            };
            ui.label(
                RichText::new(tr(locale, source).as_ref())
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
                    results::virtual_rows(
                        ui,
                        widgets::video_card_height(ui, width) + gap,
                        videos.len().div_ceil(columns),
                        state.cursor_moved.then_some(state.cursor / columns),
                        |ui, row| {
                            ui.horizontal_top(|ui| {
                                ui.spacing_mut().item_spacing.x = gap;
                                for index in row * columns..((row + 1) * columns).min(videos.len())
                                {
                                    feed_card(ui, state, &videos, index, width, actions);
                                }
                            });
                            ui.add_space(gap);
                        },
                    );
                    if state.for_you_more.is_some()
                        || !matches!(state.for_you_more_state, Fetch::Idle)
                    {
                        widgets::more_row(
                            ui,
                            &state.palette,
                            &state.for_you_more_state,
                            state.for_you_more.is_some(),
                            actions,
                            locale,
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
    let locale = state.settings.language;

    let video = videos[index];
    let selected = index == state.cursor;
    let watched = widgets::watched(&state.library, video);
    let response = widgets::video_card(ui, &state.palette, video, watched, selected, width, locale);
    if selected && state.cursor_moved {
        response.clone().scroll_to_me(None);
    }
    if index == videos.len() - 1 && ui.is_rect_visible(response.rect) {
        actions.push(Action::LoadMore);
    }
    widgets::video_interactions(ui, &response, state, video, actions);
}

/// The Shorts shelf above the feed's grid, when there are shorts.
fn shorts_shelf(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let Listing::Ready(shorts) = &state.shorts else {
        return;
    };
    if shorts.is_empty() {
        return;
    }
    widgets::section(
        ui,
        &state.palette,
        tr(locale, "Shorts").as_ref(),
        Some(shorts.len()),
        |ui| {
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
        },
    );
    ui.add_space(24.0);
}

/// A 9:16 card: the rounded, cropped thumbnail, a two-line title in
/// semibold, the views in secondary.
fn short_card(ui: &mut Ui, state: &State, video: &Video, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    let palette = &state.palette;
    let width = 162.0;
    let thumb = vec2(width, 288.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, thumb.y + 64.0), Sense::click());
    if !ui.is_rect_visible(rect) {
        return;
    }
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
            util::views(views, locale),
            theme::regular(13.0),
            palette.secondary,
        );
    }
    widgets::video_interactions(ui, &response, state, video, actions);
}

pub fn subscriptions(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    results::panel(ui, |ui| {
        let count = match state.source() {
            Source::Account => state.account_subscriptions.items().len(),
            Source::Device => state.library.subscriptions.len(),
        };
        results::title(ui, state, tr(locale, "Subscriptions").as_ref(), Some(count));
        controls(ui, state, actions, |ui, actions| {
            if widgets::button(
                ui,
                &state.palette,
                tr(locale, "Import from Takeout").as_ref(),
                widgets::ButtonStyle::Normal,
            )
            .on_hover_text(
                tr(
                    locale,
                    "Adds channels from a Takeout subscriptions.csv to this device",
                )
                .as_ref(),
            )
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
                    tr(locale, "The account's subscriptions").as_ref(),
                    false,
                    actions,
                );
                widgets::more_row(
                    ui,
                    &state.palette,
                    &state.subscriptions_more_state,
                    state.subscriptions_more.is_some(),
                    actions,
                    locale,
                );
            }
            Source::Device if state.library.subscriptions.is_empty() => results::nothing(
                ui,
                state,
                tr(locale, "Subscribe from a video's page or import a Takeout").as_ref(),
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

pub(super) fn channel_rows(
    ui: &mut Ui,
    state: &State,
    channels: &[Channel],
    removable: bool,
    actions: &mut Vec<Action>,
) {
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            channel_rows_content(ui, state, channels, removable, actions);
        });
}

/// Draw just the rows so a caller can keep its continuation in the same scroll area.
pub(super) fn channel_rows_content(
    ui: &mut Ui,
    state: &State,
    channels: &[Channel],
    removable: bool,
    actions: &mut Vec<Action>,
) {
    results::virtual_rows(
        ui,
        64.0,
        channels.len(),
        state.cursor_moved.then_some(state.cursor),
        |ui, index| {
            channel_row(
                ui,
                state,
                &channels[index],
                index == state.cursor,
                removable,
                actions,
            );
        },
    );
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
    let locale = state.settings.language;

    results::panel(ui, |ui| {
        let device = state.source() == Source::Device;
        let count = if device {
            state.library.history.len()
        } else {
            state.account_history.items().len()
        };
        results::title(ui, state, tr(locale, "History").as_ref(), Some(count));
        controls(ui, state, actions, |ui, actions| {
            if device
                && !state.library.history.is_empty()
                && widgets::button(
                    ui,
                    &state.palette,
                    tr(locale, "Clear history").as_ref(),
                    widgets::ButtonStyle::Normal,
                )
                .on_hover_text(
                    tr(locale, "Clear this device's history and resume positions").as_ref(),
                )
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
                Listing::Ready(rows) if rows.is_empty() => results::nothing(
                    ui,
                    state,
                    tr(locale, "The account's watch history").as_ref(),
                ),
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
                                    locale,
                                );
                                if selected && state.cursor_moved {
                                    response.clone().scroll_to_me(None);
                                }
                                widgets::video_interactions(ui, &response, state, video, actions);
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
                locale,
            );
            return;
        }
        if state.library.history.is_empty() {
            results::nothing(
                ui,
                state,
                tr(locale, "Videos you watch appear here").as_ref(),
            );
            return;
        }
        let mut query = state.history_query.clone();
        widgets::search_field(
            ui,
            &state.palette,
            &mut query,
            tr(locale, "Search this device's history").as_ref(),
            egui::Id::new("history search"),
            480.0_f32.min(ui.available_width()),
        );
        if query != state.history_query {
            actions.push(Action::SetHistoryQuery(query));
        }
        ui.add_space(12.0);
        let matching: Vec<_> = state
            .library
            .history_matching(&state.history_query)
            .collect();
        if matching.is_empty() {
            results::nothing(ui, state, tr(locale, "No videos found in history").as_ref());
            return;
        }
        let now = util::now();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for (index, watched) in matching.into_iter().enumerate() {
                    let mut note = trf(
                        locale,
                        "Watched {age}",
                        &[("age", util::ago(watched.at, now, locale))],
                    );
                    if watched.position > 0.0 {
                        note.push_str(&trf(
                            locale,
                            " · left at {time}",
                            &[("time", util::duration(watched.position))],
                        ));
                    }
                    let video = &watched.video;
                    let fraction = video
                        .duration
                        .map(|duration| watched.position / duration)
                        .map(|f| f as f32);
                    let selected = index == state.cursor;
                    ui.push_id(&video.id, |ui| {
                        ui.horizontal(|ui| {
                            let width = (ui.available_width() - 44.0).max(0.0);
                            ui.allocate_ui_with_layout(
                                egui::vec2(width, 106.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    let response = widgets::video_row(
                                        ui,
                                        &state.palette,
                                        video,
                                        Some(&note),
                                        fraction,
                                        selected,
                                        locale,
                                    );
                                    if selected && state.cursor_moved {
                                        response.clone().scroll_to_me(None);
                                    }
                                    widgets::video_interactions(
                                        ui, &response, state, video, actions,
                                    );
                                },
                            );
                            if widgets::icon_button(
                                ui,
                                crate::appearance::icons::Icon::Close,
                                32.0,
                                tr(locale, "Remove from this device's history").as_ref(),
                            )
                            .clicked()
                            {
                                actions.push(Action::RemoveHistory(video.id.clone()));
                            }
                        });
                    });
                }
            });
    });
}

pub fn watch_later(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    results::panel(ui, |ui| {
        let count = match state.source() {
            Source::Account => state.account_watch_later.items().len(),
            Source::Device => state.library.watch_later.len(),
        };
        results::title(ui, state, tr(locale, "Watch later").as_ref(), Some(count));
        controls(ui, state, actions, |_, _| {});
        match state.source() {
            Source::Account => {
                rows_listing(
                    ui,
                    state,
                    &state.account_watch_later,
                    tr(locale, "The account's Watch later").as_ref(),
                    actions,
                );
                widgets::more_row(
                    ui,
                    &state.palette,
                    &state.watch_later_more_state,
                    state.watch_later_more.is_some(),
                    actions,
                    locale,
                );
            }
            Source::Device if state.library.watch_later.is_empty() => results::nothing(
                ui,
                state,
                tr(locale, "Save videos from their page").as_ref(),
            ),
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
    let locale = state.settings.language;

    results::panel(ui, |ui| {
        results::videos(
            ui,
            state,
            tr(locale, "Videos you liked").as_ref(),
            &state.liked,
            tr(locale, "The account's liked videos").as_ref(),
            actions,
        );
        widgets::more_row(
            ui,
            &state.palette,
            &state.liked_more_state,
            state.liked_more.is_some(),
            actions,
            locale,
        );
    });
}

pub fn playlists(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    results::panel(ui, |ui| {
        results::title(
            ui,
            state,
            tr(locale, "Playlists").as_ref(),
            Some(state.playlists.items().len()),
        );
        match &state.playlists {
            Listing::Idle => {}
            Listing::Loading => results::loading(ui, state, "playlists"),
            Listing::Failed(error) => results::failed(ui, state, error, actions),
            Listing::Ready(playlists) if playlists.is_empty() => {
                results::nothing(ui, state, tr(locale, "The account's playlists").as_ref())
            }
            Listing::Ready(playlists) => {
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        results::virtual_rows(
                            ui,
                            106.0,
                            playlists.len(),
                            state.cursor_moved.then_some(state.cursor),
                            |ui, index| {
                                let playlist = &playlists[index];
                                let selected = index == state.cursor;
                                let response = widgets::playlist_row(
                                    ui,
                                    &state.palette,
                                    playlist,
                                    selected,
                                    true,
                                    locale,
                                );
                                if selected && state.cursor_moved {
                                    response.clone().scroll_to_me(None);
                                }
                                if response.clicked() {
                                    actions.push(Action::OpenPlaylist {
                                        id: playlist.id.clone(),
                                        name: playlist.name.clone(),
                                    });
                                }
                            },
                        );
                    });
            }
        }
        widgets::more_row(
            ui,
            &state.palette,
            &state.playlists_more_state,
            state.playlists_more.is_some(),
            actions,
            locale,
        );
    });
}

/// One playlist's videos.
pub fn page(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    results::panel(ui, |ui| {
        let Some(Page::Playlist { name, .. }) = &state.page else {
            return;
        };
        let details = state.playlist_details.as_ref();
        let title = details
            .filter(|details| !details.name.is_empty())
            .map(|details| details.name.as_str())
            .unwrap_or(name);
        ui.horizontal_top(|ui| {
            if let Some(url) = details.and_then(|details| details.thumbnail.as_deref()) {
                let width = (ui.available_width() * 0.3).clamp(96.0, 240.0);
                ui.add(
                    super::images::remote(ui, url)
                        .fit_to_exact_size(vec2(width, width * 9.0 / 16.0))
                        .corner_radius(egui::CornerRadius::same(theme::RADIUS)),
                );
                ui.add_space(12.0);
            }
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(title)
                        .font(theme::bold(24.0))
                        .color(state.palette.text),
                );
                if let Some(details) = details {
                    let mut metadata = Vec::new();
                    if let Some(channel) = &details.channel {
                        metadata.push(channel.clone());
                    }
                    if let Some(count) = details.videos {
                        metadata.push(widgets::video_count_label(count, locale));
                    }
                    ui.label(RichText::new(metadata.join(" · ")).color(state.palette.secondary));
                    if !details.description.is_empty() {
                        egui::CollapsingHeader::new(tr(locale, "Description").as_ref()).show(
                            ui,
                            |ui| {
                                ui.label(
                                    egui::RichText::new(&details.description)
                                        .color(state.palette.secondary),
                                );
                            },
                        );
                    }
                }
                if !state.page_videos.items().is_empty()
                    && widgets::button(
                        ui,
                        &state.palette,
                        tr(locale, "Play all").as_ref(),
                        widgets::ButtonStyle::Primary,
                    )
                    .clicked()
                {
                    actions.push(Action::PlayPlaylist);
                }
            });
        });
        ui.add_space(16.0);
        match &state.page_videos {
            Listing::Ready(videos) => {
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        if videos.is_empty() {
                            results::nothing(
                                ui,
                                state,
                                tr(locale, "No videos in this playlist").as_ref(),
                            );
                        }
                        results::virtual_rows(
                            ui,
                            106.0,
                            videos.len(),
                            state.cursor_moved.then_some(state.cursor),
                            |ui, index| {
                                let video = &videos[index];
                                let playing = state
                                    .watching
                                    .as_ref()
                                    .is_some_and(|playing| playing.id == video.id);
                                let note = if playing {
                                    trf(
                                        locale,
                                        "{number} · Playing",
                                        &[("number", (index + 1).to_string())],
                                    )
                                } else {
                                    format!("{}", index + 1)
                                };
                                let response = widgets::video_row(
                                    ui,
                                    &state.palette,
                                    video,
                                    Some(&note),
                                    widgets::watched(&state.library, video),
                                    index == state.cursor || playing,
                                    locale,
                                );
                                widgets::video_interactions(ui, &response, state, video, actions);
                            },
                        );
                        widgets::more_row(
                            ui,
                            &state.palette,
                            &state.page_more_state,
                            state.page_more.is_some(),
                            actions,
                            locale,
                        );
                    });
            }
            _ => rows_listing(
                ui,
                state,
                &state.page_videos,
                tr(locale, "No videos in this playlist").as_ref(),
                actions,
            ),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_account_subscriptions_and_watch_later_use_the_explicit_locale() {
        use crate::appearance::i18n::Locale;
        for locale in [Locale::English, Locale::PortugueseBrazil] {
            for (view, english, portuguese) in [
                (
                    View::Subscriptions,
                    "The account's subscriptions",
                    "As inscrições da conta",
                ),
                (
                    View::WatchLater,
                    "The account's Watch later",
                    "Os vídeos para assistir mais tarde da conta",
                ),
            ] {
                let ctx = egui::Context::default();
                crate::appearance::theme::install_fonts(&ctx);
                let mut state = State {
                    view,
                    account: AccountState::SignedIn(1),
                    source: Source::Account,
                    account_subscriptions: Listing::Ready(Vec::new()),
                    account_watch_later: Listing::Ready(Vec::new()),
                    ..Default::default()
                };
                state.settings.language = locale;
                let expected = if locale == Locale::English {
                    english
                } else {
                    portuguese
                };
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            egui::Pos2::ZERO,
                            vec2(960.0, 720.0),
                        )),
                        ..Default::default()
                    },
                    |ui| match view {
                        View::Subscriptions => subscriptions(ui, &state, &mut Vec::new()),
                        View::WatchLater => watch_later(ui, &state, &mut Vec::new()),
                        _ => unreachable!(),
                    },
                );
                assert!(output.shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Text(text) if text.galley.text()==expected)),"empty state missing its localized hint: {view:?}/{locale:?}");
                output.textures_delta.clear();
            }
        }
    }

    #[test]
    fn large_channel_playlist_and_feed_lists_request_only_visible_images() {
        for kind in 0..3 {
            let ctx = egui::Context::default();
            crate::appearance::theme::install_fonts(&ctx);
            let mut state = State::default();
            if kind == 0 {
                state.library.subscriptions = (0..20_000)
                    .map(|index| Channel {
                        id: format!("UC{index:022}"),
                        name: format!("Channel {index}"),
                        avatar: Some(format!("https://i.ytimg.com/synthetic-avatar-{index}")),
                    })
                    .collect();
            } else if kind == 1 {
                state.playlists = Listing::Ready(
                    (0..1000)
                        .map(|index| crate::model::PlaylistSummary {
                            id: format!("playlist-{index}"),
                            name: format!("Playlist {index}"),
                            thumbnail: Some(format!(
                                "https://i.ytimg.com/synthetic-playlist-{index}"
                            )),
                            channel: None,
                            videos: Some(10),
                        })
                        .collect(),
                );
            } else {
                state.feed = Listing::Ready(
                    (0..1000)
                        .map(|index| Video::from_id(format!("{index:011}")))
                        .collect(),
                );
                state.feed_shown = 1000;
                state.feed_filter = FeedFilter::All;
            }
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(640.0, 480.0))),
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        let mut actions = Vec::new();
                        match kind {
                            0 => channel_rows(
                                ui,
                                &state,
                                &state.library.subscriptions,
                                true,
                                &mut actions,
                            ),
                            1 => playlists(ui, &state, &mut actions),
                            _ => feed_grid(ui, &state, &mut actions),
                        }
                    });
                },
            );
            output.textures_delta.clear();
            let drawn = super::super::images::drawn_count(&ctx);
            assert!(drawn > 0 && drawn < 20, "list {kind} drew {drawn} images");
        }
    }
}
