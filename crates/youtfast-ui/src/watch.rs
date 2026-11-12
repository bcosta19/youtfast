//! The watch page, laid out as YouTube's: the video pane and its
//! controls, the title, the channel's line with the actions, the
//! description, chapters and comments, and the Up next pane beside them.

use egui::{
    Align, Color32, CornerRadius, Frame, Layout, Rect, Response, RichText, Sense, Stroke, Ui, pos2,
    vec2,
};

use super::player::{self, Mode};
use super::{results, widgets};
use youtfast_app::Action;
use youtfast_app::State;
use youtfast_appearance::icons::Icon;
use youtfast_appearance::theme::{self, Palette};
use youtfast_core::model::{Channel, CommentRow, Fetch, Rating, Video};
use youtfast_core::util;
use youtfast_i18n::{Locale, format as trf, gettext as tr};
use youtfast_mpv::{Status, VideoSurface};

/// The "Up next" pane's narrowest width beside the video in a wide window.
const SIDEBAR_WIDTH: f32 = 380.0;
/// The widest the pane grows when the video is narrower than the column.
const SIDEBAR_MAX: f32 = 600.0;
/// A floated sidebar width just over the minimum would otherwise flip the
/// rows to the large layout.
const LARGE_ROWS_AFTER: f32 = SIDEBAR_WIDTH + 0.5;
/// The 16:9 box used for square, portrait and unknown videos.
const DEFAULT_ASPECT: f64 = 16.0 / 9.0;
/// The widest the video is drawn, as YouTube caps it.
const MAX_PLAYER_WIDTH: f32 = 1280.0;
/// Wider than any real video; a corrupt property falls back to 16:9 instead
/// of collapsing the player to a sliver.
const MAX_ASPECT: f32 = 10.0;
/// The title line, the gap above it, and a 24 px bottom inset: what the
/// normal watch page keeps for metadata above the fold. The channel and
/// actions follow below the fold so the picture gets the rest.
const METADATA_RESERVE: f32 = 72.0;
/// Theater keeps YouTube's whole metadata strip above the fold. YouTube sizes
/// its theater player to `100vh - 169px`: a 56 px masthead and 113 px for the
/// title, channel row and bottom inset. This page's content area already
/// excludes the masthead, so the player keeps the 113 px below it.
const THEATER_METADATA_RESERVE: f32 = 113.0;
/// YouTube's short-window minimum for 16:9 and narrower videos.
const MIN_VIDEO_HEIGHT: f32 = 360.0;
/// The shorter minimum for videos wider than 16:9.
const MIN_WIDE_VIDEO_HEIGHT: f32 = 240.0;
/// The gap between the video column and Up next.
const GAP: f32 = 24.0;
/// Narrower than this, the pane moves under the video.
const WIDE: f32 = 860.0;

/// The normal watch page's video box, Up next width and the inset that
/// centres the pair when Up next has reached its widest.
#[derive(Debug)]
struct WatchColumns {
    video: egui::Vec2,
    sidebar: f32,
    indent: f32,
    large: bool,
}

/// The largest box of `aspect` (width/height) that fits `available`.
/// Square and portrait videos use YouTube's 16:9 box instead. The short-window
/// target is 360 px for 16:9 and narrower, 240 px for wider; a narrower column
/// sets the size instead. On a short window the box may deliberately be taller
/// than `available.y`.
fn video_box(available: egui::Vec2, aspect: f64) -> egui::Vec2 {
    let aspect = aspect as f32;
    let aspect = if aspect.is_finite() && aspect > 0.0 && aspect <= MAX_ASPECT {
        aspect
    } else {
        DEFAULT_ASPECT as f32
    };
    let ratio = if aspect > 1.0 {
        aspect
    } else {
        DEFAULT_ASPECT as f32
    };
    let minimum = if aspect > DEFAULT_ASPECT as f32 {
        MIN_WIDE_VIDEO_HEIGHT
    } else {
        MIN_VIDEO_HEIGHT
    };
    let height = available.y.max(minimum);
    let width = available.x.min(height * ratio).max(0.0);
    vec2(width, width / ratio)
}

/// YouTube's normal watch layout: a 1280 px player cap, 72 px kept for the
/// title and its bottom inset, and an Up next column between 380 and 600 px
/// wide. The pair is centred once that column cannot grow further.
fn watch_columns(available: egui::Vec2, aspect: f64) -> WatchColumns {
    let height = (available.y - METADATA_RESERVE).max(1.0);
    let main = (available.x - SIDEBAR_WIDTH - GAP).clamp(1.0, MAX_PLAYER_WIDTH);
    let video = video_box(vec2(main, height), aspect);
    let sidebar = (available.x - GAP - video.x).clamp(SIDEBAR_WIDTH, SIDEBAR_MAX);
    let indent = ((available.x - video.x - GAP - sidebar) / 2.0).max(0.0);
    WatchColumns {
        video,
        sidebar,
        indent,
        large: sidebar > LARGE_ROWS_AFTER,
    }
}

/// YouTube's theater layout: the player across the content width, with the
/// title and channel row still above the fold. The metadata and Up next keep
/// their normal columns below it.
fn theater_columns(available: egui::Vec2) -> WatchColumns {
    WatchColumns {
        video: vec2(
            available.x.max(1.0),
            (available.y - THEATER_METADATA_RESERVE).max(1.0),
        ),
        sidebar: SIDEBAR_WIDTH,
        indent: 0.0,
        large: false,
    }
}

/// The aspect the watch page should draw the picture at. Audio-only's
/// thumbnail is 16:9, and an idle player or a missing/invalid aspect falls
/// back to the default box.
fn watch_aspect(state: &State, status: Option<&Status>) -> f64 {
    if state.audio_only {
        return DEFAULT_ASPECT;
    }
    status.map_or(DEFAULT_ASPECT, |status| {
        if status.idle || !status.video_aspect.is_finite() || status.video_aspect <= 0.0 {
            DEFAULT_ASPECT
        } else {
            status.video_aspect
        }
    })
}

pub fn show(
    ui: &mut Ui,
    state: &mut State,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
    actions: &mut Vec<Action>,
) {
    if state.watching_short() {
        // Shorts keeps the watch page's original 24 px frame; the other watch
        // views use the shared 24/16/0 panel.
        egui::CentralPanel::default()
            .frame(Frame::central_panel(ui.style()).inner_margin(egui::Margin::same(24)))
            .show(ui, |ui| shorts(ui, state, status, surface, actions));
        return;
    }
    results::panel(ui, |ui| {
        // The normal layout keeps the title visible under the video when the
        // window is tall enough; the channel line and actions may follow below
        // the fold, as on a short YouTube page. Theater keeps the title and
        // channel row visible when there is room.
        let content_height = ui.available_height();
        let height_budget = (content_height - METADATA_RESERVE).max(1.0);
        let aspect = watch_aspect(state, status);
        let Some(watching) = state.watching.clone() else {
            return;
        };
        let theater = state.settings.theater;
        let mut scrolled_past_video = false;
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt("watch-page")
            .auto_shrink(false);
        if std::mem::take(&mut state.watch_scroll_to_video) {
            scroll = scroll.vertical_scroll_offset(0.0);
        }
        scroll.show(ui, |ui| {
            if theater {
                let columns = theater_columns(vec2(ui.available_width(), content_height));
                scrolled_past_video =
                    video_pane(ui, state, status, surface, columns.video, actions);
                if ui.available_width() >= WIDE {
                    let sidebar = columns.sidebar;
                    let main = (ui.available_width() - sidebar - GAP).max(1.0);
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(main);
                            about_video(ui, state, &watching, status, actions);
                        });
                        ui.add_space((GAP - ui.spacing().item_spacing.x).max(0.0));
                        ui.vertical(|ui| {
                            ui.set_width(sidebar);
                            queue(ui, state, actions);
                            up_next(ui, state, false, actions);
                            ui.add_space(24.0);
                            related(ui, state, false, actions);
                        });
                    });
                    ui.add_space(24.0);
                } else {
                    about_video(ui, state, &watching, status, actions);
                    ui.add_space(24.0);
                    queue(ui, state, actions);
                    up_next(ui, state, false, actions);
                    ui.add_space(24.0);
                    related(ui, state, false, actions);
                    ui.add_space(24.0);
                }
            } else if ui.available_width() >= WIDE {
                let columns = watch_columns(vec2(ui.available_width(), content_height), aspect);
                ui.horizontal_top(|ui| {
                    ui.add_space(columns.indent);
                    ui.vertical(|ui| {
                        ui.set_width(columns.video.x);
                        scrolled_past_video =
                            video_pane(ui, state, status, surface, columns.video, actions);
                        about_video(ui, state, &watching, status, actions);
                    });
                    ui.add_space((GAP - ui.spacing().item_spacing.x).max(0.0));
                    ui.vertical(|ui| {
                        ui.set_width(columns.sidebar);
                        let large = columns.large;
                        queue(ui, state, actions);
                        up_next(ui, state, large, actions);
                        ui.add_space(24.0);
                        related(ui, state, large, actions);
                    });
                });
                ui.add_space(24.0);
            } else {
                let size = video_box(vec2(ui.available_width(), height_budget), aspect);
                scrolled_past_video = video_pane(ui, state, status, surface, size, actions);
                about_video(ui, state, &watching, status, actions);
                ui.add_space(24.0);
                queue(ui, state, actions);
                up_next(ui, state, false, actions);
                ui.add_space(24.0);
                related(ui, state, false, actions);
                ui.add_space(24.0);
            }
        });
        if scrolled_past_video && state.settings.scroll_miniplayer && !state.mini_closed {
            player::mini(ui.ctx(), state, status, surface, actions);
        }
    });
}

