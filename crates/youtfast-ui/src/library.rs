//! The pages of what the user follows and keeps: the home feed,
//! subscriptions, history, Watch later, liked videos, playlists, and one
//! playlist's videos. Pages that exist both ways switch between the
//! signed-in account and this device with chips under their title.

use std::collections::HashMap;

use egui::text::LayoutJob;
use egui::{
    Color32, CornerRadius, CursorIcon, FontId, Rect, RichText, Sense, Stroke, Ui, pos2, vec2,
};
use youtfast_i18n::{Locale, format as trf, gettext as tr};

use super::{results, widgets};
use youtfast_app::{AccountState, Action, ChannelActivity, Page, State};
use youtfast_appearance::icons::Icon;
use youtfast_appearance::theme::{self, Palette, RADIUS};
use youtfast_core::model::{
    Channel, ChannelMeta, FeedFilter, Fetch, Listing, Source, SubscriptionSort, Video, View,
};
use youtfast_core::util;

/// The Account / This device chips (when signed in) and a page's extra
/// buttons, on one line under the page title.
fn controls(
    ui: &mut Ui,
    state: &State,
    actions: &mut Vec<Action>,
    extra: impl FnOnce(&mut Ui, &mut Vec<Action>),
) {
    let locale = state.settings.language;

    let draw = |ui: &mut Ui| {
        if state.signed_in() {
            let merged = state.view == View::History && state.merged_history();
            let source = if merged {
                state.history_source()
            } else {
                Some(state.source())
            };
            if merged
                && widgets::chip(
                    ui,
                    &state.palette,
                    tr(locale, "All").as_ref(),
                    source.is_none(),
                )
                .clicked()
            {
                actions.push(Action::SetHistoryFilter(
                    youtfast_logic::history::HistoryFilter::All,
                ));
            }
            if widgets::chip(
                ui,
                &state.palette,
                tr(locale, "Account").as_ref(),
                source == Some(Source::Account),
            )
            .clicked()
            {
                actions.push(if merged {
                    Action::SetHistoryFilter(youtfast_logic::history::HistoryFilter::Account)
                } else {
                    Action::SetSource(Source::Account)
                });
            }
            if widgets::chip(
                ui,
                &state.palette,
                tr(locale, "This device").as_ref(),
                source == Some(Source::Device),
            )
            .clicked()
            {
                actions.push(if merged {
                    Action::SetHistoryFilter(youtfast_logic::history::HistoryFilter::Device)
                } else {
                    Action::SetSource(Source::Device)
                });
            }
            ui.add_space(8.0);
        }
        extra(ui, actions);
    };
    if matches!(state.view, View::History | View::WatchLater) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            draw(ui);
        });
    } else {
        ui.horizontal(draw);
    }
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
            let today = youtfast_core::util::Today::at(youtfast_core::util::now());
            let videos = state.visible_feed();
            let buckets: Vec<_> = videos
                .iter()
                .map(|video| today.bucket(video.published_at))
                .collect();
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
                        let day = buckets[start];
                        let end = buckets[start..]
                            .iter()
                            .position(|bucket| *bucket != day)
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
    let selected = state.is_cursor(index);
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

const SHORT_CARD_WIDTH: f32 = 156.0;
const SHORT_CARD_GAP: f32 = 12.0;
const SHORT_CARD_PAD: f32 = 6.0;

#[derive(Clone, Copy, Default)]
struct ShortsShelfState {
    collapsed: bool,
    offset: f32,
    pending_offset: Option<f32>,
}

/// The Shorts shelf above the feed's grid, when there are shorts.
fn shorts_shelf(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;
    let Listing::Ready(shorts) = &state.shorts else {
        return;
    };
    if shorts.is_empty() {
        return;
    }

    let shelf_id = egui::Id::new("shorts-shelf-state");
    let mut shelf = ui
        .ctx()
        .data(|d| d.get_temp::<ShortsShelfState>(shelf_id))
        .unwrap_or_default();
    let show_channel_slot = shorts
        .iter()
        .any(|v| v.channel.as_deref().is_some_and(|c| !c.is_empty()));

    egui::Frame::new()
        .fill(palette.panel)
        .corner_radius(CornerRadius::same(RADIUS + 4))
        .stroke(Stroke::new(1.0, palette.outline))
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let viewport_w = ui.available_width();
            let content_w = shorts.len() as f32 * SHORT_CARD_WIDTH
                + shorts.len().saturating_sub(1) as f32 * SHORT_CARD_GAP;
            let max_offset = (content_w - viewport_w).max(0.0);
            let step = (SHORT_CARD_WIDTH + SHORT_CARD_GAP) * 3.0;

            ui.horizontal(|ui| {
                let (pip_rect, _) = ui.allocate_exact_size(vec2(3.5, 16.0), Sense::hover());
                ui.painter()
                    .rect_filled(pip_rect, CornerRadius::same(2), palette.accent);
                ui.label(
                    RichText::new(tr(locale, "Shorts").as_ref())
                        .font(theme::bold(16.0))
                        .color(palette.text),
                );
                ui.label(
                    RichText::new(util::thousands(shorts.len() as u64))
                        .font(theme::regular(13.0))
                        .color(palette.secondary),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (toggle_icon, toggle_hint) = if shelf.collapsed {
                        (Icon::ChevronDown, tr(locale, "Show Shorts"))
                    } else {
                        (Icon::ChevronUp, tr(locale, "Hide Shorts"))
                    };
                    if widgets::icon_button(ui, toggle_icon, 16.0, toggle_hint.as_ref()).clicked() {
                        shelf.collapsed = !shelf.collapsed;
                    }
                    if !shelf.collapsed && max_offset > 1.0 {
                        if ui
                            .add_enabled_ui(shelf.offset + 1.0 < max_offset, |ui| {
                                widgets::icon_button(
                                    ui,
                                    Icon::ChevronRight,
                                    16.0,
                                    tr(locale, "Next Shorts").as_ref(),
                                )
                            })
                            .inner
                            .clicked()
                        {
                            shelf.pending_offset = Some((shelf.offset + step).min(max_offset));
                        }
                        if ui
                            .add_enabled_ui(shelf.offset > 1.0, |ui| {
                                widgets::icon_button(
                                    ui,
                                    Icon::ChevronLeft,
                                    16.0,
                                    tr(locale, "Previous Shorts").as_ref(),
                                )
                            })
                            .inner
                            .clicked()
                        {
                            shelf.pending_offset = Some((shelf.offset - step).max(0.0));
                        }
                    }
                });
            });

            if !shelf.collapsed {
                ui.add_space(10.0);
                let mut scroll = egui::ScrollArea::horizontal()
                    .id_salt("shorts")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .auto_shrink([false, true]);
                if let Some(target) = shelf.pending_offset.take() {
                    scroll = scroll.horizontal_scroll_offset(target);
                }
                let output = scroll.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = SHORT_CARD_GAP;
                        for video in shorts {
                            short_card(ui, state, video, show_channel_slot, actions);
                        }
                    });
                });
                shelf.offset = output.state.offset.x.clamp(0.0, max_offset);
            }
        });
    ui.ctx().data_mut(|d| d.insert_temp(shelf_id, shelf));
    ui.add_space(20.0);
}

