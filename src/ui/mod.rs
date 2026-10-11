//! The views. They draw from [`State`] and the player's status and push
//! [`Action`]s; `App::apply` carries them out after the frame. The only
//! state a view changes itself is its own (the search text, the seek bar
//! while it is dragged).
//!
//! The shell is the siblings' (Spotifast, ZapFast): a top bar with the
//! search field, a sidebar of views and subscribed channels, notices as
//! toasts at the bottom right, and `?` for the keyboard shortcuts.

mod account;
mod channel;
pub mod images;
mod library;
mod notifications;
mod player;
mod results;
mod settings;
mod watch;
mod widgets;

use egui::{
    Align, CornerRadius, Frame, Key, Layout, Margin, RichText, Sense, Stroke, UiBuilder, vec2,
};

use crate::app::{AccountState, State};
use crate::icons::Icon;
use crate::model::{Action, ChannelTab, Fetch, Listing, View};
use crate::mpv::{Status, VideoSurface};
use crate::theme::{self, RADIUS};

/// The search field's widget id, so `/` can focus it.
fn command_line_id() -> egui::Id {
    egui::Id::new("command-line")
}

pub fn show(
    ui: &mut egui::Ui,
    state: &mut State,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
    actions: &mut Vec<Action>,
) {
    keys(ui.ctx(), state, status, actions);
    if state.view == View::Watch && state.fullscreen {
        watch::fullscreen(ui, state, status, surface, actions);
        return;
    }
    top_bar(ui, state, actions);
    sidebar(ui, state, actions);
    match state.view {
        View::Home => library::home(ui, state, actions),
        View::Results => results::show(ui, state, actions),
        View::Watch => watch::show(ui, state, status, surface, actions),
        View::Subscriptions => library::subscriptions(ui, state, actions),
        View::History => library::history(ui, state, actions),
        View::WatchLater => library::watch_later(ui, state, actions),
        View::Liked => library::liked(ui, state, actions),
        View::Playlists => library::playlists(ui, state, actions),
        View::Playlist => library::page(ui, state, actions),
        View::Notifications => notifications::show(ui, state, actions),
        View::Channel => {
            if let Some(crate::app::Page::Channel(channel)) = &state.page {
                channel::show(ui, state, channel, actions);
            }
        }
        View::Account => account::show(ui, state, actions),
        View::Settings => settings::show(ui, state, actions),
    }
    // The video goes on in the corner while other pages show.
    if state.view != View::Watch && !state.mini_closed {
        player::mini(ui.ctx(), state, status, surface, actions);
    }
    notice_toast(ui.ctx(), state, actions);
    if state.keys_open {
        keys_overlay(ui.ctx(), state);
    }
}

/// The top bar's height.
const TOP_BAR: f32 = 56.0;

/// The top bar: the back arrow when there is a page to go back to, the
/// search field centred (Enter searches or opens a link, Esc leaves it),
/// and the keyboard shortcuts' button at the right.
fn top_bar(ui: &mut egui::Ui, state: &mut State, actions: &mut Vec<Action>) {
    let palette = state.palette.clone();
    egui::Panel::top("top-bar")
        .exact_size(TOP_BAR)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(palette.panel)
                .inner_margin(Margin::symmetric(16, 0)),
        )
        .show(ui, |ui| {
            let bar = ui.max_rect();
            let place =
                |rect: egui::Rect, layout: Layout| UiBuilder::new().max_rect(rect).layout(layout);
            let back = matches!(
                state.view,
                View::Watch | View::Channel | View::Playlist | View::Results
            );
            if back {
                ui.scope_builder(place(bar, Layout::left_to_right(Align::Center)), |ui| {
                    if widgets::icon_button(ui, Icon::Back, 20.0, "Back").clicked() {
                        actions.push(Action::Back);
                    }
                });
            }
            ui.scope_builder(place(bar, Layout::right_to_left(Align::Center)), |ui| {
                if widgets::icon_button(ui, Icon::Info, 20.0, "Keyboard shortcuts (?)").clicked() {
                    state.keys_open = !state.keys_open;
                }
            });
            // Centred, clear of the 32 px buttons at both ends.
            let width = (bar.width() - 2.0 * 48.0).clamp(120.0, 560.0);
            let field_rect = egui::Rect::from_center_size(bar.center(), vec2(width, 36.0));
            ui.scope_builder(
                place(field_rect, Layout::left_to_right(Align::Center)),
                |ui| {
                    let field = widgets::search_field(
                        ui,
                        &palette,
                        &mut state.query,
                        "Search, or paste a link",
                        command_line_id(),
                        width,
                    );
                    if field.has_focus() && ui.input(|input| input.key_pressed(Key::Escape)) {
                        field.surrender_focus();
                        state.suggestions.clear();
                    }
                    if field.changed() {
                        state.suggestion_cursor = None;
                    }
                    let submitted =
                        field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
                    if submitted {
                        let picked = state
                            .suggestion_cursor
                            .and_then(|index| state.suggestions.get(index).cloned());
                        if let Some(picked) = picked {
                            state.query = picked;
                        }
                        actions.push(Action::Search(state.query.clone()));
                    }
                    suggestions(ui, state, &field, field_rect, actions);
                },
            );
        });
}

