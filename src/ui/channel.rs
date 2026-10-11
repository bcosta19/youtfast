//! A channel's page, YouTube's way: the header (round avatar, the name,
//! the numbers, Subscribe), the underlined tab bar with the channel's own
//! search at its right, and the tab: a grid of video cards, playlist rows,
//! or the About cards.

use egui::{Align, Layout, RichText, Sense, Ui, pos2, vec2};

use super::{results, widgets};
use crate::app::State;
use crate::model::{Action, Channel, ChannelAbout, ChannelHeader, ChannelTab, Fetch, Listing};
use crate::theme;
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

pub fn show(ui: &mut Ui, state: &State, channel: &Channel, actions: &mut Vec<Action>) {
    results::panel(ui, |ui| {
        header(ui, state, channel, actions);
        ui.add_space(16.0);
        tab_bar(ui, state, actions);
        ui.add_space(16.0);
        match &state.channel_tab {
            ChannelTab::About => about(ui, state, actions),
            ChannelTab::Playlists => playlists(ui, state, actions),
            tab => videos(ui, state, tab, actions),
        }
    });
}

/// The header: the round avatar, the name in bold, the handle and the
/// numbers in secondary, and Subscribe as the primary pill.
fn header(ui: &mut Ui, state: &State, channel: &Channel, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let header = state.channel_header.as_ref();
    let title = header
        .map(|header| header.name.clone())
        .unwrap_or_else(|| channel.name.clone());
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(80.0, 80.0), Sense::hover());
        let avatar = header.and_then(|header| header.avatar.as_deref());
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
                    parts.push(format!("{} subscribers", util::compact(subscribers)));
                }
                if let Some(videos) = header.videos {
                    parts.push(format!("{} videos", util::compact(videos)));
                }
                if !parts.is_empty() {
                    ui.label(
                        RichText::new(parts.join(" · "))
                            .font(theme::regular(14.0))
                            .color(palette.secondary),
                    );
                }
            }
            ui.add_space(4.0);
            widgets::subscribe(ui, state, channel, widgets::ButtonStyle::Primary, actions);
        });
    });
}

/// The tab's name in the bar.
fn tab_label(tab: &ChannelTab) -> String {
    match tab {
        ChannelTab::Videos => "Videos".to_owned(),
        ChannelTab::Shorts => "Shorts".to_owned(),
        ChannelTab::Live => "Live".to_owned(),
        ChannelTab::Playlists => "Playlists".to_owned(),
        ChannelTab::About => "About".to_owned(),
        ChannelTab::Search(query) => format!("Search: {query}"),
    }
}

/// YouTube's tab bar: the names in medium, the current one in `text` over
/// a 2 px underline, the others in `secondary`; a hairline under the bar;
/// the channel's own search at its right.
fn tab_bar(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let height = 40.0;
    let bar = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 24.0;
        for tab in tabs(state.channel_header.as_ref(), &state.channel_tab) {
            let current = same_kind(&tab, &state.channel_tab);
            let galley =
                ui.painter()
                    .layout_no_wrap(tab_label(&tab), theme::medium(15.0), palette.text);
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
    let mut query = state.channel_query.clone();
    let field = widgets::search_field(
        ui,
        &state.palette,
        &mut query,
        "Search this channel",
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
    let what = match tab {
        ChannelTab::Videos => "videos",
        ChannelTab::Shorts => "shorts",
        ChannelTab::Live => "live streams",
        ChannelTab::Search(_) => "results",
        _ => unreachable!(),
    };
    match &state.page_videos {
        Listing::Idle => {}
        Listing::Loading => results::loading(ui, state, what),
        Listing::Failed(error) => results::failed(ui, state, error, actions),
        Listing::Ready(videos) if videos.is_empty() => match tab {
            ChannelTab::Search(query) => results::empty(
                ui,
                state,
                "No results",
                &format!("Nothing on this channel matches \u{201c}{query}\u{201d}"),
            ),
            _ => results::empty(
                ui,
                state,
                &format!("No {what}"),
                "This channel has not posted any here yet",
            ),
        },
        Listing::Ready(videos) => {
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    let (columns, width) =
                        widgets::grid_columns(ui.available_width(), CARD_WIDTH, GAP, 5);
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
                                );
                                if selected && state.cursor_moved {
                                    response.clone().scroll_to_me(None);
                                }
                                // The last card visible asks for the next page.
                                if index == videos.len() - 1 && ui.is_rect_visible(response.rect) {
                                    actions.push(Action::LoadMore);
                                }
                                if response.clicked() {
                                    actions.push(Action::Open(video.clone()));
                                }
                                results::video_menu(&response, state, video, actions);
                            }
                        });
                    }
                    ui.add_space(GAP);
                    more_row(ui, state, actions);
                });
        }
    }
}

/// The tab's paged list's tail: the page on its way, Retry, or Load more.
fn more_row(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    widgets::more_row(
        ui,
        &state.palette,
        &state.page_more_state,
        state.page_more.is_some(),
        actions,
    );
}

/// The playlists tab: rows of thumbnail, name, `N videos`.
fn playlists(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    match &state.page_playlists {
        Listing::Idle => {}
        Listing::Loading => results::loading(ui, state, "playlists"),
        Listing::Failed(error) => results::failed(ui, state, error, actions),
        Listing::Ready(playlists) if playlists.is_empty() => results::empty(
            ui,
            state,
            "No playlists",
            "This channel has no public playlists",
        ),
        Listing::Ready(playlists) => {
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    for (index, playlist) in playlists.iter().enumerate() {
                        let selected = index == state.cursor;
                        let response =
                            widgets::playlist_row(ui, &state.palette, playlist, selected, false);
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
                    }
                    ui.add_space(GAP);
                    more_row(ui, state, actions);
                });
        }
    }
}

/// The About tab: the description, the links and the details, each a card
/// under its heading.
fn about(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| match &state.channel_about {
            Fetch::Idle => {}
            Fetch::Loading => results::loading(ui, state, "about"),
            Fetch::Failed(error) => results::failed(ui, state, error, actions),
            Fetch::Ready(about) => {
                // set_max_width also widens: a narrow window would cut
                // the text off at its edge instead of wrapping it.
                ui.set_max_width(ABOUT_WIDTH.min(ui.available_width()));
                about_cards(ui, state, about, actions);
            }
        });
}

/// A label in secondary and its value in text, one line of a card's grid.
fn detail(ui: &mut Ui, state: &State, label: &str, value: &str) {
    ui.label(
        RichText::new(label)
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
    let palette = &state.palette;
    let header = &state.channel_header;
    if !about.description.is_empty() {
        widgets::section(ui, palette, "Description", None, |ui| {
            widgets::card(ui, palette, |ui| {
                ui.label(
                    RichText::new(&about.description)
                        .font(theme::regular(14.0))
                        .color(palette.text),
                );
            });
        });
        ui.add_space(24.0);
    }
    if !about.links.is_empty() {
        widgets::section(ui, palette, "Links", None, |ui| {
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
    widgets::section(ui, palette, "More info", None, |ui| {
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
}