/// A 9:16 card: rounded cropped thumbnail with hairline border, a two-line title
/// clearing the `More` button, optional clickable channel name, and views.
fn short_card(
    ui: &mut Ui,
    state: &State,
    video: &Video,
    show_channel_slot: bool,
    actions: &mut Vec<Action>,
) {
    let locale = state.settings.language;
    let palette = &state.palette;
    let thumb_w = SHORT_CARD_WIDTH - 2.0 * SHORT_CARD_PAD;
    let thumb = vec2(thumb_w, (thumb_w * 16.0 / 9.0).round());
    let text_area = if show_channel_slot { 88.0 } else { 72.0 };
    let (rect, response) = ui.allocate_exact_size(
        vec2(SHORT_CARD_WIDTH, SHORT_CARD_PAD + thumb.y + text_area),
        Sense::click(),
    );
    if !ui.is_rect_visible(rect) {
        return;
    }

    let card_radius = CornerRadius::same(RADIUS + 4);
    widgets::row_fill_radius(ui, palette, rect, &response, false, card_radius);

    let thumb_rect = Rect::from_min_size(rect.min + vec2(SHORT_CARD_PAD, SHORT_CARD_PAD), thumb);
    let thumb_radius = CornerRadius::same(RADIUS);
    ui.painter()
        .rect_filled(thumb_rect, thumb_radius, palette.surface);
    youtfast_app::images::remote(ui, &video.thumbnail())
        .uv(widgets::short_uv())
        .fit_to_exact_size(thumb)
        .corner_radius(thumb_radius)
        .paint_at(ui, thumb_rect);
    let hairline = if palette.dark {
        Color32::from_white_alpha(18)
    } else {
        Color32::from_black_alpha(16)
    };
    ui.painter().rect_stroke(
        thumb_rect,
        thumb_radius,
        Stroke::new(1.0, hairline),
        egui::StrokeKind::Inside,
    );

    // Leave clearance on the right for the 30x30 `More` button at `rect.right() - 16`.
    let text_w = (thumb.x - 30.0).max(20.0);
    let text_x = thumb_rect.left();
    let painter = ui.painter();
    let mut job = LayoutJob::single_section(
        video.title.clone(),
        egui::TextFormat::simple(theme::semibold(13.5), palette.text),
    );
    job.wrap.max_width = text_w;
    job.wrap.max_rows = 2;
    job.wrap.overflow_character = Some('…');
    let galley = painter.layout_job(job);
    let title_pos = pos2(text_x, thumb_rect.bottom() + 8.0);
    let mut y = title_pos.y + galley.size().y;
    painter.galley(title_pos, galley, palette.text);

    if let Some(channel) = video.channel.as_deref().filter(|c| !c.is_empty()) {
        y += 4.0;
        let mut channel_job = LayoutJob::single_section(
            channel.to_owned(),
            egui::TextFormat::simple(theme::regular(12.0), palette.secondary),
        );
        channel_job.wrap.max_width = text_w;
        channel_job.wrap.max_rows = 1;
        channel_job.wrap.overflow_character = Some('…');
        let channel_galley = painter.layout_job(channel_job);
        let channel_size = channel_galley.size();
        let channel_pos = pos2(text_x, y);
        painter.galley(channel_pos, channel_galley, palette.secondary);
        if video.channel_id.is_some() {
            ui.ctx().data_mut(|d| {
                d.insert_temp(
                    response.id.with("channel-rect"),
                    Rect::from_min_size(channel_pos, channel_size),
                );
            });
        }
        y += channel_size.y;
    }

    if let Some(views) = video.views {
        y += 2.0;
        let mut views_job = LayoutJob::single_section(
            util::views(views, locale),
            egui::TextFormat::simple(theme::regular(12.0), palette.secondary),
        );
        views_job.wrap.max_width = text_w;
        views_job.wrap.max_rows = 1;
        views_job.wrap.overflow_character = Some('…');
        let views_galley = painter.layout_job(views_job);
        painter.galley(pos2(text_x, y), views_galley, palette.secondary);
    }

    widgets::video_interactions(ui, &response, state, video, actions);
}

const CHANNEL_CARD_WIDTH: f32 = 360.0;
const CHANNEL_CARD_HEIGHT: f32 = 194.0;
const CHANNEL_GRID_GAP: f32 = 14.0;

pub fn subscriptions(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let activity = state.subscription_activity();
    let total = state.subscription_list().len();

    results::panel(ui, |ui| {
        results::title(ui, state, tr(locale, "Subscriptions").as_ref(), Some(total));
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
        let has_items = total > 0 || !state.subscriptions_query.is_empty();
        if has_items {
            subscription_toolbar(ui, state, &activity, total, actions);
        }
        match state.source() {
            Source::Account => match &state.account_subscriptions {
                Listing::Idle => {}
                Listing::Loading => results::loading(ui, state, "subscriptions"),
                Listing::Failed(error) => results::failed(ui, state, error, actions),
                Listing::Ready(channels) if channels.is_empty() => {
                    results::nothing(
                        ui,
                        state,
                        tr(locale, "The account's subscriptions").as_ref(),
                    );
                }
                Listing::Ready(_) => {
                    let visible = state.visible_subscriptions_with_activity(&activity);
                    subscription_body(ui, state, &visible, &activity, false, true, actions);
                }
            },
            Source::Device if state.library.subscriptions.is_empty() => results::nothing(
                ui,
                state,
                tr(locale, "Subscribe from a video's page or import a Takeout").as_ref(),
            ),
            Source::Device => {
                let visible = state.visible_subscriptions_with_activity(&activity);
                subscription_body(ui, state, &visible, &activity, true, false, actions);
            }
        }
    });
}

fn subscription_toolbar(
    ui: &mut Ui,
    state: &State,
    activity: &HashMap<&str, ChannelActivity<'_>>,
    total: usize,
    actions: &mut Vec<Action>,
) {
    let locale = state.settings.language;
    let palette = &state.palette;
    let active_channels = state
        .subscription_list()
        .iter()
        .filter(|channel| {
            activity
                .get(channel.id.as_str())
                .is_some_and(|act| act.recent_count > 0)
        })
        .count();

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        let mut query = state.subscriptions_query.clone();
        widgets::search_field(
            ui,
            palette,
            &mut query,
            tr(locale, "Filter channels").as_ref(),
            locale,
            egui::Id::new("subscriptions search"),
            260.0_f32.min(ui.available_width()),
        );
        if query != state.subscriptions_query {
            actions.push(Action::SetSubscriptionsQuery(query));
        }
        ui.add_space(4.0);

        for sort in SubscriptionSort::ALL {
            let label = match sort {
                SubscriptionSort::Name => "Name",
                SubscriptionSort::Recent => "Recent",
                SubscriptionSort::Watched => "Most watched",
            };
            if widgets::chip(
                ui,
                palette,
                tr(locale, label).as_ref(),
                state.subscriptions_sort == sort,
            )
            .clicked()
            {
                actions.push(Action::SetSubscriptionsSort(sort));
            }
        }

        let summary = if !state.subscriptions_query.trim().is_empty() {
            let visible_count = state.visible_subscriptions_with_activity(activity).len();
            Some(widgets::compact_count_label(
                visible_count as u64,
                "{count} matching",
                "{count} matching",
                locale,
            ))
        } else if active_channels > 0 && total > 0 {
            Some(widgets::compact_count_label(
                active_channels as u64,
                "{count} active",
                "{count} active",
                locale,
            ))
        } else {
            None
        };
        if let Some(summary) = summary {
            ui.add_space(4.0);
            ui.label(
                RichText::new(summary)
                    .font(theme::regular(13.0))
                    .color(palette.secondary),
            );
        }
    });
    ui.add_space(12.0);
}

fn subscription_body(
    ui: &mut Ui,
    state: &State,
    visible: &[&Channel],
    activity: &HashMap<&str, ChannelActivity<'_>>,
    removable: bool,
    with_more: bool,
    actions: &mut Vec<Action>,
) {
    let locale = state.settings.language;
    if visible.is_empty() {
        filtered_empty_state(
            ui,
            &state.palette,
            tr(locale, "No channels match this filter").as_ref(),
            tr(locale, "Clear filter").as_ref(),
            || actions.push(Action::SetSubscriptionsQuery(String::new())),
        );
        return;
    }
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            channel_grid_content(ui, state, visible, activity, removable, actions);
            if with_more {
                widgets::more_row(
                    ui,
                    &state.palette,
                    &state.subscriptions_more_state,
                    state.subscriptions_more.is_some(),
                    actions,
                    locale,
                );
            }
        });
}

