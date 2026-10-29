//! The views. They draw from [`State`] and the player's status and push
//! [`Action`]s; `App::apply` carries them out after the frame. The only
//! state a view changes itself is its own (the search text, the seek bar
//! while it is dragged).
//!
//! The shell is the siblings' (Spotifast, ZapFast): a top bar with the
//! search field, a sidebar of views and subscribed channels, notices as
//! toasts at the bottom right, and `?` for the keyboard shortcuts.

use crate::appearance::i18n::{format as trf, gettext as tr};
mod account;
mod discovery;
mod extras;
mod keys;
pub use player::release_speed_hold;
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
use crate::appearance::icons::Icon;
use crate::appearance::theme::{self, RADIUS};
use crate::model::{Action, ChannelTab, Fetch, Listing, View};
use crate::mpv::{Status, VideoSurface};
use crate::storage::settings::SidebarButton;

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
    state.scrolling.apply(ui.ctx());
    keys::handle(ui.ctx(), state, status, actions);
    if state.view == View::Watch && state.fullscreen {
        watch::fullscreen(ui, state, status, surface, actions);
        if state.keys_open {
            keys_overlay(ui.ctx(), state);
        }
        return;
    }
    top_bar(ui, state, actions);
    if !(state.view == View::Watch && state.settings.theater) {
        sidebar(ui, state, actions);
    }
    ui.push_id(state.navigation_key(), |ui| {
        let page_scroll = std::mem::take(&mut state.page_scroll);
        if page_scroll != 0.0 {
            ui.scroll_with_delta_animation(
                vec2(0.0, page_scroll),
                egui::style::ScrollAnimation::none(),
            );
            ui.ctx().request_discard("key scroll");
        }
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
    });
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
    let locale = state.settings.language;

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
            let wide = bar.width() > 760.0;
            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(bar)
                    .layout(Layout::left_to_right(Align::Center)),
                |ui| {
                    if widgets::icon_button(
                        ui,
                        Icon::PanelLeft,
                        20.0,
                        tr(locale, "Sidebar (b)").as_ref(),
                    )
                    .clicked()
                    {
                        actions.push(Action::ToggleSidebar);
                    }
                    if wide
                        && ui
                            .add(
                                egui::Button::new(
                                    RichText::new("YouTFast").font(theme::bold(18.0)),
                                )
                                .frame(false),
                            )
                            .clicked()
                    {
                        actions.push(Action::Navigate(View::Home));
                    }
                    ui.add_enabled_ui(state.can_go_back, |ui| {
                        if widgets::icon_button(
                            ui,
                            Icon::Back,
                            18.0,
                            tr(locale, "Back (Alt+←)").as_ref(),
                        )
                        .clicked()
                        {
                            actions.push(Action::Back);
                        }
                    });
                    ui.add_enabled_ui(state.can_go_forward, |ui| {
                        if widgets::icon_button(
                            ui,
                            Icon::Forward,
                            18.0,
                            tr(locale, "Forward (Alt+→)").as_ref(),
                        )
                        .clicked()
                        {
                            actions.push(Action::Forward);
                        }
                    });
                },
            );
            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(bar)
                    .layout(Layout::right_to_left(Align::Center)),
                |ui| {
                    if widgets::icon_button(ui, Icon::User, 20.0, tr(locale, "Account").as_ref())
                        .clicked()
                    {
                        actions.push(Action::Navigate(View::Account));
                    }
                    let hint = trf(
                        locale,
                        "Notifications: {count} new",
                        &[("count", state.alerts.len().to_string())],
                    );
                    let bell = widgets::icon_button(ui, Icon::Bell, 20.0, &hint);
                    if !state.alerts.is_empty() {
                        ui.painter().circle_filled(
                            bell.rect.right_top() + vec2(-5.0, 5.0),
                            4.0,
                            palette.accent,
                        );
                    }
                    if bell.clicked() {
                        actions.push(Action::Navigate(View::Notifications));
                    }
                    if widgets::icon_button(
                        ui,
                        Icon::Info,
                        20.0,
                        tr(locale, "Keyboard shortcuts (?)").as_ref(),
                    )
                    .clicked()
                    {
                        state.keys_open = !state.keys_open;
                    }
                },
            );
            let left = bar.left() + if wide { 220.0 } else { 106.0 };
            let right = bar.right() - 116.0;
            let width = (right - left - 36.0).clamp(80.0, 560.0);
            let center = egui::pos2((left + right - 36.0) / 2.0, bar.center().y);
            let field_rect = egui::Rect::from_center_size(center, vec2(width, 36.0));
            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(field_rect)
                    .layout(Layout::left_to_right(Align::Center)),
                |ui| {
                    let field = widgets::search_field(
                        ui,
                        &palette,
                        &mut state.query,
                        tr(locale, "Search or paste a link").as_ref(),
                        locale,
                        command_line_id(),
                        width,
                    );
                    if field.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
                        field.surrender_focus();
                        state.suggestions.clear();
                    }
                    if field.changed() {
                        state.suggestions.clear();
                        state.suggestion_cursor = None;
                    }
                    if field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        if let Some(text) = state
                            .suggestion_cursor
                            .and_then(|at| state.suggestions.get(at))
                        {
                            state.query = text.clone();
                        }
                        actions.push(Action::Search(state.query.clone()));
                    }
                    suggestions(ui, state, &field, field_rect, actions);
                },
            );
            let button = egui::Rect::from_center_size(
                egui::pos2(field_rect.right() + 20.0, bar.center().y),
                vec2(36.0, 36.0),
            );
            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(button)
                    .layout(Layout::left_to_right(Align::Center)),
                |ui| {
                    if widgets::icon_button(
                        ui,
                        Icon::Search,
                        20.0,
                        tr(locale, "Search (Enter)").as_ref(),
                    )
                    .clicked()
                    {
                        actions.push(Action::Search(state.query.clone()));
                    }
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
    // The watch page and narrow windows start folded to the rail; from the
    // toggle's first press the saved choice rules every page (#25).
    let automatic_rail =
        crate::app::sidebar_uses_automatic_rail(state, ui.ctx().content_rect().width());
    let collapsed = state.settings.sidebar_collapsed || (!state.sidebar_toggled && automatic_rail);
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
            for (index, (icon, view, label, count)) in sidebar_items(state).into_iter().enumerate()
            {
                let selected = state.view == view
                    || (view == View::Playlists && state.view == View::Playlist)
                    || (view == View::Subscriptions && state.view == View::Channel);
                if view == View::Account && index > 0 {
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

/// The buttons the sidebar draws, in the order the user chose (the number
/// keys' order too): hidden ones are left out, and Liked videos and
/// Playlists need a signed-in account whatever the setting says. The stored
/// list is already sanitized (on load, by default and by `SetSidebar`).
fn sidebar_buttons(state: &State) -> impl Iterator<Item = SidebarButton> + '_ {
    state
        .settings
        .sidebar
        .iter()
        .filter(|entry| entry.visible)
        .map(|entry| entry.button)
        .filter(|button| match button {
            SidebarButton::Liked | SidebarButton::Playlists => state.signed_in(),
            _ => true,
        })
}

/// The views of [`sidebar_buttons`], for the number keys.
fn sidebar_views(state: &State) -> Vec<View> {
    sidebar_buttons(state)
        .filter_map(|button| match button {
            SidebarButton::Home => Some(View::Home),
            SidebarButton::Subscriptions => Some(View::Subscriptions),
            SidebarButton::History => Some(View::History),
            SidebarButton::WatchLater => Some(View::WatchLater),
            SidebarButton::Liked => Some(View::Liked),
            SidebarButton::Playlists => Some(View::Playlists),
            SidebarButton::Notifications => Some(View::Notifications),
            SidebarButton::Account => Some(View::Account),
            SidebarButton::Settings => Some(View::Settings),
            SidebarButton::Unknown => None,
        })
        .collect()
}

/// The sidebar's views: icon, view, label and a count to show.
fn sidebar_items(state: &State) -> Vec<(Icon, View, String, Option<usize>)> {
    let locale = state.settings.language;
    sidebar_buttons(state)
        .filter_map(|button| {
            let item = |icon, view, label: &'static str, count| {
                Some((icon, view, tr(locale, label).into_owned(), count))
            };
            match button {
                SidebarButton::Home => item(Icon::Home, View::Home, "Home", None),
                SidebarButton::Subscriptions => item(
                    Icon::Subscriptions,
                    View::Subscriptions,
                    "Subscriptions",
                    None,
                ),
                SidebarButton::History => item(Icon::History, View::History, "History", None),
                SidebarButton::WatchLater => {
                    item(Icon::Clock, View::WatchLater, "Watch later", None)
                }
                SidebarButton::Liked => item(Icon::ThumbsUp, View::Liked, "Liked videos", None),
                SidebarButton::Playlists => {
                    item(Icon::Playlists, View::Playlists, "Playlists", None)
                }
                SidebarButton::Notifications => item(
                    Icon::Bell,
                    View::Notifications,
                    "Notifications",
                    Some(state.alerts.len()),
                ),
                SidebarButton::Account => {
                    let name = state
                        .active_account()
                        .map(|account| account.name.clone())
                        .unwrap_or_else(|| match &state.account {
                            AccountState::SigningIn => tr(locale, "Signing in").into_owned(),
                            _ => tr(locale, "Sign in").into_owned(),
                        });
                    Some((Icon::User, View::Account, name, None))
                }
                SidebarButton::Settings => {
                    let update_badge = matches!(
                        state.update,
                        crate::desktop::updater::UpdateState::Available(_)
                    )
                    .then_some(1);
                    item(Icon::Settings, View::Settings, "Settings", update_badge)
                }
                SidebarButton::Unknown => None,
            }
        })
        .collect()
}

/// YouTube's sidebar lists the channels subscribed to under the views:
/// the account's when signed in and loaded, this device's otherwise.
/// `collapsed` draws only their avatars, without the heading.
fn sidebar_channels(ui: &mut egui::Ui, state: &State, collapsed: bool, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

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
                RichText::new(tr(locale, "Subscriptions").as_ref())
                    .font(theme::semibold(14.0))
                    .color(state.palette.secondary),
            );
        });
        ui.add_space(4.0);
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            results::virtual_rows(
                ui,
                if collapsed { 38.0 } else { 36.0 },
                channels.len(),
                None,
                |ui, index| {
                    let channel = &channels[index];
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
                },
            );
        });
}

