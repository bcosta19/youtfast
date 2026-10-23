//! Lists of videos as rows on a page: search results, and every other
//! page that shows videos (the feed, Watch later, liked videos, a
//! playlist, a channel). Each list carries its loading, empty and failed
//! states in its place. Also the page frame the list pages share: the
//! page padding and the bold page title.

use crate::appearance::i18n::{format as trf, gettext as tr};
use egui::{Align, Layout, RichText, Ui};

use super::widgets;
use crate::app::State;
use crate::appearance::theme;
use crate::model::{Action, Listing, Video};

/// A page's central panel with DESIGN.md's padding: 24 px at the sides,
/// 16 px at the top.
pub fn panel(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    let frame = egui::Frame::central_panel(ui.style()).inner_margin(egui::Margin {
        left: 24,
        right: 24,
        top: 16,
        bottom: 0,
    });
    egui::CentralPanel::default()
        .frame(frame)
        .show(ui, |ui| add_contents(ui));
}

/// A page's own title in bold 24, with a secondary count after it.
pub fn title(ui: &mut Ui, state: &State, title: &str, count: Option<usize>) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(title)
                .font(theme::bold(24.0))
                .color(state.palette.text),
        );
        if let Some(count) = count {
            ui.label(
                RichText::new(crate::util::thousands(count as u64))
                    .font(theme::regular(14.0))
                    .color(state.palette.secondary),
            );
        }
    });
    ui.add_space(12.0);
}

pub fn show(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    panel(ui, |ui| {
        super::discovery::filters(ui, state, actions);
        if state.search_options.kind != crate::youtube::innertube::SearchKind::Videos {
            title(ui, state, tr(locale, "Search results").as_ref(), None);
            super::discovery::other_results(ui, state, actions);
            return;
        }
        let listing = &state.results;
        title(
            ui,
            state,
            tr(locale, "Search results").as_ref(),
            Some(listing.items().len()),
        );
        match listing {
            Listing::Idle => {}
            Listing::Loading => loading(ui, state, "results"),
            Listing::Failed(error) => failed(ui, state, error, actions),
            Listing::Ready(videos) if videos.is_empty() => nothing(
                ui,
                state,
                tr(locale, "Search for a video, channel or playlist").as_ref(),
            ),
            Listing::Ready(videos) => rows(ui, state, videos, true, true, actions),
        }
    });
}

/// A titled list of video rows, or what stands in for it: the spinner,
/// the error with Retry, or the empty state with `empty` as its hint.
pub fn videos(
    ui: &mut Ui,
    state: &State,
    title: &str,
    listing: &Listing<Video>,
    empty: &str,
    actions: &mut Vec<Action>,
) {
    self::title(ui, state, title, Some(listing.items().len()));
    match listing {
        Listing::Idle => {}
        Listing::Loading => loading(ui, state, "videos"),
        Listing::Failed(error) => failed(ui, state, error, actions),
        Listing::Ready(videos) if videos.is_empty() => nothing(ui, state, empty),
        Listing::Ready(videos) => video_rows(ui, state, videos, actions),
    }
}

/// The rows themselves, in a scroll area.
pub fn video_rows<'a>(
    ui: &mut Ui,
    state: &State,
    videos: impl IntoIterator<Item = &'a Video>,
    actions: &mut Vec<Action>,
) {
    rows(ui, state, videos, false, false, actions);
}