/// YouTube's suggestions under the search field while it is focused; ↑/↓
/// pick one for Enter, a click searches it.
fn suggestions(
    ui: &mut egui::Ui,
    state: &mut State,
    field: &egui::Response,
    field_rect: egui::Rect,
    actions: &mut Vec<Action>,
) {
    let id = egui::Id::new("search-suggestions");
    // A click on a row takes the focus from the field first; the list stays
    // for that frame while the pointer is over it.
    let hovered_last = ui
        .ctx()
        .data(|data| data.get_temp::<egui::Rect>(id))
        .is_some_and(|rect| ui.rect_contains_pointer(rect));
    if state.suggestions.is_empty() || state.query.trim().is_empty() {
        return;
    }
    if !field.has_focus() && !hovered_last {
        return;
    }
    if field.has_focus() {
        let (down, up) = ui.input(|input| {
            (
                input.key_pressed(Key::ArrowDown),
                input.key_pressed(Key::ArrowUp),
            )
        });
        let last = state.suggestions.len() - 1;
        if down {
            state.suggestion_cursor =
                Some(state.suggestion_cursor.map_or(0, |at| (at + 1).min(last)));
        }
        if up {
            state.suggestion_cursor = state.suggestion_cursor.and_then(|at| at.checked_sub(1));
        }
    }
    let palette = state.palette.clone();
    let area = egui::Area::new(id.with("area"))
        .order(egui::Order::Foreground)
        .fixed_pos(field_rect.left_bottom() + vec2(0.0, 4.0))
        .show(ui.ctx(), |ui| {
            Frame::new()
                .fill(palette.overlay)
                .stroke(Stroke::new(1.0, palette.outline))
                .corner_radius(CornerRadius::same(RADIUS + 4))
                .inner_margin(Margin::symmetric(0, 8))
                .shadow(ui.visuals().popup_shadow)
                .show(ui, |ui| {
                    ui.set_width(field_rect.width());
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (index, text) in state.suggestions.iter().enumerate() {
                        let (rect, response) =
                            ui.allocate_exact_size(vec2(field_rect.width(), 32.0), Sense::click());
                        let selected = state.suggestion_cursor == Some(index);
                        if response.hovered() || selected {
                            ui.painter().rect_filled(
                                rect,
                                CornerRadius::ZERO,
                                palette.surface_hover,
                            );
                        }
                        let icon = egui::Rect::from_center_size(
                            egui::pos2(rect.left() + 20.0, rect.center().y),
                            vec2(14.0, 14.0),
                        );
                        Icon::Search
                            .image(palette.secondary, 14.0)
                            .paint_at(ui, icon);
                        ui.painter().text(
                            egui::pos2(rect.left() + 40.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            text,
                            theme::regular(14.0),
                            palette.text,
                        );
                        if response
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            actions.push(Action::Search(text.clone()));
                        }
                    }
                });
        });
    ui.ctx()
        .data_mut(|data| data.insert_temp(id, area.response.rect));
    if let Some(Action::Search(text)) = actions.last() {
        state.query.clone_from(text);
    }
}

/// The sidebar's width, open and collapsed to its rail.
const SIDEBAR_OPEN: f32 = 240.0;
const SIDEBAR_RAIL: f32 = 72.0;

