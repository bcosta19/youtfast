//! The watch page, laid out as YouTube's: the video pane and its
//! controls, the title, the channel's line with the actions, the
//! description, chapters and comments, and the Up next pane beside them.

use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Frame, Layout, Rect, RichText, Sense, Slider,
    Stroke, Ui, UiBuilder, pos2, vec2,
};

use super::{results, widgets};
use crate::app::State;
use crate::icons::Icon;
use crate::model::{Action, Channel, CommentRow, Fetch, Quality, Rating, Resolution, Video};
use crate::mpv::{Status, VideoSurface};
use crate::util;

/// The "Up next" pane's width beside the video in a wide window.
const SIDEBAR_WIDTH: f32 = 380.0;
/// Narrower than this, the pane moves under the video.
const WIDE: f32 = 1000.0;

pub fn show(
    ui: &mut Ui,
    state: &mut State,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
    actions: &mut Vec<Action>,
) {
    egui::CentralPanel::default().show(ui, |ui| {
        // The video never takes more than most of the window's height, so
        // its controls stay in view.
        let max_video_height = ui.available_height() * 0.75;
        let Some(watching) = state.watching.clone() else {
            return;
        };
        let theater = state.settings.theater;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                // Room for the first panes' titles, which sit on their top
                // rule; the scroll area clips what is above it.
                ui.add_space(widgets::line(ui) / 2.0);
                // Theater mode: the video across the window, the columns
                // under it.
                if theater {
                    video_pane(
                        ui,
                        state,
                        &watching,
                        status,
                        surface,
                        max_video_height,
                        actions,
                    );
                }
                if ui.available_width() >= WIDE {
                    ui.horizontal_top(|ui| {
                        let gap = widgets::cell(ui);
                        let main_width = ui.available_width() - SIDEBAR_WIDTH - gap;
                        ui.vertical(|ui| {
                            ui.set_width(main_width);
                            if !theater {
                                video_pane(
                                    ui,
                                    state,
                                    &watching,
                                    status,
                                    surface,
                                    max_video_height,
                                    actions,
                                );
                            }
                            about_video(ui, state, &watching, status, actions);
                        });
                        ui.add_space(gap);
                        ui.vertical(|ui| {
                            ui.set_width(SIDEBAR_WIDTH);
                            up_next(ui, state, actions);
                            ui.add_space(widgets::line(ui));
                            related(ui, state, actions);
                        });
                    });
                } else {
                    if !theater {
                        video_pane(
                            ui,
                            state,
                            &watching,
                            status,
                            surface,
                            max_video_height,
                            actions,
                        );
                    }
                    about_video(ui, state, &watching, status, actions);
                    ui.add_space(widgets::line(ui));
                    up_next(ui, state, actions);
                    ui.add_space(widgets::line(ui));
                    related(ui, state, actions);
                }
            });
    });
}

/// The video over the whole window, with the controls over its bottom edge
/// while the pointer is there.
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
            video(ui, rect, state, status, surface, actions);
            let strip = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 96.0), rect.max);
            // The quality menu opens outside the strip; keep the strip while
            // it is open.
            let pinned = state.seek_preview.is_some() || ui.ctx().any_popup_open();
            if ui.rect_contains_pointer(strip) || pinned {
                ui.painter()
                    .rect_filled(strip, CornerRadius::ZERO, Color32::from_black_alpha(170));
                let mut bar = ui.new_child(
                    UiBuilder::new()
                        .max_rect(strip.shrink2(vec2(24.0, 16.0)))
                        .layout(Layout::top_down(Align::Min)),
                );
                controls(&mut bar, state, status, actions);
            }
        });
}

