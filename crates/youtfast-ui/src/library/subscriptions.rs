//! The Subscriptions page and channel card grid shared with discovery.

use std::collections::HashMap;

use egui::text::LayoutJob;
use egui::{
    Color32, CornerRadius, CursorIcon, FontId, Rect, RichText, Sense, Stroke, Ui, pos2, vec2,
};
use youtfast_i18n::{Locale, format as trf, gettext as tr};

use super::{controls, filtered_empty_state};
use crate::{results, widgets};
use youtfast_app::{Action, ChannelActivity, State};
use youtfast_appearance::icons::Icon;
use youtfast_appearance::theme::{self, Palette, RADIUS};
use youtfast_core::model::{Channel, ChannelMeta, Listing, Source, SubscriptionSort, Video};

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
                Listing::Ready(channels)
                    if channels.is_empty() && state.subscriptions_more.is_none() =>
                {
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
            !state.library.is_channel_blocked(&channel.id)
                && activity
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
            widgets::SearchField::new(
                egui::Id::new("subscriptions search"),
                260.0_f32.min(ui.available_width()),
            ),
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
        ui.add_space(4.0);
        if widgets::toggle_chip(
            ui,
            palette,
            tr(locale, "Hide watched").as_ref(),
            state.settings.hide_watched,
        )
        .clicked()
        {
            actions.push(Action::SetHideWatched(!state.settings.hide_watched));
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
    let draw_more = |ui: &mut Ui, actions: &mut Vec<Action>| {
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
    };
    if visible.is_empty() {
        if state.subscriptions_query.trim().is_empty() {
            results::nothing(
                ui,
                state,
                tr(locale, "No channels match this filter").as_ref(),
            );
        } else {
            filtered_empty_state(
                ui,
                &state.palette,
                tr(locale, "No channels match this filter").as_ref(),
                tr(locale, "Clear filter").as_ref(),
                || actions.push(Action::SetSubscriptionsQuery(String::new())),
            );
        }
        draw_more(ui, actions);
        return;
    }
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            channel_grid_content(ui, state, visible, activity, removable, actions);
            draw_more(ui, actions);
        });
}

#[cfg(test)]
pub(crate) fn channel_rows(
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
pub(crate) fn channel_rows_content(
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
        .or_else(|| {
            (!state.settings.hide_watched)
                .then_some(activity.last_watched)
                .flatten()
                .map(|video| (video, false))
        });
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
        state,
        video,
        thumb_rect,
        widgets::watched(&state.library, video),
        locale,
    );

    let text_left = thumb_rect.right() + 10.0;
    let text_w = (strip_rect.right() - text_left - 10.0).max(20.0);
    let display_title = state.display_title(video);
    let title_galley = single_line_galley(
        ui,
        display_title,
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
        let hover = if state.dearrow_replacement_title(video).is_some() && !video.title.is_empty() {
            trf(
                locale,
                "Original title: {title}",
                &[("title", video.title.clone())],
            )
        } else {
            video.title.clone()
        };
        strip_resp.clone().on_hover_text(hover);
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

#[cfg(test)]
mod tests {
    use super::*;
    use youtfast_app::AccountState;
    use youtfast_core::model::View;

    #[test]
    fn blocked_account_page_still_offers_its_continuation() {
        use youtfast_youtube::innertube::{ContinuationEndpoint, More};

        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let channel = Channel {
            id: "UC_blocked".into(),
            name: "Blocked Channel".into(),
            avatar: None,
        };
        let mut state = State {
            view: View::Subscriptions,
            account: AccountState::SignedIn(1),
            source: Source::Account,
            account_subscriptions: Listing::Ready(vec![channel.clone()]),
            subscriptions_more: Some(More {
                ctoken: "next-page".into(),
                endpoint: ContinuationEndpoint::Browse,
                visitor_data: None,
                authenticated: true,
                ours: false,
                owner: None,
            }),
            ..State::default()
        };
        assert!(state.library.block_channel(channel));
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200.0, 800.0))),
                ..Default::default()
            },
            |ui| subscriptions(ui, &state, &mut Vec::new()),
        );
        let has_text = |label| {
            output.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == label)
            })
        };
        assert!(!has_text("Blocked Channel"));
        assert!(!has_text("Clear filter"));
        assert!(has_text("Load more"));
        output.textures_delta.clear();
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
    fn hide_watched_chip_emits_toggle_and_suppresses_last_watched_preview() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State {
            view: View::Subscriptions,
            library: youtfast_storage::library::Library::default(),
            ..State::default()
        };
        state.library.subscriptions = vec![Channel {
            id: "UC_ok".into(),
            name: "OK Channel".into(),
            avatar: None,
        }];
        let mut watched_vid = Video::from_id("wat00000001".into());
        watched_vid.title = "Already Watched Video".into();
        watched_vid.channel = Some("OK Channel".into());
        watched_vid.channel_id = Some("UC_ok".into());
        state.library.record_watch(watched_vid.clone(), 10);
        state.feed = Listing::Ready(vec![watched_vid]);
        state.settings.hide_watched = true;

        let mut chip_pos = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200.0, 800.0))),
                ..Default::default()
            },
            |ui| subscriptions(ui, &state, &mut Vec::new()),
        );
        let texts: Vec<String> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => {
                    if text.galley.text() == "Hide watched" {
                        chip_pos = Some(Rect::from_min_size(text.pos, text.galley.size()).center());
                    }
                    Some(text.galley.text().to_owned())
                }
                _ => None,
            })
            .collect();
        output.textures_delta.clear();
        assert!(!texts.iter().any(|t| t == "Already Watched Video"));
        assert!(!texts.iter().any(|t| t == "Last watched"));

        let pos = chip_pos.expect("Hide watched chip");
        let mut actions = Vec::new();
        for pressed in [true, false] {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200.0, 800.0))),
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
                |ui| subscriptions(ui, &state, &mut actions),
            );
            out.textures_delta.clear();
        }
        assert_eq!(actions, [Action::SetHideWatched(false)]);
    }
}