/// The sidebar: the `YouTFast` wordmark and the collapse toggle, the
/// views as nav items (icon and label), then the channels subscribed to.
/// Collapsed (`b`), it is a rail of the views' icons and the channels'
/// avatars, each named on hover.
fn sidebar(ui: &mut egui::Ui, state: &State, actions: &mut Vec<Action>) {
    let collapsed = state.settings.sidebar_collapsed;
    let palette = &state.palette;
    egui::Panel::left("sidebar")
        .exact_size(if collapsed {
            SIDEBAR_RAIL
        } else {
            SIDEBAR_OPEN
        })
        .resizable(false)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(palette.panel)
                .inner_margin(Margin::symmetric(12, 8)),
        )
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            let hint = if collapsed {
                "Show the sidebar (b)"
            } else {
                "Collapse the sidebar (b)"
            };
            let toggled = ui
                .allocate_ui_with_layout(
                    vec2(ui.available_width(), 40.0),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.set_min_height(40.0);
                        if collapsed {
                            ui.add_space(((ui.available_width() - 32.0) / 2.0).max(0.0));
                            return widgets::icon_button(ui, Icon::PanelLeft, 20.0, hint).clicked();
                        }
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("YouTFast")
                                .font(theme::bold(18.0))
                                .color(palette.text),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            widgets::icon_button(ui, Icon::PanelLeft, 20.0, hint).clicked()
                        })
                        .inner
                    },
                )
                .inner;
            if toggled {
                actions.push(Action::ToggleSidebar);
            }
            ui.add_space(8.0);
            for (icon, view, label, count) in sidebar_items(state) {
                let selected = state.view == view
                    || (view == View::Playlists && state.view == View::Playlist)
                    || (view == View::Subscriptions && state.view == View::Channel);
                if view == View::Account {
                    ui.add_space(12.0);
                }
                if widgets::nav_item(ui, palette, icon, &label, count, selected, collapsed)
                    .clicked()
                {
                    actions.push(Action::Navigate(view));
                }
            }
            sidebar_channels(ui, state, collapsed, actions);
        });
}

/// The sidebar's views, in the number keys' order: icon, view, label and
/// a count to show.
fn sidebar_items(state: &State) -> Vec<(Icon, View, String, Option<usize>)> {
    let mut items = vec![
        (Icon::Home, View::Home, "Home".to_owned(), None),
        (
            Icon::Subscriptions,
            View::Subscriptions,
            "Subscriptions".to_owned(),
            None,
        ),
        (Icon::History, View::History, "History".to_owned(), None),
        (
            Icon::Clock,
            View::WatchLater,
            "Watch later".to_owned(),
            None,
        ),
    ];
    if state.signed_in() {
        items.push((Icon::ThumbsUp, View::Liked, "Liked".to_owned(), None));
        items.push((
            Icon::Playlists,
            View::Playlists,
            "Playlists".to_owned(),
            None,
        ));
    }
    items.push((
        Icon::Bell,
        View::Notifications,
        "Notifications".to_owned(),
        Some(state.alerts.len()),
    ));
    let account = state
        .active_account()
        .map(|account| account.name.clone())
        .unwrap_or_else(|| match &state.account {
            AccountState::SigningIn => "Signing in".to_owned(),
            _ => "Sign in".to_owned(),
        });
    items.push((Icon::User, View::Account, account, None));
    items.push((Icon::Settings, View::Settings, "Settings".to_owned(), None));
    items
}

/// YouTube's sidebar lists the channels subscribed to under the views:
/// the account's when signed in and loaded, this device's otherwise.
/// `collapsed` draws only their avatars, without the heading.
fn sidebar_channels(ui: &mut egui::Ui, state: &State, collapsed: bool, actions: &mut Vec<Action>) {
    let channels = match state.account_subscriptions.items() {
        channels if state.signed_in() && !channels.is_empty() => channels,
        _ => state.library.subscriptions.as_slice(),
    };
    if channels.is_empty() {
        return;
    }
    let open = match &state.page {
        Some(crate::app::Page::Channel(channel)) if state.view == View::Channel => {
            Some(channel.id.as_str())
        }
        _ => None,
    };
    ui.add_space(16.0);
    if !collapsed {
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(
                RichText::new("Subscriptions")
                    .font(theme::semibold(14.0))
                    .color(state.palette.secondary),
            );
        });
        ui.add_space(4.0);
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for channel in channels {
                let selected = open == Some(channel.id.as_str());
                let response = if collapsed {
                    widgets::channel_avatar(ui, &state.palette, channel, selected)
                        .on_hover_text(&channel.name)
                } else {
                    widgets::channel_item(ui, &state.palette, channel, selected)
                };
                if response.clicked() {
                    actions.push(Action::OpenChannel(channel.clone()));
                }
            }
        });
}