/// [`video_rows`]; `large` draws search's big thumbnails, and `more` ends
/// the list with the continuation's row.
fn rows<'a>(
    ui: &mut Ui,
    state: &State,
    videos: impl IntoIterator<Item = &'a Video>,
    large: bool,
    more: bool,
    actions: &mut Vec<Action>,
) {
    let locale = state.settings.language;

    let videos: Vec<&Video> = videos.into_iter().collect();
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            let height = if large {
                (ui.available_width() / 2.0).clamp(168.0, 360.0) * 9.0 / 16.0 + 12.0
            } else {
                106.0
            };
            virtual_rows(
                ui,
                height,
                videos.len(),
                state.cursor_moved.then_some(state.cursor),
                |ui, index| {
                    let video = videos[index];
                    let selected = index == state.cursor;
                    let watched = widgets::watched(&state.library, video);
                    let response = if large {
                        widgets::video_row_large(
                            ui,
                            &state.palette,
                            video,
                            watched,
                            selected,
                            locale,
                        )
                    } else {
                        widgets::video_row(
                            ui,
                            &state.palette,
                            video,
                            None,
                            watched,
                            selected,
                            locale,
                        )
                    };
                    if selected && state.cursor_moved {
                        response.clone().scroll_to_me(None);
                    }
                    widgets::video_interactions(ui, &response, state, video, actions);
                },
            );
            if more {
                widgets::more_row(
                    ui,
                    &state.palette,
                    &state.results_more_state,
                    state.results_more.is_some(),
                    actions,
                    locale,
                );
            }
        });
}

pub fn virtual_rows(
    ui: &mut Ui,
    height: f32,
    count: usize,
    selected: Option<usize>,
    mut row: impl FnMut(&mut Ui, usize),
) {
    if count == 0 {
        return;
    }
    let stride = height + ui.spacing().item_spacing.y;
    let top = ui.next_widget_position().y;
    let range = visible_rows(
        ui.clip_rect().top() - top,
        ui.clip_rect().bottom() - top,
        stride,
        count,
    );
    let total = egui::Rect::from_min_size(
        egui::pos2(ui.next_widget_position().x, top),
        egui::vec2(
            ui.available_width(),
            stride * count as f32 - ui.spacing().item_spacing.y,
        ),
    );
    if let Some(index) = selected.filter(|index| *index < count) {
        let rect = egui::Rect::from_min_size(
            total.min + egui::vec2(0.0, index as f32 * stride),
            egui::vec2(total.width(), height),
        );
        ui.scroll_to_rect(rect, None);
    }
    let rect = egui::Rect::from_min_max(
        total.min + egui::vec2(0.0, range.start as f32 * stride),
        egui::pos2(total.right(), top + range.end as f32 * stride),
    );
    let mut rows = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    rows.skip_ahead_auto_ids(range.start);
    for index in range {
        let rect = egui::Rect::from_min_size(
            total.min + egui::vec2(0.0, index as f32 * stride),
            egui::vec2(total.width(), height),
        );
        let mut item = rows.new_child(egui::UiBuilder::new().id_salt(index).max_rect(rect));
        row(&mut item, index);
    }
    ui.advance_cursor_after_rect(total);
}

fn visible_rows(top: f32, bottom: f32, stride: f32, count: usize) -> std::ops::Range<usize> {
    let first = ((top / stride).floor().max(0.0) as usize)
        .saturating_sub(1)
        .min(count);
    let last = (((bottom / stride).ceil() + 1.0).max(0.0) as usize).min(count);
    first..last.max(first)
}

/// A video's right-click menu: the queue, and this device's Watch later.
pub fn video_menu(
    response: &egui::Response,
    state: &State,
    video: &Video,
    actions: &mut Vec<Action>,
) {
    let locale = state.settings.language;

    response.context_menu(|ui| {
        if ui.button(tr(locale, "Play next").as_ref()).clicked() {
            actions.push(Action::Enqueue {
                video: video.clone(),
                next: true,
            });
            ui.close();
        }
        if ui.button(tr(locale, "Add to queue").as_ref()).clicked() {
            actions.push(Action::Enqueue {
                video: video.clone(),
                next: false,
            });
            ui.close();
        }
        let later = state.library.in_watch_later(&video.id);
        let label = if later {
            "Remove from Watch later"
        } else {
            "Watch later on this device"
        };
        if ui.button(tr(locale, label).as_ref()).clicked() {
            actions.push(Action::ToggleWatchLater(video.clone()));
            ui.close();
        }
    });
}

