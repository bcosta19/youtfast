//! Watch later, Liked videos, Playlists, and single playlist pages.

use egui::{RichText, Ui, vec2};
use youtfast_i18n::{format as trf, gettext as tr};

use super::{
    CARD_WIDTH, controls, filtered_empty_state, library_video_toolbar, rows_listing,
    toolbar_summary, video_card_grid,
};
use crate::{results, widgets};
use youtfast_app::{Action, Page, State};
use youtfast_appearance::theme;
use youtfast_core::model::{Listing, Source};

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
    use egui::Rect;
    use youtfast_app::AccountState;
    use youtfast_core::model::{Video, View};

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