#[cfg(test)]
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

/// Draw channels as a responsive card grid so callers can keep continuation controls in the same scroll area.
pub(super) fn channel_rows_content(
    ui: &mut Ui,
    state: &State,
    channels: &[Channel],
    removable: bool,
    actions: &mut Vec<Action>,
) {
    let activity = state.subscription_activity();
    let refs: Vec<&Channel> = channels.iter().collect();
    channel_grid_content(ui, state, &refs, &activity, removable, actions);
}

fn channel_grid_content(
    ui: &mut Ui,
    state: &State,
    channels: &[&Channel],
    activity: &HashMap<&str, ChannelActivity<'_>>,
    removable: bool,
    actions: &mut Vec<Action>,
) {
    let (columns, width) = widgets::grid_columns(
        ui.available_width(),
        CHANNEL_CARD_WIDTH,
        CHANNEL_GRID_GAP,
        4,
    );
    let selected_row = state.cursor_moved.then(|| state.cursor / columns);
    results::virtual_rows(
        ui,
        CHANNEL_CARD_HEIGHT + CHANNEL_GRID_GAP,
        channels.len().div_ceil(columns),
        selected_row,
        |ui, row| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = CHANNEL_GRID_GAP;
                let start = row * columns;
                let end = (start + columns).min(channels.len());
                for (offset, channel) in channels[start..end].iter().enumerate() {
                    let index = start + offset;
                    channel_card(
                        ui,
                        state,
                        channel,
                        state.channel_info.get(&channel.id),
                        activity
                            .get(channel.id.as_str())
                            .copied()
                            .unwrap_or_default(),
                        state.is_cursor(index),
                        removable,
                        width,
                        actions,
                    );
                }
            });
            ui.add_space(CHANNEL_GRID_GAP);
        },
    );
}

#[allow(
    clippy::too_many_arguments,
    reason = "Card renders pre-indexed channel metadata, activity and selection width without redundant lookups"
)]
fn channel_card(
    ui: &mut Ui,
    state: &State,
    channel: &Channel,
    meta: Option<&ChannelMeta>,
    activity: ChannelActivity<'_>,
    selected: bool,
    removable: bool,
    width: f32,
    actions: &mut Vec<Action>,
) {
    let palette = &state.palette;
    let locale = state.settings.language;
    let (rect, response) = ui.allocate_exact_size(vec2(width, CHANNEL_CARD_HEIGHT), Sense::click());
    if !ui.is_rect_visible(rect) {
        return;
    }

    let radius = CornerRadius::same(RADIUS + 4);
    ui.painter().rect_filled(rect, radius, palette.panel);
    widgets::row_fill_radius(ui, palette, rect, &response, selected, radius);
    let hover = widgets::hover_fade(ui, &response);
    let border = widgets::mix(palette.outline, palette.secondary, hover * 0.45);
    ui.painter().rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, border),
        egui::StrokeKind::Inside,
    );
    if selected && state.cursor_moved {
        response.clone().scroll_to_me(None);
    }

    let inner = rect.shrink(14.0);
    let avatar_rect = Rect::from_min_size(inner.min, vec2(48.0, 48.0));
    let avatar_url = channel
        .avatar
        .as_deref()
        .or_else(|| {
            activity
                .latest_video
                .and_then(|v| v.channel_avatar.as_deref())
        })
        .or_else(|| {
            activity
                .last_watched
                .and_then(|v| v.channel_avatar.as_deref())
        });
    widgets::avatar(ui, palette, avatar_url, &channel.name, avatar_rect);

    let right_limit = paint_card_right_slot(
        ui,
        state,
        channel,
        activity,
        inner,
        avatar_rect,
        removable,
        actions,
    );

    let text_left = avatar_rect.right() + 12.0;
    let text_right = (right_limit - 8.0).max(text_left + 40.0);
    let verified = meta.is_some_and(|m| m.verified);
    let title_budget = (text_right - text_left - if verified { 20.0 } else { 0.0 }).max(24.0);
    let name_galley = single_line_galley(
        ui,
        &channel.name,
        theme::semibold(15.0),
        palette.text,
        title_budget,
    );
    let name_width = name_galley.size().x;
    let name_height = name_galley.rect.height();
    ui.painter().galley(
        pos2(text_left, inner.top() + 2.0),
        name_galley,
        palette.text,
    );

    if verified {
        let badge_rect = Rect::from_min_size(
            pos2(
                text_left + name_width + 5.0,
                inner.top() + 2.0 + (name_height - 14.0) * 0.5,
            ),
            vec2(14.0, 14.0),
        );
        paint_verified_badge(
            ui,
            palette,
            response.id.with("verified"),
            badge_rect,
            locale,
        );
    }

    let subline = meta
        .and_then(|m| m.handle.as_deref())
        .map(str::to_owned)
        .unwrap_or_else(|| tr(locale, "YouTube channel").into_owned());
    let handle_galley = single_line_galley(
        ui,
        &subline,
        theme::regular(12.5),
        palette.secondary,
        (text_right - text_left).max(24.0),
    );
    ui.painter().galley(
        pos2(text_left, inner.top() + 26.0),
        handle_galley,
        palette.secondary,
    );

    let stats_y = avatar_rect.bottom() + 9.0;
    let stats_line = channel_stats_line(meta, activity, locale);
    let bio_y = if let Some(stats) = &stats_line {
        let galley = single_line_galley(
            ui,
            stats,
            theme::medium(12.5),
            palette.text.gamma_multiply(0.86),
            inner.width(),
        );
        ui.painter()
            .galley(pos2(inner.left(), stats_y), galley, palette.text);
        stats_y + 20.0
    } else {
        avatar_rect.bottom() + 10.0
    };

    let strip_rect = Rect::from_min_size(
        pos2(inner.left(), inner.bottom() - 52.0),
        vec2(inner.width(), 52.0),
    );
    if let Some(desc) = meta.and_then(|m| m.description.as_deref())
        && bio_y + 16.0 <= strip_rect.top() - 4.0
    {
        let mut job = LayoutJob::default();
        job.append(
            desc,
            0.0,
            egui::TextFormat::simple(theme::regular(12.5), palette.secondary),
        );
        job.wrap.max_width = inner.width();
        job.wrap.max_rows = 2;
        job.wrap.overflow_character = Some('…');
        let bio_galley = ui.painter().layout_job(job);
        ui.painter()
            .galley(pos2(inner.left(), bio_y), bio_galley, palette.secondary);
    }

    let preview = activity
        .latest_video
        .map(|video| (video, true))
        .or_else(|| activity.last_watched.map(|video| (video, false)));
    let preview_clicked =
        paint_card_preview_strip(ui, state, strip_rect, response.id, preview, actions);

    if response.clicked() && !preview_clicked {
        actions.push(Action::OpenChannel(channel.clone()));
    }
    channel_context_menu(&response, channel, meta, preview, actions, locale);
}