/// Keep the list's proportions in place while its first page is loading.
pub fn loading(ui: &mut Ui, state: &State, what: &'static str) {
    let locale = state.settings.language;

    ui.label(
        RichText::new(trf(
            locale,
            "Loading {what}…",
            &[("what", tr(locale, what).into_owned())],
        ))
        .color(state.palette.secondary),
    );
    ui.add_space(12.0);
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            loading_placeholders(ui, state);
        });
}

/// The placeholder content and label of [`loading`], without a scroll area.
pub fn loading_content(ui: &mut Ui, state: &State, what: &'static str) {
    let locale = state.settings.language;

    ui.label(
        RichText::new(trf(
            locale,
            "Loading {what}…",
            &[("what", tr(locale, what).into_owned())],
        ))
        .color(state.palette.secondary),
    );
    ui.add_space(12.0);
    loading_placeholders(ui, state);
}

fn loading_placeholders(ui: &mut Ui, state: &State) -> Vec<egui::Rect> {
    use crate::model::{ChannelTab, View};
    let cards = state.view == View::Home
        || (state.view == View::Channel
            && matches!(
                state.channel_tab,
                ChannelTab::Videos | ChannelTab::Shorts | ChannelTab::Live | ChannelTab::Search(_)
            ));
    let mut rects = Vec::new();
    if cards {
        let gap = 16.0;
        let (columns, width) = widgets::grid_columns(ui.available_width(), 280.0, gap, 5);
        for _ in 0..3 {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for _ in 0..columns {
                    let height = widgets::video_card_height(ui, width);
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
                    let thumb =
                        egui::Rect::from_min_size(rect.min, egui::vec2(width, width * 9.0 / 16.0));
                    placeholder(ui, state, thumb);
                    placeholder(
                        ui,
                        state,
                        egui::Rect::from_min_size(
                            thumb.left_bottom() + egui::vec2(0.0, 12.0),
                            egui::vec2(width * 0.9, 14.0),
                        ),
                    );
                    placeholder(
                        ui,
                        state,
                        egui::Rect::from_min_size(
                            thumb.left_bottom() + egui::vec2(0.0, 36.0),
                            egui::vec2(width * 0.6, 12.0),
                        ),
                    );
                    rects.push(rect);
                }
            });
            ui.add_space(gap);
        }
    } else {
        let channels = state.view == View::Subscriptions
            || (state.view == View::Results
                && state.search_options.kind == crate::youtube::innertube::SearchKind::Channels);
        let large = state.view == View::Results
            && state.search_options.kind == crate::youtube::innertube::SearchKind::Videos;
        let width = ui.available_width();
        let thumbnail_width = if channels {
            48.0
        } else if large {
            (width / 2.0).clamp(168.0, 360.0)
        } else {
            168.0
        };
        let height = if channels {
            64.0
        } else if large {
            thumbnail_width * 9.0 / 16.0 + 12.0
        } else {
            106.0
        };
        for _ in 0..6 {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
            let thumb = egui::Rect::from_min_size(
                rect.min + egui::vec2(0.0, 8.0),
                egui::vec2(thumbnail_width, if channels { 48.0 } else { height - 12.0 }),
            );
            if channels {
                ui.painter()
                    .circle_filled(thumb.center(), 24.0, state.palette.surface);
            } else {
                placeholder(ui, state, thumb);
            }
            let text_left = thumb.right() + 16.0;
            let available = (rect.right() - text_left).max(0.0);
            placeholder(
                ui,
                state,
                egui::Rect::from_min_size(
                    egui::pos2(text_left, rect.top() + 16.0),
                    egui::vec2(available * 0.85, 14.0),
                ),
            );
            placeholder(
                ui,
                state,
                egui::Rect::from_min_size(
                    egui::pos2(text_left, rect.top() + 40.0),
                    egui::vec2(available * 0.55, 12.0),
                ),
            );
            rects.push(rect);
        }
    }
    rects
}

