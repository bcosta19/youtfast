//! Watch later, Liked videos, Playlists, and single playlist pages.

use egui::{RichText, Ui, vec2};
use youtfast_i18n::{format as trf, gettext as tr};

use super::{
    CARD_WIDTH, controls, filtered_empty_state, library_video_toolbar, rows_listing,
    toolbar_summary, video_card_grid,
};
use crate::{results, widgets};
use youtfast_app::{Action, Page, State};
use youtfast_appearance::icons::Icon;
use youtfast_appearance::theme;
use youtfast_core::model::{Listing, PlaylistSummary, Source};
use youtfast_storage::library::DevicePlaylist;

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
        let total = state.library.playlists.len() + state.playlists.items().len();
        let visible = state.visible_playlists();
        results::title(ui, state, tr(locale, "Playlists").as_ref(), Some(total));
        playlists_toolbar(ui, state, total, &visible, actions);
        if !visible.is_empty() {
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
                                for (offset, playlist) in visible[start..end].iter().enumerate() {
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
                    if state.signed_in() {
                        match &state.playlists {
                            Listing::Loading => widgets::loading(ui, palette, "playlists", locale),
                            Listing::Failed(error) => {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(
                                            youtfast_i18n::message(locale, error).as_ref(),
                                        )
                                        .color(palette.red),
                                    );
                                    if widgets::button(
                                        ui,
                                        palette,
                                        tr(locale, "Retry").as_ref(),
                                        widgets::ButtonStyle::Normal,
                                    )
                                    .clicked()
                                    {
                                        actions.push(Action::Refresh);
                                    }
                                });
                            }
                            _ => widgets::more_row(
                                ui,
                                palette,
                                &state.playlists_more_state,
                                state.playlists_more.is_some(),
                                actions,
                                locale,
                            ),
                        }
                    }
                });
            return;
        }
        if state.signed_in() {
            match &state.playlists {
                Listing::Idle => return,
                Listing::Loading => {
                    results::loading(ui, state, "playlists");
                    return;
                }
                Listing::Failed(error) => {
                    results::failed(ui, state, error, actions);
                    return;
                }
                Listing::Ready(_) => {}
            }
        }
        if !state.playlists_query.trim().is_empty() && total > 0 {
            filtered_empty_state(
                ui,
                palette,
                tr(locale, "No playlists found").as_ref(),
                tr(locale, "Clear filter").as_ref(),
                || actions.push(Action::SetPlaylistsQuery(String::new())),
            );
            return;
        }
        if state.signed_in() {
            results::nothing(
                ui,
                state,
                tr(
                    locale,
                    "Create a playlist on this device or save one on your account",
                )
                .as_ref(),
            );
        } else {
            results::nothing(
                ui,
                state,
                tr(locale, "Create a playlist to save videos on this device").as_ref(),
            );
        }
    });
}