/// The last notice as a toast at the bottom right: an icon (the alert one
/// for a failure), the text, and a button that dismisses it.
fn notice_toast(ctx: &egui::Context, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

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
                                    RichText::new(
                                        crate::appearance::i18n::message(locale, notice).as_ref(),
                                    )
                                    .font(theme::medium(13.5))
                                    .color(palette.text),
                                )
                                .wrap(),
                            );
                        });
                        if matches!(
                            notice.as_str(),
                            "A new version of YouTFast is available."
                                | "A new major version of YouTFast is available."
                        ) && matches!(
                            state.update,
                            crate::desktop::updater::UpdateState::Available(_)
                        ) {
                            let raised = widgets::on_card(palette);
                            if widgets::button(
                                ui,
                                &raised,
                                tr(locale, "Update now").as_ref(),
                                widgets::ButtonStyle::Primary,
                            )
                            .clicked()
                            {
                                actions.push(Action::InstallUpdate);
                            }
                            if state.view != View::Settings
                                && widgets::button(
                                    ui,
                                    &raised,
                                    tr(locale, "About").as_ref(),
                                    widgets::ButtonStyle::Normal,
                                )
                                .clicked()
                            {
                                actions.push(Action::Navigate(View::Settings));
                            }
                        }
                        if widgets::icon_button(ui, Icon::Close, 16.0, tr(locale, "Close").as_ref())
                            .clicked()
                        {
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
        View::Home if state.feed_filter == crate::model::FeedFilter::ForYou => tag(&state.for_you),
        View::Home => tag(&state.feed) + 10 * state.feed_filter as usize,
        View::Results => match state.search_options.kind {
            crate::youtube::innertube::SearchKind::Videos => tag(&state.results),
            crate::youtube::innertube::SearchKind::Channels => tag(&state.results_channels),
            crate::youtube::innertube::SearchKind::Playlists => tag(&state.results_playlists),
        },
        View::Subscriptions => tag(&state.account_subscriptions) + source,
        View::History => tag(&state.account_history) + 10 * state.history_filter as usize + source,
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
        View::Results => match state.search_options.kind {
            crate::youtube::innertube::SearchKind::Videos => state.results.items().len(),
            crate::youtube::innertube::SearchKind::Channels => state.results_channels.items().len(),
            crate::youtube::innertube::SearchKind::Playlists => {
                state.results_playlists.items().len()
            }
        },
        View::Subscriptions => match state.source() {
            crate::model::Source::Account => state.account_subscriptions.items().len(),
            crate::model::Source::Device => state.library.subscriptions.len(),
        },
        View::History => state.history_rows().len(),
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
        View::Results => match state.search_options.kind {
            crate::youtube::innertube::SearchKind::Videos => video(state.results.items()),
            crate::youtube::innertube::SearchKind::Channels => state
                .results_channels
                .items()
                .get(index)
                .cloned()
                .map(Action::OpenChannel),
            crate::youtube::innertube::SearchKind::Playlists => state
                .results_playlists
                .items()
                .get(index)
                .map(|p| Action::OpenPlaylist {
                    id: p.id.clone(),
                    name: p.name.clone(),
                }),
        },
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
        View::History => state
            .history_rows()
            .get(index)
            .map(|row| Action::Open(row.video.clone())),
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
fn keys_overlay(ctx: &egui::Context, state: &mut State) {
    let locale = state.settings.language;

    let palette = state.palette.clone();
    let mut close = false;
    let response = egui::Modal::new(egui::Id::new("keys")).show(ctx, |ui| {
        let width = (ctx.content_rect().width() - 64.0).clamp(280.0, 840.0);
        ui.set_width(width);
        ui.horizontal(|ui| {
            ui.heading(tr(locale, "Keyboard shortcuts").as_ref());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                close = widgets::icon_button(ui, Icon::Close, 18.0, tr(locale, "Close").as_ref())
                    .clicked();
            });
        });
        ui.add_space(12.0);
        let shell: &[(&'static str, &'static str)] = &[
            ("/", "Focus search"),
            ("Enter", "Search or open link"),
            ("1–9", "Views (outside the video)"),
            ("?", "Show this list"),
            ("Esc", "Close or go back"),
            ("Alt+← / →", "Back / forward"),
            ("Space", "Play/pause in miniplayer"),
            ("r", "Refresh page"),
            ("↑ / ↓", "Scroll page"),
            ("j", "Next item"),
            ("k", "Previous item"),
            ("g / G", "First / last item"),
            ("Enter", "Open item"),
            ("h / l", "Channel tabs"),
            ("a", "Next account"),
            ("b", "Sidebar"),
        ];
        let player: &[(&'static str, &'static str)] = &[
            ("Space / k", "Play or pause"),
            ("← / →", "Back / forward 5 s"),
            ("j / l", "Back / forward 10 s"),
            ("0–9", "Seek to 0–90%"),
            (", / .", "Previous / next frame"),
            ("< / >", "Playback speed"),
            ("m", "Mute"),
            ("c", "Captions"),
            ("Shift+N / P", "Next / previous"),
            ("i", "Miniplayer"),
            ("Home / End", "Start / end"),
            ("Ctrl+← / →", "Chapters (Option on Mac)"),
            ("↑ / ↓", "Volume: 5%; switch Shorts"),
            ("f", "Full screen"),
            ("t", "Theater mode"),
            ("Shift+A", "Audio-only mode"),
        ];
        let column = |ui: &mut egui::Ui, id: &str, bindings: &[(&'static str, &'static str)]| {
            egui::Grid::new(id)
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    for (key, label) in bindings {
                        key_pill(ui, &palette, tr(locale, key).as_ref());
                        ui.add(egui::Label::new(tr(locale, label).as_ref()).wrap());
                        ui.end_row();
                    }
                });
        };
        egui::ScrollArea::vertical()
            .max_height((ctx.content_rect().height() - 110.0).max(120.0))
            .show(ui, |ui| {
                if width > 740.0 {
                    ui.horizontal_top(|ui| {
                        column(ui, "keys-shell", shell);
                        ui.add_space(24.0);
                        column(ui, "keys-player", player);
                    });
                } else {
                    column(ui, "keys-shell", shell);
                    ui.add_space(20.0);
                    column(ui, "keys-player", player);
                }
            });
    });
    if close || response.should_close() {
        state.keys_open = false;
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Video;
    use crate::storage::settings::{SidebarEntry, default_sidebar, sanitize_sidebar};

    fn views(state: &State) -> Vec<View> {
        sidebar_items(state)
            .into_iter()
            .map(|item| item.1)
            .collect()
    }

    #[test]
    fn the_default_sidebar_keeps_its_original_buttons_and_order() {
        let mut state = State::default();
        assert_eq!(
            views(&state),
            [
                View::Home,
                View::Subscriptions,
                View::History,
                View::WatchLater,
                View::Notifications,
                View::Account,
                View::Settings,
            ]
        );
        state.account = crate::app::AccountState::SignedIn(1);
        assert_eq!(views(&state).len(), 9);
        assert_eq!(views(&state)[4..6], [View::Liked, View::Playlists]);
    }

    #[test]
    fn the_sidebar_hides_and_reorders_buttons_and_keeps_signed_in_ones_gated() {
        let mut state = State::default();
        let mut entries = default_sidebar();
        entries.swap(0, 2);
        entries[1].visible = false;
        entries[4].visible = true;
        state.settings.sidebar = sanitize_sidebar(entries);
        assert_eq!(
            views(&state),
            [
                View::History,
                View::Home,
                View::WatchLater,
                View::Notifications,
                View::Account,
                View::Settings,
            ]
        );
        // Shown in the setting, but still only for a signed-in account.
        state.account = crate::app::AccountState::SignedIn(1);
        assert!(views(&state).contains(&View::Liked));
        assert!(!views(&state).contains(&View::Subscriptions));
    }

    #[test]
    fn account_and_settings_stay_in_a_sidebar_saved_with_them_hidden() {
        let mut state = State::default();
        state.settings.sidebar = sanitize_sidebar(
            default_sidebar()
                .into_iter()
                .map(|e| SidebarEntry {
                    visible: false,
                    ..e
                })
                .collect(),
        );
        assert_eq!(views(&state), [View::Account, View::Settings]);
    }

    #[test]
    fn the_badges_follow_their_buttons_when_moved() {
        let mut state = State::default();
        let mut entries = default_sidebar();
        entries.reverse();
        state.settings.sidebar = entries;
        let items = sidebar_items(&state);
        assert_eq!(items[0].1, View::Settings);
        assert_eq!(items[0].3, None);
        let notifications = items.iter().find(|i| i.1 == View::Notifications).unwrap();
        assert_eq!(notifications.3, Some(0));
    }

    /// Draws one frame and reports the left edge of the sidebar's nav label:
    /// `Some` only when the sidebar is open, `None` on the rail.
    fn sidebar_label_x(
        ctx: &egui::Context,
        state: &mut State,
        frame: usize,
        events: Vec<egui::Event>,
        actions: &mut Vec<Action>,
    ) -> Option<f32> {
        sidebar_label_x_at_width(ctx, state, frame, 1512.0, events, actions)
    }

    fn sidebar_label_x_at_width(
        ctx: &egui::Context,
        state: &mut State,
        frame: usize,
        width: f32,
        events: Vec<egui::Event>,
        actions: &mut Vec<Action>,
    ) -> Option<f32> {
        let mut found = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    vec2(width, 906.0),
                )),
                time: Some(frame as f64 / 10.0),
                events,
                ..Default::default()
            },
            |ui| show(ui, state, None, None, actions),
        );
        output.textures_delta.clear();
        for shape in &output.shapes {
            let egui::Shape::Text(text) = &shape.shape else {
                continue;
            };
            if text.galley.text() == "Subscriptions" && text.pos.x < 240.0 {
                found = Some(text.pos.x);
            }
        }
        found
    }

    /// The frame's events for pressing `b`.
    fn toggle_press() -> Vec<egui::Event> {
        vec![
            egui::Event::ModifiersChanged(egui::Modifiers::NONE),
            egui::Event::Key {
                key: egui::Key::B,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }

    #[test]
    fn editing_search_clears_stale_suggestions_before_the_next_response() {
        let ctx = egui::Context::default();
        crate::appearance::theme::install_fonts(&ctx);
        let mut state = State {
            query: "old".into(),
            suggestions: vec!["old suggestion".into()],
            suggestion_cursor: Some(0),
            ..Default::default()
        };
        let mut actions = Vec::new();
        sidebar_label_x(&ctx, &mut state, 0, Vec::new(), &mut actions);
        ctx.memory_mut(|memory| memory.request_focus(command_line_id()));
        sidebar_label_x(
            &ctx,
            &mut state,
            1,
            vec![egui::Event::Text("x".into())],
            &mut actions,
        );
        assert_ne!(state.query, "old");
        assert!(state.suggestions.is_empty());
        assert!(state.suggestion_cursor.is_none());
        assert!(
            !actions
                .iter()
                .any(|action| matches!(action, Action::Search(_)))
        );
    }

    #[test]
    fn the_watch_page_rail_yields_to_the_toggle() {
        let ctx = egui::Context::default();
        crate::appearance::theme::install_fonts(&ctx);
        let mut state = State {
            view: View::Watch,
            watching: Some(Video::from_id("aaaaaaaaaaa".into())),
            settings: crate::storage::settings::Settings {
                sidebar_collapsed: true,
                ..Default::default()
            },
            ..State::default()
        };
        let mut actions = Vec::new();

        // Untouched, the watch page folds the sidebar to its rail.
        let mut label = None;
        for frame in 0..3 {
            label = sidebar_label_x(&ctx, &mut state, frame, Vec::new(), &mut actions);
        }
        assert_eq!(label, None, "the watch page starts on the rail");

        // The toggle's first press there opens it (#25).
        assert!(sidebar_label_x(&ctx, &mut state, 3, toggle_press(), &mut actions).is_none());
        let action = actions.pop().expect("the `b` key emits the toggle action");
        assert!(
            state.settings.sidebar_collapsed,
            "the flag waits for the action to apply"
        );
        assert!(crate::app::apply_sidebar_action(&mut state, action, 1512.0));
        assert!(!state.settings.sidebar_collapsed);
        assert!(state.sidebar_toggled);
        let mut label = None;
        for frame in 4..7 {
            label = sidebar_label_x(&ctx, &mut state, frame, Vec::new(), &mut actions);
        }
        assert!(label.is_some(), "the sidebar opens on the watch page");

        let restart_ctx = egui::Context::default();
        crate::appearance::theme::install_fonts(&restart_ctx);
        let mut restarted = State {
            view: View::Watch,
            settings: state.settings.clone(),
            ..State::default()
        };
        let mut restart_actions = Vec::new();
        assert_eq!(
            sidebar_label_x(
                &restart_ctx,
                &mut restarted,
                0,
                Vec::new(),
                &mut restart_actions
            ),
            None,
            "the automatic Watch rail returns on a new run"
        );

        state.view = View::Home;
        assert!(
            sidebar_label_x_at_width(&ctx, &mut state, 7, 800.0, Vec::new(), &mut actions)
                .is_some(),
            "the open choice overrides Home's narrow-window rail"
        );
        state.view = View::Watch;
        assert!(
            sidebar_label_x(&ctx, &mut state, 8, Vec::new(), &mut actions).is_some(),
            "the open choice persists when returning to watch"
        );

        // The next press folds it back; it applies after the frame, so the
        // visuals are read on the frames that follow. The choice then rules
        // every page.
        sidebar_label_x(&ctx, &mut state, 9, toggle_press(), &mut actions);
        let action = actions
            .pop()
            .expect("the second `b` emits the toggle action");
        assert!(crate::app::apply_sidebar_action(&mut state, action, 1512.0));
        assert!(state.settings.sidebar_collapsed);
        let mut label = Some(0.0);
        for frame in 10..12 {
            label = sidebar_label_x(&ctx, &mut state, frame, Vec::new(), &mut actions);
        }
        assert_eq!(label, None, "the next press folds it back");
        state.view = View::Home;
        assert_eq!(
            sidebar_label_x(&ctx, &mut state, 12, Vec::new(), &mut actions),
            None,
            "the saved fold rules home too"
        );
    }

    #[test]
    fn the_first_toggle_on_wide_home_folds_and_the_next_opens() {
        let ctx = egui::Context::default();
        crate::appearance::theme::install_fonts(&ctx);
        let mut state = State::default();
        let mut actions = Vec::new();

        assert!(
            sidebar_label_x(&ctx, &mut state, 0, Vec::new(), &mut actions).is_some(),
            "wide Home starts open"
        );
        sidebar_label_x(&ctx, &mut state, 1, toggle_press(), &mut actions);
        let action = actions.pop().expect("the `b` key emits the toggle action");
        assert!(crate::app::apply_sidebar_action(&mut state, action, 1512.0));
        assert_eq!(
            sidebar_label_x(&ctx, &mut state, 2, Vec::new(), &mut actions),
            None,
            "the first press folds wide Home"
        );

        sidebar_label_x(&ctx, &mut state, 3, toggle_press(), &mut actions);
        let action = actions
            .pop()
            .expect("the second `b` emits the toggle action");
        assert!(crate::app::apply_sidebar_action(&mut state, action, 1512.0));
        assert!(
            sidebar_label_x(&ctx, &mut state, 4, Vec::new(), &mut actions).is_some(),
            "the second press opens it again"
        );
    }

    #[test]
    fn first_sidebar_toggle_opens_the_automatic_rail_in_a_narrow_window() {
        let ctx = egui::Context::default();
        crate::appearance::theme::install_fonts(&ctx);
        let mut state = State::default();
        let mut actions = Vec::new();

        assert_eq!(
            sidebar_label_x_at_width(&ctx, &mut state, 0, 800.0, Vec::new(), &mut actions),
            None,
            "a narrow window starts on the rail"
        );
        assert!(crate::app::apply_sidebar_action(
            &mut state,
            Action::ToggleSidebar,
            800.0
        ));
        assert!(
            sidebar_label_x_at_width(&ctx, &mut state, 1, 800.0, Vec::new(), &mut actions)
                .is_some(),
            "the first toggle opens the rail even while narrow"
        );
    }

    #[test]
    fn wheel_notches_scroll_at_the_platform_step() {
        let ctx = egui::Context::default();
        crate::appearance::theme::install_fonts(&ctx);
        let mut state = State::default();
        let mut actions = Vec::new();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    vec2(1280.0, 800.0),
                )),
                time: Some(0.0),
                ..Default::default()
            },
            |ui| show(ui, &mut state, None, None, &mut actions),
        );
        output.textures_delta.clear();
        assert_eq!(
            ctx.options(|options| options.input_options.line_scroll_speed),
            fastframe_scroll::WHEEL_STEP,
            "ui::show must apply fastframe_scroll's 120-point wheel step"
        );
    }

    #[test]
    fn arrow_keys_scroll_the_active_page_vertically_without_moving_the_card_cursor() {
        let ctx = egui::Context::default();
        crate::appearance::theme::install_fonts(&ctx);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 600.0));
        let videos: Vec<Video> = (0..40)
            .map(|i| Video {
                title: format!("Video card {i}"),
                ..Video::from_id(format!("{i:011}"))
            })
            .collect();
        let mut state = State {
            view: View::Home,
            for_you: Listing::Ready(videos.clone()),
            feed: Listing::Ready(videos),
            feed_shown: 40,
            ..State::default()
        };
        state.library.subscriptions.push(crate::model::Channel {
            id: "UC123".into(),
            name: "Channel".into(),
            avatar: None,
        });
        let mut actions = Vec::new();
        let card_top = |output: &egui::FullOutput| {
            output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "Video card 0" => Some(text.pos.y),
                _ => None,
            })
        };

        let mut initial = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                time: Some(0.0),
                ..Default::default()
            },
            |ui| show(ui, &mut state, None, None, &mut actions),
        );
        initial.textures_delta.clear();
        let y0 = card_top(&initial).expect("first card rendered on frame 0");

        let mut after_down = initial;
        for (frame, events) in [
            (
                1,
                vec![egui::Event::Key {
                    key: Key::ArrowDown,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            ),
            (2, Vec::new()),
        ] {
            after_down = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    time: Some(frame as f64 / 10.0),
                    events,
                    ..Default::default()
                },
                |ui| show(ui, &mut state, None, None, &mut actions),
            );
            after_down.textures_delta.clear();
        }
        let y_down = card_top(&after_down).expect("first card still visible after scrolling down");
        assert!(
            y_down < y0,
            "ArrowDown must scroll the page vertically down: y0={y0}, y_down={y_down}"
        );
        assert_eq!(state.cursor, 0, "ArrowDown must not move the card cursor");

        let mut after_up = after_down;
        for (frame, events) in [
            (
                3,
                vec![egui::Event::Key {
                    key: Key::ArrowUp,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            ),
            (4, Vec::new()),
        ] {
            after_up = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    time: Some(frame as f64 / 10.0),
                    events,
                    ..Default::default()
                },
                |ui| show(ui, &mut state, None, None, &mut actions),
            );
            after_up.textures_delta.clear();
        }
        let y_up = card_top(&after_up).expect("first card visible after scrolling back up");
        assert!(
            y_up > y_down && (y_up - y0).abs() < 1.0,
            "ArrowUp must scroll the page back up: y0={y0}, y_down={y_down}, y_up={y_up}"
        );
        assert_eq!(state.cursor, 0, "ArrowUp must not move the card cursor");
    }
}