fn placeholder(ui: &Ui, state: &State, rect: egui::Rect) {
    if ui.is_rect_visible(rect) {
        ui.painter()
            .rect_filled(rect, theme::RADIUS, state.palette.surface);
    }
}

/// The empty state: a centred title, then what would fill the list.
pub fn nothing(ui: &mut Ui, state: &State, hint: &str) {
    let locale = state.settings.language;

    empty(ui, state, tr(locale, "Nothing here").as_ref(), hint);
}

/// An empty state with its own title.
pub fn empty(ui: &mut Ui, state: &State, title: &str, hint: &str) {
    centered(ui, |ui| {
        ui.label(
            RichText::new(title)
                .font(theme::semibold(16.0))
                .color(state.palette.text),
        );
        ui.label(
            RichText::new(hint)
                .font(theme::regular(14.0))
                .color(state.palette.secondary),
        );
    });
}

/// The error in red, and Retry.
pub fn failed(ui: &mut Ui, state: &State, error: &str, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    centered(ui, |ui| {
        ui.label(
            RichText::new(crate::appearance::i18n::message(locale, error).as_ref())
                .font(theme::regular(14.0))
                .color(state.palette.red),
        );
        ui.add_space(8.0);
        if widgets::button(
            ui,
            &state.palette,
            tr(locale, "Retry").as_ref(),
            widgets::ButtonStyle::Normal,
        )
        .clicked()
        {
            actions.push(Action::Refresh);
        }
    });
}