fn playlists_toolbar(
    ui: &mut Ui,
    state: &State,
    total: usize,
    visible: &[PlaylistSummary],
    actions: &mut Vec<Action>,
) {
    let locale = state.settings.language;
    let palette = &state.palette;
    let new_open_id = ui.id().with("new-playlist-open");
    let new_name_id = ui.id().with("new-playlist-name");
    let new_input_id = ui.id().with("new-playlist-input");
    let mut creating = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(new_open_id))
        .unwrap_or(false);
    let mut draft = ui
        .ctx()
        .data(|d| d.get_temp::<String>(new_name_id))
        .unwrap_or_default();

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        if total > 0 || !state.playlists_query.trim().is_empty() {
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
        }
        if !creating {
            if widgets::icon_text_button(
                ui,
                palette,
                Some(Icon::Plus),
                tr(locale, "New playlist").as_ref(),
                widgets::ButtonStyle::Primary,
            )
            .clicked()
            {
                creating = true;
                draft.clear();
                ui.memory_mut(|m| m.request_focus(new_input_id));
            }
        } else {
            let response = ui.add(
                egui::TextEdit::singleline(&mut draft)
                    .id(new_input_id)
                    .hint_text(tr(locale, "Playlist name").as_ref())
                    .desired_width(180.0),
            );
            let can_create = !draft.trim().is_empty();
            let enter = response.lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                && can_create;
            let escape = ui.input(|i| i.key_pressed(egui::Key::Escape))
                && (response.has_focus() || response.lost_focus());
            let create_clicked = ui
                .add_enabled_ui(can_create, |ui| {
                    widgets::button(
                        ui,
                        palette,
                        tr(locale, "Create").as_ref(),
                        widgets::ButtonStyle::Primary,
                    )
                })
                .inner
                .clicked();
            if enter || create_clicked {
                actions.push(Action::CreatePlaylist(draft.trim().to_owned()));
                creating = false;
                draft.clear();
            } else if escape
                || widgets::button(
                    ui,
                    palette,
                    tr(locale, "Cancel").as_ref(),
                    widgets::ButtonStyle::Normal,
                )
                .clicked()
            {
                creating = false;
                draft.clear();
            }
        }
        if total > 0 || !state.playlists_query.trim().is_empty() {
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
        }
    });
    ui.ctx().data_mut(|d| {
        d.insert_temp(new_open_id, creating);
        d.insert_temp(new_name_id, draft);
    });
    ui.add_space(12.0);
}

