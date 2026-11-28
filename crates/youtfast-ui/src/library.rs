//! The pages of what the user follows and keeps: the home feed,
//! subscriptions, history, Watch later, liked videos, playlists, and one
//! playlist's videos. Pages that exist both ways switch between the
//! signed-in account and this device with chips under their title.

mod collections;
mod history;
mod home;
mod subscriptions;

pub use collections::{liked, page, playlists, watch_later};
pub use history::history;
pub use home::home;
#[cfg(test)]
pub(super) use subscriptions::channel_rows;
pub(super) use subscriptions::channel_rows_content;
pub use subscriptions::subscriptions;

use egui::{RichText, Ui, vec2};
use youtfast_i18n::gettext as tr;

use super::{results, widgets};
use youtfast_app::{Action, State};
use youtfast_appearance::theme::{self, Palette};
use youtfast_core::model::{Fetch, Listing, Source, Video, View};
use youtfast_core::util;

/// The smallest a home or library card gets before the grid drops a column.
pub(super) const CARD_WIDTH: f32 = 280.0;

/// The Account / This device chips (when signed in) and a page's extra
/// buttons, on one line under the page title.
pub(super) fn controls(
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

pub(super) fn library_video_toolbar(
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
        widgets::SearchField::new(
            egui::Id::new(search_salt),
            220.0_f32.min(ui.available_width()),
        ),
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
pub(super) fn toolbar_summary(ui: &mut Ui, palette: &Palette, text: &str) {
    ui.add(
        egui::Label::new(
            RichText::new(text)
                .font(theme::regular(13.0))
                .color(palette.secondary),
        )
        .wrap_mode(egui::TextWrapMode::Extend),
    );
}

pub(super) fn video_card_grid(
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
                                ui, state, video, watched, selected, width, locale,
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
                widgets::more_row(ui, state, more_state, has_more, actions);
            }
        });
}

pub(super) fn filtered_empty_state(
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
pub(super) fn rows_listing(
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

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Rect;
    use youtfast_app::AccountState;
    use youtfast_core::model::{Channel, FeedFilter};

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
                assert!(
                    output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == expected)),
                    "empty state missing its localized hint: {view:?}/{locale:?}"
                );
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
                            _ => home::feed_grid(ui, &state, &mut actions),
                        }
                    });
                },
            );
            output.textures_delta.clear();
            let drawn = youtfast_app::images::drawn_count(&ctx);
            assert!(drawn > 0 && drawn < 20, "list {kind} drew {drawn} images");
        }
    }
}
