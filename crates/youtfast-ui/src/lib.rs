//! The views. They draw from [`State`] and the player's status and push
//! [`Action`]s; `App::apply` carries them out after the frame. The only
//! state a view changes itself is its own (the search text, the seek bar
//! while it is dragged).
//!
//! The shell is the siblings' (Spotifast, ZapFast): a top bar with the
//! search field, a sidebar of views and subscribed channels, notices as
//! toasts at the bottom right, and `?` for the keyboard shortcuts.

use std::borrow::Cow;

use youtfast_i18n::{format as trf, gettext as tr};
mod account;
#[cfg(test)]
mod bench;
mod cast;
mod discovery;
mod extras;
mod keys;
pub use player::release_speed_hold;
mod channel;
mod library;
mod notifications;
mod player;
mod results;
mod settings;
mod video_tabs;
mod watch;
mod widgets;

use egui::{
    Align, CornerRadius, Frame, Key, Layout, Margin, RichText, Sense, Stroke, UiBuilder, vec2,
};

use youtfast_app::Action;
use youtfast_app::{AccountState, State};
use youtfast_appearance::icons::Icon;
use youtfast_appearance::theme::{self, RADIUS};
use youtfast_core::model::{ChannelTab, Fetch, View};
use youtfast_mpv::{Status, VideoSurface};
use youtfast_storage::settings::SidebarButton;

/// The search field's widget id, so `/` can focus it.
fn command_line_id() -> egui::Id {
    egui::Id::new("command-line")
}

/// One frame of the views: everything drawn, then a held speed-up released
/// (the [`youtfast_app::Views`] the application calls).
pub fn frame(
    ui: &mut egui::Ui,
    state: &mut State,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
    actions: &mut Vec<Action>,
) {
    show(ui, state, status, surface, actions);
    release_speed_hold(ui.ctx(), state, actions);
}