/// One playlist's videos.
pub fn page(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    results::panel(ui, |ui| {
        let Some(Page::Playlist { id, name }) = &state.page else {
            return;
        };
        let id = id.clone();
        let is_local = DevicePlaylist::is_local_id(&id);
        let details = state.playlist_details.as_ref();
        let title = details
            .filter(|details| !details.name.is_empty())
            .map(|details| details.name.as_str())
            .unwrap_or(name)
            .to_owned();
        let rename_open_id = ui.id().with(("local-playlist-rename-open", id.as_str()));
        let rename_draft_id = ui.id().with(("local-playlist-rename-draft", id.as_str()));
        let rename_input_id = ui.id().with(("local-playlist-rename-input", id.as_str()));
        let delete_confirm_id = ui.id().with(("local-playlist-delete", id.as_str()));
        let mut renaming = ui
            .ctx()
            .data(|d| d.get_temp::<bool>(rename_open_id))
            .unwrap_or(false);
        let mut rename_draft = ui
            .ctx()
            .data(|d| d.get_temp::<String>(rename_draft_id))
            .unwrap_or_default();
        let mut deleting = ui
            .ctx()
            .data(|d| d.get_temp::<bool>(delete_confirm_id))
            .unwrap_or(false);

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
                    RichText::new(&title)
                        .font(theme::bold(24.0))
                        .color(state.palette.text),
                );
                if is_local {
                    let count = state.page_videos.items().len() as u64;
                    let metadata = format!(
                        "{} · {}",
                        tr(locale, "On this device"),
                        widgets::video_count_label(count, locale)
                    );
                    ui.label(RichText::new(metadata).color(state.palette.secondary));
                } else if let Some(details) = details {
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
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
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
                    if is_local {
                        if renaming {
                            let response = ui.add(
                                egui::TextEdit::singleline(&mut rename_draft)
                                    .id(rename_input_id)
                                    .hint_text(tr(locale, "Playlist name").as_ref())
                                    .desired_width(180.0),
                            );
                            let can_save = !rename_draft.trim().is_empty();
                            let enter = response.lost_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                && can_save;
                            let escape = ui.input(|i| i.key_pressed(egui::Key::Escape))
                                && (response.has_focus() || response.lost_focus());
                            let save_clicked = ui
                                .add_enabled_ui(can_save, |ui| {
                                    widgets::button(
                                        ui,
                                        &state.palette,
                                        tr(locale, "Save").as_ref(),
                                        widgets::ButtonStyle::Primary,
                                    )
                                })
                                .inner
                                .clicked();
                            if enter || save_clicked {
                                actions.push(Action::RenamePlaylist {
                                    id: id.clone(),
                                    name: rename_draft.trim().to_owned(),
                                });
                                renaming = false;
                            } else if escape
                                || widgets::button(
                                    ui,
                                    &state.palette,
                                    tr(locale, "Cancel").as_ref(),
                                    widgets::ButtonStyle::Normal,
                                )
                                .clicked()
                            {
                                renaming = false;
                            }
                        } else if deleting {
                            ui.label(
                                RichText::new(trf(
                                    locale,
                                    "Delete “{name}”?",
                                    &[("name", title.clone())],
                                ))
                                .font(theme::medium(14.0))
                                .color(state.palette.red),
                            );
                            if widgets::button(
                                ui,
                                &state.palette,
                                tr(locale, "Delete").as_ref(),
                                widgets::ButtonStyle::Primary,
                            )
                            .clicked()
                            {
                                actions.push(Action::DeletePlaylist(id.clone()));
                                deleting = false;
                            }
                            if widgets::button(
                                ui,
                                &state.palette,
                                tr(locale, "Cancel").as_ref(),
                                widgets::ButtonStyle::Normal,
                            )
                            .clicked()
                            {
                                deleting = false;
                            }
                        } else {
                            if widgets::icon_text_button(
                                ui,
                                &state.palette,
                                Some(Icon::Pencil),
                                tr(locale, "Rename").as_ref(),
                                widgets::ButtonStyle::Normal,
                            )
                            .clicked()
                            {
                                renaming = true;
                                rename_draft = title.clone();
                                ui.memory_mut(|m| m.request_focus(rename_input_id));
                            }
                            if widgets::icon_text_button(
                                ui,
                                &state.palette,
                                Some(Icon::Trash),
                                tr(locale, "Delete").as_ref(),
                                widgets::ButtonStyle::Normal,
                            )
                            .clicked()
                            {
                                deleting = true;
                            }
                        }
                    }
                });
            });
        });
        ui.ctx().data_mut(|d| {
            d.insert_temp(rename_open_id, renaming);
            d.insert_temp(rename_draft_id, rename_draft);
            d.insert_temp(delete_confirm_id, deleting);
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
                                if is_local {
                                    ui.horizontal(|ui| {
                                        let controls_width = 108.0;
                                        let row_width =
                                            (ui.available_width() - controls_width).max(120.0);
                                        let response = ui
                                            .allocate_ui_with_layout(
                                                vec2(row_width, 106.0),
                                                egui::Layout::top_down(egui::Align::Min),
                                                |ui| {
                                                    widgets::video_row(
                                                        ui,
                                                        &state.palette,
                                                        video,
                                                        Some(&note),
                                                        widgets::watched(&state.library, video),
                                                        state.is_cursor(index) || playing,
                                                        locale,
                                                    )
                                                },
                                            )
                                            .inner;
                                        widgets::video_interactions(
                                            ui, &response, state, video, actions,
                                        );
                                        ui.add_enabled_ui(index > 0, |ui| {
                                            if widgets::icon_button(
                                                ui,
                                                Icon::ChevronUp,
                                                16.0,
                                                tr(locale, "Move up").as_ref(),
                                            )
                                            .clicked()
                                            {
                                                actions.push(Action::MovePlaylistVideo {
                                                    playlist: id.clone(),
                                                    from: index,
                                                    to: index - 1,
                                                });
                                            }
                                        });
                                        ui.add_enabled_ui(index + 1 < videos.len(), |ui| {
                                            if widgets::icon_button(
                                                ui,
                                                Icon::ChevronDown,
                                                16.0,
                                                tr(locale, "Move down").as_ref(),
                                            )
                                            .clicked()
                                            {
                                                actions.push(Action::MovePlaylistVideo {
                                                    playlist: id.clone(),
                                                    from: index,
                                                    to: index + 1,
                                                });
                                            }
                                        });
                                        if widgets::icon_button(
                                            ui,
                                            Icon::Close,
                                            16.0,
                                            tr(locale, "Remove from playlist").as_ref(),
                                        )
                                        .clicked()
                                        {
                                            actions.push(Action::RemovePlaylistVideo {
                                                playlist: id.clone(),
                                                video_id: video.id.clone(),
                                            });
                                        }
                                    });
                                } else {
                                    let response = widgets::video_row(
                                        ui,
                                        &state.palette,
                                        video,
                                        Some(&note),
                                        widgets::watched(&state.library, video),
                                        state.is_cursor(index) || playing,
                                        locale,
                                    );
                                    widgets::video_interactions(
                                        ui, &response, state, video, actions,
                                    );
                                }
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
    fn filtered_device_playlists_do_not_hide_the_account_load_or_retry() {
        let mut state = State {
            view: View::Playlists,
            account: AccountState::SignedIn(1),
            source: Source::Account,
            playlists_query: "no match".into(),
            ..State::default()
        };
        state.library.create_playlist("Device").unwrap();
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);

        for (listing, expected) in [
            (Listing::Loading, "Loading playlists…"),
            (Listing::Failed("Account unavailable".into()), "Retry"),
            (Listing::Ready(Vec::new()), "No playlists found"),
        ] {
            state.playlists = listing;
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 800.0))),
                    ..Default::default()
                },
                |ui| playlists(ui, &state, &mut Vec::new()),
            );
            let has_text = |wanted: &str| {
                output.shapes.iter().any(|shape| {
                    matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == wanted)
                })
            };
            assert!(has_text(expected), "missing {expected}");
            assert_eq!(
                has_text("No playlists found"),
                expected == "No playlists found"
            );
        }
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

    fn collect_texts(output: &egui::FullOutput) -> Vec<String> {
        fn push(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => out.push(text.galley.text().to_owned()),
                egui::Shape::Vec(shapes) => {
                    for s in shapes {
                        push(s, out);
                    }
                }
                _ => {}
            }
        }
        let mut texts = Vec::new();
        for shape in &output.shapes {
            push(&shape.shape, &mut texts);
        }
        texts
    }

    fn find_text_center(output: &egui::FullOutput, label: &str) -> Option<egui::Pos2> {
        fn search(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos + text.galley.size() / 2.0)
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| search(s, label)),
                _ => None,
            }
        }
        output
            .shapes
            .iter()
            .find_map(|shape| search(&shape.shape, label))
    }

    fn click_at(
        ctx: &egui::Context,
        state: &State,
        draw: fn(&mut Ui, &State, &mut Vec<Action>),
        pos: egui::Pos2,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        for pressed in [true, false] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 800.0))),
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
                |ui| draw(ui, state, &mut actions),
            );
            output.textures_delta.clear();
        }
        actions
    }

    #[test]
    fn device_playlists_render_signed_out_and_signed_in_with_on_this_device_marker() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State {
            view: View::Playlists,
            ..State::default()
        };
        let id = state.library.create_playlist("Offline Mix").unwrap();
        state.library.add_to_playlist(
            &id,
            Video {
                title: "First Song".into(),
                ..Video::from_id("loc00000001".into())
            },
        );
        // Signed out: shows device playlist and "On this device" + "New playlist".
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 800.0))),
                ..Default::default()
            },
            |ui| playlists(ui, &state, &mut Vec::new()),
        );
        let texts = collect_texts(&output);
        output.textures_delta.clear();
        assert!(texts.iter().any(|t| t == "Offline Mix"));
        assert!(texts.iter().any(|t| t == "On this device"));
        assert!(texts.iter().any(|t| t == "New playlist"));

        // Clicking "New playlist", typing a name, and clicking "Create" emits CreatePlaylist.
        let new_btn = find_text_center(&output, "New playlist").unwrap();
        click_at(&ctx, &state, playlists, new_btn);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 800.0))),
                events: vec![egui::Event::Text("Study List".into())],
                ..Default::default()
            },
            |ui| playlists(ui, &state, &mut Vec::new()),
        );
        let create_btn = find_text_center(&output, "Create").unwrap();
        output.textures_delta.clear();
        let actions = click_at(&ctx, &state, playlists, create_btn);
        assert_eq!(actions, vec![Action::CreatePlaylist("Study List".into())]);

        // Signed in: shows both device and account playlists.
        state.account = AccountState::SignedIn(1);
        state.playlists = Listing::Ready(vec![PlaylistSummary {
            id: "PL_cloud".into(),
            name: "Cloud Mix".into(),
            thumbnail: None,
            channel: Some("Creator".into()),
            videos: Some(4),
        }]);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 800.0))),
                ..Default::default()
            },
            |ui| playlists(ui, &state, &mut Vec::new()),
        );
        let texts = collect_texts(&output);
        output.textures_delta.clear();
        assert!(texts.iter().any(|t| t == "Offline Mix"));
        assert!(texts.iter().any(|t| t == "On this device"));
        assert!(texts.iter().any(|t| t == "Cloud Mix"));
    }

    #[test]
    fn local_playlist_page_renders_controls_and_reorder_remove_actions() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State {
            view: View::Playlist,
            ..State::default()
        };
        let id = state.library.create_playlist("My Local List").unwrap();
        let v1 = Video {
            title: "Alpha Video".into(),
            ..Video::from_id("loc00000001".into())
        };
        let v2 = Video {
            title: "Beta Video".into(),
            ..Video::from_id("loc00000002".into())
        };
        state.library.add_to_playlist(&id, v1.clone());
        state.library.add_to_playlist(&id, v2.clone());
        state.page = Some(Page::Playlist {
            id: id.clone(),
            name: "My Local List".into(),
        });
        state.page_videos = Listing::Ready(vec![v1, v2]);

        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 800.0))),
                ..Default::default()
            },
            |ui| page(ui, &state, &mut Vec::new()),
        );
        let texts = collect_texts(&output);
        output.textures_delta.clear();
        assert!(texts.iter().any(|t| t == "My Local List"));
        assert!(texts.iter().any(|t| t == "On this device · 2 videos"));
        assert!(texts.iter().any(|t| t == "Play all"));
        assert!(texts.iter().any(|t| t == "Rename"));
        assert!(texts.iter().any(|t| t == "Delete"));

        // Test Play all action.
        let play_btn = find_text_center(&output, "Play all").unwrap();
        assert_eq!(
            click_at(&ctx, &state, page, play_btn),
            vec![Action::PlayPlaylist]
        );

        // Test Delete confirmation flow.
        let del_btn = find_text_center(&output, "Delete").unwrap();
        click_at(&ctx, &state, page, del_btn);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 800.0))),
                ..Default::default()
            },
            |ui| page(ui, &state, &mut Vec::new()),
        );
        let confirm_btn = find_text_center(&output, "Delete").unwrap();
        output.textures_delta.clear();
        assert_eq!(
            click_at(&ctx, &state, page, confirm_btn),
            vec![Action::DeletePlaylist(id.clone())]
        );

        // Test Rename flow.
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 800.0))),
                ..Default::default()
            },
            |ui| page(ui, &state, &mut Vec::new()),
        );
        let rename_btn = find_text_center(&output, "Rename").unwrap();
        output.textures_delta.clear();
        click_at(&ctx, &state, page, rename_btn);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 800.0))),
                events: vec![egui::Event::Text(" Renamed".into())],
                ..Default::default()
            },
            |ui| page(ui, &state, &mut Vec::new()),
        );
        let save_btn = find_text_center(&output, "Save").unwrap();
        output.textures_delta.clear();
        assert_eq!(
            click_at(&ctx, &state, page, save_btn),
            vec![Action::RenamePlaylist {
                id,
                name: "My Local List Renamed".into(),
            }]
        );
    }
}