/// The pane with the video and its controls, titled with the channel's
/// name.
fn video_pane(
    ui: &mut Ui,
    state: &mut State,
    watching: &Video,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
    max_video_height: f32,
    actions: &mut Vec<Action>,
) {
    let pane_title = watching
        .channel
        .clone()
        .or_else(|| watching.channel_id.clone())
        .unwrap_or_else(|| "watch".to_owned());
    let palette = state.palette.clone();
    widgets::pane(ui, &palette, &pane_title, None, false, None, |ui| {
        let width = ui.available_width();
        let height = (width * 9.0 / 16.0).min(max_video_height);
        let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        video(ui, rect, state, status, surface, actions);
        ui.add_space(4.0);
        controls(ui, state, status, actions);
    });
    ui.add_space(widgets::line(ui));
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
    let palette = state.palette.clone();
    // A video opened from a link has no title until mpv reads it.
    let title = Some(watching.title.as_str())
        .filter(|title| !title.is_empty())
        .or_else(|| status.and_then(|status| status.title.as_deref()))
        .unwrap_or("Loading…")
        .to_owned();
    ui.label(
        RichText::new(title)
            .font(FontId::monospace(16.0))
            .strong()
            .color(palette.text),
    );
    ui.add_space(widgets::line(ui) / 2.0);
    channel_line(ui, state, watching, status, actions);
    ui.add_space(widgets::line(ui) / 2.0);
    description(ui, state, watching, actions);
    chapters(ui, state, status, actions);
    comments(ui, state, actions);
}

/// YouTube's line under the title: the channel's avatar, name and
/// subscribers with `[ subscribe ]`, then the rating, `[ share ]` and
/// `[ save ]` at the right. Filled from rustypipe's details when they
/// arrive; the spinner or the error until then.
fn channel_line(
    ui: &mut Ui,
    state: &State,
    watching: &Video,
    status: Option<&Status>,
    actions: &mut Vec<Action>,
) {
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
    let avatar = 2.0 * widgets::line(ui);
    let mut wrapped = false;
    ui.horizontal(|ui| {
        match channel.as_ref().and_then(|channel| channel.avatar.clone()) {
            Some(url) => {
                ui.add(
                    super::images::remote(ui, &url)
                        .fit_to_exact_size(vec2(avatar, avatar))
                        .corner_radius(CornerRadius::ZERO),
                );
            }
            None => {
                let (rect, _) = ui.allocate_exact_size(vec2(avatar, avatar), Sense::hover());
                ui.painter()
                    .rect_filled(rect, CornerRadius::ZERO, palette.surface);
            }
        }
        ui.vertical(|ui| {
            let name = channel
                .as_ref()
                .map(|channel| channel.name.clone())
                .or_else(|| watching.channel.clone())
                .unwrap_or_default();
            let link = ui.link(RichText::new(name).monospace().strong().color(palette.blue));
            if link.clicked()
                && let Some(channel) = &channel
            {
                actions.push(Action::OpenChannel(channel.clone()));
            }
            let under = match &state.details {
                Fetch::Ready(details) => details
                    .subscribers
                    .map(|count| format!("{} subscribers", util::compact(count)))
                    .unwrap_or_default(),
                Fetch::Failed(_) => String::new(),
                _ => format!("{} details", widgets::spinner_frame(ui)),
            };
            ui.label(
                RichText::new(under)
                    .monospace()
                    .small()
                    .color(palette.secondary),
            );
        });
        if let Some(channel) = &channel {
            widgets::subscribe(ui, state, channel, widgets::ButtonStyle::Primary, actions);
        }
        // The actions sit at the right while they fit; under it otherwise.
        let needed = 44.0 * widgets::cell(ui);
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
            ui.label(RichText::new(error.as_str()).monospace().color(palette.red));
            if widgets::bracket_button(ui, palette, "r retry", widgets::ButtonStyle::Normal)
                .clicked()
            {
                actions.push(Action::Refresh);
            }
        });
    }
}

/// The rating, `[ share ]` and `[ save ]`, in that order as read;
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
        rating_buttons(ui, state, true, actions);
    } else {
        rating_buttons(ui, state, false, actions);
        share_menu(ui, state, watching, status, actions);
        save_menu(ui, state, watching, actions);
    }
}