/// Shorts have their own upright stage and action rail. Details and
/// comments remain available in a separate scrolling panel rather than
/// making the video scroll away when the wheel switches Shorts.
fn shorts(
    ui: &mut Ui,
    state: &mut State,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
    actions: &mut Vec<Action>,
) {
    let locale: Locale = state.settings.language;
    let Some(watching) = state.watching.clone() else {
        return;
    };
    let panel_id = ui.id().with("short-details");
    let mut details_open = ui
        .ctx()
        .data(|data| data.get_temp::<bool>(panel_id))
        .unwrap_or(false);
    let has_panel = details_open && ui.available_width() > 900.0;
    // The widest action includes an icon, its gap and the pill's padding.
    // Reserve room for the scrollbar too, so its labels remain readable.
    let share_width = ui
        .painter()
        .layout_no_wrap(
            tr(locale, "Share").into(),
            theme::medium(14.0),
            state.palette.text,
        )
        .size()
        .x
        + 18.0
        + 6.0
        + 28.0;
    let rail_width = share_width + ui.spacing().scroll.bar_width + 12.0;
    let gap = 16.0;
    let panel_width = if has_panel { 340.0 } else { 0.0 };
    let available = (ui.available_width() - panel_width - rail_width - gap).max(100.0);
    let height = (ui.available_height() - 8.0)
        .clamp(180.0, 900.0)
        .min(available * 16.0 / 9.0);
    let width = height * 9.0 / 16.0;
    let total = width + rail_width + gap + panel_width;
    let left = ui.max_rect().left() + ((ui.available_width() - total) / 2.0).max(0.0);
    let video = Rect::from_min_size(pos2(left, ui.cursor().top()), vec2(width, height));
    player::player(ui, video, state, status, surface, Mode::Short, actions);
    let meta = Rect::from_min_size(
        video.min + vec2(12.0, 12.0),
        vec2((width - 24.0).max(1.0), 64.0),
    );
    let mut metadata = ui.new_child(egui::UiBuilder::new().max_rect(meta));
    Frame::new()
        .fill(Color32::from_black_alpha(120))
        .corner_radius(8)
        .inner_margin(8)
        .show(&mut metadata, |ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(watching.title.replace(['\r', '\n'], " "))
                        .font(theme::semibold(15.0))
                        .color(Color32::WHITE),
                )
                .truncate(),
            )
            .on_hover_text(&watching.title);
            if let Some(channel) = &watching.channel {
                let response = ui
                    .add(
                        egui::Label::new(
                            RichText::new(channel.replace(['\r', '\n'], " ")).color(Color32::WHITE),
                        )
                        .truncate()
                        .sense(Sense::click()),
                    )
                    .on_hover_text(channel);
                if response.clicked()
                    && let Some(id) = &watching.channel_id
                {
                    actions.push(Action::OpenChannel(Channel {
                        id: id.clone(),
                        name: channel.clone(),
                        avatar: None,
                    }));
                }
            }
        });
    let rail_rect = Rect::from_min_size(
        pos2(video.right() + gap, video.top()),
        vec2(rail_width, height),
    );
    let mut rail = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rail_rect)
            .layout(Layout::top_down(Align::Center)),
    );
    rail.set_clip_rect(rail_rect.intersect(ui.clip_rect()));
    let previous =
        youtfast_app::player::previous_video(state.watching.as_ref(), &state.up_next).is_some();
    let related = state
        .details
        .ready()
        .map_or(&[][..], |details| details.related.as_slice());
    let next = youtfast_app::player::next_video(
        state.watching.as_ref(),
        &state.queue,
        &state.up_next,
        related,
    )
    .is_some()
        || state.playing_more.is_some();
    egui::ScrollArea::vertical()
        .id_salt("short-actions")
        .max_height(height)
        .auto_shrink(false)
        .show(&mut rail, |rail| {
            rail.spacing_mut().item_spacing.y = 12.0;
            rail.add_space(((height - 420.0) / 2.0).max(0.0));
            rail.add_enabled_ui(previous, |ui| {
                if widgets::icon_button(
                    ui,
                    Icon::ChevronUp,
                    24.0,
                    &tr(locale, "Previous Short (↑)"),
                )
                .clicked()
                {
                    actions.push(Action::PlayPrevious);
                }
            });
            rail.add_enabled_ui(next, |ui| {
                if widgets::icon_button(ui, Icon::ChevronDown, 24.0, &tr(locale, "Next Short (↓)"))
                    .clicked()
                {
                    actions.push(Action::PlayNext);
                }
            });
            let rating = state.rating.ready().copied();
            for (icon, wanted, hint) in [
                (Icon::ThumbsUp, Rating::Like, tr(locale, "Liked")),
                (Icon::ThumbsDown, Rating::Dislike, tr(locale, "Disliked")),
            ] {
                let response = rail
                    .add_enabled_ui(
                        state.signed_in()
                            && rating.is_some()
                            && !state.rating_pending.contains(&watching.id),
                        |ui| {
                            if widgets::icon_button(ui, icon, 24.0, &hint).clicked() {
                                actions.push(Action::Rate(if rating == Some(wanted) {
                                    Rating::None
                                } else {
                                    wanted
                                }));
                            }
                        },
                    )
                    .response;
                if !state.signed_in() {
                    response.on_disabled_hover_text(tr(locale, "Sign in to rate"));
                }
            }
            if widgets::button(
                rail,
                &state.palette,
                &tr(locale, "Comments"),
                widgets::ButtonStyle::Normal,
            )
            .clicked()
            {
                details_open = !details_open;
            }
            share_menu(rail, state, &watching, status, actions);
            save_menu(rail, state, &watching, actions);
            if widgets::button(
                rail,
                &state.palette,
                &tr(locale, "Details"),
                widgets::ButtonStyle::Normal,
            )
            .clicked()
            {
                details_open = !details_open;
            }
        });
    // Throttle wheel navigation: one gesture must not consume an entire
    // source list, and scrolling the details panel never changes video.
    if ui.rect_contains_pointer(video) && !state.scrolling.gliding() {
        let delta = ui.input(|input| input.smooth_scroll_delta.y);
        let now = ui.input(|input| input.time);
        let wheel_id = ui.id().with("short-wheel-at");
        let previous = ui
            .ctx()
            .data(|data| data.get_temp::<f64>(wheel_id))
            .unwrap_or(-1.0);
        if delta.abs() > 2.0 && now - previous > 0.65 {
            actions.push(if delta < 0.0 {
                Action::PlayNext
            } else {
                Action::PlayPrevious
            });
            ui.ctx().data_mut(|data| data.insert_temp(wheel_id, now));
        }
    }
    ui.ctx()
        .data_mut(|data| data.insert_temp(panel_id, details_open));
    if details_open {
        if has_panel {
            let rect = Rect::from_min_size(
                pos2(rail_rect.right() + 8.0, video.top()),
                vec2(panel_width - 8.0, height),
            );
            let mut panel = ui.new_child(egui::UiBuilder::new().max_rect(rect));
            egui::ScrollArea::vertical()
                .id_salt("short-comments")
                .show(&mut panel, |ui| {
                    if ui.button(tr(locale, "Close details")).clicked() {
                        ui.ctx().data_mut(|data| data.insert_temp(panel_id, false));
                    }
                    about_video(ui, state, &watching, status, actions);
                });
        } else {
            egui::Window::new(tr(locale, "Description and comments"))
                .id(ui.id().with("short-details-window"))
                .open(&mut details_open)
                .default_width(360.0)
                .resizable(true)
                .show(ui.ctx(), |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        about_video(ui, state, &watching, status, actions);
                    });
                });
            ui.ctx()
                .data_mut(|data| data.insert_temp(panel_id, details_open));
        }
    }
}

/// The video over the whole window, with the controls over its bottom edge.
pub fn fullscreen(
    ui: &mut Ui,
    state: &mut State,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
    actions: &mut Vec<Action>,
) {
    egui::CentralPanel::default()
        .frame(Frame::new().fill(Color32::BLACK))
        .show(ui, |ui| {
            let rect = ui.max_rect();
            player::player(ui, rect, state, status, surface, Mode::Fullscreen, actions);
        });
}

/// The video, `size` large, centred in the column with its controls over it.
/// Theater mode passes the whole content width and lets mpv letterbox the
/// picture inside it.
fn video_pane(
    ui: &mut Ui,
    state: &mut State,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
    size: egui::Vec2,
    actions: &mut Vec<Action>,
) -> bool {
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), size.y), Sense::hover());
    let rect = Rect::from_center_size(row.center(), size);
    player::player(ui, rect, state, status, surface, Mode::Page, actions);
    ui.add_space(16.0);
    rect.bottom() <= ui.clip_rect().top()
}

/// Under the video, as on YouTube: the title, the channel's line with the
/// actions, the description, chapters and comments.
fn about_video(
    ui: &mut Ui,
    state: &mut State,
    watching: &Video,
    status: Option<&Status>,
    actions: &mut Vec<Action>,
) {
    let locale: Locale = state.settings.language;
    let palette = state.palette;
    // A video opened from a link has no title until mpv reads it.
    let title = Some(watching.title.as_str())
        .filter(|title| !title.is_empty())
        .or_else(|| status.and_then(|status| status.title.as_deref()))
        .unwrap_or(&tr(locale, "Loading…"))
        .to_owned();
    ui.label(
        RichText::new(title)
            .font(theme::bold(20.0))
            .color(palette.text),
    );
    ui.add_space(12.0);
    channel_line(ui, state, watching, status, actions);
    ui.add_space(16.0);
    description(ui, state, watching, actions);
    super::extras::transcript(ui, state, status, actions);
    chapters(ui, state, status, actions);
    comments(ui, state, actions);
}