pub fn show(
    ui: &mut egui::Ui,
    state: &mut State,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
    actions: &mut Vec<Action>,
) {
    state.scrolling.apply(ui.ctx());
    if !state.cast.open {
        keys::handle(ui.ctx(), state, status, actions);
    }
    if state.view == View::Watch && state.fullscreen {
        watch::fullscreen(ui, state, status, surface, actions);
        if state.keys_open {
            keys_overlay(ui.ctx(), state);
        }
        cast::show(ui.ctx(), state, actions);
        return;
    }
    top_bar(ui, state, actions);
    video_tabs::show(ui, state, actions);
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
                if let Some(youtfast_app::Page::Channel(channel)) = &state.page {
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
    cast::show(ui.ctx(), state, actions);
}

/// The top bar's height.
const TOP_BAR: f32 = 56.0;

/// The top bar: back, forward and refresh, the search field centred (Enter
/// searches or opens a link, Esc leaves it), and the keyboard shortcuts'
/// button at the right.
fn top_bar(ui: &mut egui::Ui, state: &mut State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;

    let palette = state.palette;
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
                    if widgets::icon_button(
                        ui,
                        Icon::Refresh,
                        18.0,
                        tr(locale, "Refresh page").as_ref(),
                    )
                    .clicked()
                    {
                        actions.push(Action::Refresh);
                    }
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
                    let cast_button = widgets::icon_button(
                        ui,
                        Icon::Cast,
                        20.0,
                        tr(locale, "Cast to TV").as_ref(),
                    );
                    if state.cast.session.is_some() {
                        ui.painter().circle_filled(
                            cast_button.rect.right_top() + vec2(-5.0, 5.0),
                            3.0,
                            palette.accent,
                        );
                    }
                    if cast_button.clicked() {
                        actions.push(Action::Cast(youtfast_cast::Action::Open));
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
                    let mut open = state.notifications_popup_open;
                    if bell.clicked() {
                        open = !open;
                        if open {
                            actions.push(Action::VisitNotifications);
                        } else {
                            egui::Popup::close_all(ui.ctx());
                        }
                    }
                    let nested_open = egui::Popup::is_any_open(ui.ctx());
                    let popup = egui::Popup::menu(&bell)
                        .open_bool(&mut open)
                        .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
                        .show(|ui| notifications::popup(ui, state, actions));
                    if let Some(popup) = popup
                        && !nested_open
                        && !egui::Popup::is_any_open(ui.ctx())
                        && popup.response.clicked_elsewhere()
                        && !bell.clicked()
                    {
                        open = false;
                    }
                    state.notifications_popup_open = open;
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
            // Room for the sidebar, the title when wide, and the three
            // navigation buttons.
            let left = bar.left() + if wide { 268.0 } else { 160.0 };
            let right = bar.right() - 156.0;
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
    let palette = state.palette;
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
        youtfast_app::sidebar_uses_automatic_rail(state, ui.ctx().content_rect().width());
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
fn sidebar_items(state: &State) -> Vec<(Icon, View, Cow<'_, str>, Option<usize>)> {
    let locale = state.settings.language;
    sidebar_buttons(state)
        .filter_map(|button| {
            let item = |icon, view, label: &'static str, count| {
                Some((icon, view, tr(locale, label), count))
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
                    let name = state.active_account().map_or_else(
                        || match &state.account {
                            AccountState::SigningIn => tr(locale, "Signing in"),
                            _ => tr(locale, "Sign in"),
                        },
                        |account| Cow::Borrowed(account.name.as_str()),
                    );
                    Some((Icon::User, View::Account, name, None))
                }
                SidebarButton::Settings => {
                    let update_badge = matches!(
                        state.update,
                        youtfast_desktop::updater::UpdateState::Available(_)
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
        Some(youtfast_app::Page::Channel(channel)) if state.view == View::Channel => {
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
                                    RichText::new(youtfast_i18n::message(locale, notice).as_ref())
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
                            youtfast_desktop::updater::UpdateState::Available(_)
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

/// How many rows the current view's list has (0 = no cursor there).
fn cursor_count(state: &State) -> usize {
    match state.view {
        View::Home => state.visible_feed().len(),
        View::Results => match state.search_options.kind {
            youtfast_youtube::innertube::SearchKind::Videos => state.results.items().len(),
            youtfast_youtube::innertube::SearchKind::Channels => {
                state.results_channels.items().len()
            }
            youtfast_youtube::innertube::SearchKind::Playlists => {
                state.results_playlists.items().len()
            }
        },
        View::Subscriptions => match state.source() {
            youtfast_core::model::Source::Account => state.account_subscriptions.items().len(),
            youtfast_core::model::Source::Device => state.library.subscriptions.len(),
        },
        View::History => state.history_rows().len(),
        View::WatchLater => match state.source() {
            youtfast_core::model::Source::Account => state.account_watch_later.items().len(),
            youtfast_core::model::Source::Device => state.library.watch_later.len(),
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
    let video =
        |videos: &[youtfast_core::model::Video]| videos.get(index).cloned().map(Action::Open);
    match state.view {
        View::Home => state
            .visible_feed()
            .get(index)
            .map(|video| Action::Open((*video).clone())),
        View::Results => match state.search_options.kind {
            youtfast_youtube::innertube::SearchKind::Videos => video(state.results.items()),
            youtfast_youtube::innertube::SearchKind::Channels => state
                .results_channels
                .items()
                .get(index)
                .cloned()
                .map(Action::OpenChannel),
            youtfast_youtube::innertube::SearchKind::Playlists => state
                .results_playlists
                .items()
                .get(index)
                .map(|p| Action::OpenPlaylist {
                    id: p.id.clone(),
                    name: p.name.clone(),
                }),
        },
        View::Subscriptions => match state.source() {
            youtfast_core::model::Source::Account => state
                .account_subscriptions
                .items()
                .get(index)
                .cloned()
                .map(Action::OpenChannel),
            youtfast_core::model::Source::Device => state
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
            youtfast_core::model::Source::Account => video(state.account_watch_later.items()),
            youtfast_core::model::Source::Device => video(&state.library.watch_later),
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

    let palette = state.palette;
    let mut close = false;
    let id = egui::Id::new("keys");
    // The area's first sizing pass otherwise caps the modal at egui's
    // default height, so the list below would scroll despite the room.
    let height = (ctx.content_rect().height() - 96.0).clamp(240.0, 680.0);
    let margin_y = 16;
    let response = egui::Modal::new(id)
        .area(egui::Modal::default_area(id).default_height(height))
        .frame(
            egui::Frame::popup(&ctx.style_of(ctx.theme()))
                .inner_margin(egui::Margin::symmetric(20, margin_y)),
        )
        .show(ctx, |ui| {
            let shell: &[(&'static str, &'static str)] = &[
                ("/", "Focus search"),
                ("Enter", "Search or open link"),
                ("1–9", "Views (outside the video)"),
                ("?", "Show this list"),
                ("Esc", "Close or go back"),
                ("Alt+← / →", "Back / forward"),
                ("Ctrl+Tab / Ctrl+Shift+Tab", "Next / previous video tab"),
                ("Ctrl/Cmd+W", "Close tab"),
                ("Space", "Play/pause in miniplayer"),
                ("Ctrl/Cmd+R", "Refresh page"),
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
            let measured = |bindings: &[(&'static str, &'static str)]| {
                let key = bindings
                    .iter()
                    .map(|(key, _)| {
                        ui.painter()
                            .layout_no_wrap(
                                tr(locale, key).into_owned(),
                                theme::medium(13.0),
                                palette.text,
                            )
                            .size()
                            .x
                            + 16.0
                    })
                    .fold(24.0, f32::max);
                let body = egui::TextStyle::Body.resolve(ui.style());
                let label = bindings
                    .iter()
                    .map(|(_, label)| {
                        ui.painter()
                            .layout_no_wrap(
                                tr(locale, label).into_owned(),
                                body.clone(),
                                palette.text,
                            )
                            .size()
                            .x
                    })
                    .fold(0.0, f32::max);
                key + 12.0 + label
            };
            let shell_width = measured(shell);
            let player_width = measured(player);
            let gap = 48.0;
            let natural = shell_width + gap + player_width;
            let available = ctx.content_rect().width() - 64.0;
            let two_columns = natural <= available;
            let width = if two_columns {
                // Room for the floating vertical scrollbar on shorter windows.
                natural + 24.0
            } else {
                available.clamp(280.0, 1040.0)
            };
            ui.set_width(width);
            // `default_height` seeds only the first sizing pass, so pin the
            // height every frame or a smaller window keeps the old size.
            ui.set_height(height - 2.0 * f32::from(margin_y));
            ui.horizontal(|ui| {
                ui.heading(tr(locale, "Keyboard shortcuts").as_ref());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    close =
                        widgets::icon_button(ui, Icon::Close, 18.0, tr(locale, "Close").as_ref())
                            .clicked();
                });
            });
            ui.add_space(12.0);
            let column = |ui: &mut egui::Ui,
                          id: &str,
                          title: &'static str,
                          bindings: &[(&'static str, &'static str)]| {
                ui.label(
                    RichText::new(tr(locale, title))
                        .font(theme::medium(13.0))
                        .color(palette.secondary),
                );
                ui.add_space(6.0);
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
                // The floating scrollbar sits on this margin instead of
                // against the longest description.
                .content_margin(egui::Margin {
                    right: 24,
                    ..egui::Margin::ZERO
                })
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if two_columns {
                        ui.horizontal_top(|ui| {
                            ui.allocate_ui_with_layout(
                                vec2(shell_width, 0.0),
                                Layout::top_down(Align::Min),
                                |ui| column(ui, "keys-shell", "General", shell),
                            );
                            ui.add_space((gap - ui.spacing().item_spacing.x).max(0.0));
                            ui.allocate_ui_with_layout(
                                vec2(player_width, 0.0),
                                Layout::top_down(Align::Min),
                                |ui| column(ui, "keys-player", "Player", player),
                            );
                        });
                    } else if width > 740.0 {
                        ui.columns(2, |columns| {
                            column(&mut columns[0], "keys-shell", "General", shell);
                            column(&mut columns[1], "keys-player", "Player", player);
                        });
                    } else {
                        column(ui, "keys-shell", "General", shell);
                        ui.add_space(20.0);
                        column(ui, "keys-player", "Player", player);
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
    use youtfast_core::model::{Listing, Video};
    use youtfast_storage::settings::{SidebarEntry, default_sidebar, sanitize_sidebar};

    fn screen(width: f32, height: f32) -> egui::Rect {
        egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(width, height))
    }

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
        state.account = youtfast_app::AccountState::SignedIn(1);
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
        state.account = youtfast_app::AccountState::SignedIn(1);
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

    #[test]
    fn the_shortcuts_modal_fits_its_columns_and_keeps_the_padding() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 800.0));
        let mut state = State {
            keys_open: true,
            ..State::default()
        };
        let mut output = egui::FullOutput::default();
        for _ in 0..3 {
            output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    time: Some(0.0),
                    ..Default::default()
                },
                |_| keys_overlay(&ctx, &mut state),
            );
            output.textures_delta.clear();
        }
        let text_x = |wanted: &str| {
            output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == wanted => Some(text.pos.x),
                _ => None,
            })
        };
        let general = text_x("General").expect("the general column is named");
        let player = text_x("Player").expect("the player column is named");
        let rect = ctx
            .memory(|memory| memory.area_rect(egui::Id::new("keys")))
            .expect("the modal has an area");
        let right = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.pos.y > rect.top() + 60.0 => {
                    Some(text.pos.x + text.galley.size().x)
                }
                _ => None,
            })
            .fold(0.0, f32::max);
        assert!(
            (general - rect.left() - 20.0).abs() < 6.0,
            "the list keeps the modal's left padding"
        );
        assert!(
            player > general + 300.0,
            "the player column is beside the first list instead of below it"
        );
        assert!(
            rect.width() < 900.0,
            "the modal shrinks to its content instead of leaving a gap"
        );
        assert!(
            (40.0..55.0).contains(&(rect.right() - right)),
            "the labels stop at the scrollbar gutter instead of the modal edge"
        );
        assert!(
            rect.height() > 500.0,
            "the modal is tall enough to show the list without scrolling"
        );
        assert!(
            screen.contains_rect(rect),
            "the modal stays inside the window"
        );
        assert!(
            text_x("Sidebar").is_some(),
            "the last General row is visible instead of scrolled away"
        );
        assert!(
            text_x("Audio-only mode").is_some(),
            "the last Player row is visible instead of scrolled away"
        );
    }

    #[test]
    fn the_shortcuts_modal_follows_a_resized_window() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State {
            keys_open: true,
            ..State::default()
        };
        let mut run = |screen: egui::Rect, open: bool| {
            for _ in 0..3 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(screen),
                        time: Some(0.0),
                        ..Default::default()
                    },
                    |_| {
                        if open {
                            keys_overlay(&ctx, &mut state);
                        }
                    },
                );
                output.textures_delta.clear();
            }
            ctx.memory(|memory| memory.area_rect(egui::Id::new("keys")))
                .expect("the modal has an area")
        };

        let tall = run(screen(1280.0, 800.0), true);
        let short = run(screen(1280.0, 500.0), true);
        assert!(
            short.height() < tall.height(),
            "the modal shrinks with the window instead of keeping its first size"
        );
        assert!(
            screen(1280.0, 500.0).contains_rect(short),
            "the shrunk modal stays inside the window"
        );

        let grown = run(screen(800.0, 800.0), true);
        assert!(
            grown.height() > short.height(),
            "the modal grows again after the window does"
        );
        assert!(
            screen(800.0, 800.0).contains_rect(grown),
            "the grown modal stays inside the window"
        );

        let stacked = run(screen(600.0, 600.0), true);
        assert!(
            stacked.width() < 600.0,
            "a narrow window gets the stacked layout instead of overflowing"
        );
        assert!(
            screen(600.0, 600.0).contains_rect(stacked),
            "the stacked modal stays inside the window"
        );

        let _ = run(screen(1280.0, 800.0), false);
        let reopened = run(screen(1280.0, 500.0), true);
        assert!(
            reopened.height() < tall.height(),
            "reopening on a short window does not restore the old tall size"
        );
        assert!(
            screen(1280.0, 500.0).contains_rect(reopened),
            "the reopened modal stays inside the short window"
        );
    }

    fn top_bar_frame(
        ctx: &egui::Context,
        state: &mut State,
        events: Vec<egui::Event>,
        actions: &mut Vec<Action>,
    ) -> egui::FullOutput {
        top_bar_frame_at(ctx, state, events, actions, 1280.0)
    }

    fn top_bar_frame_at(
        ctx: &egui::Context,
        state: &mut State,
        events: Vec<egui::Event>,
        actions: &mut Vec<Action>,
        width: f32,
    ) -> egui::FullOutput {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    vec2(width, 800.0),
                )),
                events,
                ..Default::default()
            },
            |ui| top_bar(ui, state, actions),
        );
        output.textures_delta.clear();
        output
    }

    fn pointer_click(
        ctx: &egui::Context,
        state: &mut State,
        pos: egui::Pos2,
        button: egui::PointerButton,
        actions: &mut Vec<Action>,
    ) {
        pointer_click_at(ctx, state, pos, button, actions, 1280.0);
    }

    fn pointer_click_at(
        ctx: &egui::Context,
        state: &mut State,
        pos: egui::Pos2,
        button: egui::PointerButton,
        actions: &mut Vec<Action>,
        width: f32,
    ) {
        for pressed in [true, false] {
            let _ = top_bar_frame_at(
                ctx,
                state,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                actions,
                width,
            );
        }
    }

    #[test]
    fn refresh_button_asks_the_page_to_reload() {
        // 800 is just above the wide layout's threshold, where the search
        // field is narrowest and closest to the navigation buttons.
        for width in [1280.0, 800.0] {
            let ctx = egui::Context::default();
            youtfast_appearance::theme::install_fonts(&ctx);
            let mut state = State::default();
            let mut actions = Vec::new();
            let _ = top_bar_frame_at(&ctx, &mut state, Vec::new(), &mut actions, width);
            pointer_click_at(
                &ctx,
                &mut state,
                egui::pos2(242.0, 28.0),
                egui::PointerButton::Primary,
                &mut actions,
                width,
            );
            assert_eq!(
                actions
                    .iter()
                    .filter(|action| matches!(action, Action::Refresh))
                    .count(),
                1,
                "the refresh button should ask for one reload at {width}px"
            );
        }
    }

    #[test]
    fn refresh_button_stays_clickable_in_a_narrow_window() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State::default();
        let mut actions = Vec::new();
        let _ = top_bar_frame_at(&ctx, &mut state, Vec::new(), &mut actions, 760.0);
        pointer_click_at(
            &ctx,
            &mut state,
            egui::pos2(164.0, 28.0),
            egui::PointerButton::Primary,
            &mut actions,
            760.0,
        );
        assert_eq!(
            actions
                .iter()
                .filter(|action| matches!(action, Action::Refresh))
                .count(),
            1,
            "the search field must not cover the refresh button"
        );
    }

    #[test]
    fn bell_opens_notifications_without_navigating_and_closes_on_second_click() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State::default();
        state.alerts.push(Video::from_id("test-video".into()));
        let bell = egui::pos2(1168.0, 28.0);
        let mut actions = Vec::new();
        let _ = top_bar_frame(&ctx, &mut state, Vec::new(), &mut actions);
        pointer_click(
            &ctx,
            &mut state,
            bell,
            egui::PointerButton::Primary,
            &mut actions,
        );
        assert!(state.notifications_popup_open, "bell should open the popup");
        pointer_click(
            &ctx,
            &mut state,
            bell,
            egui::PointerButton::Primary,
            &mut actions,
        );
        assert!(
            !state.notifications_popup_open,
            "second click should close it"
        );
        assert_eq!(state.view, View::Home, "opening the bell must not navigate");
        assert_eq!(
            actions
                .iter()
                .filter(|action| matches!(action, Action::VisitNotifications))
                .count(),
            1,
            "only opening marks alerts visited"
        );
        assert!(
            !actions
                .iter()
                .any(|action| matches!(action, Action::Navigate(_)))
        );
    }

    #[test]
    fn notifications_popup_closes_on_outside_click() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State {
            notifications_popup_open: true,
            ..State::default()
        };
        let mut actions = Vec::new();
        let _ = top_bar_frame(&ctx, &mut state, Vec::new(), &mut actions);
        let _ = top_bar_frame(&ctx, &mut state, Vec::new(), &mut actions);
        pointer_click(
            &ctx,
            &mut state,
            egui::pos2(300.0, 500.0),
            egui::PointerButton::Primary,
            &mut actions,
        );
        assert!(!state.notifications_popup_open);
    }

    #[test]
    fn notification_context_menu_remains_usable() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut video = Video::from_id("test-video".into());
        video.title = "A notification video".into();
        let mut state = State {
            notifications_popup_open: true,
            alerts_visit: vec![video],
            ..State::default()
        };
        let mut actions = Vec::new();
        let _ = top_bar_frame(&ctx, &mut state, Vec::new(), &mut actions);
        let output = top_bar_frame(&ctx, &mut state, Vec::new(), &mut actions);
        let row = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "A notification video" => {
                    Some(text.pos + vec2(3.0, 3.0))
                }
                _ => None,
            })
            .expect("notification row is visible");
        pointer_click(
            &ctx,
            &mut state,
            row,
            egui::PointerButton::Secondary,
            &mut actions,
        );
        let output = top_bar_frame(&ctx, &mut state, Vec::new(), &mut actions);
        assert!(
            state.notifications_popup_open,
            "context menu must not close the notification list"
        );
        let play_next = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "Play next" => {
                    Some(text.pos + vec2(3.0, 3.0))
                }
                _ => None,
            })
            .expect("context menu remains visible on the next frame");
        pointer_click(
            &ctx,
            &mut state,
            play_next,
            egui::PointerButton::Primary,
            &mut actions,
        );
        assert!(
            actions
                .iter()
                .any(|action| matches!(action, Action::Enqueue { next: true, .. }))
        );
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
        youtfast_appearance::theme::install_fonts(&ctx);
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
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State {
            view: View::Watch,
            watching: Some(Video::from_id("aaaaaaaaaaa".into())),
            settings: youtfast_storage::settings::Settings {
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
        assert!(youtfast_app::apply_sidebar_action(
            &mut state, action, 1512.0
        ));
        assert!(!state.settings.sidebar_collapsed);
        assert!(state.sidebar_toggled);
        let mut label = None;
        for frame in 4..7 {
            label = sidebar_label_x(&ctx, &mut state, frame, Vec::new(), &mut actions);
        }
        assert!(label.is_some(), "the sidebar opens on the watch page");

        let restart_ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&restart_ctx);
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
        assert!(youtfast_app::apply_sidebar_action(
            &mut state, action, 1512.0
        ));
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
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State::default();
        let mut actions = Vec::new();

        assert!(
            sidebar_label_x(&ctx, &mut state, 0, Vec::new(), &mut actions).is_some(),
            "wide Home starts open"
        );
        sidebar_label_x(&ctx, &mut state, 1, toggle_press(), &mut actions);
        let action = actions.pop().expect("the `b` key emits the toggle action");
        assert!(youtfast_app::apply_sidebar_action(
            &mut state, action, 1512.0
        ));
        assert_eq!(
            sidebar_label_x(&ctx, &mut state, 2, Vec::new(), &mut actions),
            None,
            "the first press folds wide Home"
        );

        sidebar_label_x(&ctx, &mut state, 3, toggle_press(), &mut actions);
        let action = actions
            .pop()
            .expect("the second `b` emits the toggle action");
        assert!(youtfast_app::apply_sidebar_action(
            &mut state, action, 1512.0
        ));
        assert!(
            sidebar_label_x(&ctx, &mut state, 4, Vec::new(), &mut actions).is_some(),
            "the second press opens it again"
        );
    }

    #[test]
    fn first_sidebar_toggle_opens_the_automatic_rail_in_a_narrow_window() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State::default();
        let mut actions = Vec::new();

        assert_eq!(
            sidebar_label_x_at_width(&ctx, &mut state, 0, 800.0, Vec::new(), &mut actions),
            None,
            "a narrow window starts on the rail"
        );
        assert!(youtfast_app::apply_sidebar_action(
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
        youtfast_appearance::theme::install_fonts(&ctx);
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
        youtfast_appearance::theme::install_fonts(&ctx);
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
        state
            .library
            .subscriptions
            .push(youtfast_core::model::Channel {
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
