//! The Home page: subscription feed grid, For you recommendations, and Shorts shelf.

use egui::text::LayoutJob;
use egui::{Color32, CornerRadius, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use youtfast_i18n::{format as trf, gettext as tr};

use super::CARD_WIDTH;
use crate::{notifications, results, widgets};
use youtfast_app::{AccountState, Action, State};
use youtfast_appearance::icons::Icon;
use youtfast_appearance::theme::{self, RADIUS};
use youtfast_core::model::{FeedFilter, Fetch, Listing, Video, View};
use youtfast_core::util;
use youtfast_storage::settings::{
    RecommendationChannels as Channels, RecommendationDuration as Duration,
    RecommendationPriority as Priority, RecommendationTopic, Recommendations,
};

pub fn home(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    results::panel(ui, |ui| {
        // With a history, For you has something to offer already.
        let follows_nothing = state.library.subscriptions.is_empty()
            && state.library.history.is_empty()
            && !state.signed_in()
            && state.account != AccountState::SigningIn;
        if follows_nothing && state.feed_filter != FeedFilter::ForYou {
            feed_chips(ui, state, actions);
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
        if widgets::toggle_chip(
            ui,
            &state.palette,
            tr(locale, "Hide watched").as_ref(),
            state.settings.hide_watched,
        )
        .clicked()
        {
            actions.push(Action::SetHideWatched(!state.settings.hide_watched));
        }
    });
    ui.add_space(16.0);
}

/// The feed as YouTube's grid of cards, day-grouped, capped at
/// `feed_shown`, under the chips and the shorts shelf; the last card in
/// view asks for more.
pub(super) fn feed_grid(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
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
                        let heading = notifications::capitalized(day, locale);
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
    recommendation_controls(ui, state, actions);
    match &state.for_you {
        Listing::Idle => {}
        Listing::Loading => results::loading(ui, state, "recommendations"),
        Listing::Failed(error) => results::failed(ui, state, error, actions),
        Listing::Ready(videos) if videos.is_empty() => results::nothing(
            ui,
            state,
            tr(
                locale,
                if !state.has_local_for_you_inputs() {
                    "Watch a few videos or select topics to get recommendations"
                } else {
                    "No video matches this filter"
                },
            )
            .as_ref(),
        ),
        Listing::Ready(_) => {
            let source = if state.for_you_from_account {
                "Recommended by YouTube for your account"
            } else {
                "Ranked on this device"
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

fn recommendation_controls(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let saved = &state.settings.recommendations;
    egui::CollapsingHeader::new(tr(locale, "Local recommendation settings"))
        .id_salt("local recommendation settings")
        .show(ui, |ui| {
            if state.signed_in() {
                let mut prefer_local = saved.prefer_local;
                if ui
                    .checkbox(
                        &mut prefer_local,
                        tr(
                            locale,
                            "Use local ranking instead of YouTube recommendations",
                        ),
                    )
                    .changed()
                {
                    let mut next = saved.clone();
                    next.prefer_local = prefer_local;
                    actions.push(Action::SetRecommendations(next));
                }
            }
            let local_fallback =
                matches!(state.for_you, Listing::Ready(_)) && !state.for_you_from_account;
            let local_active = !state.signed_in() || saved.prefer_local || local_fallback;
            let text_id = ui.id().with("recommendation text draft");
            let saved_text = (saved.interests.clone(), saved.muted.clone());
            let (mut interests_draft, mut muted_draft) = ui
                .data_mut(|data| data.get_temp::<((String, String), (String, String))>(text_id))
                .filter(|(last_saved, _)| last_saved == &saved_text)
                .map_or_else(|| saved_text.clone(), |(_, draft)| draft);
            let mut reset_draft = false;

            ui.add_enabled_ui(local_active, |ui| {
                ui.label(
                    RichText::new(tr(locale, "Topics").as_ref())
                        .font(theme::medium(13.0))
                        .color(state.palette.secondary),
                );
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    for &topic in RecommendationTopic::ALL {
                        let selected = saved.topics.contains(&topic);
                        if widgets::toggle_chip(
                            ui,
                            &state.palette,
                            tr(locale, topic.label()).as_ref(),
                            selected,
                        )
                        .clicked()
                        {
                            let mut next = saved.clone();
                            next.toggle_topic(topic);
                            actions.push(Action::SetRecommendations(next));
                        }
                    }
                });
                if saved.topics.len() > Recommendations::MAX_PRESET_TOPIC_QUERIES {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(
                            tr(
                                locale,
                                "The first four selected topics fetch videos from YouTube; additional topics boost matching titles locally.",
                            )
                            .as_ref(),
                        )
                        .font(theme::regular(12.0))
                        .color(state.palette.secondary),
                    );
                }
                ui.add_space(8.0);

                let mut next_choices = saved.clone();
                let mut choices_changed = false;
                ui.horizontal_wrapped(|ui| {
                    choices_changed |= recommendation_choice(
                        ui,
                        locale,
                        "Duration",
                        &mut next_choices.duration,
                        &[
                            (Duration::Any, "Any duration"),
                            (Duration::Short, "Under 4 minutes"),
                            (Duration::Medium, "4–20 minutes"),
                            (Duration::Long, "Over 20 minutes"),
                        ],
                    );
                    choices_changed |= recommendation_choice(
                        ui,
                        locale,
                        "Channels",
                        &mut next_choices.channels,
                        &[
                            (Channels::Balanced, "Balanced"),
                            (Channels::Discover, "Discover new channels"),
                            (Channels::Familiar, "Favor familiar channels"),
                        ],
                    );
                    choices_changed |= recommendation_choice(
                        ui,
                        locale,
                        "Ranking priority",
                        &mut next_choices.priority,
                        &[
                            (Priority::Balanced, "Balanced"),
                            (Priority::Recent, "Recent uploads"),
                            (Priority::FewerViews, "Fewer views"),
                        ],
                    );
                });
                if choices_changed {
                    actions.push(Action::SetRecommendations(next_choices));
                }
                ui.label(
                    RichText::new(
                        tr(locale, "Select a topic to fetch new videos that match this duration. History-based results are only filtered.").as_ref(),
                    )
                    .font(theme::regular(12.0))
                    .color(state.palette.secondary),
                );
                ui.add_space(4.0);

                let has_custom_text =
                    !saved.interests.trim().is_empty() || !saved.muted.trim().is_empty();
                egui::CollapsingHeader::new(tr(locale, "Custom interests & muted terms"))
                    .id_salt("recommendation custom filters")
                    .default_open(has_custom_text)
                    .show(ui, |ui| {
                        let mut submit_text = false;
                        for (label, value) in [
                            ("Custom interests (comma-separated)", &mut interests_draft),
                            ("Muted title terms (comma-separated)", &mut muted_draft),
                        ] {
                            let label = ui.label(tr(locale, label));
                            let response = ui
                                .add(
                                    egui::TextEdit::singleline(value)
                                        .char_limit(Recommendations::MAX_TEXT_CHARS),
                                )
                                .labelled_by(label.id);
                            if response.lost_focus()
                                && ui.input(|input| input.key_pressed(egui::Key::Enter))
                            {
                                submit_text = true;
                            }
                        }
                        let mut next = saved.clone();
                        let search_without_history = ui
                            .checkbox(
                                &mut next.search_interests,
                                tr(locale, "Search YouTube for custom interests without watch history (signed out)"),
                            )
                            .changed();
                        let search_with_history = ui
                            .checkbox(
                                &mut next.search_interests_with_history,
                                tr(locale, "Search YouTube for custom interests with watch history (signed out)"),
                            )
                            .changed();
                        let text_dirty =
                            interests_draft != saved.interests || muted_draft != saved.muted;
                        let apply_clicked = ui
                            .add_enabled(text_dirty, egui::Button::new(tr(locale, "Apply")))
                            .clicked();
                        if search_without_history
                            || search_with_history
                            || (submit_text && text_dirty)
                            || apply_clicked
                        {
                            next.interests = interests_draft.clone();
                            next.muted = muted_draft.clone();
                            actions.push(Action::SetRecommendations(next));
                        }
                    });
            });

            let defaults = Recommendations::default();
            let text_dirty = interests_draft != saved.interests || muted_draft != saved.muted;
            ui.add_space(4.0);
            if ui
                .add_enabled(
                    *saved != defaults || text_dirty,
                    egui::Button::new(tr(locale, "Reset to defaults")),
                )
                .clicked()
            {
                reset_draft = true;
                actions.push(Action::SetRecommendations(defaults));
            }
            if reset_draft {
                interests_draft.clear();
                muted_draft.clear();
            }
            ui.data_mut(|data| {
                data.insert_temp(text_id, (saved_text, (interests_draft, muted_draft)));
            });
        });
    ui.add_space(8.0);
}

fn recommendation_choice<T: Copy + PartialEq>(
    ui: &mut Ui,
    locale: youtfast_i18n::Locale,
    label: &'static str,
    current: &mut T,
    choices: &[(T, &'static str)],
) -> bool {
    let before = *current;
    let selected = choices
        .iter()
        .find(|(value, _)| value == current)
        .map_or("", |(_, label)| *label);
    egui::ComboBox::new(label, tr(locale, label))
        .selected_text(tr(locale, selected))
        .show_ui(ui, |ui| {
            for &(value, label) in choices {
                ui.selectable_value(current, value, tr(locale, label));
            }
        });
    *current != before
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
    let response = widgets::video_card(ui, state, video, watched, selected, width, locale);
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
    let shorts: Vec<&Video> = shorts
        .iter()
        .filter(|video| state.shows_feed_video(video))
        .collect();
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
    let (rect, mut response) = ui.allocate_exact_size(
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
    widgets::video_thumbnail(ui, state, video)
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
        state.display_title(video).to_owned(),
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

    if state.dearrow_replacement_title(video).is_some() && !video.title.is_empty() {
        response = response.on_hover_text(trf(
            locale,
            "Original title: {title}",
            &[("title", video.title.clone())],
        ));
    }
    widgets::video_interactions(ui, &response, state, video, actions);
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn shorts_shelf_and_hide_watched_chip_respect_filters() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State {
            feed_filter: FeedFilter::All,
            feed: Listing::Ready(vec![Video::from_id("feed0000001".into())]),
            feed_shown: 10,
            ..State::default()
        };
        let mut hidden_short = Video::from_id("short_hid01".into());
        hidden_short.title = "Hidden Short".into();
        hidden_short.short = true;
        let mut watched_short = Video::from_id("short_wat01".into());
        watched_short.title = "Watched Short".into();
        watched_short.short = true;
        let mut visible_short = Video::from_id("short_vis01".into());
        visible_short.title = "Visible Short".into();
        visible_short.short = true;

        state.library.hide_video("short_hid01".into());
        state.library.record_watch(watched_short.clone(), 10);
        state.settings.hide_watched = true;
        state.shorts = Listing::Ready(vec![hidden_short, watched_short, visible_short]);

        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(900.0, 600.0))),
                ..Default::default()
            },
            |ui| shorts_shelf(ui, &state, &mut Vec::new()),
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
        assert!(texts.iter().any(|t| t == "Visible Short"));
        assert!(!texts.iter().any(|t| t == "Hidden Short"));
        assert!(!texts.iter().any(|t| t == "Watched Short"));

        // Clicking the "Hide watched" chip toggles settings.hide_watched.
        let mut chip_pos = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(900.0, 600.0))),
                ..Default::default()
            },
            |ui| feed_chips(ui, &state, &mut Vec::new()),
        );
        for shape in &output.shapes {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.text() == "Hide watched"
            {
                chip_pos = Some(Rect::from_min_size(text.pos, text.galley.size()).center());
            }
        }
        output.textures_delta.clear();
        let pos = chip_pos.expect("Hide watched chip");
        let mut actions = Vec::new();
        for pressed in [true, false] {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(900.0, 600.0))),
                    events: vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| feed_chips(ui, &state, &mut actions),
            );
            out.textures_delta.clear();
        }
        assert_eq!(actions, [Action::SetHideWatched(false)]);
    }

    fn find_text_center(shapes: &[egui::epaint::ClippedShape], needle: &str) -> Option<egui::Pos2> {
        fn visit(shape: &egui::Shape, needle: &str) -> Option<egui::Pos2> {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == needle => {
                    Some(Rect::from_min_size(text.pos, text.galley.size()).center())
                }
                egui::Shape::Vec(children) => {
                    children.iter().find_map(|child| visit(child, needle))
                }
                _ => None,
            }
        }
        shapes
            .iter()
            .find_map(|clipped| visit(&clipped.shape, needle))
    }

    struct RecommendationControlsDriver<'a> {
        ctx: egui::Context,
        state: &'a State,
        actions: Vec<Action>,
    }

    impl<'a> RecommendationControlsDriver<'a> {
        fn new(state: &'a State) -> Self {
            let ctx = egui::Context::default();
            youtfast_appearance::theme::install_fonts(&ctx);
            Self {
                ctx,
                state,
                actions: Vec::new(),
            }
        }

        fn frame(&mut self, time: f64, events: Vec<egui::Event>) -> egui::FullOutput {
            let mut output = self.ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(900.0, 600.0))),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| recommendation_controls(ui, self.state, &mut self.actions),
            );
            output.textures_delta.clear();
            output
        }

        fn click(&mut self, pos: egui::Pos2, time: f64) {
            for (step, pressed) in [true, false].into_iter().enumerate() {
                self.frame(
                    time + step as f64 * 0.1,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
        }

        fn open(&mut self) -> egui::FullOutput {
            let output = self.frame(0.0, vec![]);
            let header = find_text_center(&output.shapes, "Local recommendation settings")
                .expect("Local recommendation settings header");
            self.click(header, 0.1);
            // Let both headers settle, including Custom interests when open by default.
            self.frame(2.0, vec![])
        }
    }

    #[test]
    fn clicking_a_preset_topic_chip_applies_recommendations_immediately() {
        let state = State::default();
        let mut driver = RecommendationControlsDriver::new(&state);
        let output = driver.open();
        let chip =
            find_text_center(&output.shapes, "Computing & Tech").expect("Computing & Tech chip");
        driver.click(chip, 2.1);
        let expected = Recommendations {
            topics: vec![RecommendationTopic::Computing],
            ..Recommendations::default()
        };
        assert_eq!(driver.actions, [Action::SetRecommendations(expected)]);
    }

    #[test]
    fn enter_in_custom_interests_submits_and_keeps_apply_button_rendered() {
        let mut state = State::default();
        state.settings.recommendations.interests = "rust".into();
        let mut driver = RecommendationControlsDriver::new(&state);
        let output = driver.open();
        assert!(find_text_center(
            &output.shapes,
            "Select a topic to fetch new videos that match this duration. History-based results are only filtered.",
        )
        .is_some());
        let input = find_text_center(&output.shapes, "rust").expect("interests text input");
        driver.click(input, 2.1);
        driver.frame(2.4, vec![egui::Event::Text(", linux".into())]);
        let output = driver.frame(
            2.5,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(
            find_text_center(&output.shapes, "Apply").is_some(),
            "Apply button must still render on the Enter frame"
        );
        assert_eq!(driver.actions.len(), 1);
        let Action::SetRecommendations(updated) = &driver.actions[0] else {
            panic!("expected SetRecommendations");
        };
        assert!(updated.interests.contains("linux"));
    }

    #[test]
    fn signed_in_loading_account_feed_keeps_local_topic_chips_disabled() {
        let state = State {
            account: AccountState::SignedIn(1),
            for_you: Listing::Loading,
            for_you_from_account: false,
            ..State::default()
        };
        let mut driver = RecommendationControlsDriver::new(&state);
        let output = driver.open();
        let chip =
            find_text_center(&output.shapes, "Computing & Tech").expect("Computing & Tech chip");
        driver.click(chip, 2.1);
        assert!(
            driver.actions.is_empty(),
            "disabled chip must not emit SetRecommendations while account feed is loading"
        );
    }
}