/// YouTube's line under the title: the channel's round avatar, name and
/// subscribers with the Subscribe pill, then the rating, Share and Save at
/// the right. Filled from the watch page's details when they arrive; the
/// spinner or the error until then.
fn channel_line(
    ui: &mut Ui,
    state: &State,
    watching: &Video,
    status: Option<&Status>,
    actions: &mut Vec<Action>,
) {
    let locale: Locale = state.settings.language;
    let palette = &state.palette;
    let details = state.details.ready();
    let channel = match details {
        Some(details) => Some(Channel {
            id: details.channel.id.clone(),
            name: details.channel.name.clone(),
            avatar: details.channel.avatar.clone(),
        }),
        None => watching.channel_id.clone().map(|id| Channel {
            id,
            name: watching.channel.clone().unwrap_or_default(),
            avatar: None,
        }),
    };
    let avatar = 40.0;
    let mut wrapped = false;
    ui.horizontal(|ui| {
        let name = channel
            .as_ref()
            .map(|channel| channel.name.clone())
            .or_else(|| watching.channel.clone())
            .unwrap_or_default();
        let (rect, response) = ui.allocate_exact_size(vec2(avatar, avatar), Sense::click());
        widgets::avatar(
            ui,
            palette,
            channel
                .as_ref()
                .and_then(|channel| channel.avatar.as_deref()),
            &name,
            rect,
        );
        if let Some(channel) = &channel
            && response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
        {
            actions.push(Action::OpenChannel(channel.clone()));
        }
        ui.add_space(4.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            if text_link(ui, &name, theme::semibold(15.0), palette.text).clicked()
                && let Some(channel) = &channel
            {
                actions.push(Action::OpenChannel(channel.clone()));
            }
            match &state.details {
                Fetch::Ready(details) => {
                    if let Some(count) = details.subscribers {
                        ui.label(
                            RichText::new(trf(
                                locale,
                                "{count} subscribers",
                                &[("count", util::compact(count))],
                            ))
                            .font(theme::regular(13.0))
                            .color(palette.secondary),
                        );
                    }
                }
                Fetch::Failed(_) => {}
                _ => widgets::spinner(ui, 13.0),
            }
        });
        ui.add_space(8.0);
        if let Some(channel) = &channel {
            widgets::subscribe(ui, state, channel, widgets::ButtonStyle::Primary, actions);
        }
        // The actions sit at the right while they fit; under it otherwise.
        let needed = 360.0;
        if ui.available_width() >= needed {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                video_actions(ui, state, watching, status, true, actions);
            });
        } else {
            wrapped = true;
        }
    });
    if wrapped {
        ui.horizontal(|ui| video_actions(ui, state, watching, status, false, actions));
    }
    if let Fetch::Failed(error) = &state.details {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(youtfast_i18n::message(locale, error))
                    .font(theme::regular(14.0))
                    .color(palette.red),
            );
            if widgets::button(
                ui,
                palette,
                &tr(locale, "Retry"),
                widgets::ButtonStyle::Normal,
            )
            .clicked()
            {
                actions.push(Action::Refresh);
            }
        });
    }
}

/// The rating, Share and Save, in that order as read;
/// `right_to_left` when the caller's layout runs that way.
fn video_actions(
    ui: &mut Ui,
    state: &State,
    watching: &Video,
    status: Option<&Status>,
    right_to_left: bool,
    actions: &mut Vec<Action>,
) {
    if right_to_left {
        save_menu(ui, state, watching, actions);
        share_menu(ui, state, watching, status, actions);
        rating_buttons(ui, state, actions);
    } else {
        rating_buttons(ui, state, actions);
        share_menu(ui, state, watching, status, actions);
        save_menu(ui, state, watching, actions);
    }
}

/// One pill as on YouTube: like (with the count) and dislike, a thin rule
/// between them. The account's rating fills its half; a click on it takes
/// it back. The count moves with the rating given here. Without an
/// account the pill only shows the count.
fn rating_buttons(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale: Locale = state.settings.language;
    let palette = &state.palette;
    let rating = state.rating.ready().copied();
    let likes = state
        .details
        .ready()
        .and_then(|details| details.likes)
        .map(|likes| {
            let now = i64::from(rating == Some(Rating::Like));
            let before = i64::from(state.rating_base == Rating::Like);
            (i64::from(likes) + now - before).max(0) as u64
        });
    let enabled = state.signed_in()
        && rating.is_some()
        && state
            .watching
            .as_ref()
            .is_some_and(|video| !state.rating_pending.contains(&video.id));
    let hint = if !state.signed_in() {
        tr(locale, "Sign in to rate videos")
    } else if matches!(state.rating, Fetch::Failed(_)) {
        tr(locale, "Your rating could not be read")
    } else {
        std::borrow::Cow::Borrowed("")
    };
    let liked = rating == Some(Rating::Like);
    let disliked = rating == Some(Rating::Dislike);
    let (like, dislike) = rating_pill(
        ui,
        palette,
        likes.map(util::compact),
        liked,
        disliked,
        enabled,
    );
    // Disabled halves only hover, with the reason as their hint.
    let hover = |response: Response, on_text: &str, off_text: &str, on: bool| {
        if enabled {
            response.on_hover_text(if on { on_text } else { off_text })
        } else if hint.is_empty() {
            response
        } else {
            response.on_hover_text(&*hint)
        }
    };
    if hover(
        like,
        &tr(locale, "Liked"),
        &tr(locale, "I like this"),
        liked,
    )
    .clicked()
    {
        actions.push(Action::Rate(if liked {
            Rating::None
        } else {
            Rating::Like
        }));
    }
    if hover(
        dislike,
        &tr(locale, "Disliked"),
        &tr(locale, "I dislike this"),
        disliked,
    )
    .clicked()
    {
        actions.push(Action::Rate(if disliked {
            Rating::None
        } else {
            Rating::Dislike
        }));
    }
}

/// Draws the rating pill: a rounded `surface` pill split in a like half
/// (thumb and count) and a dislike half, each with its own hover and its
/// own click; the given rating's half is filled `surface_active` with a
/// bold count. Returns the two halves' responses.
fn rating_pill(
    ui: &mut Ui,
    palette: &Palette,
    count: Option<String>,
    liked: bool,
    disliked: bool,
    enabled: bool,
) -> (Response, Response) {
    let height = 34.0;
    let icon = 18.0;
    let pad = 14.0;
    let font = if liked {
        theme::semibold(14.0)
    } else {
        theme::medium(14.0)
    };
    let galley = count.map(|count| ui.painter().layout_no_wrap(count, font, palette.text));
    let like_width =
        pad + icon + galley.as_ref().map_or(0.0, |galley| 6.0 + galley.size().x) + 12.0;
    let dislike_width = 12.0 + icon + pad;
    let (rect, _) = ui.allocate_exact_size(
        vec2(like_width + 1.0 + dislike_width, height),
        Sense::hover(),
    );
    let like_rect = Rect::from_min_size(rect.min, vec2(like_width, height));
    let dislike_rect = Rect::from_min_max(pos2(rect.right() - dislike_width, rect.top()), rect.max);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let cursor = if enabled {
        egui::CursorIcon::PointingHand
    } else {
        egui::CursorIcon::Default
    };
    let like = ui
        .interact(like_rect, ui.id().with("like"), sense)
        .on_hover_cursor(cursor);
    let dislike = ui
        .interact(dislike_rect, ui.id().with("dislike"), sense)
        .on_hover_cursor(cursor);
    let radius = (height / 2.0) as u8;
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(radius), palette.surface);
    let half = |response: &Response, on: bool, corners: CornerRadius| {
        let fill = if on {
            Some(palette.surface_active)
        } else if enabled && response.hovered() {
            Some(palette.surface_hover)
        } else {
            None
        };
        if let Some(fill) = fill {
            painter.rect_filled(response.rect, corners, fill);
        }
    };
    half(
        &like,
        liked,
        CornerRadius {
            nw: radius,
            sw: radius,
            ne: 0,
            se: 0,
        },
    );
    half(
        &dislike,
        disliked,
        CornerRadius {
            nw: 0,
            sw: 0,
            ne: radius,
            se: radius,
        },
    );
    painter.vline(
        like_rect.right() + 0.5,
        (rect.top() + 8.0)..=(rect.bottom() - 8.0),
        Stroke::new(1.0, palette.outline),
    );
    let tint = if enabled {
        palette.text
    } else {
        palette.secondary
    };
    let icon_rect = |left: f32| {
        Rect::from_center_size(pos2(left + icon / 2.0, rect.center().y), vec2(icon, icon))
    };
    Icon::ThumbsUp
        .image(tint, icon)
        .paint_at(ui, icon_rect(like_rect.left() + pad));
    if let Some(galley) = galley {
        let pos = pos2(
            like_rect.left() + pad + icon + 6.0,
            rect.center().y - galley.size().y / 2.0,
        );
        ui.painter().galley(pos, galley, tint);
    }
    Icon::ThumbsDown
        .image(tint, icon)
        .paint_at(ui, icon_rect(dislike_rect.left() + 12.0));
    (like, dislike)
}