/// The last notice as a toast at the bottom right: an icon (the alert one
/// for a failure), the text, and a button that dismisses it.
fn notice_toast(ctx: &egui::Context, state: &State, actions: &mut Vec<Action>) {
    let Some(notice) = &state.notice else {
        return;
    };
    let palette = &state.palette;
    let (icon, tint) = if notice.starts_with("Could not") {
        (Icon::Alert, palette.red)
    } else {
        (Icon::Info, palette.secondary)
    };
    egui::Area::new(egui::Id::new("notice"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_BOTTOM, vec2(-20.0, -20.0))
        .show(ctx, |ui| {
            Frame::new()
                .fill(palette.overlay)
                .stroke(Stroke::new(1.0, palette.outline))
                .corner_radius(CornerRadius::same(RADIUS))
                .inner_margin(Margin::symmetric(14, 10))
                .shadow(ui.visuals().popup_shadow)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(icon.image(tint, 18.0));
                        ui.add_space(4.0);
                        ui.vertical(|ui| {
                            ui.set_max_width(320.0);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(notice)
                                        .font(theme::medium(13.5))
                                        .color(palette.text),
                                )
                                .wrap(),
                            );
                        });
                        if widgets::icon_button(ui, Icon::Close, 16.0, "Dismiss").clicked() {
                            actions.push(Action::DismissNotice);
                        }
                    });
                });
        });
}

/// The cursor's reset key: which view's listing it is on, and whether it
/// is still coming in. A switch of view, source or listing state restarts
/// it at the top.
pub fn cursor_key(state: &State) -> usize {
    fn tag<T>(listing: &Listing<T>) -> usize {
        match listing {
            Listing::Idle => 0,
            Listing::Loading => 1,
            Listing::Ready(_) => 2,
            Listing::Failed(_) => 3,
        }
    }
    let source = usize::from(state.source() == crate::model::Source::Device) * 10;
    match state.view {
        View::Home => tag(&state.feed) + 10 * state.feed_filter as usize,
        View::Results => tag(&state.results),
        View::Subscriptions => tag(&state.account_subscriptions) + source,
        View::History => tag(&state.account_history) + source,
        View::WatchLater => tag(&state.account_watch_later) + source,
        View::Liked => tag(&state.liked),
        View::Playlists => tag(&state.playlists),
        View::Playlist => tag(&state.page_videos),
        View::Notifications => 80 + state.alerts_visit.len().min(1),
        View::Channel => {
            let tab = match state.channel_tab {
                ChannelTab::Videos => 0,
                ChannelTab::Shorts => 10,
                ChannelTab::Live => 20,
                ChannelTab::Playlists => 30,
                ChannelTab::About => 40,
                ChannelTab::Search(_) => 50,
            };
            match state.channel_tab {
                ChannelTab::Playlists => tab + tag(&state.page_playlists),
                ChannelTab::About => tab + matches!(state.channel_about, Fetch::Ready(_)) as usize,
                _ => tab + tag(&state.page_videos),
            }
        }
        _ => 0,
    }
}

