//! A channel's page: the header (avatar, name, numbers, its own search),
//! the tab line, and the tab's pane — video rows, playlist rows, or the
//! About pane's key/value lines.

use egui::{Align2, RichText, Sense, TextEdit, Ui, vec2};

use super::{results, widgets};
use crate::app::State;
use crate::model::{Action, Channel, ChannelAbout, ChannelHeader, ChannelTab, Fetch, Listing};
use crate::util;

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
    egui::CentralPanel::default().show(ui, |ui| {
        header(ui, state, channel, actions);
        ui.add_space(widgets::line(ui) / 2.0);
        tab_row(ui, state, actions);
        ui.add_space(widgets::line(ui) / 2.0);
        match &state.channel_tab {
            ChannelTab::About => about(ui, state, actions),
            ChannelTab::Playlists => playlists(ui, state, actions),
            tab => videos(ui, state, tab, actions),
        }
    });
}

/// The header pane: the square avatar, the name in blue bold, the
/// numbers in secondary, subscribe and the channel's own search.
fn header(ui: &mut Ui, state: &State, channel: &Channel, actions: &mut Vec<Action>) {
    let title = state
        .channel_header
        .as_ref()
        .map(|header| header.name.clone())
        .unwrap_or_else(|| channel.name.clone());
    widgets::pane(ui, &state.palette, &title, None, true, None, |ui| {
        ui.horizontal(|ui| {
            // The 64 px square avatar, or a placeholder initial.
            let size = vec2(64.0, 64.0);
            let avatar = state
                .channel_header
                .as_ref()
                .and_then(|header| header.avatar.as_deref());
            match avatar {
                Some(url) => {
                    ui.add(
                        super::images::remote(ui, url)
                            .fit_to_exact_size(size)
                            .corner_radius(egui::CornerRadius::ZERO),
                    );
                }
                None => {
                    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                    ui.painter()
                        .rect_filled(rect, egui::CornerRadius::ZERO, state.palette.surface);
                    let initial: String = title.chars().take(1).collect();
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        initial.to_uppercase(),
                        egui::FontId::monospace(28.0),
                        state.palette.dim,
                    );
                }
            }
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(&title)
                        .monospace()
                        .strong()
                        .color(state.palette.blue),
                );
                if let Some(header) = &state.channel_header {
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
                                .monospace()
                                .color(state.palette.secondary),
                        );
                    }
                }
            });
        });
        ui.horizontal(|ui| {
            widgets::subscribe(ui, state, channel, widgets::ButtonStyle::Normal, actions);
            channel_search(ui, state, actions);
        });
    });
}

/// The borderless field that searches this channel, underlined.
fn channel_search(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    ui.add_space(widgets::cell(ui));
    let mut query = state.channel_query.clone();
    let field = ui.add(
        TextEdit::singleline(&mut query)
            .desired_width(240.0)
            .hint_text(RichText::new("search this channel").color(state.palette.dim))
            .font(egui::FontId::monospace(14.0))
            .frame(egui::Frame::NONE),
    );
    if field.changed() {
        actions.push(Action::SetChannelQuery(query));
    }
    // The underline: `outline`, `accent` while focused.
    let color = if field.has_focus() {
        state.palette.accent
    } else {
        state.palette.outline
    };
    ui.painter().hline(
        field.rect.left()..=field.rect.right(),
        field.rect.bottom() - 1.0,
        egui::Stroke::new(1.0, color),
    );
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

/// `[ videos ] [ shorts ] [ live ] [ playlists ] [ about ]`, the active
/// one On.
fn tab_row(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        for tab in tabs(state.channel_header.as_ref(), &state.channel_tab) {
            let label = match &tab {
                ChannelTab::Videos => "videos".to_owned(),
                ChannelTab::Shorts => "shorts".to_owned(),
                ChannelTab::Live => "live".to_owned(),
                ChannelTab::Playlists => "playlists".to_owned(),
                ChannelTab::About => "about".to_owned(),
                ChannelTab::Search(query) => format!("search: {query}"),
            };
            let style = if same_kind(&tab, &state.channel_tab) {
                widgets::ButtonStyle::On
            } else {
                widgets::ButtonStyle::Normal
            };
            if widgets::bracket_button(ui, &state.palette, &label, style).clicked() {
                actions.push(Action::SetChannelTab(tab));
            }
        }
    });
}

