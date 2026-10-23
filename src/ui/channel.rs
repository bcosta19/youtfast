//! A channel's page, YouTube's way: the header (round avatar, the name,
//! the numbers, Subscribe), the underlined tab bar with the channel's own
//! search at its right, and the tab: a grid of video cards, playlist rows,
//! or the About cards.

use crate::appearance::i18n::{Locale, format as trf, gettext as tr};
use egui::{Align, Layout, RichText, Sense, Ui, pos2, vec2};

use super::{results, widgets};
use crate::app::State;
use crate::appearance::theme;
use crate::model::{Action, Channel, ChannelAbout, ChannelHeader, ChannelTab, Fetch, Listing};
use crate::util;

/// The smallest a video card gets before the grid drops a column.
const CARD_WIDTH: f32 = 280.0;
/// The space between cards, rows and the page's parts.
const GAP: f32 = 8.0;
/// The widest the About cards read well.
const ABOUT_WIDTH: f32 = 720.0;

/// The tabs in the order they show, given what the header knows. Before
/// the header arrives, Shorts and Live cannot be known to exist. An active
/// search keeps its entry at the end.
pub fn tabs(header: Option<&ChannelHeader>, active: &ChannelTab) -> Vec<ChannelTab> {
    let mut tabs = vec![ChannelTab::Videos];
    if let Some(header) = header {
        if header.has_shorts {
            tabs.push(ChannelTab::Shorts);
        }
        if header.has_live {
            tabs.push(ChannelTab::Live);
        }
    }
    tabs.push(ChannelTab::Playlists);
    tabs.push(ChannelTab::About);
    if let ChannelTab::Search(query) = active {
        tabs.push(ChannelTab::Search(query.clone()));
    }
    tabs
}

/// The tab `step` over `visible` from `current`, wrapping around.
pub fn cycle_tab(visible: &[ChannelTab], current: &ChannelTab, step: isize) -> ChannelTab {
    let index = visible
        .iter()
        .position(|tab| same_kind(tab, current))
        .unwrap_or(0);
    let len = visible.len() as isize;
    let next = (index as isize + step).rem_euclid(len);
    // An active Search's entry already carries its query.
    visible[next as usize].clone()
}

/// Two tabs are the same line item when they are the same kind.
fn same_kind(a: &ChannelTab, b: &ChannelTab) -> bool {
    matches!(
        (a, b),
        (ChannelTab::Videos, ChannelTab::Videos)
            | (ChannelTab::Shorts, ChannelTab::Shorts)
            | (ChannelTab::Live, ChannelTab::Live)
            | (ChannelTab::Playlists, ChannelTab::Playlists)
            | (ChannelTab::About, ChannelTab::About)
            | (ChannelTab::Search(_), ChannelTab::Search(_))
    )
}

/// Centred object-fit: cover, preserving the source image's aspect ratio.
fn banner_uv(image: egui::Vec2, target: egui::Vec2) -> egui::Rect {
    let scale = (target.x / image.x).max(target.y / image.y);
    let visible = target / (image * scale);
    egui::Rect::from_center_size(egui::pos2(0.5, 0.5), visible)
}

pub fn show(ui: &mut Ui, state: &State, channel: &Channel, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    results::panel(ui, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                if let Some(url) = state
                    .channel_header
                    .as_ref()
                    .and_then(|header| header.banner.as_deref())
                {
                    let width = ui.available_width();
                    let height = (width / 6.0).clamp(80.0, 200.0);
                    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
                    let mut image = super::images::remote(ui, url);
                    if let Ok(texture) = image.load_for_size(ui.ctx(), rect.size())
                        && let Some(size) = texture.size()
                    {
                        image = image.uv(banner_uv(size, rect.size()));
                    }
                    image
                        .fit_to_exact_size(rect.size())
                        .corner_radius(egui::CornerRadius::same(theme::RADIUS))
                        .paint_at(ui, rect);
                    ui.add_space(16.0);
                }
                header(ui, state, channel, actions);
                ui.add_space(16.0);
                tab_bar(ui, state, actions);
                ui.add_space(16.0);
                if matches!(
                    state.channel_tab,
                    ChannelTab::Videos | ChannelTab::Shorts | ChannelTab::Live
                ) {
                    ui.horizontal_wrapped(|ui| {
                        for &order in crate::youtube::innertube::ChannelOrder::ALL {
                            if widgets::chip(
                                ui,
                                &state.palette,
                                tr(locale, order.label()).as_ref(),
                                state.channel_order == order,
                            )
                            .clicked()
                            {
                                actions.push(Action::SetChannelOrder(order));
                            }
                        }
                    });
                    ui.add_space(12.0);
                }
                match &state.channel_tab {
                    ChannelTab::About => about(ui, state, actions),
                    ChannelTab::Playlists => playlists(ui, state, actions),
                    tab => videos(ui, state, tab, actions),
                }
            });
    });
}