/// A plain clickable label (YouTube's "more" and "Show less").
fn text_link(ui: &mut Ui, text: &str, font: egui::FontId, color: Color32) -> Response {
    ui.add(
        egui::Label::new(RichText::new(text).font(font).color(color))
            .sense(Sense::click())
            .selectable(false),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Share: the link, "Start at" the moment playing, copy, and each site's
/// share page.
fn share_menu(
    ui: &mut Ui,
    state: &State,
    watching: &Video,
    status: Option<&Status>,
    actions: &mut Vec<Action>,
) {
    let locale: Locale = state.settings.language;
    let palette = &state.palette;
    let response = widgets::icon_text_button(
        ui,
        palette,
        Some(Icon::Share),
        &tr(locale, "Share"),
        widgets::ButtonStyle::Normal,
    );
    egui::Popup::menu(&response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(260.0);
            let position = status.map_or(0.0, |status| status.position);
            let at = state.share_at_time.then_some(position as u64);
            let link = util::video_link(&watching.id, at);
            ui.label(
                RichText::new(&link)
                    .font(theme::regular(14.0))
                    .color(palette.text),
            );
            let mut start_at = state.share_at_time;
            if ui
                .checkbox(
                    &mut start_at,
                    RichText::new(trf(
                        locale,
                        "Start at {time}",
                        &[("time", util::duration(position))],
                    ))
                    .font(theme::regular(14.0)),
                )
                .changed()
            {
                actions.push(Action::SetShareAtTime(start_at));
            }
            if widgets::button(
                ui,
                palette,
                &tr(locale, "Copy link"),
                widgets::ButtonStyle::Primary,
            )
            .clicked()
            {
                actions.push(Action::CopyLink(link.clone()));
                ui.close();
            }
            ui.separator();
            let title = state
                .details
                .ready()
                .map(|details| details.title.as_str())
                .filter(|title| !title.is_empty())
                .unwrap_or(&watching.title);
            for (site, url) in util::share_targets(&link, title) {
                if widgets::button(ui, palette, site, widgets::ButtonStyle::Normal).clicked() {
                    actions.push(Action::OpenUrl(url));
                    ui.close();
                }
            }
        });
}

/// Save: Watch later on this computer and, signed in, the account's
/// playlists, each a box to tick. The account's list is asked for when
/// the menu first opens.
fn save_menu(ui: &mut Ui, state: &State, watching: &Video, actions: &mut Vec<Action>) {
    let locale: Locale = state.settings.language;
    let palette = &state.palette;
    let local = state.library.in_watch_later(&watching.id);
    let account = state
        .save_targets
        .ready()
        .is_some_and(|targets| targets.iter().any(|target| target.saved));
    let style = if local || account {
        widgets::ButtonStyle::On
    } else {
        widgets::ButtonStyle::Normal
    };
    let (icon, label) = if local || account {
        (Icon::BookmarkFilled, tr(locale, "Saved"))
    } else {
        (Icon::Bookmark, tr(locale, "Save"))
    };
    let response = widgets::icon_text_button(ui, palette, Some(icon), &label, style);
    egui::Popup::menu(&response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(240.0);
            let mut on_device = local;
            if ui
                .checkbox(
                    &mut on_device,
                    RichText::new(tr(locale, "Watch later on this computer"))
                        .font(theme::regular(14.0)),
                )
                .changed()
            {
                actions.push(Action::ToggleWatchLater(watching.clone()));
            }
            ui.separator();
            if !state.signed_in() {
                ui.label(
                    RichText::new(tr(locale, "Sign in to save to the account's playlists"))
                        .font(theme::regular(13.0))
                        .color(palette.secondary),
                );
                return;
            }
            match &state.save_targets {
                Fetch::Idle => actions.push(Action::LoadSaveTargets),
                Fetch::Loading => widgets::loading(ui, palette, "playlists", locale),
                Fetch::Failed(error) => {
                    ui.label(
                        RichText::new(youtfast_i18n::message(locale, error))
                            .font(theme::regular(14.0))
                            .color(palette.red),
                    );
                }
                Fetch::Ready(targets) => {
                    for target in targets {
                        let mut saved = target.saved;
                        if ui
                            .add_enabled(
                                !state
                                    .save_pending
                                    .contains(&(watching.id.clone(), target.id.clone())),
                                egui::Checkbox::new(
                                    &mut saved,
                                    RichText::new(&target.name).font(theme::regular(14.0)),
                                ),
                            )
                            .changed()
                        {
                            actions.push(Action::SetSaved {
                                playlist: target.id.clone(),
                                saved,
                            });
                        }
                    }
                }
            }
        });
}

/// The description card, YouTube's grey box: `views · published` on its
/// first line, then the text collapsed to three rows with "more".
/// Expanded, the counts are exact and the date is added.
fn description(ui: &mut Ui, state: &State, watching: &Video, actions: &mut Vec<Action>) {
    let locale: Locale = state.settings.language;
    let details = state.details.ready();
    let open = state.description_open;
    let views = details.map(|details| details.views).or(watching.views);
    // A live stream's count is of the people watching it now.
    let live = watching.live;
    let views = views.map(|views| {
        if live {
            trf(
                locale,
                "{count} watching now",
                &[("count", util::thousands(views))],
            )
        } else if open {
            trf(
                locale,
                "{count} views",
                &[("count", util::thousands(views))],
            )
        } else {
            util::views(views, locale)
        }
    });
    let published = match details {
        Some(details) => match (&details.published, &details.published_date) {
            (Some(ago), Some(date)) if open => Some(format!("{ago} ({date})")),
            (Some(ago), _) => Some(ago.clone()),
            (None, date) => date.clone(),
        },
        None => watching.published.clone(),
    };
    let head = [views, published]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
    let spans = details.map(|details| details.description.as_slice());
    if head.is_empty() && spans.is_none_or(|spans| spans.is_empty()) {
        return;
    }
    widgets::card(ui, &state.palette, |ui| {
        if !head.is_empty() {
            ui.label(
                RichText::new(head)
                    .font(theme::semibold(14.0))
                    .color(state.palette.text),
            );
        }
        let Some(spans) = spans.filter(|spans| !spans.is_empty()) else {
            return;
        };
        let collapsed =
            !open && widgets::spans_truncated(ui, ui.id().with("description"), spans, 3);
        let max_rows = if collapsed { Some(3) } else { None };
        if let Some(action) = widgets::rich_text(ui, &state.palette, spans, max_rows, locale) {
            actions.push(action);
        }
        if collapsed || open {
            let label = if collapsed {
                tr(locale, "...more")
            } else {
                tr(locale, "Show less")
            };
            if text_link(ui, &label, theme::semibold(14.0), state.palette.text).clicked() {
                actions.push(Action::ToggleDescription);
            }
        }
    });
}

/// The chapters section: `00:00  Intro` rows, the current one selected.
fn chapters(ui: &mut Ui, state: &State, status: Option<&Status>, actions: &mut Vec<Action>) {
    let locale: Locale = state.settings.language;
    let Fetch::Ready(details) = &state.details else {
        return;
    };
    if details.chapters.is_empty() {
        return;
    }
    let position = status.map(|status| status.position).unwrap_or(0.0);
    // The chapter the playhead is in: the last one that has started.
    let current = details
        .chapters
        .iter()
        .rposition(|chapter| f64::from(chapter.seconds) <= position);
    ui.add_space(24.0);
    widgets::section(
        ui,
        &state.palette,
        &tr(locale, "Chapters"),
        Some(details.chapters.len()),
        |ui| {
            for (index, chapter) in details.chapters.iter().enumerate() {
                let parts = [
                    (
                        util::duration(f64::from(chapter.seconds)),
                        state.palette.secondary,
                    ),
                    (format!("  {}", chapter.name), state.palette.text),
                ];
                if widgets::row(ui, &state.palette, Some(index) == current, &parts).clicked() {
                    actions.push(Action::SeekTo {
                        seconds: f64::from(chapter.seconds),
                        exact: true,
                    });
                }
            }
        },
    );
}

/// The comments section: one [`comment_row`] each, then the next page's
/// "Load more".
fn comments(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale: Locale = state.settings.language;
    let Fetch::Ready(details) = &state.details else {
        return;
    };
    if details.comments_token.is_none() && details.comment_count.is_none() {
        return;
    }
    ui.add_space(24.0);
    let count = details
        .comment_count
        .map(|count| count as usize)
        .or_else(|| state.comments.ready().map(|rows| rows.len()));
    let title = match count {
        Some(1) => tr(locale, "1 comment").into_owned(),
        Some(count) => trf(
            locale,
            "{count} comments",
            &[("count", util::thousands(count as u64))],
        ),
        None => tr(locale, "Comments").into_owned(),
    };
    widgets::section(ui, &state.palette, &title, None, |ui| {
        match &state.comments {
            Fetch::Idle => {}
            Fetch::Loading => widgets::loading(ui, &state.palette, "comments", locale),
            Fetch::Failed(error) => results::failed(ui, state, error, actions),
            Fetch::Ready(rows) => {
                if rows.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new(tr(locale, "No comments yet"))
                                .font(theme::semibold(16.0))
                                .color(state.palette.text),
                        );
                        ui.label(
                            RichText::new(tr(locale, "Nobody has commented on this video."))
                                .font(theme::regular(14.0))
                                .color(state.palette.secondary),
                        );
                    });
                }
                ui.add_space(8.0);
                let avatar = 40.0;
                for comment in rows {
                    comment_row(ui, state, comment, avatar, actions);
                }
                // The tail row: the page on its way, a failed page's
                // retry, or the button.
                match &state.comments_more {
                    Fetch::Loading => widgets::loading(ui, &state.palette, "comments", locale),
                    Fetch::Failed(error) => {
                        ui.label(
                            RichText::new(youtfast_i18n::message(locale, error))
                                .font(theme::regular(14.0))
                                .color(state.palette.red),
                        );
                        if widgets::button(
                            ui,
                            &state.palette,
                            &tr(locale, "Retry"),
                            widgets::ButtonStyle::Normal,
                        )
                        .clicked()
                        {
                            actions.push(Action::LoadMoreComments);
                        }
                    }
                    _ => {
                        if state.comments_next.is_some()
                            && widgets::button(
                                ui,
                                &state.palette,
                                &tr(locale, "Load more"),
                                widgets::ButtonStyle::Normal,
                            )
                            .clicked()
                        {
                            actions.push(Action::LoadMoreComments);
                        }
                    }
                }
            }
        }
    });
}