/// How many rows the current view's list has (0 = no cursor there).
fn cursor_count(state: &State) -> usize {
    match state.view {
        View::Home => state.visible_feed().len(),
        View::Results => state.results.items().len(),
        View::Subscriptions => match state.source() {
            crate::model::Source::Account => state.account_subscriptions.items().len(),
            crate::model::Source::Device => state.library.subscriptions.len(),
        },
        View::History => match state.source() {
            crate::model::Source::Account => state.account_history.items().len(),
            crate::model::Source::Device => state.library.history.len(),
        },
        View::WatchLater => match state.source() {
            crate::model::Source::Account => state.account_watch_later.items().len(),
            crate::model::Source::Device => state.library.watch_later.len(),
        },
        View::Liked => state.liked.items().len(),
        View::Playlists => state.playlists.items().len(),
        View::Playlist => state.page_videos.items().len(),
        View::Notifications => state.alerts_visit.len(),
        View::Channel => match state.channel_tab {
            ChannelTab::Playlists => state.page_playlists.items().len(),
            ChannelTab::About => 0,
            _ => state.page_videos.items().len(),
        },
        _ => 0,
    }
}

/// What Enter does on the cursor's row.
fn open_at(state: &State, index: usize) -> Option<Action> {
    let video = |videos: &[crate::model::Video]| videos.get(index).cloned().map(Action::Open);
    match state.view {
        View::Home => state
            .visible_feed()
            .get(index)
            .map(|video| Action::Open((*video).clone())),
        View::Results => video(state.results.items()),
        View::Subscriptions => match state.source() {
            crate::model::Source::Account => state
                .account_subscriptions
                .items()
                .get(index)
                .cloned()
                .map(Action::OpenChannel),
            crate::model::Source::Device => state
                .library
                .subscriptions
                .get(index)
                .cloned()
                .map(Action::OpenChannel),
        },
        View::History => match state.source() {
            crate::model::Source::Account => state
                .account_history
                .items()
                .get(index)
                .map(|row| Action::Open(row.video.clone())),
            crate::model::Source::Device => state
                .library
                .history
                .get(index)
                .map(|watched| Action::Open(watched.video.clone())),
        },
        View::WatchLater => match state.source() {
            crate::model::Source::Account => video(state.account_watch_later.items()),
            crate::model::Source::Device => video(&state.library.watch_later),
        },
        View::Liked => video(state.liked.items()),
        View::Playlists => {
            state
                .playlists
                .items()
                .get(index)
                .map(|playlist| Action::OpenPlaylist {
                    id: playlist.id.clone(),
                    name: playlist.name.clone(),
                })
        }
        View::Playlist => video(state.page_videos.items()),
        View::Notifications => video(&state.alerts_visit),
        View::Channel => match state.channel_tab {
            ChannelTab::Playlists => {
                state
                    .page_playlists
                    .items()
                    .get(index)
                    .map(|playlist| Action::OpenPlaylist {
                        id: playlist.id.clone(),
                        name: playlist.name.clone(),
                    })
            }
            ChannelTab::About => None,
            _ => video(state.page_videos.items()),
        },
        _ => None,
    }
}

/// The `?` modal over a dimmed window: every binding in two columns, the
/// key in a pill and what it does beside it.
fn keys_overlay(ctx: &egui::Context, state: &State) {
    let palette = &state.palette;
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("keys-backdrop"),
    ))
    .rect_filled(ctx.content_rect(), 0.0, palette.shadow);
    egui::Area::new(egui::Id::new("keys"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            Frame::new()
                .fill(palette.overlay)
                .stroke(Stroke::new(1.0, palette.outline))
                .corner_radius(CornerRadius::same(RADIUS + 4))
                .inner_margin(Margin::same(24))
                .shadow(ui.visuals().popup_shadow)
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("Keyboard shortcuts")
                            .font(theme::bold(18.0))
                            .color(palette.text),
                    );
                    ui.add_space(16.0);
                    let column = |ui: &mut egui::Ui, id: &str, bindings: &[(&str, &str)]| {
                        egui::Grid::new(id)
                            .num_columns(2)
                            .spacing([12.0, 8.0])
                            .show(ui, |ui| {
                                for (key, what) in bindings {
                                    key_pill(ui, palette, key);
                                    ui.label(
                                        RichText::new(*what)
                                            .font(theme::regular(14.0))
                                            .color(palette.text),
                                    );
                                    ui.end_row();
                                }
                            });
                    };
                    ui.horizontal_top(|ui| {
                        column(
                            ui,
                            "keys-shell",
                            &[
                                ("/", "Focus the search field"),
                                ("Enter", "Search, or open a link"),
                                ("1-9", "The views (not while watching)"),
                                ("?", "This list"),
                                ("Esc", "Close, or go back"),
                                ("r", "Load the page again"),
                                ("j / ↓", "Next row"),
                                ("k / ↑", "The row before"),
                                ("g / G", "First / last row"),
                                ("Enter", "Open the row"),
                                ("h / l", "A channel's tabs"),
                                ("a", "The next account"),
                                ("b", "Collapse the sidebar"),
                            ],
                        );
                        ui.add_space(32.0);
                        column(
                            ui,
                            "keys-player",
                            &[
                                ("Space / k", "Play or pause"),
                                ("← / →", "5 s back or ahead"),
                                ("j / l", "10 s back or ahead"),
                                ("0-9", "Seek to 0-90%"),
                                (", / .", "A frame back or ahead"),
                                ("< / >", "Slower / faster"),
                                ("m", "Mute"),
                                ("c", "Captions"),
                                ("N / P", "Next / previous video"),
                                ("↑ / ↓", "A Short's neighbours"),
                                ("f", "Full screen"),
                                ("t", "Theater mode"),
                                ("Esc", "Leave full screen, back"),
                            ],
                        );
                    });
                });
        });
}