/// A video tab: Videos, Shorts, Live, or search results — rows with the
/// cursor, paging as the last row scrolls into view.
fn videos(ui: &mut Ui, state: &State, tab: &ChannelTab, actions: &mut Vec<Action>) {
    let title = match tab {
        ChannelTab::Videos => "Videos".to_owned(),
        ChannelTab::Shorts => "Shorts".to_owned(),
        ChannelTab::Live => "Live".to_owned(),
        ChannelTab::Search(query) => format!("search: {query}"),
        _ => unreachable!(),
    };
    widgets::pane(
        ui,
        &state.palette,
        &title,
        Some(state.page_videos.items().len()),
        true,
        None,
        |ui| match &state.page_videos {
            Listing::Idle => {}
            Listing::Loading => results::loading(ui, state, &title),
            Listing::Failed(error) => results::failed(ui, state, error, actions),
            Listing::Ready(videos) if videos.is_empty() => {
                results::nothing(ui, state, "nothing here")
            }
            Listing::Ready(videos) => {
                let videos = videos.clone();
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        for (index, video) in videos.iter().enumerate() {
                            let selected = index == state.cursor;
                            let watched = widgets::watched(&state.library, video);
                            let response = widgets::video_row(
                                ui,
                                &state.palette,
                                video,
                                None,
                                watched,
                                selected,
                            );
                            if selected {
                                response.clone().scroll_to_me(None);
                            }
                            // The last row visible asks for the next page.
                            if index == videos.len() - 1 && ui.is_rect_visible(response.rect) {
                                actions.push(Action::LoadMore);
                            }
                            if response.clicked() {
                                actions.push(Action::Open(video.clone()));
                            }
                        }
                        more_row(ui, state, actions);
                    });
            }
        },
    );
}

/// The tab's paged list's tail row.
fn more_row(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    widgets::more_row(
        ui,
        &state.palette,
        &state.page_more_state,
        state.page_more.is_some(),
        actions,
    );
}

/// The playlists tab: thumbnail, name, `N videos`.
fn playlists(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    widgets::pane(
        ui,
        &state.palette,
        "Playlists",
        Some(state.page_playlists.items().len()),
        true,
        None,
        |ui| match &state.page_playlists {
            Listing::Idle => {}
            Listing::Loading => results::loading(ui, state, "playlists"),
            Listing::Failed(error) => results::failed(ui, state, error, actions),
            Listing::Ready(playlists) if playlists.is_empty() => {
                results::nothing(ui, state, "no playlists")
            }
            Listing::Ready(playlists) => {
                let playlists = playlists.clone();
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        for (index, playlist) in playlists.iter().enumerate() {
                            let selected = index == state.cursor;
                            let response = widgets::playlist_row(
                                ui,
                                &state.palette,
                                playlist,
                                selected,
                                false,
                            );
                            if selected {
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
                        more_row(ui, state, actions);
                    });
            }
        },
    );
}

/// The About pane: key/value lines, then the description, then links.
fn about(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    widgets::pane(ui, &state.palette, "About", None, true, None, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| match &state.channel_about {
                Fetch::Idle => {}
                Fetch::Loading => results::loading(ui, state, "about"),
                Fetch::Failed(error) => results::failed(ui, state, error, actions),
                Fetch::Ready(about) => about_rows(ui, state, about, actions),
            });
    });
}

/// `key ········· value` rows, then the text, then the links.
fn about_rows(ui: &mut Ui, state: &State, about: &ChannelAbout, actions: &mut Vec<Action>) {
    let header = &state.channel_header;
    let kv = |ui: &mut Ui, key: &str, value: &str| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{key:·<12}"))
                    .monospace()
                    .color(state.palette.dim),
            );
            ui.label(RichText::new(value).monospace().color(state.palette.text));
        });
    };
    if let Some(handle) = header.as_ref().and_then(|header| header.handle.clone()) {
        kv(ui, "handle", &handle);
    }
    let subscribers = about
        .subscribers
        .or_else(|| header.as_ref().and_then(|header| header.subscribers));
    if let Some(count) = subscribers {
        kv(ui, "subscribers", &util::compact(count));
    }
    let videos = about
        .videos
        .or_else(|| header.as_ref().and_then(|header| header.videos));
    if let Some(count) = videos {
        kv(ui, "videos", &util::compact(count));
    }
    if let Some(views) = about.views {
        kv(ui, "views", &util::thousands(views));
    }
    if let Some(created) = &about.created {
        kv(ui, "joined", created);
    }
    if let Some(country) = &about.country {
        kv(ui, "country", country);
    }
    kv(ui, "url", &about.url);
    if !about.description.is_empty() {
        ui.add_space(widgets::line(ui) / 2.0);
        ui.label(
            RichText::new(&about.description)
                .monospace()
                .color(state.palette.text),
        );
    }
    if !about.links.is_empty() {
        ui.add_space(widgets::line(ui) / 2.0);
        for (title, url) in &about.links {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{title:·<16}"))
                        .monospace()
                        .color(state.palette.dim),
                );
                let link = ui.link(RichText::new(url).monospace().color(state.palette.cyan));
                if link.clicked() {
                    actions.push(Action::OpenUrl(url.clone()));
                }
            });
        }
    }
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