/// The rows a long comment shows until "Read more".
const COMMENT_ROWS: usize = 5;

/// One comment as on YouTube: the author's round avatar at the left;
/// beside it `@author  3 days ago`, the text (long ones cut to a few rows),
/// then the likes, the heart and the replies' toggle, and the replies under
/// it when open. `avatar` is the avatar's side: replies draw it smaller.
fn comment_row(
    ui: &mut Ui,
    state: &State,
    comment: &CommentRow,
    avatar: f32,
    actions: &mut Vec<Action>,
) {
    let locale: Locale = state.settings.language;
    let top = ui.next_widget_position();
    let width = ui.available_width();
    // One entry per comment, replaced when the width changes. Its signature
    // covers what else decides the height without hashing the whole text.
    let key = egui::Id::new(("comment-layout", comment.id.as_str()));
    let signature = comment_signature(
        comment,
        state.settings.language.native_name(),
        avatar,
        ui.text_style_height(&egui::TextStyle::Body),
    );
    let closed =
        !state.comments_open.contains(&comment.id) && !state.replies.contains_key(&comment.id);
    let cached = ui
        .ctx()
        .data(|data| data.get_temp::<CommentHeight>(key))
        .filter(|cached| cached.width == width.to_bits() && cached.signature == signature);
    if closed
        && let Some(CommentHeight { height, .. }) = cached
        && !ui.is_rect_visible(Rect::from_min_size(top, vec2(width, height)))
    {
        ui.allocate_exact_size(vec2(width, height), Sense::hover());
        return;
    }
    // YouTube's handles already start with `@`.
    let unknown_author = tr(locale, "someone");
    let author = comment
        .author
        .as_deref()
        .unwrap_or(&unknown_author)
        .trim_start_matches('@');
    let channel = comment.author_id.as_ref().map(|id| Channel {
        id: id.clone(),
        name: author.to_owned(),
        avatar: comment.author_avatar.clone(),
    });
    let palette = &state.palette;
    ui.horizontal_top(|ui| {
        let (rect, response) = ui.allocate_exact_size(vec2(avatar, avatar), Sense::click());
        widgets::avatar(ui, palette, comment.author_avatar.as_deref(), author, rect);
        if let Some(channel) = &channel
            && response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
        {
            actions.push(Action::OpenChannel(channel.clone()));
        }
        ui.add_space(if avatar >= 40.0 { 12.0 } else { 8.0 });
        ui.vertical(|ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 4.0;
            comment_header(ui, state, comment, author, channel.as_ref(), actions);
            let open = state.comments_open.contains(&comment.id);
            let collapsed = !open
                && widgets::spans_truncated(
                    ui,
                    ui.id().with(("comment", &comment.id)),
                    &comment.text,
                    COMMENT_ROWS,
                );
            let max_rows = collapsed.then_some(COMMENT_ROWS);
            if let Some(action) = widgets::rich_text(ui, palette, &comment.text, max_rows, locale) {
                actions.push(action);
            }
            if (collapsed || open)
                && text_link(
                    ui,
                    &if collapsed {
                        tr(locale, "Read more")
                    } else {
                        tr(locale, "Show less")
                    },
                    theme::semibold(13.0),
                    palette.text,
                )
                .clicked()
            {
                actions.push(Action::ToggleCommentText(comment.id.clone()));
            }
            if let Some(url) = &comment.image {
                comment_image(ui, url, locale);
            }
            comment_footer(ui, state, comment, actions);
            if let Some(replies) = state.replies.get(&comment.id) {
                ui.add_space(4.0);
                reply_column(ui, state, replies, actions);
            }
        });
    });
    ui.add_space(16.0);
    if closed {
        let height = (ui.next_widget_position().y - top.y - ui.spacing().item_spacing.y).max(0.0);
        ui.ctx().data_mut(|data| {
            data.insert_temp(
                key,
                CommentHeight {
                    width: width.to_bits(),
                    signature,
                    height,
                },
            );
        });
    }
}

/// A comment's measured height at one width (`f32::to_bits`).
#[derive(Clone, Copy)]
struct CommentHeight {
    width: u32,
    signature: u64,
    height: f32,
}

/// What besides the width decides a closed comment's height: the language,
/// the sizes it is laid out with and its fields. The text counts by shape
/// (spans and characters) rather than being hashed on every frame.
fn comment_signature(comment: &CommentRow, language: &str, avatar: f32, body: f32) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (language, avatar.to_bits(), body.to_bits()).hash(&mut hasher);
    (
        &comment.author,
        &comment.published,
        comment.pinned,
        comment.by_owner,
        comment.hearted,
        comment.likes,
        comment.replies,
        &comment.image,
    )
        .hash(&mut hasher);
    comment.text.len().hash(&mut hasher);
    for span in &comment.text {
        (std::mem::discriminant(span), span.text().len()).hash(&mut hasher);
    }
    hasher.finish()
}

/// Reserve a stable box so loading an attachment does not move the comments.
/// Only visible attachments fetch, decode or schedule animation frames.
fn comment_image(ui: &mut Ui, url: &str, locale: Locale) {
    let size = vec2(ui.available_width().min(320.0), 200.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    if ui.is_rect_visible(rect) {
        youtfast_app::images::attachment(ui, url, rect, locale);
    }
}

/// `@author  3 days ago`, then "Pinned" / "By the creator" in small
/// secondary text; the author opens their channel.
fn comment_header(
    ui: &mut Ui,
    state: &State,
    comment: &CommentRow,
    author: &str,
    channel: Option<&Channel>,
    actions: &mut Vec<Action>,
) {
    let locale: Locale = state.settings.language;
    ui.horizontal(|ui| {
        let response = text_link(
            ui,
            &format!("@{author}"),
            theme::semibold(13.0),
            state.palette.text,
        );
        if let Some(channel) = channel
            && response.clicked()
        {
            actions.push(Action::OpenChannel(channel.clone()));
        }
        ui.label(
            RichText::new(&comment.published)
                .font(theme::regular(12.0))
                .color(state.palette.secondary),
        );
    });
    let mut badges = Vec::new();
    if comment.pinned {
        badges.push(tr(locale, "Pinned"));
    }
    if comment.by_owner {
        badges.push(tr(locale, "By the creator"));
    }
    if !badges.is_empty() {
        ui.label(
            RichText::new(badges.join(" · "))
                .font(theme::regular(12.0))
                .color(state.palette.secondary),
        );
    }
}

/// Under the text: the likes, the creator's heart and the replies'
/// toggle.
fn comment_footer(ui: &mut Ui, state: &State, comment: &CommentRow, actions: &mut Vec<Action>) {
    let locale: Locale = state.settings.language;
    let likes = comment.likes.filter(|likes| *likes > 0);
    if likes.is_none() && !comment.hearted && comment.replies == 0 {
        return;
    }
    ui.horizontal(|ui| {
        if let Some(likes) = likes {
            ui.add(Icon::ThumbsUp.image(state.palette.secondary, 14.0));
            ui.label(
                RichText::new(util::compact(u64::from(likes)))
                    .font(theme::regular(12.0))
                    .color(state.palette.secondary),
            );
        }
        if comment.hearted {
            ui.label(
                RichText::new("♥")
                    .font(theme::regular(13.0))
                    .color(state.palette.red),
            )
            .on_hover_text(tr(locale, "Hearted by the creator"));
        }
        if comment.replies > 0 {
            let open = state.replies.contains_key(&comment.id);
            let label = if open {
                tr(locale, "Hide replies").into_owned()
            } else if comment.replies == 1 {
                tr(locale, "1 reply").into_owned()
            } else {
                trf(
                    locale,
                    "{count} replies",
                    &[("count", comment.replies.to_string())],
                )
            };
            ui.add_space(8.0);
            let icon = if open {
                Icon::ChevronUp
            } else {
                Icon::ChevronDown
            };
            let toggle = ui
                .horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.add(icon.image(state.palette.blue, 16.0));
                    ui.label(
                        RichText::new(label)
                            .font(theme::semibold(13.0))
                            .color(state.palette.blue),
                    );
                })
                .response;
            if ui
                .interact(
                    toggle.rect,
                    ui.id().with(("replies", &comment.id)),
                    Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                actions.push(Action::ToggleReplies(comment.id.clone()));
            }
        }
    });
}