#[allow(
    clippy::too_many_arguments,
    reason = "Header right slot paints either the subscribe control or an activity pill and returns its left boundary"
)]
fn paint_card_right_slot(
    ui: &mut Ui,
    state: &State,
    channel: &Channel,
    activity: ChannelActivity<'_>,
    inner: Rect,
    avatar_rect: Rect,
    removable: bool,
    actions: &mut Vec<Action>,
) -> f32 {
    let palette = &state.palette;
    let locale = state.settings.language;
    if removable {
        let slot = Rect::from_min_max(
            pos2(avatar_rect.right() + 12.0, inner.top()),
            pos2(inner.right(), inner.top() + 32.0),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(slot)
                .layout(egui::Layout::right_to_left(egui::Align::Min)),
        );
        widgets::subscribe(
            &mut child,
            state,
            channel,
            widgets::ButtonStyle::Normal,
            actions,
        );
        return child.min_rect().left();
    }
    if activity.recent_count == 0 {
        return inner.right();
    }
    let is_live = activity.latest_video.is_some_and(|video| video.live);
    let label = if is_live {
        tr(locale, "Live").into_owned()
    } else {
        widgets::compact_count_label(
            activity.recent_count as u64,
            "{count} new",
            "{count} new",
            locale,
        )
    };
    let text_color = if is_live { palette.red } else { palette.blue };
    let galley = ui
        .painter()
        .layout_no_wrap(label, theme::semibold(11.5), text_color);
    let pad_x = 8.0;
    let pill_w = galley.size().x + 2.0 * pad_x + if is_live { 10.0 } else { 0.0 };
    let pill_h = 22.0;
    let pill_rect = Rect::from_min_size(
        pos2(inner.right() - pill_w, inner.top()),
        vec2(pill_w, pill_h),
    );
    let bg = if is_live {
        palette.red.gamma_multiply(0.14)
    } else {
        palette.blue.gamma_multiply(0.14)
    };
    ui.painter()
        .rect_filled(pill_rect, CornerRadius::same(11), bg);
    let mut text_x = pill_rect.left() + pad_x;
    if is_live {
        ui.painter()
            .circle_filled(pos2(text_x + 3.0, pill_rect.center().y), 3.0, palette.red);
        text_x += 10.0;
    }
    ui.painter().galley(
        pos2(text_x, pill_rect.center().y - galley.rect.height() * 0.5),
        galley,
        text_color,
    );
    pill_rect.left()
}

fn single_line_galley(
    ui: &Ui,
    text: &str,
    font: FontId,
    color: Color32,
    max_width: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = LayoutJob::default();
    job.append(text, 0.0, egui::TextFormat::simple(font, color));
    job.wrap.max_width = max_width.max(16.0);
    job.wrap.max_rows = 1;
    job.wrap.overflow_character = Some('…');
    ui.painter().layout_job(job)
}

fn paint_verified_badge(ui: &mut Ui, palette: &Palette, id: egui::Id, rect: Rect, locale: Locale) {
    let radius = rect.width() * 0.5;
    ui.painter()
        .circle_filled(rect.center(), radius, palette.blue.gamma_multiply(0.18));
    ui.painter().circle_stroke(
        rect.center(),
        radius,
        Stroke::new(1.0, palette.blue.gamma_multiply(0.45)),
    );
    Icon::Check
        .image(palette.blue, 9.0)
        .paint_at(ui, Rect::from_center_size(rect.center(), vec2(9.0, 9.0)));
    ui.interact(rect, id, Sense::hover())
        .on_hover_text(tr(locale, "Verified channel").as_ref());
}

