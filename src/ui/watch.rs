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
use crate::model::{Action, Channel, CommentRow, Fetch, Quality, Resolution, Video};
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
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                // Room for the first panes' titles, which sit on their top
                // rule; the scroll area clips what is above it.
                ui.add_space(widgets::line(ui) / 2.0);
                if ui.available_width() >= WIDE {
                    ui.horizontal_top(|ui| {
                        let gap = widgets::cell(ui);
                        let main_width = ui.available_width() - SIDEBAR_WIDTH - gap;
                        ui.vertical(|ui| {
                            ui.set_width(main_width);
                            main_column(ui, state, status, surface, max_video_height, actions);
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
                    main_column(ui, state, status, surface, max_video_height, actions);
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

fn main_column(
    ui: &mut Ui,
    state: &mut State,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
    max_video_height: f32,
    actions: &mut Vec<Action>,
) {
    let Some(watching) = state.watching.clone() else {
        return;
    };
    // The video pane takes the channel's name.
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

    // Under the video, as on YouTube: the title, the channel's line with
    // the actions, then the description.
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
    channel_line(ui, state, &watching, actions);
    ui.add_space(widgets::line(ui) / 2.0);
    description(ui, state, &watching, actions);
    chapters(ui, state, status, actions);
    comments(ui, state, actions);
}

/// YouTube's line under the title: the channel's avatar, name and
/// subscribers with `[ subscribe ]`, then likes, `[ share ]` and
/// `[ watch later ]` at the right. Filled from rustypipe's details when
/// they arrive; the spinner or the error until then.
fn channel_line(ui: &mut Ui, state: &State, watching: &Video, actions: &mut Vec<Action>) {
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
                    egui::Image::new(url)
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
            subscribe(ui, state, channel, actions);
        }
        // The actions sit at the right while they fit; under it otherwise.
        let needed = 34.0 * widgets::cell(ui);
        if ui.available_width() >= needed {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                video_actions(ui, state, watching, true, actions);
            });
        } else {
            wrapped = true;
        }
    });
    if wrapped {
        ui.horizontal(|ui| video_actions(ui, state, watching, false, actions));
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

/// Likes, `[ share ]` and `[ watch later ]`, in that order as read;
/// `right_to_left` when the caller's layout runs that way.
fn video_actions(
    ui: &mut Ui,
    state: &State,
    watching: &Video,
    right_to_left: bool,
    actions: &mut Vec<Action>,
) {
    let palette = &state.palette;
    let likes = |ui: &mut Ui| {
        if let Some(likes) = state.details.ready().and_then(|details| details.likes) {
            ui.label(
                RichText::new(format!(" ▲ {} ", util::compact(u64::from(likes))))
                    .monospace()
                    .color(palette.text)
                    .background_color(palette.surface),
            )
            .on_hover_text("Likes. Rating a video needs youtube.com.");
        }
    };
    let share = |ui: &mut Ui, actions: &mut Vec<Action>| {
        if widgets::bracket_button(ui, palette, "share", widgets::ButtonStyle::Normal)
            .on_hover_text("Copy the link")
            .clicked()
        {
            actions.push(Action::CopyLink(format!(
                "https://youtu.be/{}",
                watching.id
            )));
        }
    };
    let saved = state.library.in_watch_later(&watching.id);
    if right_to_left {
        watch_later_button(ui, state, watching, saved, actions);
        share(ui, actions);
        likes(ui);
    } else {
        likes(ui);
        share(ui, actions);
        watch_later_button(ui, state, watching, saved, actions);
    }
}

/// The watch-later bracket button.
fn watch_later_button(
    ui: &mut Ui,
    state: &State,
    watching: &Video,
    saved: bool,
    actions: &mut Vec<Action>,
) {
    let (label, style) = if saved {
        ("saved", widgets::ButtonStyle::On)
    } else {
        ("watch later", widgets::ButtonStyle::Normal)
    };
    if widgets::bracket_button(ui, &state.palette, label, style)
        .on_hover_text("Watch later on this computer")
        .clicked()
    {
        actions.push(Action::ToggleWatchLater(watching.clone()));
    }
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

/// The comments pane: `@author · 3d · ▲ 120 · pinned` under the title,
/// the text under that, replies indented under a `│` rule.
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
                for comment in rows.clone() {
                    comment_row(ui, state, &comment, actions);
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

/// One comment, and its replies under the `│` rule when open.
fn comment_row(ui: &mut Ui, state: &State, comment: &CommentRow, actions: &mut Vec<Action>) {
    // `@author · 3d · ▲ 120 · pinned`
    ui.horizontal(|ui| {
        // YouTube's handles already start with `@`.
        let author = comment
            .author
            .as_deref()
            .unwrap_or("someone")
            .trim_start_matches('@');
        let response = ui.link(
            RichText::new(format!("@{author}"))
                .monospace()
                .color(state.palette.blue),
        );
        if response.clicked()
            && let Some(id) = &comment.author_id
        {
            actions.push(Action::OpenChannel(Channel {
                id: id.clone(),
                name: author.to_owned(),
                avatar: None,
            }));
        }
        let mut meta = vec![comment.published.clone()];
        if let Some(likes) = comment.likes.filter(|likes| *likes > 0) {
            meta.push(format!("▲ {}", util::compact(u64::from(likes))));
        }
        if comment.pinned {
            meta.push("pinned".to_owned());
        }
        if comment.by_owner {
            meta.push("by creator".to_owned());
        }
        if comment.hearted {
            meta.push("♥".to_owned());
        }
        ui.label(
            RichText::new(format!("· {}", meta.join(" · ")))
                .monospace()
                .small()
                .color(state.palette.secondary),
        );
    });
    if let Some(action) = widgets::rich_text(ui, &state.palette, &comment.text, None) {
        actions.push(action);
    }
    if comment.replies > 0 {
        let open = state.replies.contains_key(&comment.id);
        let label = if open {
            "hide replies".to_owned()
        } else {
            format!("{} replies", comment.replies)
        };
        if widgets::bracket_button(ui, &state.palette, &label, widgets::ButtonStyle::Normal)
            .clicked()
        {
            actions.push(Action::ToggleReplies(comment.id.clone()));
        }
        if let Some(replies) = state.replies.get(&comment.id) {
            ui.add_space(widgets::line(ui) / 2.0);
            reply_column(ui, state, replies, actions);
        }
    }
    ui.add_space(widgets::line(ui) / 2.0);
}

/// Replies two cells in, a 1 px `outline` rule at their left.
fn reply_column(
    ui: &mut Ui,
    state: &State,
    replies: &Fetch<Vec<CommentRow>>,
    actions: &mut Vec<Action>,
) {
    let cell = widgets::cell(ui);
    let width = ui.available_width() - 2.0 * cell;
    ui.horizontal(|ui| {
        let left = ui.cursor().left();
        let top = ui.cursor().top();
        let mut inner = ui.new_child(
            UiBuilder::new()
                .max_rect(Rect::from_min_size(
                    pos2(left + 2.0 * cell, top),
                    vec2(width, f32::INFINITY),
                ))
                .layout(Layout::top_down(Align::Min)),
        );
        match replies {
            Fetch::Idle | Fetch::Loading => results::loading(&mut inner, state, "replies"),
            Fetch::Failed(error) => results::failed(&mut inner, state, error, actions),
            Fetch::Ready(rows) => {
                for reply in rows.clone() {
                    comment_row(&mut inner, state, &reply, actions);
                }
            }
        }
        let bottom = inner.min_rect().bottom();
        // Leave the rule's column to the replies' left.
        ui.allocate_space(vec2(2.0 * cell + width, bottom - top));
        ui.painter().vline(
            left + cell,
            top..=bottom,
            Stroke::new(1.0, state.palette.outline),
        );
    });
}

/// Subscribe on this device, or show that the account already does.
fn subscribe(ui: &mut Ui, state: &State, channel: &Channel, actions: &mut Vec<Action>) {
    if state.subscribed_in_account(&channel.id) {
        ui.label(
            RichText::new("[ subscribed ]")
                .monospace()
                .color(state.palette.accent)
                .strong(),
        )
        .on_hover_text("Your account is subscribed. Manage it on youtube.com.");
        return;
    }
    let subscribed = state.library.is_subscribed(&channel.id);
    let (label, style) = if subscribed {
        ("subscribed", widgets::ButtonStyle::On)
    } else {
        ("subscribe", widgets::ButtonStyle::Primary)
    };
    let hint = if subscribed {
        "Unsubscribe on this computer"
    } else {
        "Follow this channel on this computer: its videos join your feed"
    };
    if widgets::bracket_button(ui, &state.palette, label, style)
        .on_hover_text(hint)
        .clicked()
    {
        actions.push(Action::ToggleSubscription(channel.clone()));
    }
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
            results::video_rows(ui, state, &next, &|_| None, actions);
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
            results::video_rows(ui, state, &related, &|_| None, actions);
        },
    );
}