/// The replies, in the comment's text column under its footer. Laid out in
/// the parent's flow (horizontal_top, a gap, then a vertical): a child with
/// an unbounded height made egui's alignment NaN under the loading line.
fn reply_column(
    ui: &mut Ui,
    state: &State,
    replies: &Fetch<Vec<CommentRow>>,
    actions: &mut Vec<Action>,
) {
    let locale: Locale = state.settings.language;
    ui.horizontal_top(|ui| {
        ui.add_space(4.0);
        ui.vertical(|ui| match replies {
            Fetch::Idle | Fetch::Loading => widgets::loading(ui, &state.palette, "replies", locale),
            Fetch::Failed(error) => results::failed(ui, state, error, actions),
            Fetch::Ready(rows) => {
                for reply in rows {
                    comment_row(ui, state, reply, 24.0, actions);
                }
            }
        });
    });
}

#[derive(Clone)]
struct QueueDrag(String);

/// The videos queued to play next. A handle, up/down buttons and an
/// inline remove control keep every action visible and keyboard reachable.
fn queue(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale: Locale = state.settings.language;
    if state.queue.is_empty() {
        return;
    }
    widgets::section(
        ui,
        &state.palette,
        &tr(locale, "Queue"),
        Some(state.queue.len()),
        |ui| {
            if widgets::button(
                ui,
                &state.palette,
                &tr(locale, "Clear"),
                widgets::ButtonStyle::Normal,
            )
            .clicked()
            {
                actions.push(Action::ClearQueue);
            }
            for (index, video) in state.queue.iter().enumerate() {
                let row = ui
                    .push_id(&video.id, |ui| {
                        ui.horizontal_top(|ui| {
                            let width = (ui.available_width() - 36.0).max(120.0);
                            let response = ui
                                .vertical(|ui| {
                                    ui.set_width(width);
                                    let watched = widgets::watched(&state.library, video);
                                    widgets::video_row(
                                        ui,
                                        &state.palette,
                                        video,
                                        None,
                                        watched,
                                        false,
                                        locale,
                                    )
                                })
                                .inner;
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                let handle = ui
                                    .add(
                                        Icon::More
                                            .image(state.palette.secondary, 16.0)
                                            .sense(Sense::drag()),
                                    )
                                    .on_hover_text(tr(locale, "Drag to reorder"));
                                handle.dnd_set_drag_payload(QueueDrag(video.id.clone()));
                                ui.add_enabled_ui(index > 0, |ui| {
                                    if widgets::icon_button(
                                        ui,
                                        Icon::ChevronUp,
                                        14.0,
                                        &tr(locale, "Move up"),
                                    )
                                    .clicked()
                                    {
                                        actions.push(Action::MoveQueue {
                                            from: index,
                                            to: index - 1,
                                        });
                                    }
                                });
                                ui.add_enabled_ui(index + 1 < state.queue.len(), |ui| {
                                    if widgets::icon_button(
                                        ui,
                                        Icon::ChevronDown,
                                        14.0,
                                        &tr(locale, "Move down"),
                                    )
                                    .clicked()
                                    {
                                        actions.push(Action::MoveQueue {
                                            from: index,
                                            to: index + 1,
                                        });
                                    }
                                });
                                if widgets::icon_button(
                                    ui,
                                    Icon::Close,
                                    14.0,
                                    &tr(locale, "Remove from queue"),
                                )
                                .clicked()
                                {
                                    actions.push(Action::Dequeue(video.id.clone()));
                                }
                            });
                            response
                        })
                    })
                    .inner;
                let response = row.inner;
                if response.clicked() {
                    actions.push(Action::Open(video.clone()));
                }
                if row.response.dnd_hover_payload::<QueueDrag>().is_some() {
                    ui.painter().hline(
                        row.response.rect.x_range(),
                        row.response.rect.top(),
                        Stroke::new(2.0, state.palette.accent),
                    );
                }
                if let Some(payload) = row.response.dnd_release_payload::<QueueDrag>()
                    && let Some(from) = state.queue.iter().position(|video| video.id == payload.0)
                {
                    actions.push(Action::MoveQueue { from, to: index });
                }
                response.context_menu(|ui| {
                    if ui.button(tr(locale, "Remove from queue")).clicked() {
                        actions.push(Action::Dequeue(video.id.clone()));
                        ui.close();
                    }
                });
            }
        },
    );
    ui.add_space(24.0);
}

fn up_next(ui: &mut Ui, state: &State, large: bool, actions: &mut Vec<Action>) {
    let locale: Locale = state.settings.language;
    let watching = state.watching.as_ref().map(|video| video.id.as_str());
    let next: Vec<&Video> = state
        .up_next
        .iter()
        .filter(|video| Some(video.id.as_str()) != watching)
        .collect();
    if next.is_empty() {
        return;
    }
    widgets::section(
        ui,
        &state.palette,
        &tr(locale, "Up next"),
        Some(next.len()),
        |ui| {
            ui.push_id("watch-up-next", |ui| {
                if large {
                    results::video_rows_large(ui, state, next, actions);
                } else {
                    results::video_rows(ui, state, next, actions);
                }
            });
        },
    );
}