fn channel_stats_line(
    meta: Option<&ChannelMeta>,
    activity: ChannelActivity<'_>,
    locale: Locale,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(subs) = meta.and_then(|m| m.subscribers) {
        parts.push(widgets::compact_count_label(
            subs,
            "{count} subscriber",
            "{count} subscribers",
            locale,
        ));
    }
    if let Some(videos) = meta.and_then(|m| m.videos) {
        parts.push(widgets::compact_count_label(
            videos,
            "{count} video",
            "{count} videos",
            locale,
        ));
    }
    if activity.watched_count > 0 {
        parts.push(widgets::compact_count_label(
            activity.watched_count as u64,
            "{count} watched",
            "{count} watched",
            locale,
        ));
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

fn paint_card_preview_strip(
    ui: &mut Ui,
    state: &State,
    strip_rect: Rect,
    card_id: egui::Id,
    preview: Option<(&Video, bool)>,
    actions: &mut Vec<Action>,
) -> bool {
    let palette = &state.palette;
    let locale = state.settings.language;
    let radius = CornerRadius::same(RADIUS);

    let Some((video, from_feed)) = preview else {
        ui.painter()
            .rect_filled(strip_rect, radius, palette.surface.gamma_multiply(0.75));
        ui.painter().rect_stroke(
            strip_rect,
            radius,
            Stroke::new(1.0, palette.outline.gamma_multiply(0.75)),
            egui::StrokeKind::Inside,
        );
        let prompt = single_line_galley(
            ui,
            tr(locale, "Open channel").as_ref(),
            theme::medium(12.5),
            palette.secondary,
            (strip_rect.width() - 42.0).max(20.0),
        );
        let label_y = strip_rect.center().y - prompt.rect.height() * 0.5;
        ui.painter().galley(
            pos2(strip_rect.left() + 12.0, label_y),
            prompt,
            palette.secondary,
        );
        Icon::ChevronRight.image(palette.secondary, 14.0).paint_at(
            ui,
            Rect::from_center_size(
                pos2(strip_rect.right() - 16.0, strip_rect.center().y),
                vec2(14.0, 14.0),
            ),
        );
        return false;
    };

    let strip_resp = ui.interact(strip_rect, card_id.with("preview"), Sense::click());
    if strip_resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let fill = widgets::mix(
        palette.surface,
        palette.surface_hover,
        widgets::hover_fade(ui, &strip_resp),
    );
    ui.painter().rect_filled(strip_rect, radius, fill);
    ui.painter().rect_stroke(
        strip_rect,
        radius,
        Stroke::new(1.0, palette.outline),
        egui::StrokeKind::Inside,
    );

    let thumb_rect = Rect::from_min_size(strip_rect.min + vec2(5.0, 5.0), vec2(74.0, 42.0));
    widgets::thumbnail(
        ui,
        palette,
        video,
        thumb_rect,
        widgets::watched(&state.library, video),
        locale,
    );

    let text_left = thumb_rect.right() + 10.0;
    let text_w = (strip_rect.right() - text_left - 10.0).max(20.0);
    let title_galley = single_line_galley(
        ui,
        &video.title,
        theme::semibold(12.5),
        palette.text,
        text_w,
    );
    ui.painter().galley(
        pos2(text_left, strip_rect.top() + 7.0),
        title_galley,
        palette.text,
    );

    let prefix = tr(locale, if from_feed { "Latest" } else { "Last watched" });
    let detail = widgets::meta(video, locale);
    let meta_text = if detail.is_empty() {
        prefix.into_owned()
    } else {
        format!("{prefix} · {detail}")
    };
    let meta_galley = single_line_galley(
        ui,
        &meta_text,
        theme::regular(11.5),
        palette.secondary,
        text_w,
    );
    ui.painter().galley(
        pos2(text_left, strip_rect.top() + 26.0),
        meta_galley,
        palette.secondary,
    );

    if strip_resp.clicked() {
        actions.push(Action::Open(video.clone()));
    }
    strip_resp.hovered().then(|| {
        strip_resp.clone().on_hover_text(&video.title);
    });
    strip_resp.clicked()
}

fn channel_context_menu(
    response: &egui::Response,
    channel: &Channel,
    meta: Option<&ChannelMeta>,
    preview: Option<(&Video, bool)>,
    actions: &mut Vec<Action>,
    locale: Locale,
) {
    let url = if channel.id.starts_with("UC") {
        format!("https://www.youtube.com/channel/{}", channel.id)
    } else if let Some(handle) = meta.and_then(|m| m.handle.as_deref()) {
        format!("https://www.youtube.com/{handle}")
    } else {
        format!("https://www.youtube.com/channel/{}", channel.id)
    };
    response.context_menu(|ui| {
        if ui.button(tr(locale, "Open channel").as_ref()).clicked() {
            actions.push(Action::OpenChannel(channel.clone()));
            ui.close();
        }
        if let Some((video, from_feed)) = preview {
            let label = if from_feed {
                tr(locale, "Open latest video")
            } else {
                tr(locale, "Last watched")
            };
            if ui.button(label.as_ref()).clicked() {
                actions.push(Action::Open(video.clone()));
                ui.close();
            }
        }
        ui.separator();
        if ui
            .button(tr(locale, "Copy channel link").as_ref())
            .clicked()
        {
            ui.ctx().copy_text(url.clone());
            ui.close();
        }
        if ui.button(tr(locale, "Open in browser").as_ref()).clicked() {
            actions.push(Action::OpenUrl(url.clone()));
            ui.close();
        }
    });
}

pub fn history(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;
    results::panel(ui, |ui| {
        let source = state.history_source();
        let account = source != Some(Source::Device);
        let cache = state.history_cache();
        results::title(
            ui,
            state,
            tr(locale, "History").as_ref(),
            Some(cache.entries.len()),
        );
        let in_progress = cache
            .entries
            .iter()
            .filter(|entry| {
                entry
                    .device
                    .and_then(|idx| state.library.history.get(idx))
                    .is_some_and(|w| w.position > 0.0)
            })
            .count();
        controls(ui, state, actions, |ui, actions| {
            let mut query = state.history_query.clone();
            let hint = if state.merged_history() || account {
                "Search history"
            } else {
                "Search this device's history"
            };
            widgets::search_field(
                ui,
                palette,
                &mut query,
                tr(locale, hint).as_ref(),
                locale,
                egui::Id::new("history search"),
                220.0_f32.min(ui.available_width()),
            );
            if query != state.history_query {
                actions.push(Action::SetHistoryQuery(query));
            }
            if source != Some(Source::Account)
                && !state.library.history.is_empty()
                && widgets::button(
                    ui,
                    palette,
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
            if in_progress > 0 {
                toolbar_summary(
                    ui,
                    palette,
                    &widgets::compact_count_label(
                        in_progress as u64,
                        "{count} in progress",
                        "{count} in progress",
                        locale,
                    ),
                );
            }
        });
        if account {
            match &state.account_history {
                Listing::Loading => results::loading(ui, state, "history"),
                Listing::Failed(error) => results::failed(ui, state, error, actions),
                _ => {}
            }
        }
        if cache.entries.is_empty()
            && (!account || matches!(state.account_history, Listing::Ready(_)))
        {
            if !state.history_query.trim().is_empty() {
                filtered_empty_state(
                    ui,
                    palette,
                    tr(locale, "No videos found in history").as_ref(),
                    tr(locale, "Clear filter").as_ref(),
                    || actions.push(Action::SetHistoryQuery(String::new())),
                );
            } else {
                let text = if source == Some(Source::Account) {
                    "The account's watch history"
                } else {
                    "Videos you watch appear here"
                };
                results::nothing(ui, state, tr(locale, text).as_ref());
            }
            return;
        }
        let now = util::now();
        // Rows are one height, so each day's run is drawn only where it shows.
        let show_remove = source != Some(Source::Account) || state.merged_history();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for (run_idx, run) in cache.runs.iter().enumerate() {
                    if run_idx > 0 {
                        ui.add_space(12.0);
                    }
                    egui::Frame::new()
                        .fill(palette.panel)
                        .corner_radius(CornerRadius::same(RADIUS + 4))
                        .stroke(Stroke::new(1.0, palette.outline))
                        .inner_margin(egui::Margin::symmetric(12, 10))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            if let Some(heading) = &run.heading {
                                ui.horizontal(|ui| {
                                    let (pip, _) =
                                        ui.allocate_exact_size(vec2(3.5, 16.0), Sense::hover());
                                    ui.painter().rect_filled(
                                        pip,
                                        CornerRadius::same(2),
                                        palette.accent,
                                    );
                                    ui.label(
                                        RichText::new(heading)
                                            .font(theme::bold(16.0))
                                            .color(palette.text),
                                    );
                                    ui.label(
                                        RichText::new(util::thousands(
                                            (run.end - run.start) as u64,
                                        ))
                                        .font(theme::regular(13.0))
                                        .color(palette.secondary),
                                    );
                                });
                                ui.add_space(8.0);
                            }
                            let selected = (state.cursor_moved
                                && (run.start..run.end).contains(&state.cursor))
                            .then(|| state.cursor - run.start);
                            results::virtual_rows(
                                ui,
                                HISTORY_ROW,
                                run.end - run.start,
                                selected,
                                |ui, row| {
                                    let index = run.start + row;
                                    let row = state.history_row(&cache.entries[index]);
                                    history_row(ui, state, &row, index, show_remove, now, actions);
                                },
                            );
                        });
                }
                if account {
                    ui.add_space(12.0);
                    widgets::more_row(
                        ui,
                        palette,
                        &state.history_more_state,
                        state.history_more.is_some(),
                        actions,
                        locale,
                    );
                }
            });
    });
}

/// A history row's height.
const HISTORY_ROW: f32 = 106.0;

/// One History row: the video, how long ago it was watched here, and the
/// button that forgets it.
fn history_row(
    ui: &mut Ui,
    state: &State,
    row: &youtfast_logic::history::Row<'_>,
    index: usize,
    controls: bool,
    now: i64,
    actions: &mut Vec<Action>,
) {
    let locale = state.settings.language;
    let note = row.device.map(|watched| {
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
        note
    });
    let video = row.video;
    let fraction = row
        .device
        .and_then(|watched| {
            video
                .duration
                .map(|duration| (watched.position / duration) as f32)
        })
        .or_else(|| widgets::watched(&state.library, video));
    let selected = state.is_cursor(index);
    ui.push_id(&video.id, |ui| {
        ui.horizontal(|ui| {
            // Reserve the same trailing control space for every merged row.
            let reserve = if controls {
                32.0 + ui.spacing().item_spacing.x
            } else {
                0.0
            };
            let width = (ui.available_width() - reserve).max(0.0);
            ui.allocate_ui_with_layout(
                vec2(width, HISTORY_ROW),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    let response = widgets::video_row(
                        ui,
                        &state.palette,
                        video,
                        note.as_deref(),
                        fraction,
                        selected,
                        locale,
                    );
                    widgets::video_interactions(ui, &response, state, video, actions);
                },
            );
            if row.device.is_some() {
                if widgets::icon_button(
                    ui,
                    youtfast_appearance::icons::Icon::Close,
                    16.0,
                    tr(locale, "Remove from this device's history").as_ref(),
                )
                .clicked()
                {
                    actions.push(Action::RemoveHistory(video.id.clone()));
                }
            } else if controls {
                ui.allocate_space(vec2(32.0, 32.0));
            }
        });
    });
}