/// A key's name in a small `surface` pill, for the shortcuts' list.
fn key_pill(ui: &mut egui::Ui, palette: &theme::Palette, key: &str) {
    let galley = ui
        .painter()
        .layout_no_wrap(key.to_owned(), theme::medium(13.0), palette.text);
    let height = 24.0;
    let size = vec2((galley.size().x + 16.0).max(height), height);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(
        rect,
        CornerRadius::same((height / 2.0) as u8),
        palette.surface,
    );
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, palette.text);
}

/// The shell's keys, while nobody is typing.
fn keys(
    ctx: &egui::Context,
    state: &mut State,
    status: Option<&Status>,
    actions: &mut Vec<Action>,
) {
    state.cursor_moved = false;
    if ctx.text_edit_focused() {
        return;
    }
    ctx.input(|input| {
        if input.key_pressed(Key::Questionmark) {
            state.keys_open = !state.keys_open;
        }
        if state.keys_open {
            if input.key_pressed(Key::Escape) {
                state.keys_open = false;
            }
            return;
        }
        if input.key_pressed(Key::Slash) {
            ctx.memory_mut(|memory| memory.request_focus(command_line_id()));
        }
        // On the watch page the number keys seek, as on YouTube: 0 is the
        // start, 5 the middle.
        if state.view == View::Watch {
            let digit = [
                Key::Num0,
                Key::Num1,
                Key::Num2,
                Key::Num3,
                Key::Num4,
                Key::Num5,
                Key::Num6,
                Key::Num7,
                Key::Num8,
                Key::Num9,
            ]
            .into_iter()
            .position(|key| input.key_pressed(key));
            if let Some(digit) = digit {
                actions.push(Action::SeekFraction(digit as f64 / 10.0));
            }
        }
        // Elsewhere they jump to the views.
        let digit = [
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ]
        .into_iter()
        .position(|key| input.key_pressed(key))
        .filter(|_| state.view != View::Watch);
        if let Some(digit) = digit {
            let views = [
                View::Home,
                View::Subscriptions,
                View::History,
                View::WatchLater,
                View::Liked,
                View::Playlists,
                View::Notifications,
                View::Account,
                View::Settings,
            ];
            let view = views[digit];
            if matches!(view, View::Liked | View::Playlists) && !state.signed_in() {
                return;
            }
            actions.push(Action::Navigate(view));
        }
        if input.key_pressed(Key::R) {
            actions.push(Action::Refresh);
        }
        if input.key_pressed(Key::B) {
            actions.push(Action::ToggleSidebar);
        }
        // `a` switches to the next account.
        if input.key_pressed(Key::A)
            && state.view != View::Watch
            && state.library.accounts.len() > 1
        {
            let accounts = &state.library.accounts;
            let index = accounts
                .iter()
                .position(|account| Some(account.id) == state.library.active_account)
                .map(|index| index + 1)
                .unwrap_or(0)
                % accounts.len();
            actions.push(Action::SwitchAccount(accounts[index].id));
        }
        // The cursor: j/k or the arrows move it, g/G jump, Enter opens.
        let count = cursor_count(state);
        if count > 0 && state.view != View::Watch {
            if input.key_pressed(Key::J) || input.key_pressed(Key::ArrowDown) {
                state.cursor = (state.cursor + 1).min(count - 1);
                state.cursor_moved = true;
            }
            if input.key_pressed(Key::K) || input.key_pressed(Key::ArrowUp) {
                state.cursor = state.cursor.saturating_sub(1);
                state.cursor_moved = true;
            }
            if input.key_pressed(Key::G) && input.modifiers.shift {
                state.cursor = count - 1;
                state.cursor_moved = true;
            } else if input.key_pressed(Key::G) {
                state.cursor = 0;
                state.cursor_moved = true;
            }
            if input.key_pressed(Key::Enter)
                && let Some(action) = open_at(state, state.cursor)
            {
                actions.push(action);
            }
        }
        if state.view == View::Channel {
            // h/l walk the channel's tabs.
            let visible = channel::tabs(state.channel_header.as_ref(), &state.channel_tab);
            if input.key_pressed(Key::H) {
                actions.push(Action::SetChannelTab(channel::cycle_tab(
                    &visible,
                    &state.channel_tab,
                    -1,
                )));
            }
            if input.key_pressed(Key::L) {
                actions.push(Action::SetChannelTab(channel::cycle_tab(
                    &visible,
                    &state.channel_tab,
                    1,
                )));
            }
        }
        if matches!(state.view, View::Channel | View::Playlist) {
            if input.key_pressed(Key::Escape) {
                actions.push(Action::Back);
            }
            return;
        }
        if state.view != View::Watch {
            return;
        }
        if input.key_pressed(Key::Space) || input.key_pressed(Key::K) {
            actions.push(Action::TogglePause);
        }
        if input.key_pressed(Key::M) {
            actions.push(Action::ToggleMute);
        }
        if input.key_pressed(Key::C) {
            actions.push(Action::ToggleCaptions);
        }
        if input.key_pressed(Key::N) && input.modifiers.shift {
            actions.push(Action::PlayNext);
        }
        if input.key_pressed(Key::P) && input.modifiers.shift {
            actions.push(Action::PlayPrevious);
        }
        // A Short's ↑/↓ go through the shelf, as YouTube's do.
        if state.watching_short() {
            if input.key_pressed(Key::ArrowDown) {
                actions.push(Action::PlayNext);
            }
            if input.key_pressed(Key::ArrowUp) {
                actions.push(Action::PlayPrevious);
            }
        }
        // `<` and `>` step the speed, `,` and `.` a frame: by the text
        // typed, so any keyboard layout works.
        let speed = status.map_or(1.0, |status| status.speed);
        for event in &input.events {
            if let egui::Event::Text(text) = event {
                match text.as_str() {
                    "<" => actions.push(Action::SetSpeed(crate::app::player::step_speed(
                        speed, false,
                    ))),
                    ">" => actions.push(Action::SetSpeed(crate::app::player::step_speed(
                        speed, true,
                    ))),
                    "," => actions.push(Action::FrameStep(false)),
                    "." => actions.push(Action::FrameStep(true)),
                    _ => {}
                }
            }
        }
        if input.key_pressed(Key::ArrowLeft) {
            actions.push(Action::SeekBy(-5.0));
        }
        if input.key_pressed(Key::ArrowRight) {
            actions.push(Action::SeekBy(5.0));
        }
        if input.key_pressed(Key::J) {
            actions.push(Action::SeekBy(-10.0));
        }
        if input.key_pressed(Key::L) {
            actions.push(Action::SeekBy(10.0));
        }
        if input.key_pressed(Key::F) {
            actions.push(Action::ToggleFullscreen);
        }
        if input.key_pressed(Key::T) && !state.fullscreen {
            actions.push(Action::ToggleTheater);
        }
        if input.key_pressed(Key::Escape) {
            if state.player_menu != crate::app::PlayerMenu::Closed {
                state.player_menu = crate::app::PlayerMenu::Closed;
            } else if state.autoplay.is_some() {
                actions.push(Action::CancelAutoplay);
            } else {
                actions.push(if state.fullscreen {
                    Action::ExitFullscreen
                } else {
                    Action::Back
                });
            }
        }
    });
}