/// `[ ▲ 19K ]` and `[ ▼ ]`: the account's rating, lit when given; a click
/// on the lit one takes it back. The count moves with the rating given
/// here. Without an account they only show the count.
fn rating_buttons(ui: &mut Ui, state: &State, right_to_left: bool, actions: &mut Vec<Action>) {
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
    let enabled = state.signed_in() && rating.is_some();
    let hint = if !state.signed_in() {
        "Sign in to rate videos"
    } else if matches!(state.rating, Fetch::Failed(_)) {
        "Your rating could not be read"
    } else {
        ""
    };
    let style = |on: bool| {
        if on {
            widgets::ButtonStyle::On
        } else {
            widgets::ButtonStyle::Normal
        }
    };
    let like = |ui: &mut Ui, actions: &mut Vec<Action>| {
        let label = match likes {
            Some(likes) => format!("▲ {}", util::compact(likes)),
            None => "▲".to_owned(),
        };
        let liked = rating == Some(Rating::Like);
        let response = ui
            .add_enabled_ui(enabled, |ui| {
                widgets::bracket_button(ui, palette, &label, style(liked))
            })
            .inner;
        let response = if hint.is_empty() {
            response.on_hover_text(if liked { "Liked" } else { "I like this" })
        } else {
            response.on_disabled_hover_text(hint)
        };
        if response.clicked() {
            actions.push(Action::Rate(if liked {
                Rating::None
            } else {
                Rating::Like
            }));
        }
    };
    let dislike = |ui: &mut Ui, actions: &mut Vec<Action>| {
        let disliked = rating == Some(Rating::Dislike);
        let response = ui
            .add_enabled_ui(enabled, |ui| {
                widgets::bracket_button(ui, palette, "▼", style(disliked))
            })
            .inner;
        let response = if hint.is_empty() {
            response.on_hover_text(if disliked {
                "Disliked"
            } else {
                "I dislike this"
            })
        } else {
            response.on_disabled_hover_text(hint)
        };
        if response.clicked() {
            actions.push(Action::Rate(if disliked {
                Rating::None
            } else {
                Rating::Dislike
            }));
        }
    };
    if right_to_left {
        dislike(ui, actions);
        like(ui, actions);
    } else {
        like(ui, actions);
        dislike(ui, actions);
    }
}

/// `[ share ]`: the link, `Start at` the moment playing, copy, and each
/// site's share page.
fn share_menu(
    ui: &mut Ui,
    state: &State,
    watching: &Video,
    status: Option<&Status>,
    actions: &mut Vec<Action>,
) {
    let palette = &state.palette;
    let response = widgets::bracket_button(ui, palette, "share", widgets::ButtonStyle::Normal);
    egui::Popup::menu(&response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(260.0);
            let position = status.map_or(0.0, |status| status.position);
            let at = state.share_at_time.then_some(position as u64);
            let link = util::video_link(&watching.id, at);
            ui.label(RichText::new(&link).monospace().color(palette.text));
            let mut start_at = state.share_at_time;
            if ui
                .checkbox(
                    &mut start_at,
                    RichText::new(format!("Start at {}", util::duration(position))).monospace(),
                )
                .changed()
            {
                actions.push(Action::SetShareAtTime(start_at));
            }
            if widgets::bracket_button(ui, palette, "copy link", widgets::ButtonStyle::Primary)
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
                if widgets::bracket_button(ui, palette, site, widgets::ButtonStyle::Normal)
                    .clicked()
                {
                    actions.push(Action::OpenUrl(url));
                    ui.close();
                }
            }
        });
}