/// The header: the round avatar, the name in bold, the handle and the
/// numbers in secondary, and Subscribe as the primary pill.
fn header(ui: &mut Ui, state: &State, channel: &Channel, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    let palette = &state.palette;
    let header = state.channel_header.as_ref();
    let title = header
        .map(|header| header.name.clone())
        .or_else(|| (!channel.name.is_empty()).then(|| channel.name.clone()))
        .unwrap_or_else(|| tr(locale, "Loading…").into_owned());
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(80.0, 80.0), Sense::hover());
        let avatar = header
            .and_then(|header| header.avatar.as_deref())
            .or(channel.avatar.as_deref());
        widgets::avatar(ui, palette, avatar, &title, rect);
        ui.add_space(8.0);
        ui.vertical(|ui| {
            ui.label(
                RichText::new(&title)
                    .font(theme::bold(24.0))
                    .color(palette.text),
            );
            if let Some(header) = header {
                let mut parts = Vec::new();
                if let Some(handle) = &header.handle {
                    parts.push(handle.clone());
                }
                if let Some(subscribers) = header.subscribers {
                    parts.push(channel_count(
                        subscribers,
                        "{count} subscriber",
                        "{count} subscribers",
                        locale,
                    ));
                }
                if let Some(videos) = header.videos {
                    parts.push(channel_count(
                        videos,
                        "{count} video",
                        "{count} videos",
                        locale,
                    ));
                }
                if !parts.is_empty() {
                    ui.label(
                        RichText::new(parts.join(" · "))
                            .font(theme::regular(14.0))
                            .color(palette.secondary),
                    );
                }
            }
            if channel.id.starts_with("UC") && !channel.name.is_empty() {
                ui.add_space(4.0);
                widgets::subscribe(ui, state, channel, widgets::ButtonStyle::Primary, actions);
            }
        });
    });
}

fn channel_count(
    count: u64,
    singular: &'static str,
    plural: &'static str,
    locale: Locale,
) -> String {
    let phrase = crate::appearance::i18n::ngettext(
        locale,
        singular,
        plural,
        count.min(u32::MAX as u64) as u32,
    );
    crate::appearance::i18n::interpolate(&phrase, &[("count", util::compact(count))])
}

/// The tab's name in the bar.
fn tab_label(tab: &ChannelTab, locale: Locale) -> String {
    match tab {
        ChannelTab::Videos => tr(locale, "Videos").into_owned(),
        ChannelTab::Shorts => tr(locale, "Shorts").into_owned(),
        ChannelTab::Live => tr(locale, "Live").into_owned(),
        ChannelTab::Playlists => tr(locale, "Playlists").into_owned(),
        ChannelTab::About => tr(locale, "About").into_owned(),
        ChannelTab::Search(query) => trf(locale, "Search: {query}", &[("query", query.clone())]),
    }
}

/// YouTube's tab bar: the names in medium, the current one in `text` over
/// a 2 px underline, the others in `secondary`; a hairline under the bar;
/// the channel's own search at its right.
fn tab_bar(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    let palette = &state.palette;
    let height = 40.0;
    let bar = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 24.0;
        for tab in tabs(state.channel_header.as_ref(), &state.channel_tab) {
            let current = same_kind(&tab, &state.channel_tab);
            let galley = ui.painter().layout_no_wrap(
                tab_label(&tab, locale),
                theme::medium(15.0),
                palette.text,
            );
            let (rect, response) =
                ui.allocate_exact_size(vec2(galley.size().x, height), Sense::click());
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            let color = if current || response.hovered() {
                palette.text
            } else {
                palette.secondary
            };
            let text_at = pos2(rect.left(), rect.center().y - galley.size().y / 2.0);
            ui.painter()
                .galley_with_override_text_color(text_at, galley, color);
            if current {
                let underline = egui::Rect::from_min_max(
                    pos2(rect.left(), rect.bottom() - 2.0),
                    rect.right_bottom(),
                );
                ui.painter().rect_filled(underline, 1.0, palette.text);
            }
            if response.clicked() {
                actions.push(Action::SetChannelTab(tab));
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            channel_search(ui, state, actions);
        });
    });
    let rect = bar.response.rect;
    ui.painter().hline(
        rect.left()..=rect.right(),
        rect.bottom(),
        egui::Stroke::new(1.0, palette.outline),
    );
}