pub fn watch_later(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    results::panel(ui, |ui| {
        let total = state.watch_later_list().len();
        let visible = state.visible_watch_later();
        results::title(ui, state, tr(locale, "Watch later").as_ref(), Some(total));
        controls(ui, state, actions, |ui, actions| {
            library_video_toolbar(
                ui,
                state,
                &state.watch_later_query,
                "watch_later search",
                &visible,
                Action::SetWatchLaterQuery,
                actions,
            );
        });
        match state.source() {
            Source::Account => match &state.account_watch_later {
                Listing::Idle => {}
                Listing::Loading => results::loading(ui, state, "videos"),
                Listing::Failed(error) => results::failed(ui, state, error, actions),
                Listing::Ready(videos) if videos.is_empty() => {
                    results::nothing(ui, state, tr(locale, "The account's Watch later").as_ref())
                }
                Listing::Ready(_) => video_card_grid(
                    ui,
                    state,
                    &visible,
                    !state.watch_later_query.trim().is_empty(),
                    Action::SetWatchLaterQuery(String::new()),
                    Some((
                        &state.watch_later_more_state,
                        state.watch_later_more.is_some(),
                    )),
                    actions,
                ),
            },
            Source::Device if state.library.watch_later.is_empty() => results::nothing(
                ui,
                state,
                tr(locale, "Save videos from their page").as_ref(),
            ),
            Source::Device => video_card_grid(
                ui,
                state,
                &visible,
                !state.watch_later_query.trim().is_empty(),
                Action::SetWatchLaterQuery(String::new()),
                None,
                actions,
            ),
        }
    });
}

pub fn liked(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    results::panel(ui, |ui| {
        let total = state.liked.items().len();
        let visible = state.visible_liked();
        results::title(
            ui,
            state,
            tr(locale, "Videos you liked").as_ref(),
            Some(total),
        );
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            library_video_toolbar(
                ui,
                state,
                &state.liked_query,
                "liked search",
                &visible,
                Action::SetLikedQuery,
                actions,
            );
        });
        ui.add_space(12.0);
        match &state.liked {
            Listing::Idle => {}
            Listing::Loading => results::loading(ui, state, "videos"),
            Listing::Failed(error) => results::failed(ui, state, error, actions),
            Listing::Ready(videos) if videos.is_empty() => {
                results::nothing(ui, state, tr(locale, "The account's liked videos").as_ref())
            }
            Listing::Ready(_) => video_card_grid(
                ui,
                state,
                &visible,
                !state.liked_query.trim().is_empty(),
                Action::SetLikedQuery(String::new()),
                Some((&state.liked_more_state, state.liked_more.is_some())),
                actions,
            ),
        }
    });
}

pub fn playlists(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;

    results::panel(ui, |ui| {
        let total = state.playlists.items().len();
        let visible = state.visible_playlists();
        results::title(ui, state, tr(locale, "Playlists").as_ref(), Some(total));
        if total > 0 || !state.playlists_query.trim().is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                let mut query = state.playlists_query.clone();
                widgets::search_field(
                    ui,
                    palette,
                    &mut query,
                    tr(locale, "Filter playlists").as_ref(),
                    locale,
                    egui::Id::new("playlists search"),
                    220.0_f32.min(ui.available_width()),
                );
                if query != state.playlists_query {
                    actions.push(Action::SetPlaylistsQuery(query));
                }
                let summary = if !state.playlists_query.trim().is_empty() {
                    Some(widgets::compact_count_label(
                        visible.len() as u64,
                        "{count} matching",
                        "{count} matching",
                        locale,
                    ))
                } else {
                    let sum_videos: u64 = visible.iter().filter_map(|p| p.videos).sum();
                    (sum_videos > 0).then(|| widgets::video_count_label(sum_videos, locale))
                };
                if let Some(summary) = summary {
                    toolbar_summary(ui, palette, &summary);
                }
            });
            ui.add_space(12.0);
        }
        match &state.playlists {
            Listing::Idle => {}
            Listing::Loading => results::loading(ui, state, "playlists"),
            Listing::Failed(error) => results::failed(ui, state, error, actions),
            Listing::Ready(playlists) if playlists.is_empty() => {
                results::nothing(ui, state, tr(locale, "The account's playlists").as_ref())
            }
            Listing::Ready(_) if visible.is_empty() => filtered_empty_state(
                ui,
                palette,
                tr(locale, "No playlists found").as_ref(),
                tr(locale, "Clear filter").as_ref(),
                || actions.push(Action::SetPlaylistsQuery(String::new())),
            ),
            Listing::Ready(_) => {
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        let gap = 16.0;
                        let (columns, width) =
                            widgets::grid_columns(ui.available_width(), CARD_WIDTH, gap, 5);
                        results::virtual_rows(
                            ui,
                            widgets::playlist_card_height(ui, width) + gap,
                            visible.len().div_ceil(columns),
                            state.cursor_moved.then_some(state.cursor / columns),
                            |ui, row| {
                                ui.horizontal_top(|ui| {
                                    ui.spacing_mut().item_spacing.x = gap;
                                    let start = row * columns;
                                    let end = (start + columns).min(visible.len());
                                    for (offset, playlist) in
                                        visible[start..end].iter().copied().enumerate()
                                    {
                                        let index = start + offset;
                                        let selected = state.is_cursor(index);
                                        let response = widgets::playlist_card(
                                            ui, palette, playlist, selected, width, locale,
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
                                    }
                                });
                                ui.add_space(gap);
                            },
                        );
                        widgets::more_row(
                            ui,
                            palette,
                            &state.playlists_more_state,
                            state.playlists_more.is_some(),
                            actions,
                            locale,
                        );
                    });
            }
        }
    });
}

fn library_video_toolbar(
    ui: &mut Ui,
    state: &State,
    current_query: &str,
    search_salt: &'static str,
    visible: &[&Video],
    set_query: impl FnOnce(String) -> Action,
    actions: &mut Vec<Action>,
) {
    let locale = state.settings.language;
    let palette = &state.palette;
    let mut query = current_query.to_owned();
    widgets::search_field(
        ui,
        palette,
        &mut query,
        tr(locale, "Filter videos").as_ref(),
        locale,
        egui::Id::new(search_salt),
        220.0_f32.min(ui.available_width()),
    );
    if query != current_query {
        actions.push(set_query(query));
    }
    if let Some(first) = visible.first()
        && widgets::button(
            ui,
            palette,
            tr(locale, "Play all").as_ref(),
            widgets::ButtonStyle::Primary,
        )
        .clicked()
    {
        actions.push(Action::Open((*first).clone()));
    }
    let total_secs: f64 = visible.iter().filter_map(|v| v.duration).sum();
    let count_text = if !current_query.trim().is_empty() {
        Some(widgets::compact_count_label(
            visible.len() as u64,
            "{count} matching",
            "{count} matching",
            locale,
        ))
    } else if !visible.is_empty() && total_secs > 0.0 {
        Some(widgets::video_count_label(visible.len() as u64, locale))
    } else {
        None
    };
    if let Some(mut summary) = count_text {
        if total_secs >= 1.0 && !visible.is_empty() {
            summary.push_str(" · ");
            summary.push_str(&util::duration(total_secs));
        }
        toolbar_summary(ui, palette, &summary);
    }
}

/// Renders a toolbar summary count as an unbreakable label so `horizontal_wrapped`
/// moves the whole phrase to the next row instead of splitting mid-metric.
fn toolbar_summary(ui: &mut Ui, palette: &Palette, text: &str) {
    ui.add(
        egui::Label::new(
            RichText::new(text)
                .font(theme::regular(13.0))
                .color(palette.secondary),
        )
        .wrap_mode(egui::TextWrapMode::Extend),
    );
}