pub fn centered(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.add_space(ui.available_height() * 0.25);
        add_contents(ui);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loading_placeholders_keep_stable_geometry_at_phone_and_desktop_widths() {
        for width in [480.0, 1280.0] {
            for view in [
                crate::model::View::Home,
                crate::model::View::Results,
                crate::model::View::Channel,
                crate::model::View::Subscriptions,
                crate::model::View::Playlist,
            ] {
                let ctx = egui::Context::default();
                crate::appearance::theme::install_fonts(&ctx);
                let state = State {
                    view,
                    ..Default::default()
                };
                let mut previous = Vec::new();
                for frame in 0..3 {
                    let mut rects = Vec::new();
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, 720.0),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            egui::CentralPanel::default().show(ui, |ui| {
                                rects = loading_placeholders(ui, &state);
                            });
                        },
                    );
                    output.textures_delta.clear();
                    assert!(!rects.is_empty());
                    assert!(rects.iter().all(|rect| rect.is_finite()
                        && rect.width() > 0.0
                        && rect.height() > 0.0
                        && rect.left() >= 0.0
                        && rect.right() <= width));
                    if frame == 2 {
                        assert_eq!(
                            rects, previous,
                            "loading geometry moved for {view:?} at {width}"
                        );
                    }
                    previous = rects;
                }
                assert_eq!(
                    super::super::images::drawn_count(&ctx),
                    0,
                    "placeholders must not request real thumbnails"
                );
            }
        }
    }

    fn click_label(
        state: &State,
        label: &str,
        render: fn(&mut Ui, &State, &mut Vec<Action>),
    ) -> Vec<Action> {
        let ctx = egui::Context::default();
        crate::appearance::theme::install_fonts(&ctx);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 720.0));
        let mut first = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ui| render(ui, state, &mut Vec::new()),
        );
        let position = first
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if text.galley.text().contains(label) => {
                    Some(text.pos + text.galley.size() / 2.0)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("button {label} missing"));
        first.textures_delta.clear();
        let mut actions = Vec::new();
        for pressed in [true, false] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events: vec![
                        egui::Event::PointerMoved(position),
                        egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| render(ui, state, &mut actions),
            );
            output.textures_delta.clear();
        }
        actions
    }

    #[test]
    fn retry_on_failed_typed_search_requests_a_refresh() {
        let mut state = State {
            view: crate::model::View::Results,
            search_query: "Rust".into(),
            ..Default::default()
        };
        state.search_options.kind = crate::youtube::innertube::SearchKind::Channels;
        state.results_channels = Listing::Failed("Falha temporária".into());
        assert_eq!(click_label(&state, "Retry", show), [Action::Refresh]);
    }

    #[test]
    fn playlist_with_metadata_exposes_play_all_as_an_action() {
        let state = State {
            view: crate::model::View::Playlist,
            page: Some(crate::app::Page::Playlist {
                id: "synthetic-playlist".into(),
                name: "Título anterior".into(),
            }),
            playlist_details: Some(crate::youtube::innertube::PlaylistDetails {
                name: "Aulas de Rust".into(),
                channel: Some("Canal".into()),
                description: "Aprenda Rust".into(),
                videos: Some(30),
                ..Default::default()
            }),
            page_videos: Listing::Ready(vec![Video::from_id("aaaaaaaaaaa".into())]),
            ..Default::default()
        };
        assert_eq!(
            click_label(&state, "Play all", super::super::library::page),
            [Action::PlayPlaylist]
        );
    }

    fn scroll_to_footer_and_click(
        state: &State,
        width: f32,
        render: fn(&mut Ui, &State, &mut Vec<Action>),
    ) {
        let ctx = egui::Context::default();
        crate::appearance::theme::install_fonts(&ctx);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 720.0));
        let mut button = None;
        for frame in 0..40 {
            let mut events = vec![egui::Event::PointerMoved(egui::pos2(width / 2.0, 560.0))];
            if frame > 0 {
                events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    phase: egui::TouchPhase::Move,
                    delta: egui::vec2(0.0, -1000.0),
                    modifiers: egui::Modifiers::NONE,
                });
            }
            let mut output = ctx.run_ui(
                egui::RawInput {
                    time: Some(frame as f64 / 10.0),
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |ui| render(ui, state, &mut Vec::new()),
            );
            button = output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text)
                    if matches!(text.galley.text(), "Load more" | "Carregar mais") =>
                {
                    let rect = egui::Rect::from_min_size(text.pos, text.galley.size());
                    (shape.clip_rect.contains_rect(rect) && screen.contains_rect(rect))
                        .then_some(rect)
                }
                _ => None,
            });
            output.textures_delta.clear();
            if button.is_some() {
                break;
            }
        }
        let rect =
            button.unwrap_or_else(|| panic!("continuation footer is unreachable at width {width}"));
        assert!(rect.bottom() <= screen.bottom() && rect.right() <= screen.right());
        let mut actions = Vec::new();
        for (frame, pressed) in [true, false].into_iter().enumerate() {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    time: Some(5.0 + frame as f64 / 10.0),
                    screen_rect: Some(screen),
                    events: vec![
                        egui::Event::PointerMoved(rect.center()),
                        egui::Event::PointerButton {
                            pos: rect.center(),
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| render(ui, state, &mut actions),
            );
            output.textures_delta.clear();
        }
        assert_eq!(
            actions,
            [Action::LoadMore],
            "footer did not receive the click at width {width}"
        );
    }

    #[test]
    fn channel_search_and_playlist_footers_are_reachable_and_clickable_after_long_lists() {
        let more = crate::youtube::innertube::More {
            ctoken: "synthetic-next-page".into(),
            endpoint: rustypipe::model::paginator::ContinuationEndpoint::Browse,
            visitor_data: None,
            authenticated: false,
            ours: false,
        };
        let mut channels = State {
            view: crate::model::View::Results,
            results_more: Some(more.clone()),
            ..Default::default()
        };
        channels.search_options.kind = crate::youtube::innertube::SearchKind::Channels;
        channels.results_channels = Listing::Ready(
            (0..60)
                .map(|index| crate::model::Channel {
                    id: format!("UC{index:022}"),
                    name: format!("Canal {index}"),
                    avatar: None,
                })
                .collect(),
        );
        let playlist = State {
            view: crate::model::View::Playlist,
            page: Some(crate::app::Page::Playlist {
                id: "synthetic-playlist".into(),
                name: "Playlist sintética".into(),
            }),
            page_more: Some(more),
            page_videos: Listing::Ready(
                (0..60)
                    .map(|index| Video::from_id(format!("{index:011}")))
                    .collect(),
            ),
            ..Default::default()
        };
        for width in [480.0, 1280.0] {
            scroll_to_footer_and_click(&channels, width, show);
            scroll_to_footer_and_click(&playlist, width, super::super::library::page);
            let mut empty = State {
                view: crate::model::View::Playlist,
                page: playlist.page.clone(),
                page_videos: Listing::Ready(Vec::new()),
                ..State::default()
            };
            empty.page_more = playlist.page_more.clone();
            scroll_to_footer_and_click(&empty, width, super::super::library::page);
        }
    }

    #[test]
    fn rows_with_nested_controls_keep_their_identity_when_scrolled() {
        let ctx = egui::Context::default();
        let mut ids = Vec::new();
        for offset in [0.0, 200.0] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(640.0, 480.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .vertical_scroll_offset(offset)
                            .show(ui, |ui| {
                                virtual_rows(ui, 32.0, 1000, None, |ui, index| {
                                    let (rect, response) = ui.allocate_exact_size(
                                        egui::vec2(ui.available_width(), 32.0),
                                        egui::Sense::click(),
                                    );
                                    let mut inner =
                                        ui.new_child(egui::UiBuilder::new().max_rect(rect));
                                    inner.label("nested control");
                                    if index == 7 {
                                        ids.push(response.id);
                                    }
                                });
                            });
                    });
                },
            );
            output.textures_delta.clear();
        }
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], ids[1]);
    }

    #[test]
    fn only_visible_rows_and_a_small_margin_are_laid_out() {
        assert_eq!(visible_rows(0.0, 400.0, 100.0, 1000), 0..5);
        assert_eq!(visible_rows(99000.0, 100000.0, 100.0, 1000), 989..1000);
        assert_eq!(visible_rows(-500.0, -100.0, 100.0, 1000), 0..0);
        assert_eq!(visible_rows(-500.0, -10.0, 100.0, 1000), 0..1);
        let ctx = egui::Context::default();
        crate::appearance::theme::install_fonts(&ctx);
        let state = State::default();
        let videos: Vec<_> = (0..1000)
            .map(|index| Video::from_id(format!("{index:011}")))
            .collect();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 480.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .show(ctx, |ui| video_rows(ui, &state, &videos, &mut Vec::new()));
            },
        );
        output.textures_delta.clear();
        let drawn = super::super::images::drawn_count(&ctx);
        assert!(drawn > 0 && drawn < 10, "drawn thumbnails: {drawn}");
    }

    #[test]
    fn keyboard_selection_reaches_an_offscreen_row_and_the_footer_stays_in_the_flow() {
        let ctx = egui::Context::default();
        let mut seen = false;
        let mut footer = false;
        for frame in 0..60 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    time: Some(frame as f64 / 10.0),
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(640.0, 480.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            virtual_rows(
                                ui,
                                106.0,
                                1000,
                                (frame == 1).then_some(999),
                                |ui, index| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(ui.available_width(), 106.0),
                                        egui::Sense::hover(),
                                    );
                                    if index == 999 && ui.is_rect_visible(rect) {
                                        seen = true;
                                    }
                                },
                            );
                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 32.0),
                                egui::Sense::hover(),
                            );
                            footer |= ui.is_rect_visible(rect);
                            if frame == 30 {
                                ui.scroll_to_rect(rect, Some(egui::Align::BOTTOM));
                            }
                        });
                    });
                },
            );
            output.textures_delta.clear();
        }
        assert!(seen, "the selected row never reached the viewport");
        assert!(footer, "the continuation footer never reached the viewport");
    }
}