/// The recommended videos the watch page sent with the details.
fn related(ui: &mut Ui, state: &State, large: bool, actions: &mut Vec<Action>) {
    let locale: Locale = state.settings.language;
    let Fetch::Ready(details) = &state.details else {
        return;
    };
    let watching = state.watching.as_ref().map(|video| video.id.as_str());
    let related: Vec<&Video> = details
        .related
        .iter()
        .filter(|video| Some(video.id.as_str()) != watching)
        .collect();
    if related.is_empty() {
        return;
    }
    widgets::section(
        ui,
        &state.palette,
        &tr(locale, "Related"),
        Some(related.len()),
        |ui| {
            ui.push_id("watch-related", |ui| {
                if large {
                    results::video_rows_large(ui, state, related, actions);
                } else {
                    results::video_rows(ui, state, related, actions);
                }
            });
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use youtfast_core::model::Span;

    #[test]
    fn short_actions_scroll_to_details_without_switching_video_in_a_small_window() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut video = Video::from_id("aaaaaaaaaaa".into());
        video.short = true;
        video.title = "Um título longo que precisa deixar o vídeo visível ".repeat(8);
        video.channel = Some("Um canal com um nome muito longo para esta janela".into());
        let mut state = State {
            view: youtfast_core::model::View::Watch,
            watching: Some(video.clone()),
            up_next: vec![video.clone(), video.clone()],
            ..State::default()
        };
        state.settings.sidebar_collapsed = true;
        state.settings.language = Locale::PortugueseBrazil;
        let screen = Rect::from_min_size(egui::Pos2::ZERO, vec2(480.0, 360.0));
        let mut pointer = None;
        let mut details = None;
        let mut actions = Vec::new();
        for frame in 0..12 {
            let mut events = Vec::new();
            if let Some(position) = pointer {
                events.push(egui::Event::PointerMoved(position));
                events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    phase: egui::TouchPhase::Move,
                    delta: vec2(0.0, -1000.0),
                    modifiers: egui::Modifiers::NONE,
                });
            }
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    time: Some(frame as f64 / 10.0),
                    events,
                    ..Default::default()
                },
                |ui| super::super::show(ui, &mut state, None, None, &mut actions),
            );
            for shape in &output.shapes {
                let egui::Shape::Text(text) = &shape.shape else {
                    continue;
                };
                let label = text.galley.text();
                let rect = Rect::from_min_size(text.pos, text.galley.size());
                if label == video.title || Some(label) == video.channel.as_deref() {
                    assert_eq!(
                        text.galley.rows.len(),
                        1,
                        "Short metadata must stay compact"
                    );
                }
                if matches!(label, "Comentários" | "Compartilhar")
                    && shape.clip_rect.intersects(rect)
                {
                    pointer = Some(shape.clip_rect.intersect(screen).center());
                }
                if label == "Detalhes"
                    && shape.clip_rect.contains_rect(rect)
                    && screen.contains_rect(rect)
                {
                    details = Some(rect.center());
                }
            }
            output.textures_delta.clear();
        }
        let button = details.expect("scrolling the Short rail must reveal Detalhes");
        for (frame, pressed) in [true, false].into_iter().enumerate() {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    time: Some(2.0 + frame as f64 / 10.0),
                    events: vec![
                        egui::Event::PointerMoved(button),
                        egui::Event::PointerButton {
                            pos: button,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| super::super::show(ui, &mut state, None, None, &mut actions),
            );
            output.textures_delta.clear();
        }
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                time: Some(2.2),
                ..Default::default()
            },
            |ui| super::super::show(ui, &mut state, None, None, &mut actions),
        );
        assert!(
            output.shapes.iter().any(|shape| matches!(
                &shape.shape,
                egui::Shape::Text(text) if text.galley.text() == "Descrição e comentários"
            )),
            "clicking Detalhes must open the details window"
        );
        assert!(
            !actions
                .iter()
                .any(|action| matches!(action, Action::PlayNext | Action::PlayPrevious)),
            "scrolling the action rail must not navigate between Shorts",
        );
        output.textures_delta.clear();
    }

    #[test]
    fn watch_scroll_areas_do_not_share_ids_with_queue_and_recommendations_visible() {
        fn has_scroll_id_clash(shape: &egui::Shape) -> bool {
            match shape {
                egui::Shape::Text(text) => text.galley.text().contains("ScrollArea ID"),
                egui::Shape::Vec(shapes) => shapes.iter().any(has_scroll_id_clash),
                _ => false,
            }
        }

        for (width, theater) in [(1280.0, false), (800.0, false), (1280.0, true)] {
            let videos: Vec<_> = (0..4)
                .map(|index| Video::from_id(format!("synthetic-{index}")))
                .collect();
            let mut state = State {
                view: youtfast_core::model::View::Watch,
                watching: Some(videos[0].clone()),
                queue: videos[1..].to_vec(),
                up_next: videos.clone(),
                details: Fetch::Ready(youtfast_core::model::Details {
                    related: videos[1..].to_vec(),
                    ..Default::default()
                }),
                comments: Fetch::Ready(Vec::new()),
                ..State::default()
            };
            state.settings.theater = theater;
            let ctx = egui::Context::default();
            youtfast_appearance::theme::install_fonts(&ctx);
            ctx.options_mut(|options| options.warn_on_id_clash = true);
            for _ in 0..3 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            egui::Pos2::ZERO,
                            vec2(width, 800.0),
                        )),
                        ..Default::default()
                    },
                    |ui| show(ui, &mut state, None, None, &mut Vec::new()),
                );
                assert!(
                    !output
                        .shapes
                        .iter()
                        .any(|shape| has_scroll_id_clash(&shape.shape)),
                    "watch scroll areas reused an ID at width={width}, theater={theater}",
                );
                output.textures_delta.clear();
            }
        }
    }

    #[test]
    fn scrolling_miniplayer_is_optional_and_returning_reveals_the_video() {
        fn contains_miniplayer(shape: &egui::Shape) -> bool {
            match shape {
                egui::Shape::Rect(rect) => {
                    rect.stroke.width == 3.5
                        && rect.corner_radius == CornerRadius::same(theme::RADIUS)
                }
                egui::Shape::Vec(shapes) => shapes.iter().any(contains_miniplayer),
                _ => false,
            }
        }
        for (width, theater) in [(1280.0, false), (600.0, false), (1280.0, true)] {
            let ctx = egui::Context::default();
            youtfast_appearance::theme::install_fonts(&ctx);
            let mut state = State {
                view: youtfast_core::model::View::Watch,
                watching: Some(Video::from_id("synthetic".into())),
                details: Fetch::Ready(youtfast_core::model::Details {
                    comment_count: Some(30),
                    ..Default::default()
                }),
                comments: Fetch::Ready((0..30).map(|i| row(&i.to_string(), 0)).collect()),
                ..Default::default()
            };
            state.settings.theater = theater;
            let status = Status {
                paused: true,
                ..Default::default()
            };
            let draw = |state: &mut State, wheel: f32, time: f64| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(width, 600.0))),
                        time: Some(time),
                        events: vec![
                            egui::Event::PointerMoved(pos2(300.0, 300.0)),
                            egui::Event::MouseWheel {
                                unit: egui::MouseWheelUnit::Point,
                                phase: egui::TouchPhase::Move,
                                delta: vec2(0.0, wheel),
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                        ..Default::default()
                    },
                    |ui| show(ui, state, Some(&status), None, &mut Vec::new()),
                );
                let mini = output
                    .shapes
                    .iter()
                    .any(|shape| contains_miniplayer(&shape.shape));
                output.textures_delta.clear();
                mini
            };
            for frame in 0..20 {
                assert!(!draw(&mut state, -120.0, frame as f64 * 0.1));
            }
            state.settings.scroll_miniplayer = true;
            // egui measures a newly opened window before painting it.
            for frame in 0..3 {
                draw(&mut state, 0.0, 2.0 + frame as f64 * 0.1);
            }
            assert!(
                draw(&mut state, 0.0, 2.3),
                "width={width}, theater={theater}"
            );
            state.mini_closed = true;
            assert!(!draw(&mut state, 0.0, 2.4));
            state.mini_closed = false;
            state.watch_scroll_to_video = true;
            for frame in 0..4 {
                assert!(!draw(&mut state, 0.0, 2.5 + frame as f64 * 0.1));
            }
            assert!(!state.watch_scroll_to_video);
        }
    }

    fn row(id: &str, replies: u32) -> CommentRow {
        CommentRow {
            id: id.to_owned(),
            author: Some("@someone".to_owned()),
            author_id: Some("UCx".to_owned()),
            author_avatar: None,
            text: vec![Span::Text("a comment ".repeat(40))],
            image: None,
            published: "1 day ago".to_owned(),
            likes: Some(3),
            replies,
            replies_token: None,
            pinned: false,
            by_owner: false,
            hearted: true,
        }
    }

    #[test]
    fn cached_comment_heights_preserve_scroll_geometry_without_requesting_offscreen_avatars() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let state = State::default();
        let comments: Vec<_> = (0..1000)
            .map(|index| {
                let mut comment = row(&format!("comment-{index}"), 0);
                comment.author_avatar =
                    Some(format!("https://i.ytimg.com/synthetic-avatar-{index}"));
                comment.image = Some(format!("https://example.invalid/comment-{index}.gif"));
                comment
            })
            .collect();
        let mut heights = Vec::new();
        for _ in 0..3 {
            let mut height = 0.0;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(900.0, 480.0))),
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let top = ui.next_widget_position().y;
                            for comment in &comments {
                                comment_row(ui, &state, comment, 40.0, &mut Vec::new());
                            }
                            height = ui.next_widget_position().y - top;
                        });
                    });
                },
            );
            output.textures_delta.clear();
            heights.push(height);
        }
        assert!(
            heights
                .iter()
                .all(|height| (height - heights[0]).abs() < 1.0),
            "comment heights drifted: {heights:?}"
        );
        let drawn = youtfast_app::images::drawn_count(&ctx);
        assert!(drawn > 0 && drawn < 10, "drawn avatars: {drawn}");
    }

    /// Replies drawn inside a comment's column, in either layout and
    /// either mode, lay out without a NaN or a negative rect (egui's debug
    /// assertions panic on either).
    #[test]
    fn the_watch_page_draws_a_comment_with_its_replies_open() {
        for (width, theater, light) in [
            (480.0, false, true),
            (900.0, false, false),
            (1600.0, false, true),
            (1600.0, true, false),
        ] {
            let mut top = row("top", 1);
            top.text.clear();
            top.image = Some("https://example.invalid/top.gif".into());
            let mut state = State {
                view: youtfast_core::model::View::Watch,
                watching: Some(Video::from_id("aaaaaaaaaaa".to_owned())),
                details: Fetch::Ready(youtfast_core::model::Details {
                    comments_token: Some("t".to_owned()),
                    comment_count: Some(1),
                    ..Default::default()
                }),
                comments: Fetch::Ready(vec![top]),
                palette: if light {
                    Palette::light()
                } else {
                    Palette::dark()
                },
                ..State::default()
            };
            state.settings.theater = theater;
            let ctx = egui::Context::default();
            // The app's named weights, as `App::new` installs them.
            youtfast_appearance::theme::install_fonts(&ctx);
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(width, 3000.0))),
                ..Default::default()
            };
            // The replies on their way, then arrived.
            let mut reply = row("reply", 0);
            reply.image = Some("https://example.invalid/reply.webp".into());
            for replies in [Fetch::Loading, Fetch::Ready(vec![reply])] {
                state.replies.insert("top".to_owned(), replies);
                for _ in 0..3 {
                    let mut output = ctx.run_ui(input.clone(), |ui| {
                        let mut actions = Vec::new();
                        show(ui, &mut state, None, None, &mut actions);
                    });
                    output.textures_delta.clear();
                }
            }
        }
    }

    fn largest_black_rect(shape: &egui::Shape) -> Option<Rect> {
        match shape {
            egui::Shape::Rect(rect) if rect.fill == Color32::BLACK => Some(rect.rect),
            egui::Shape::Vec(shapes) => shapes
                .iter()
                .filter_map(largest_black_rect)
                .max_by(|a, b| (a.width() * a.height()).total_cmp(&(b.width() * b.height()))),
            _ => None,
        }
    }

    #[test]
    fn the_video_box_keeps_every_landscape_aspect_and_uses_16_9_for_portrait() {
        for aspect in [
            16.0 / 9.0,
            16.0 / 10.0,
            3.0 / 2.0,
            4.0 / 3.0,
            5.0 / 4.0,
            1.85,
            2.0,
            2.35,
            2.39,
            21.0 / 9.0,
        ] {
            let video = video_box(vec2(1280.0, 708.0), aspect);
            assert!(
                (video.x / video.y - aspect as f32).abs() < 0.001,
                "aspect {aspect} became {}x{}",
                video.x,
                video.y
            );
        }
        for aspect in [1.0, 4.0 / 5.0, 9.0 / 16.0] {
            let video = video_box(vec2(1280.0, 708.0), aspect);
            assert!(
                (video.x / video.y - 16.0 / 9.0).abs() < 0.001,
                "square or portrait aspect {aspect} must use the 16:9 box"
            );
        }
        for aspect in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e20, f64::MAX] {
            let video = video_box(vec2(1280.0, 708.0), aspect);
            assert!((video.x / video.y - 16.0 / 9.0).abs() < 0.001);
        }
        // YouTube's 360 px short-window target keeps 16:9 and 4:3 readable
        // when the column is wide enough; wider videos only get 240 px.
        let short = video_box(vec2(639.6, 305.5), 16.0 / 9.0);
        assert!((short.y - 360.0).abs() < 0.3);
        let four_three = video_box(vec2(639.6, 305.5), 4.0 / 3.0);
        assert!((four_three.x - 480.0).abs() < 0.1 && (four_three.y - 360.0).abs() < 0.1);
        let ultrawide = video_box(vec2(639.6, 200.0), 21.0 / 9.0);
        assert!((ultrawide.x - 560.0).abs() < 0.1 && (ultrawide.y - 240.0).abs() < 0.1);
    }

    #[test]
    fn watch_columns_match_youtubes_aspect_cap_reserve_and_sidebar() {
        let wide = vec2(1757.0, 844.0);
        let sixteen_nine = watch_columns(wide, 16.0 / 9.0);
        assert!((sixteen_nine.video.x - 1280.0).abs() < 0.1);
        assert!((sixteen_nine.video.y - 720.0).abs() < 0.1);
        assert!((sixteen_nine.sidebar - 453.0).abs() < 0.1);
        assert!(sixteen_nine.indent.abs() < 0.1);
        assert!(sixteen_nine.large);

        let four_three = watch_columns(wide, 4.0 / 3.0);
        assert!((four_three.video.x - 1029.33).abs() < 0.1);
        assert!((four_three.video.y - 772.0).abs() < 0.1);
        assert!((four_three.sidebar - 600.0).abs() < 0.1);
        assert!((four_three.indent - 51.83).abs() < 0.1);
        assert!(four_three.large);

        let ultrawide = watch_columns(wide, 21.0 / 9.0);
        assert!((ultrawide.video.x - 1280.0).abs() < 0.1);
        assert!((ultrawide.video.y - 548.57).abs() < 0.1);
        assert!((ultrawide.sidebar - 453.0).abs() < 0.1);
        assert!(ultrawide.indent.abs() < 0.1);

        let portrait = watch_columns(wide, 9.0 / 16.0);
        assert!((portrait.video.x / portrait.video.y - 16.0 / 9.0).abs() < 0.001);

        let capped = watch_columns(vec2(2200.0, 1600.0), 16.0 / 9.0);
        assert!((capped.video.x - 1280.0).abs() < 0.1);
        assert!((capped.video.y - 720.0).abs() < 0.1);
        assert!((capped.sidebar - 600.0).abs() < 0.1);
        assert!((capped.indent - 148.0).abs() < 0.1);

        // A synthetic content area where the 380 px sidebar leaves 639.6 px
        // for the video. The 72 px reserve leaves 401.5 px: 16:9 is
        // width-limited, while 4:3 rises to 535x401.
        let small = vec2(1043.6, 473.5);
        let small_sixteen_nine = watch_columns(small, 16.0 / 9.0);
        assert!((small_sixteen_nine.video.x - 639.6).abs() < 0.2);
        assert!((small_sixteen_nine.video.y - 360.0).abs() < 0.3);
        assert!((small_sixteen_nine.sidebar - 380.0).abs() < 0.1);
        assert!(small_sixteen_nine.indent.abs() < 0.1);
        assert!(!small_sixteen_nine.large);

        let small_four_three = watch_columns(small, 4.0 / 3.0);
        assert!((small_four_three.video.x - 535.33).abs() < 0.2);
        assert!((small_four_three.video.y - 401.5).abs() < 0.2);
        assert!((small_four_three.sidebar - 484.27).abs() < 0.2);
        assert!(small_four_three.indent.abs() < 0.1);
        assert!(small_four_three.large);

        let small_ultrawide = watch_columns(small, 21.0 / 9.0);
        assert!((small_ultrawide.video.x - 639.6).abs() < 0.2);
        assert!((small_ultrawide.video.y - 274.1).abs() < 0.2);
        assert!((small_ultrawide.sidebar - 380.0).abs() < 0.1);
        assert!(small_ultrawide.indent.abs() < 0.1);
        assert!(!small_ultrawide.large);
    }

    #[test]
    fn narrow_wide_layouts_are_width_limited_below_the_height_targets() {
        // At 1000 px the wide layout leaves 596 px for the video, narrower
        // than 16:9 at 360 px, so the width sets 335 px. The target applies
        // when the column permits it.
        let columns = watch_columns(vec2(1000.0, 300.0), 16.0 / 9.0);
        assert!((columns.video.x - 596.0).abs() < 0.1);
        assert!((columns.video.y - 335.25).abs() < 0.1);
        // Theater follows YouTube's viewport formula without a minimum.
        let theater = theater_columns(vec2(1000.0, 300.0));
        assert!((theater.video.x - 1000.0).abs() < 0.1);
        assert!((theater.video.y - 187.0).abs() < 0.1);
    }

    #[test]
    fn theater_keeps_the_channel_row_above_the_fold_and_the_up_next_sidebar() {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let related = Video::from_id("bbbbbbbbbbb".to_owned());
        let next = Video::from_id("ccccccccccc".to_owned());
        let mut state = State {
            view: youtfast_core::model::View::Watch,
            watching: Some(Video::from_id("aaaaaaaaaaa".to_owned())),
            up_next: vec![next],
            details: Fetch::Ready(youtfast_core::model::Details {
                channel: Channel {
                    id: "UCdemo".to_owned(),
                    name: "Demo Channel".to_owned(),
                    avatar: None,
                },
                subscribers: Some(1_234),
                related: vec![related],
                ..Default::default()
            }),
            ..State::default()
        };
        state.settings.theater = true;
        let status = Status {
            paused: true,
            video_aspect: 16.0 / 9.0,
            ..Status::default()
        };
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
                ..Default::default()
            },
            |ui| {
                let mut actions = Vec::new();
                show(ui, &mut state, Some(&status), None, &mut actions);
            },
        );
        let video = output
            .shapes
            .iter()
            .filter_map(|shape| largest_black_rect(&shape.shape))
            .max_by(|a, b| (a.width() * a.height()).total_cmp(&(b.width() * b.height())))
            .expect("the theater picture must be black");

        fn text_rect(shape: &egui::Shape, wanted: &str) -> Option<Rect> {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == wanted => {
                    Some(Rect::from_min_size(text.pos, text.galley.size()))
                }
                egui::Shape::Text(_) => None,
                egui::Shape::Vec(shapes) => {
                    shapes.iter().find_map(|shape| text_rect(shape, wanted))
                }
                _ => None,
            }
        }
        let channel = output
            .shapes
            .iter()
            .find_map(|shape| text_rect(&shape.shape, "Demo Channel"))
            .expect("the channel row must be drawn");
        let up_next = output
            .shapes
            .iter()
            .find_map(|shape| text_rect(&shape.shape, "Up next"))
            .expect("theater must keep the Up next sidebar");
        output.textures_delta.clear();
        assert!((video.width() - 1232.0).abs() < 0.5);
        // The 1280x800 screen leaves 784 px after the panel's top margin;
        // theater keeps YouTube's 113 px strip below the picture.
        assert!((video.height() - 671.0).abs() < 0.5);
        assert!(
            channel.bottom() <= 800.0 - 20.0,
            "the channel row must stay above the fold, not at {channel:?}"
        );
        // The 380 px sidebar starts after the 828 px metadata column, not
        // under the full-width player.
        assert!(
            up_next.min.x > 800.0,
            "Up next must stay in the right column, not at {up_next:?}"
        );
    }

    #[test]
    fn audio_only_keeps_the_thumbnail_at_16_9() {
        let status = Status {
            video_aspect: 4.0 / 3.0,
            ..Status::default()
        };
        let mut state = State::default();
        assert!((watch_aspect(&state, Some(&status)) - 4.0 / 3.0).abs() < 0.001);
        state.audio_only = true;
        assert!((watch_aspect(&state, Some(&status)) - 16.0 / 9.0).abs() < 0.001);

        state.audio_only = false;
        let idle = Status {
            video_aspect: 4.0 / 3.0,
            idle: true,
            ..Status::default()
        };
        assert!((watch_aspect(&state, Some(&idle)) - 16.0 / 9.0).abs() < 0.001);
        let unknown = Status::default();
        assert!((watch_aspect(&state, Some(&unknown)) - 16.0 / 9.0).abs() < 0.001);
    }

    #[test]
    fn the_watch_page_draws_the_video_at_its_reported_aspect() {
        for aspect in [16.0 / 9.0, 4.0 / 3.0, 21.0 / 9.0] {
            let ctx = egui::Context::default();
            theme::install_fonts(&ctx);
            let mut state = State {
                view: youtfast_core::model::View::Watch,
                watching: Some(Video::from_id("aaaaaaaaaaa".to_owned())),
                details: Fetch::Ready(youtfast_core::model::Details::default()),
                ..State::default()
            };
            let status = Status {
                paused: true,
                video_aspect: aspect,
                ..Status::default()
            };
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
                    ..Default::default()
                },
                |ui| {
                    let mut actions = Vec::new();
                    show(ui, &mut state, Some(&status), None, &mut actions);
                },
            );
            let video = output
                .shapes
                .iter()
                .filter_map(|shape| largest_black_rect(&shape.shape))
                .max_by(|a, b| (a.width() * a.height()).total_cmp(&(b.width() * b.height())))
                .expect("the video pane must be black");
            output.textures_delta.clear();
            assert!(
                (video.width() / video.height() - aspect as f32).abs() < 0.01,
                "aspect {aspect} drew a {}x{} pane",
                video.width(),
                video.height()
            );
            assert!(
                video.width() <= 1232.0 - SIDEBAR_WIDTH - GAP + 0.5,
                "the video must fit beside the Up next column"
            );
        }
    }
}