/// The field that searches this channel: Enter opens the Search tab.
fn channel_search(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    let mut query = state.channel_query.clone();
    let field = widgets::search_field(
        ui,
        &state.palette,
        &mut query,
        tr(locale, "Search this channel").as_ref(),
        egui::Id::new("channel search"),
        240.0_f32.min(ui.available_width()),
    );
    // The clear button edits the text without the field's `changed`.
    if query != state.channel_query {
        actions.push(Action::SetChannelQuery(query));
    }
    if field.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Escape)) {
        field.surrender_focus();
    }
    if field.lost_focus()
        && ui.input(|input| input.key_pressed(egui::Key::Enter))
        && !state.channel_query.trim().is_empty()
    {
        actions.push(Action::SetChannelTab(ChannelTab::Search(
            state.channel_query.trim().to_owned(),
        )));
    }
}

/// A video tab: Videos, Shorts, Live, or search results — a grid of
/// cards with the cursor, paging as the last card scrolls into view.
fn videos(ui: &mut Ui, state: &State, tab: &ChannelTab, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    let what = match tab {
        ChannelTab::Videos => "videos",
        ChannelTab::Shorts => "shorts",
        ChannelTab::Live => "live streams",
        ChannelTab::Search(_) => "results",
        _ => unreachable!(),
    };
    match &state.page_videos {
        Listing::Idle => {}
        Listing::Loading => results::loading_content(ui, state, what),
        Listing::Failed(error) => results::failed(ui, state, error, actions),
        Listing::Ready(videos) if videos.is_empty() => match tab {
            ChannelTab::Search(query) => results::empty(
                ui,
                state,
                tr(locale, "No results").as_ref(),
                &trf(
                    locale,
                    "No video on this channel matches “{query}”",
                    &[("query", query.clone())],
                ),
            ),
            _ => results::empty(
                ui,
                state,
                &trf(
                    locale,
                    "No content in {tab}",
                    &[("tab", tr(locale, what).into_owned())],
                ),
                tr(locale, "This channel has not posted anything here yet").as_ref(),
            ),
        },
        Listing::Ready(videos) => {
            let (columns, width) = widgets::grid_columns(ui.available_width(), CARD_WIDTH, GAP, 5);
            let indices: Vec<usize> = (0..videos.len()).collect();
            for row in indices.chunks(columns) {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = GAP;
                    for &index in row {
                        let video = &videos[index];
                        let selected = index == state.cursor;
                        let watched = widgets::watched(&state.library, video);
                        let response = widgets::video_card(
                            ui,
                            &state.palette,
                            video,
                            watched,
                            selected,
                            width,
                            locale,
                        );
                        if selected && state.cursor_moved {
                            response.clone().scroll_to_me(None);
                        }
                        // The last card visible asks for the next page.
                        if index == videos.len() - 1 && ui.is_rect_visible(response.rect) {
                            actions.push(Action::LoadMore);
                        }
                        widgets::video_interactions(ui, &response, state, video, actions);
                    }
                });
            }
            ui.add_space(GAP);
            more_row(ui, state, actions);
        }
    }
}

/// The tab's paged list's tail: the page on its way, Retry, or Load more.
fn more_row(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    widgets::more_row(
        ui,
        &state.palette,
        &state.page_more_state,
        state.page_more.is_some(),
        actions,
        locale,
    );
}

/// The playlists tab: rows of thumbnail, name, `N videos`.
fn playlists(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    match &state.page_playlists {
        Listing::Idle => {}
        Listing::Loading => results::loading_content(ui, state, "playlists"),
        Listing::Failed(error) => results::failed(ui, state, error, actions),
        Listing::Ready(playlists) if playlists.is_empty() => results::empty(
            ui,
            state,
            tr(locale, "No playlists").as_ref(),
            tr(locale, "This channel has no public playlists").as_ref(),
        ),
        Listing::Ready(playlists) => {
            results::virtual_rows(
                ui,
                106.0,
                playlists.len(),
                state.cursor_moved.then_some(state.cursor),
                |ui, index| {
                    let playlist = &playlists[index];
                    let selected = index == state.cursor;
                    let response = widgets::playlist_row(
                        ui,
                        &state.palette,
                        playlist,
                        selected,
                        false,
                        locale,
                    );
                    if selected && state.cursor_moved {
                        response.clone().scroll_to_me(None);
                    }
                    if index == playlists.len() - 1 && ui.is_rect_visible(response.rect) {
                        actions.push(Action::LoadMore);
                    }
                    if response.clicked() {
                        actions.push(Action::OpenPlaylist {
                            id: playlist.id.clone(),
                            name: playlist.name.clone(),
                        });
                    }
                },
            );
            ui.add_space(GAP);
            more_row(ui, state, actions);
        }
    }
}