/// `[ save ]`: Watch later on this computer and, signed in, the account's
/// playlists, each a box to tick. The account's list is asked for when
/// the menu first opens.
fn save_menu(ui: &mut Ui, state: &State, watching: &Video, actions: &mut Vec<Action>) {
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
    let label = if local || account { "saved" } else { "save" };
    let response = widgets::bracket_button(ui, palette, label, style);
    egui::Popup::menu(&response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(240.0);
            let mut on_device = local;
            if ui
                .checkbox(
                    &mut on_device,
                    RichText::new("Watch later on this computer").monospace(),
                )
                .changed()
            {
                actions.push(Action::ToggleWatchLater(watching.clone()));
            }
            ui.separator();
            if !state.signed_in() {
                ui.label(
                    RichText::new("Sign in to save to the account's playlists")
                        .monospace()
                        .small()
                        .color(palette.dim),
                );
                return;
            }
            match &state.save_targets {
                Fetch::Idle => actions.push(Action::LoadSaveTargets),
                Fetch::Loading => results::loading(ui, state, "playlists"),
                Fetch::Failed(error) => {
                    ui.label(RichText::new(error.as_str()).monospace().color(palette.red));
                }
                Fetch::Ready(targets) => {
                    for target in targets {
                        let mut saved = target.saved;
                        if ui
                            .checkbox(&mut saved, RichText::new(&target.name).monospace())
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

/// The description pane, YouTube's grey box: `views · published` on its
/// first line, then the text collapsed to three rows with `[ more ]`.
/// Expanded, the counts are exact and the date is added.
fn description(ui: &mut Ui, state: &State, watching: &Video, actions: &mut Vec<Action>) {
    let details = state.details.ready();
    let open = state.description_open;
    let views = details.map(|details| details.views).or(watching.views);
    let views = views.map(|views| {
        if open {
            format!("{} views", util::thousands(views))
        } else {
            util::views(views)
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
    widgets::pane(
        ui,
        &state.palette,
        "description",
        None,
        false,
        Some(state.palette.surface),
        |ui| {
            ui.set_width(ui.available_width());
            if !head.is_empty() {
                ui.label(
                    RichText::new(head)
                        .monospace()
                        .strong()
                        .color(state.palette.text),
                );
            }
            let Some(spans) = spans.filter(|spans| !spans.is_empty()) else {
                return;
            };
            let collapsed = !open && widgets::spans_truncated(ui, spans, 3);
            let max_rows = if collapsed { Some(3) } else { None };
            if let Some(action) = widgets::rich_text(ui, &state.palette, spans, max_rows) {
                actions.push(action);
            }
            if collapsed || open {
                let label = if collapsed { "more" } else { "less" };
                if widgets::bracket_button(ui, &state.palette, label, widgets::ButtonStyle::Normal)
                    .clicked()
                {
                    actions.push(Action::ToggleDescription);
                }
            }
        },
    );
}

/// The chapters pane: `[00:00] Intro` rows, the current one selected.
fn chapters(ui: &mut Ui, state: &State, status: Option<&Status>, actions: &mut Vec<Action>) {
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
    ui.add_space(widgets::line(ui));
    widgets::pane(
        ui,
        &state.palette,
        "chapters",
        Some(details.chapters.len()),
        false,
        None,
        |ui| {
            for (index, chapter) in details.chapters.iter().enumerate() {
                let parts = [
                    (
                        format!("[{}]", util::duration(f64::from(chapter.seconds))),
                        state.palette.yellow,
                    ),
                    (format!(" {}", chapter.name), state.palette.text),
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

/// The comments pane: one [`comment_row`] each, then the next page's
/// `[ load more ]`.
fn comments(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let Fetch::Ready(details) = &state.details else {
        return;
    };
    if details.comments_token.is_none() && details.comment_count.is_none() {
        return;
    }
    ui.add_space(widgets::line(ui));
    let count = details
        .comment_count
        .map(|count| count as usize)
        .or_else(|| state.comments.ready().map(|rows| rows.len()));
    widgets::pane(
        ui,
        &state.palette,
        "comments",
        count,
        false,
        None,
        |ui| match &state.comments {
            Fetch::Idle => {}
            Fetch::Loading => results::loading(ui, state, "comments"),
            Fetch::Failed(error) => results::failed(ui, state, error, actions),
            Fetch::Ready(rows) => {
                if rows.is_empty() {
                    ui.label(
                        RichText::new("-- no comments --")
                            .monospace()
                            .color(state.palette.dim),
                    );
                }
                let avatar = 2.0 * widgets::line(ui);
                for comment in rows.clone() {
                    comment_row(ui, state, &comment, avatar, actions);
                }
                // The tail row: the page on its way, a failed page's
                // retry, or the button.
                match &state.comments_more {
                    Fetch::Loading => results::loading(ui, state, "comments"),
                    Fetch::Failed(error) => {
                        ui.label(
                            RichText::new(error.as_str())
                                .monospace()
                                .color(state.palette.red),
                        );
                        if widgets::bracket_button(
                            ui,
                            &state.palette,
                            "retry",
                            widgets::ButtonStyle::Normal,
                        )
                        .clicked()
                        {
                            actions.push(Action::LoadMoreComments);
                        }
                    }
                    _ => {
                        if state.comments_next.is_some()
                            && widgets::bracket_button(
                                ui,
                                &state.palette,
                                "load more",
                                widgets::ButtonStyle::Normal,
                            )
                            .clicked()
                        {
                            actions.push(Action::LoadMoreComments);
                        }
                    }
                }
            }
        },
    );
}

/// The rows a long comment shows until `[ read more ]`.
const COMMENT_ROWS: usize = 5;

/// One comment as on YouTube: the author's avatar at the left; beside it
/// `@author · 3 days ago · pinned`, the text (long ones cut to a few rows),
/// then `▲ 120 ♥ [ 4 replies ]`, and the replies under the `│` rule when
/// open. `avatar` is the avatar's side: replies draw it smaller.
fn comment_row(
    ui: &mut Ui,
    state: &State,
    comment: &CommentRow,
    avatar: f32,
    actions: &mut Vec<Action>,
) {
    // YouTube's handles already start with `@`.
    let author = comment
        .author
        .as_deref()
        .unwrap_or("someone")
        .trim_start_matches('@');
    let channel = comment.author_id.as_ref().map(|id| Channel {
        id: id.clone(),
        name: author.to_owned(),
        avatar: comment.author_avatar.clone(),
    });
    let palette = &state.palette;
    ui.horizontal_top(|ui| {
        let (rect, response) = ui.allocate_exact_size(vec2(avatar, avatar), Sense::click());
        match &comment.author_avatar {
            Some(url) => super::images::remote(ui, url)
                .fit_to_exact_size(rect.size())
                .corner_radius(CornerRadius::ZERO)
                .paint_at(ui, rect),
            None => {
                ui.painter()
                    .rect_filled(rect, CornerRadius::ZERO, palette.surface);
                let initial: String = author.chars().take(1).collect();
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    initial.to_uppercase(),
                    FontId::monospace(avatar / 2.0),
                    palette.secondary,
                );
            }
        }
        if let Some(channel) = &channel
            && response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
        {
            actions.push(Action::OpenChannel(channel.clone()));
        }
        ui.add_space(widgets::cell(ui));
        ui.vertical(|ui| {
            ui.set_width(ui.available_width());
            comment_header(ui, state, comment, author, channel.as_ref(), actions);
            let open = state.comments_open.contains(&comment.id);
            let collapsed = !open && widgets::spans_truncated(ui, &comment.text, COMMENT_ROWS);
            let max_rows = collapsed.then_some(COMMENT_ROWS);
            if let Some(action) = widgets::rich_text(ui, palette, &comment.text, max_rows) {
                actions.push(action);
            }
            if (collapsed || open)
                && widgets::bracket_button(
                    ui,
                    palette,
                    if collapsed { "read more" } else { "show less" },
                    widgets::ButtonStyle::Normal,
                )
                .clicked()
            {
                actions.push(Action::ToggleCommentText(comment.id.clone()));
            }
            comment_footer(ui, state, comment, actions);
            if let Some(replies) = state.replies.get(&comment.id) {
                ui.add_space(widgets::line(ui) / 2.0);
                reply_column(ui, state, replies, actions);
            }
        });
    });
    ui.add_space(widgets::line(ui));
}

/// `@author · 3 days ago · pinned · by creator`; the author opens their
/// channel.
fn comment_header(
    ui: &mut Ui,
    state: &State,
    comment: &CommentRow,
    author: &str,
    channel: Option<&Channel>,
    actions: &mut Vec<Action>,
) {
    ui.horizontal(|ui| {
        let response = ui.link(
            RichText::new(format!("@{author}"))
                .monospace()
                .color(state.palette.blue),
        );
        if let Some(channel) = channel
            && response.clicked()
        {
            actions.push(Action::OpenChannel(channel.clone()));
        }
        let mut meta = vec![comment.published.clone()];
        if comment.pinned {
            meta.push("pinned".to_owned());
        }
        if comment.by_owner {
            meta.push("by creator".to_owned());
        }
        ui.label(
            RichText::new(format!("· {}", meta.join(" · ")))
                .monospace()
                .small()
                .color(state.palette.secondary),
        );
    });
}

/// Under the text: the likes, the creator's heart and the replies'
/// toggle.
fn comment_footer(ui: &mut Ui, state: &State, comment: &CommentRow, actions: &mut Vec<Action>) {
    let likes = comment.likes.filter(|likes| *likes > 0);
    if likes.is_none() && !comment.hearted && comment.replies == 0 {
        return;
    }
    ui.horizontal(|ui| {
        if let Some(likes) = likes {
            ui.label(
                RichText::new(format!("▲ {}", util::compact(u64::from(likes))))
                    .monospace()
                    .small()
                    .color(state.palette.secondary),
            );
        }
        if comment.hearted {
            ui.label(
                RichText::new("♥")
                    .monospace()
                    .small()
                    .color(state.palette.red),
            )
            .on_hover_text("Hearted by the creator");
        }
        if comment.replies > 0 {
            let label = if state.replies.contains_key(&comment.id) {
                "hide replies".to_owned()
            } else if comment.replies == 1 {
                "1 reply".to_owned()
            } else {
                format!("{} replies", comment.replies)
            };
            if widgets::bracket_button(ui, &state.palette, &label, widgets::ButtonStyle::Normal)
                .clicked()
            {
                actions.push(Action::ToggleReplies(comment.id.clone()));
            }
        }
    });
}

/// Replies two cells in, a 1 px `outline` rule at their left. Laid out in
/// the parent's flow: a child with an unbounded height made egui's
/// alignment NaN under the loading line.
fn reply_column(
    ui: &mut Ui,
    state: &State,
    replies: &Fetch<Vec<CommentRow>>,
    actions: &mut Vec<Action>,
) {
    let cell = widgets::cell(ui);
    let column = ui.horizontal_top(|ui| {
        // The rule's column, left of the replies.
        ui.add_space(2.0 * cell);
        ui.vertical(|ui| match replies {
            Fetch::Idle | Fetch::Loading => results::loading(ui, state, "replies"),
            Fetch::Failed(error) => results::failed(ui, state, error, actions),
            Fetch::Ready(rows) => {
                let avatar = 1.5 * widgets::line(ui);
                for reply in rows.clone() {
                    comment_row(ui, state, &reply, avatar, actions);
                }
            }
        });
    });
    let rect = column.response.rect;
    ui.painter().vline(
        rect.left() + cell,
        rect.y_range(),
        Stroke::new(1.0, state.palette.outline),
    );
}

/// The picture, with the spinner or the error over it. A click pauses; a
/// double click goes full screen.
fn video(
    ui: &mut Ui,
    rect: Rect,
    state: &State,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
    actions: &mut Vec<Action>,
) {
    let response = ui.interact(rect, ui.id().with("video"), Sense::click());
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, Color32::BLACK);
    if let Some(surface) = surface {
        surface.paint(ui, rect);
    }
    if let Some(error) = &state.player_error {
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            error,
            FontId::monospace(16.0),
            Color32::WHITE,
        );
    } else if status.is_some_and(|status| {
        !status.idle && (status.buffering || (status.position <= 0.0 && !status.paused))
    }) {
        egui::Spinner::new()
            .size(40.0)
            .color(Color32::WHITE)
            .paint_at(ui, Rect::from_center_size(rect.center(), vec2(40.0, 40.0)));
        ui.ctx().request_repaint();
    }
    if response.double_clicked() {
        actions.push(Action::ToggleFullscreen);
    } else if response.clicked() {
        actions.push(Action::TogglePause);
    }
}

fn controls(ui: &mut Ui, state: &mut State, status: Option<&Status>, actions: &mut Vec<Action>) {
    let Some(status) = status else {
        return;
    };
    ui.horizontal(|ui| {
        let icon = if status.paused {
            Icon::Play
        } else {
            Icon::Pause
        };
        if widgets::icon_button(ui, icon, 20.0, "Play or pause (Space)").clicked() {
            actions.push(Action::TogglePause);
        }
        ui.add(Icon::Volume.image(ui.visuals().text_color(), 18.0));
        ui.spacing_mut().slider_width = 90.0;
        let mut volume = status.volume;
        if ui
            .add(Slider::new(&mut volume, 0.0..=100.0).show_value(false))
            .changed()
        {
            actions.push(Action::SetVolume(volume));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let (icon, label) = if state.fullscreen {
                (Icon::ExitFullscreen, "Exit full screen (F)")
            } else {
                (Icon::Fullscreen, "Full screen (F)")
            };
            if widgets::icon_button(ui, icon, 18.0, label).clicked() {
                actions.push(Action::ToggleFullscreen);
            }
            // Full screen has no page layout to change.
            if !state.fullscreen {
                let label = if state.settings.theater {
                    "Default view (T)"
                } else {
                    "Theater mode (T)"
                };
                if widgets::icon_button(ui, Icon::Theater, 18.0, label).clicked() {
                    actions.push(Action::ToggleTheater);
                }
            }
            quality_menu(ui, state, status, actions);
        });
    });
    seek_bar(ui, state, status, actions);
}

/// The quality chosen and the resolution playing, with the decoding details
/// on hover. Lists what the video offers once it has loaded, and the usual
/// resolutions before that.
fn quality_menu(ui: &mut Ui, state: &State, status: &Status, actions: &mut Vec<Action>) {
    let playing = (status.video_width > 0 && status.video_height > 0).then(|| {
        Resolution::from_size(
            status.video_width as u32,
            status.video_height as u32,
            status.fps.round() as u32,
        )
    });
    let text = match (state.settings.quality, playing) {
        (Quality::Auto, Some(playing)) => format!("Auto · {}", playing.label()),
        (Quality::Auto, None) => "Auto".to_owned(),
        (_, Some(playing)) => playing.label(),
        (Quality::Max(lines), None) => format!("{lines}p"),
    };
    let offered: Vec<Resolution> = if state.resolutions.is_empty() {
        [2160, 1440, 1080, 720, 480, 360]
            .into_iter()
            .map(|lines| Resolution { lines, fps: 0 })
            .collect()
    } else {
        state.resolutions.clone()
    };
    let menu = egui::ComboBox::from_id_salt("quality")
        .selected_text(RichText::new(text).monospace())
        .width(130.0)
        .show_ui(ui, |ui| {
            let auto = format!("Auto (up to {}p)", Quality::AUTO);
            if ui
                .selectable_label(state.settings.quality == Quality::Auto, auto)
                .clicked()
            {
                actions.push(Action::SetQuality(Quality::Auto));
            }
            for resolution in offered {
                let quality = Quality::Max(resolution.lines);
                if ui
                    .selectable_label(state.settings.quality == quality, resolution.label())
                    .clicked()
                {
                    actions.push(Action::SetQuality(quality));
                }
            }
        });
    menu.response.on_hover_text(playback_details(status));
}

/// "1920×1080 · 60 fps · vp9 · hardware decoding (vaapi)".
fn playback_details(status: &Status) -> String {
    if status.video_width <= 0 || status.video_height <= 0 {
        return "Quality".to_owned();
    }
    let mut parts = vec![format!("{}×{}", status.video_width, status.video_height)];
    if status.fps > 0.0 {
        parts.push(format!("{:.0} fps", status.fps));
    }
    if let Some(codec) = &status.codec {
        parts.push(codec.clone());
    }
    parts.push(match &status.hwdec {
        Some(decoder) => format!("hardware decoding ({decoder})"),
        None => "software decoding".to_owned(),
    });
    parts.join(" · ")
}

/// The seek bar: a 1-line strip, played in accent, buffered in
/// surface_active, the rest in surface; a 1-cell block marks the position
/// while hovered or dragged. Times are `00:00 / 12:04` at its right.
fn seek_bar(ui: &mut Ui, state: &mut State, status: &Status, actions: &mut Vec<Action>) {
    let palette = state.palette.clone();
    ui.horizontal(|ui| {
        let time = {
            let position = state.seek_preview.unwrap_or(status.position);
            if status.duration > 0.0 {
                format!(
                    "{} / {}",
                    util::duration(position),
                    util::duration(status.duration)
                )
            } else {
                util::duration(position)
            }
        };
        let time_width = ui
            .painter()
            .layout_no_wrap(
                "00:00 / 00:00".to_owned(),
                FontId::monospace(13.0),
                Color32::WHITE,
            )
            .size()
            .x
            + widgets::cell(ui);
        let width = ui.available_width() - time_width;
        let (rect, response) =
            ui.allocate_exact_size(vec2(width, widgets::line(ui)), Sense::click_and_drag());
        let duration = status.duration;
        if duration > 0.0 {
            let pointer = response.interact_pointer_pos().map(|pointer| {
                f64::from(((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0)) * duration
            });
            if response.drag_stopped() || response.clicked() {
                if let Some(seconds) = pointer.or(state.seek_preview) {
                    actions.push(Action::SeekTo {
                        seconds,
                        exact: true,
                    });
                }
                state.seek_preview = None;
            } else if response.dragged()
                && let Some(seconds) = pointer
            {
                if state
                    .seek_preview
                    .is_none_or(|shown| (shown - seconds).abs() >= 0.5)
                {
                    actions.push(Action::SeekTo {
                        seconds,
                        exact: false,
                    });
                }
                state.seek_preview = Some(seconds);
            }
        }

        let track = Rect::from_center_size(rect.center(), vec2(rect.width(), 6.0));
        let fraction = |seconds: f64| {
            if duration > 0.0 {
                (seconds / duration).clamp(0.0, 1.0) as f32
            } else {
                0.0
            }
        };
        let x_at = |seconds: f64| track.left() + track.width() * fraction(seconds);
        let position = state.seek_preview.unwrap_or(status.position);
        let painter = ui.painter();
        painter.rect_filled(track, CornerRadius::ZERO, palette.surface);
        let buffered = Rect::from_min_max(track.min, pos2(x_at(status.cached_until), track.max.y));
        painter.rect_filled(buffered, CornerRadius::ZERO, palette.surface_active);
        let played = Rect::from_min_max(track.min, pos2(x_at(position), track.max.y));
        painter.rect_filled(played, CornerRadius::ZERO, palette.accent);
        if (response.hovered() || response.dragged()) && duration > 0.0 {
            // A 1-cell block at the position.
            let cell = widgets::cell(ui);
            let block = Rect::from_center_size(
                pos2(
                    x_at(position).clamp(track.left() + cell / 2.0, track.right() - cell / 2.0),
                    track.center().y,
                ),
                vec2(cell, track.height() + 4.0),
            );
            painter.rect_filled(block, CornerRadius::ZERO, palette.accent);
        }
        ui.label(
            RichText::new(time)
                .monospace()
                .small()
                .color(palette.secondary),
        );
    });
}

fn up_next(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let watching = state.watching.as_ref().map(|video| video.id.as_str());
    let next: Vec<_> = state
        .up_next
        .iter()
        .filter(|video| Some(video.id.as_str()) != watching)
        .cloned()
        .collect();
    if next.is_empty() {
        return;
    }
    let palette = state.palette.clone();
    widgets::pane(
        ui,
        &palette,
        "Up next",
        Some(next.len()),
        false,
        None,
        |ui| {
            results::video_rows(ui, state, &next, actions);
        },
    );
}

/// The recommended videos rustypipe sent with the details.
fn related(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let Fetch::Ready(details) = &state.details else {
        return;
    };
    let watching = state.watching.as_ref().map(|video| video.id.as_str());
    let related: Vec<Video> = details
        .related
        .iter()
        .filter(|video| Some(video.id.as_str()) != watching)
        .cloned()
        .collect();
    if related.is_empty() {
        return;
    }
    widgets::pane(
        ui,
        &state.palette,
        "related",
        Some(related.len()),
        false,
        None,
        |ui| {
            results::video_rows(ui, state, &related, actions);
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Span;

    fn row(id: &str, replies: u32) -> CommentRow {
        CommentRow {
            id: id.to_owned(),
            author: Some("@someone".to_owned()),
            author_id: Some("UCx".to_owned()),
            author_avatar: None,
            text: vec![Span::Text("a comment ".repeat(40))],
            published: "1 day ago".to_owned(),
            likes: Some(3),
            replies,
            replies_token: None,
            pinned: false,
            by_owner: false,
            hearted: true,
        }
    }

    /// Replies drawn inside a comment's column, in either layout and
    /// either mode, lay out without a NaN or a negative rect (egui's debug
    /// assertions panic on either).
    #[test]
    fn the_watch_page_draws_a_comment_with_its_replies_open() {
        for (width, theater) in [(900.0, false), (1600.0, false), (1600.0, true)] {
            let mut state = State {
                view: crate::model::View::Watch,
                watching: Some(Video::from_id("aaaaaaaaaaa".to_owned())),
                details: Fetch::Ready(crate::model::Details {
                    comments_token: Some("t".to_owned()),
                    comment_count: Some(1),
                    ..Default::default()
                }),
                comments: Fetch::Ready(vec![row("top", 1)]),
                ..State::default()
            };
            state.settings.theater = theater;
            let ctx = egui::Context::default();
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(width, 3000.0))),
                ..Default::default()
            };
            // The replies on their way, then arrived.
            for replies in [Fetch::Loading, Fetch::Ready(vec![row("reply", 0)])] {
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
}