fn video_card_grid(
    ui: &mut Ui,
    state: &State,
    videos: &[&Video],
    filtering: bool,
    clear_action: Action,
    more: Option<(&Fetch<()>, bool)>,
    actions: &mut Vec<Action>,
) {
    let locale = state.settings.language;
    let palette = &state.palette;
    if videos.is_empty() && filtering {
        filtered_empty_state(
            ui,
            palette,
            tr(locale, "No video matches this filter").as_ref(),
            tr(locale, "Clear filter").as_ref(),
            || actions.push(clear_action),
        );
        return;
    }
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            let gap = 16.0;
            let (columns, width) = widgets::grid_columns(ui.available_width(), CARD_WIDTH, gap, 5);
            results::virtual_rows(
                ui,
                widgets::video_card_height(ui, width) + gap,
                videos.len().div_ceil(columns),
                state.cursor_moved.then_some(state.cursor / columns),
                |ui, row| {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        let start = row * columns;
                        let end = (start + columns).min(videos.len());
                        for (offset, video) in videos[start..end].iter().copied().enumerate() {
                            let index = start + offset;
                            let selected = state.is_cursor(index);
                            let watched = widgets::watched(&state.library, video);
                            let response = widgets::video_card(
                                ui, palette, video, watched, selected, width, locale,
                            );
                            if selected && state.cursor_moved {
                                response.clone().scroll_to_me(None);
                            }
                            widgets::video_interactions(ui, &response, state, video, actions);
                        }
                    });
                    ui.add_space(gap);
                },
            );
            if let Some((more_state, has_more)) = more {
                widgets::more_row(ui, palette, more_state, has_more, actions, locale);
            }
        });
}