/// The About tab: the description, the links and the details, each a card
/// under its heading.
fn about(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    match &state.channel_about {
        Fetch::Idle => {}
        Fetch::Loading => results::loading_content(ui, state, "channel information"),
        Fetch::Failed(error) => results::failed(ui, state, error, actions),
        Fetch::Ready(about) => {
            // set_max_width also widens: a narrow window would cut
            // the text off at its edge instead of wrapping it.
            ui.set_max_width(ABOUT_WIDTH.min(ui.available_width()));
            about_cards(ui, state, about, actions);
        }
    }
}

/// A label in secondary and its value in text, one line of a card's grid.
fn detail(ui: &mut Ui, state: &State, label: &'static str, value: &str) {
    let locale = state.settings.language;

    ui.label(
        RichText::new(tr(locale, label).as_ref())
            .font(theme::regular(14.0))
            .color(state.palette.secondary),
    );
    ui.label(
        RichText::new(value)
            .font(theme::regular(14.0))
            .color(state.palette.text),
    );
    ui.end_row();
}

/// Description, Links and More info, YouTube's About in cards.
fn about_cards(ui: &mut Ui, state: &State, about: &ChannelAbout, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    let palette = &state.palette;
    let header = &state.channel_header;
    if !about.description.is_empty() {
        widgets::section(
            ui,
            palette,
            tr(locale, "Description").as_ref(),
            None,
            |ui| {
                widgets::card(ui, palette, |ui| {
                    ui.label(
                        RichText::new(&about.description)
                            .font(theme::regular(14.0))
                            .color(palette.text),
                    );
                });
            },
        );
        ui.add_space(24.0);
    }
    if !about.links.is_empty() {
        widgets::section(ui, palette, tr(locale, "Links").as_ref(), None, |ui| {
            widgets::card(ui, palette, |ui| {
                egui::Grid::new("channel links")
                    .num_columns(2)
                    .spacing(vec2(24.0, GAP))
                    .show(ui, |ui| {
                        for (title, url) in &about.links {
                            ui.label(
                                RichText::new(title)
                                    .font(theme::regular(14.0))
                                    .color(palette.secondary),
                            );
                            let link = ui.link(
                                RichText::new(url)
                                    .font(theme::regular(14.0))
                                    .color(palette.blue),
                            );
                            if link.clicked() {
                                actions.push(Action::OpenUrl(url.clone()));
                            }
                            ui.end_row();
                        }
                    });
            });
        });
        ui.add_space(24.0);
    }
    widgets::section(ui, palette, tr(locale, "More info").as_ref(), None, |ui| {
        widgets::card(ui, palette, |ui| {
            egui::Grid::new("channel details")
                .num_columns(2)
                .spacing(vec2(24.0, GAP))
                .show(ui, |ui| {
                    if let Some(handle) = header.as_ref().and_then(|header| header.handle.clone()) {
                        detail(ui, state, "Handle", &handle);
                    }
                    let subscribers = about
                        .subscribers
                        .or_else(|| header.as_ref().and_then(|header| header.subscribers));
                    if let Some(count) = subscribers {
                        detail(ui, state, "Subscribers", &util::compact(count));
                    }
                    let videos = about
                        .videos
                        .or_else(|| header.as_ref().and_then(|header| header.videos));
                    if let Some(count) = videos {
                        detail(ui, state, "Videos", &util::compact(count));
                    }
                    if let Some(views) = about.views {
                        detail(ui, state, "Views", &util::thousands(views));
                    }
                    if let Some(created) = &about.created {
                        detail(ui, state, "Joined", created);
                    }
                    if let Some(country) = &about.country {
                        detail(ui, state, "Country", country);
                    }
                    detail(ui, state, "Address", &about.url);
                });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Video;

    #[test]
    fn banner_cover_crops_the_centre_without_stretching() {
        for (image, target, expected) in [
            (vec2(1200.0, 200.0), vec2(600.0, 100.0), vec2(1.0, 1.0)),
            (vec2(1200.0, 200.0), vec2(600.0, 200.0), vec2(0.5, 1.0)),
            (vec2(600.0, 400.0), vec2(600.0, 100.0), vec2(1.0, 0.25)),
        ] {
            let uv = banner_uv(image, target);
            assert_eq!(uv.center(), egui::pos2(0.5, 0.5));
            assert_eq!(uv.size(), expected);
            let cropped = image * uv.size();
            assert!((cropped.x / cropped.y - target.x / target.y).abs() < 0.001);
        }
    }

    /// A channel with a Shorts tab but no Live tab.
    fn header() -> ChannelHeader {
        ChannelHeader {
            has_shorts: true,
            ..ChannelHeader::default()
        }
    }

    #[test]
    fn only_the_tabs_the_channel_has_show() {
        // Unknown header: no Shorts or Live.
        assert_eq!(
            tabs(None, &ChannelTab::Videos),
            vec![ChannelTab::Videos, ChannelTab::Playlists, ChannelTab::About,]
        );
        let header = header();
        assert_eq!(
            tabs(Some(&header), &ChannelTab::Videos),
            vec![
                ChannelTab::Videos,
                ChannelTab::Shorts,
                ChannelTab::Playlists,
                ChannelTab::About,
            ]
        );
        // A live search keeps its entry at the end.
        assert_eq!(
            tabs(Some(&header), &ChannelTab::Search("rust".to_owned())),
            vec![
                ChannelTab::Videos,
                ChannelTab::Shorts,
                ChannelTab::Playlists,
                ChannelTab::About,
                ChannelTab::Search("rust".to_owned()),
            ]
        );
    }

    #[test]
    fn the_tab_keys_walk_and_wrap() {
        let visible = tabs(Some(&header()), &ChannelTab::Videos);
        assert_eq!(
            cycle_tab(&visible, &ChannelTab::Videos, 1),
            ChannelTab::Shorts
        );
        assert_eq!(
            cycle_tab(&visible, &ChannelTab::About, 1),
            ChannelTab::Videos
        );
        assert_eq!(
            cycle_tab(&visible, &ChannelTab::Videos, -1),
            ChannelTab::About
        );
        // Cycling stops on the search tab's query unchanged.
        let with_search = tabs(Some(&header()), &ChannelTab::Search("rust".to_owned()));
        assert_eq!(
            cycle_tab(&with_search, &ChannelTab::About, 1),
            ChannelTab::Search("rust".to_owned())
        );
    }

    #[test]
    fn mouse_wheel_outside_video_box_scrolls_channel() {
        let ctx = egui::Context::default();
        crate::appearance::theme::install_fonts(&ctx);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 720.0));
        let channel = Channel {
            id: "UC123".into(),
            name: "Test Channel".into(),
            avatar: None,
        };
        let state = State {
            view: crate::model::View::Channel,
            page: Some(crate::app::Page::Channel(channel.clone())),
            channel_header: Some(ChannelHeader {
                name: "Test Channel".into(),
                banner: Some("https://example.com/banner.jpg".into()),
                ..Default::default()
            }),
            page_videos: Listing::Ready(
                (0..30)
                    .map(|i| Video::from_id(format!("{i:011}")))
                    .collect(),
            ),
            ..Default::default()
        };

        // Find the channel name text position in an output.
        let get_title_y = |output: &egui::FullOutput| {
            output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if text.galley.text() == "Test Channel" => {
                    Some(text.pos.y)
                }
                _ => None,
            })
        };

        // Frame 0: render initial state
        let mut first = ctx.run_ui(
            egui::RawInput {
                time: Some(0.0),
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ui| show(ui, &state, &channel, &mut Vec::new()),
        );
        first.textures_delta.clear();
        let y0 = get_title_y(&first).expect("title found in frame 0");

        // The banner / header is at y < 300. We place pointer on the header (pos2(300.0, 100.0)).
        // This is outside the box with videos.
        let mut last = first;
        for frame in 1..3 {
            let scroll_event = egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                phase: egui::TouchPhase::Move,
                delta: egui::vec2(0.0, -20.0),
                modifiers: egui::Modifiers::NONE,
            };
            last = ctx.run_ui(
                egui::RawInput {
                    time: Some(frame as f64 / 10.0),
                    screen_rect: Some(screen),
                    events: vec![
                        egui::Event::PointerMoved(egui::pos2(300.0, 100.0)),
                        scroll_event,
                    ],
                    ..Default::default()
                },
                |ui| show(ui, &state, &channel, &mut Vec::new()),
            );
            last.textures_delta.clear();
        }

        let y1 = get_title_y(&last).expect("title found in last frame");
        assert!(
            y1 < y0,
            "Channel content should have scrolled up when mouse wheeled over header: y0={y0}, y1={y1}"
        );
    }
}