fn filtered_empty_state(
    ui: &mut Ui,
    palette: &Palette,
    message: &str,
    clear_label: &str,
    on_clear: impl FnOnce(),
) {
    results::centered(ui, |ui| {
        ui.label(
            RichText::new(message)
                .font(theme::regular(14.0))
                .color(palette.secondary),
        );
        ui.add_space(8.0);
        if widgets::button(ui, palette, clear_label, widgets::ButtonStyle::Normal).clicked() {
            on_clear();
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
                    youtfast_app::images::remote(ui, url)
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
                                    state.is_cursor(index) || playing,
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
    fn merged_history_controls_fit_narrow_and_wide_dark_and_light_pages() {
        use youtfast_core::model::HistoryRow;
        use youtfast_storage::library::Watched;
        use youtfast_storage::settings::HistorySync;
        for width in [360.0, 900.0] {
            for palette in [theme::Palette::dark(), theme::Palette::light()] {
                let ctx = egui::Context::default();
                ctx.enable_accesskit();
                youtfast_appearance::theme::install_fonts(&ctx);
                let mut state = State {
                    view: View::History,
                    palette,
                    account: AccountState::SignedIn(1),
                    account_history: Listing::Ready(vec![HistoryRow {
                        video: Video::from_id("account-only".into()),
                        when: Some("Today".into()),
                    }]),
                    ..Default::default()
                };
                state.settings.history_sync = HistorySync::Merged;
                state.library.history = vec![Watched {
                    video: Video::from_id("device-only".into()),
                    at: 0,
                    position: 0.0,
                }];
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            egui::Pos2::ZERO,
                            vec2(width, 900.0),
                        )),
                        ..Default::default()
                    },
                    |ui| history(ui, &state, &mut Vec::new()),
                );
                output.textures_delta.clear();
                for label in ["All", "Account", "This device"] {
                    let text = output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Text(text) if text.galley.text() == label => Some(text),
                            _ => None,
                        })
                        .unwrap_or_else(|| panic!("missing {label}"));
                    let rect =
                        Rect::from_min_size(text.pos, text.galley.size()).expand2(vec2(12.0, 0.0));
                    assert!(
                        rect.left() >= 0.0 && rect.right() <= width,
                        "{width}: {label}: {rect:?}"
                    );
                }
                let tree = output.platform_output.accesskit_update.unwrap();
                for label in ["Clear history", "Remove from this device's history"] {
                    let (_, node) = tree
                        .nodes
                        .iter()
                        .find(|(_, node)| node.label() == Some(label))
                        .unwrap_or_else(|| panic!("missing {label}"));
                    let rect = node.bounds().unwrap();
                    assert!(
                        rect.x0 >= 0.0 && rect.x1 <= f64::from(width),
                        "{width}: {label}: {rect:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn merged_history_only_offers_local_removal_and_keeps_search_and_keyboard_in_step() {
        use youtfast_core::model::HistoryRow;
        use youtfast_logic::history::HistoryFilter;
        use youtfast_storage::library::Watched;
        use youtfast_storage::settings::HistorySync;
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State {
            view: View::History,
            account: AccountState::SignedIn(1),
            account_history: Listing::Ready(vec![
                HistoryRow {
                    video: Video::from_id("shared".into()),
                    when: Some("Today".into()),
                },
                HistoryRow {
                    video: Video::from_id("account-only".into()),
                    when: Some("Today".into()),
                },
            ]),
            ..Default::default()
        };
        state.settings.history_sync = HistorySync::Merged;
        let mut local = Video::from_id("shared".into());
        local.title = "Local title".into();
        local.duration = Some(100.0);
        state.library.history = vec![Watched {
            video: local,
            at: 0,
            position: 42.0,
        }];
        assert_eq!(state.history_source(), None);
        assert_eq!(state.source(), Source::Account);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(960.0, 900.0))),
                ..Default::default()
            },
            |ui| history(ui, &state, &mut Vec::new()),
        );
        output.textures_delta.clear();
        let tree = output.platform_output.accesskit_update.unwrap();
        assert_eq!(
            tree.nodes
                .iter()
                .filter(|(_, node)| node.label() == Some("Remove from this device's history"))
                .count(),
            1
        );
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().contains("left at 0:42"))));
        state.history_query = "local".into();
        assert_eq!(super::super::cursor_count(&state), 1);
        assert!(
            matches!(super::super::open_at(&state, 0), Some(Action::Open(video)) if video.id == "shared")
        );
        state.history_filter = HistoryFilter::Device;
        assert_eq!(state.history_source(), Some(Source::Device));
        assert_eq!(state.source(), Source::Account);
        state.account = AccountState::SignedOut;
        state.history_filter = HistoryFilter::All;
        assert_eq!(state.history_source(), Some(Source::Device));
        assert_eq!(state.history_rows().len(), 1);
    }

    #[test]
    fn history_remove_buttons_stay_aligned_and_inside_the_page() {
        use youtfast_storage::library::Watched;
        for width in [480.0, 960.0, 1280.0] {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            youtfast_appearance::theme::install_fonts(&ctx);
            let mut state = State::default();
            state.library.history = (0..5)
                .map(|index| Watched {
                    video: Video::from_id(format!("history{index:04}")),
                    at: 0,
                    position: 0.0,
                })
                .collect();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(width, 900.0))),
                    ..Default::default()
                },
                |ui| history(ui, &state, &mut Vec::new()),
            );
            output.textures_delta.clear();
            let tree = output.platform_output.accesskit_update.unwrap();
            let buttons: Vec<_> = tree
                .nodes
                .iter()
                .filter(|(_, node)| node.label() == Some("Remove from this device's history"))
                .map(|(_, node)| node.bounds().unwrap())
                .collect();
            assert_eq!(buttons.len(), 5);
            for button in &buttons {
                assert!((button.x0 - buttons[0].x0).abs() < 0.1, "{buttons:?}");
                assert!(button.x1 <= f64::from(width), "{button:?}");
                assert_eq!(button.width(), 32.0);
            }
        }
    }

    #[test]
    fn empty_account_subscriptions_and_watch_later_use_the_explicit_locale() {
        use youtfast_i18n::Locale;
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
                youtfast_appearance::theme::install_fonts(&ctx);
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
            youtfast_appearance::theme::install_fonts(&ctx);
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
                        .map(|index| youtfast_core::model::PlaylistSummary {
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
            let drawn = youtfast_app::images::drawn_count(&ctx);
            assert!(drawn > 0 && drawn < 20, "list {kind} drew {drawn} images");
        }
    }

    #[test]
    fn the_history_rows_are_rebuilt_only_when_what_they_come_from_changes() {
        use youtfast_core::model::HistoryRow;
        use youtfast_logic::history::HistoryFilter;
        use youtfast_storage::library::Watched;
        use youtfast_storage::settings::HistorySync;
        let watched = |id: &str, title: &str, at: i64| {
            let mut video = Video::from_id(id.into());
            video.title = title.into();
            Watched {
                video,
                at,
                position: 0.0,
            }
        };
        let mut state = State {
            account: AccountState::SignedIn(1),
            account_history: Listing::Ready(vec![HistoryRow {
                video: Video::from_id("account-only".into()),
                when: Some("Today".into()),
            }]),
            history_requested_at: Some(1_000_000),
            ..Default::default()
        };
        state.settings.history_sync = HistorySync::Merged;
        state
            .library
            .record_watch(watched("a", "Alpha", 1).video, 1);
        let built = |state: &State| state.history_cache().entries.as_ptr();
        let first = built(&state);
        assert_eq!(state.history_rows().len(), 2);
        // Nothing changed: the same rows, not rebuilt.
        assert_eq!(built(&state), first);
        // A search rebuilds, with the same rows lowercased once.
        state.history_query = " ALPHA ".into();
        let ids = |state: &State| {
            state
                .history_rows()
                .iter()
                .map(|row| row.video.id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&state), ["a"]);
        state.history_query.clear();
        assert_eq!(state.history_rows().len(), 2);
        // The device history changing (a watch, a removal, a clear).
        state.library.record_watch(watched("b", "Beta", 2).video, 2);
        assert_eq!(state.history_rows().len(), 3);
        state.library.remove_history("a");
        assert_eq!(state.history_rows().len(), 2);
        state.library.clear_history();
        assert_eq!(state.history_rows().len(), 1);
        // A retitled video is searched by its new title.
        state
            .library
            .record_watch(watched("c", "Gamma", 3).video, 3);
        let mut renamed = Video::from_id("c".into());
        renamed.title = "Delta".into();
        state.library.update_video(&renamed);
        state.history_query = "delta".into();
        assert_eq!(ids(&state), ["c"]);
        state.history_query.clear();
        // The account's list changing, and the filter.
        state.account_history_generation += 1;
        state.account_history.items_mut().push(HistoryRow {
            video: Video::from_id("later".into()),
            when: Some("Today".into()),
        });
        assert_eq!(state.history_rows().len(), 3);
        state.history_filter = HistoryFilter::Device;
        assert_eq!(ids(&state), ["c"]);
        // The day moving on: sections of a request time are fixed, but with
        // none the key follows the clock's day.
        state.history_filter = HistoryFilter::All;
        state.history_requested_at = None;
        let key_day = state.history_cache().entries.len();
        assert_eq!(key_day, 3);
    }

    #[test]
    fn subscriptions_cards_display_channel_meta_bio_and_clickable_latest_video() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State {
            view: View::Subscriptions,
            account: AccountState::SignedIn(1),
            source: Source::Account,
            account_subscriptions: Listing::Ready(vec![Channel {
                id: "UC_comp".into(),
                name: "Computerphile".into(),
                avatar: None,
            }]),
            feed: Listing::Ready(vec![Video {
                title: "Inside Modern Compilers".into(),
                channel: Some("Computerphile".into()),
                channel_id: Some("UC_comp".into()),
                views: Some(85_000),
                published: Some("3 hours ago".into()),
                ..Video::from_id("vid_latest1".into())
            }]),
            ..State::default()
        };
        state.merge_channel_meta(
            "UC_comp".into(),
            ChannelMeta {
                handle: Some("@Computerphile".into()),
                subscribers: Some(2_640_000),
                videos: Some(850),
                description: Some("Videos all about computers and software.".into()),
                verified: true,
            },
        );

        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200.0, 800.0))),
                ..Default::default()
            },
            |ui| subscriptions(ui, &state, &mut Vec::new()),
        );
        let mut texts: Vec<String> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect();
        output.textures_delta.clear();
        texts.sort();
        assert!(texts.iter().any(|t| t == "Computerphile"));
        assert!(texts.iter().any(|t| t == "@Computerphile"));
        assert!(texts.iter().any(|t| t.contains("2.6M subscribers")));
        assert!(texts.iter().any(|t| t.contains("850 videos")));
        assert!(
            texts
                .iter()
                .any(|t| t == "Videos all about computers and software.")
        );
        assert!(texts.iter().any(|t| t == "Inside Modern Compilers"));
        assert!(texts.iter().any(|t| t.starts_with("Latest ·")));
    }

    #[test]
    fn shorts_shelf_wraps_titles_clear_of_more_button_and_renders_metadata() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let state = State {
            shorts: Listing::Ready(vec![Video {
                title: "A remarkably long Short title that must wrap well before the More button"
                    .into(),
                channel: Some("Demo channel".into()),
                channel_id: Some("UCsynthetic".into()),
                views: Some(411_000),
                short: true,
                ..Video::from_id("short000001".into())
            }]),
            ..State::default()
        };

        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(900.0, 600.0))),
                ..Default::default()
            },
            |ui| shorts_shelf(ui, &state, &mut Vec::new()),
        );
        let title_shape = output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text().starts_with("A remarkably long") => {
                Some((text.pos.x, text.galley.size().x, text.galley.rows.len()))
            }
            _ => None,
        });
        let (title_x, title_w, rows) = title_shape.expect("short title rendered");
        // Card padding + 144px thumb leaves 114px for wrapped text before the right-edge More button.
        assert!(title_w <= 114.5, "title width {title_w} encroaches on More");
        assert!(title_x >= 0.0);
        assert_eq!(rows, 2);

        let texts: Vec<String> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect();
        output.textures_delta.clear();
        assert!(texts.iter().any(|t| t == "Shorts"));
        assert!(texts.iter().any(|t| t == "Demo channel"));
        assert!(texts.iter().any(|t| t == "411K views"));
    }

    #[test]
    fn library_views_render_filter_play_all_and_playlist_cards() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let videos = vec![
            Video {
                title: "Rust Async Internals".into(),
                channel: Some("Systems Lab".into()),
                duration: Some(600.0),
                views: Some(25_000),
                ..Video::from_id("lib00000001".into())
            },
            Video {
                title: "SQL Tuning Guide".into(),
                channel: Some("Postgres TV".into()),
                duration: Some(900.0),
                views: Some(42_000),
                ..Video::from_id("lib00000002".into())
            },
        ];
        let mut state = State {
            account: AccountState::SignedIn(1),
            source: Source::Account,
            account_watch_later: Listing::Ready(videos.clone()),
            liked: Listing::Ready(videos),
            playlists: Listing::Ready(vec![youtfast_core::model::PlaylistSummary {
                id: "PL_demo".into(),
                name: "Deep Systems Architecture".into(),
                thumbnail: None,
                channel: Some("Fabio Akita".into()),
                videos: Some(14),
            }]),
            ..State::default()
        };

        type ViewDrawer = fn(&mut Ui, &State, &mut Vec<Action>);
        let views: [(View, ViewDrawer); 3] = [
            (View::WatchLater, watch_later),
            (View::Liked, liked),
            (View::Playlists, playlists),
        ];
        for (view, draw) in views {
            state.view = view;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 800.0))),
                    ..Default::default()
                },
                |ui| draw(ui, &state, &mut Vec::new()),
            );
            let texts: Vec<String> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                    _ => None,
                })
                .collect();
            output.textures_delta.clear();
            if view == View::Playlists {
                assert!(texts.iter().any(|t| t == "Deep Systems Architecture"));
                assert!(texts.iter().any(|t| t == "14 videos"));
            } else {
                assert!(texts.iter().any(|t| t == "Play all"));
                assert!(texts.iter().any(|t| t.contains("25:00")));
                assert!(texts.iter().any(|t| t == "Rust Async Internals"));
            }
        }
    }
}
