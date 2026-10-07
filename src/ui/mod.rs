//! The views. They draw from [`State`] and the player's status and push
//! [`Action`]s; `App::apply` carries them out after the frame. The only
//! state a view changes itself is its own (the search text, the seek bar
//! while it is dragged).
//!
//! The shell is a terminal's: a command line on top, a sidebar pane, a
//! status line at the bottom, and `?` for the keys overlay.

mod account;
mod library;
mod results;
mod settings;
mod watch;
mod widgets;

use egui::{Frame, Key, Margin, RichText, TextEdit};

use crate::app::{AccountState, State};
use crate::model::{Action, Listing, View};
use crate::mpv::{Status, VideoSurface};

/// The command line's widget id, so `/` can focus it.
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
    keys(ui.ctx(), state, actions);
    if state.view == View::Watch && state.fullscreen {
        watch::fullscreen(ui, state, status, surface, actions);
        return;
    }
    command_line(ui, state, actions);
    status_line(ui, state, status, actions);
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
        View::Playlist | View::Channel => library::page(ui, state, actions),
        View::Account => account::show(ui, state, actions),
        View::Settings => settings::show(ui, state, actions),
    }
    if state.keys_open {
        keys_overlay(ui.ctx(), state);
    }
}

/// `❯ search, or paste a link` — the borderless field under the prompt.
fn command_line(ui: &mut egui::Ui, state: &mut State, actions: &mut Vec<Action>) {
    let palette = state.palette.clone();
    egui::Panel::top("command-line")
        .resizable(false)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(ui.visuals().panel_fill)
                .inner_margin(Margin::symmetric(8, 6)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("❯")
                        .monospace()
                        .color(palette.accent)
                        .strong(),
                );
                let field = ui.add(
                    TextEdit::singleline(&mut state.query)
                        .id(command_line_id())
                        .hint_text(RichText::new("search, or paste a link").color(palette.dim))
                        .desired_width(f32::INFINITY)
                        .frame(egui::Frame::NONE),
                );
                // The field's underline, accent when it has the focus.
                let underline = if field.has_focus() {
                    palette.accent
                } else {
                    palette.outline
                };
                ui.painter().hline(
                    field.rect.left()..=field.rect.right(),
                    field.rect.bottom() - 1.0,
                    egui::Stroke::new(1.0, underline),
                );
                if field.has_focus() && ui.input(|input| input.key_pressed(Key::Escape)) {
                    field.surrender_focus();
                }
                let submitted =
                    field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
                if submitted {
                    actions.push(Action::Search(state.query.clone()));
                }
                if widgets::bracket_button(ui, &palette, "? keys", widgets::ButtonStyle::Normal)
                    .clicked()
                {
                    state.keys_open = !state.keys_open;
                }
            });
        });
}

/// The sidebar: the `youtfast` pane with `[n] Label` items.
fn sidebar(ui: &mut egui::Ui, state: &State, actions: &mut Vec<Action>) {
    egui::Panel::left("sidebar")
        .exact_size(176.0)
        .resizable(false)
        .show_separator_line(false)
        .frame(Frame::new().inner_margin(Margin::symmetric(8, 6)))
        .show(ui, |ui| {
            widgets::pane(ui, &state.palette, "youtfast", None, false, None, |ui| {
                let item = |ui: &mut egui::Ui,
                            key: char,
                            view: Option<View>,
                            label: &str,
                            selected: bool,
                            actions: &mut Vec<Action>| {
                    let parts = [
                        (format!("[{key}]"), state.palette.magenta),
                        (format!(" {label}"), state.palette.text),
                    ];
                    if widgets::row(ui, &state.palette, selected, &parts).clicked()
                        && let Some(view) = view
                    {
                        actions.push(Action::Navigate(view));
                    }
                };
                let on = |view: View| {
                    state.view == view
                        || (view == View::Playlists && state.view == View::Playlist)
                        || (view == View::Subscriptions && state.view == View::Channel)
                };
                item(ui, '1', Some(View::Home), "Home", on(View::Home), actions);
                item(
                    ui,
                    '2',
                    Some(View::Subscriptions),
                    "Subscriptions",
                    on(View::Subscriptions),
                    actions,
                );
                item(
                    ui,
                    '3',
                    Some(View::History),
                    "History",
                    on(View::History),
                    actions,
                );
                item(
                    ui,
                    '4',
                    Some(View::WatchLater),
                    "Watch later",
                    on(View::WatchLater),
                    actions,
                );
                if state.signed_in() {
                    item(
                        ui,
                        '5',
                        Some(View::Liked),
                        "Liked",
                        on(View::Liked),
                        actions,
                    );
                    item(
                        ui,
                        '6',
                        Some(View::Playlists),
                        "Playlists",
                        on(View::Playlists),
                        actions,
                    );
                }
                ui.add_space(widgets::line(ui));
                let account = match &state.account {
                    AccountState::SignedIn(_) => "Account",
                    AccountState::SigningIn => "Signing in",
                    AccountState::SignedOut | AccountState::Failed(_) => "Sign in",
                };
                item(
                    ui,
                    '8',
                    Some(View::Account),
                    account,
                    on(View::Account),
                    actions,
                );
                item(
                    ui,
                    '9',
                    Some(View::Settings),
                    "Settings",
                    on(View::Settings),
                    actions,
                );
            });
        });
}

/// The bottom line: the view's name inverted, what is loading or the
/// last notice, then the session, quality and theme at the right.
fn status_line(
    ui: &mut egui::Ui,
    state: &mut State,
    status: Option<&Status>,
    actions: &mut Vec<Action>,
) {
    let palette = state.palette.clone();
    egui::Panel::bottom("status-line")
        .resizable(false)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(ui.visuals().panel_fill)
                .inner_margin(Margin::symmetric(8, 4)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // ` HOME ` as an inverted block.
                let name = format!(" {} ", view_name(state.view).to_uppercase());
                ui.label(
                    RichText::new(name)
                        .monospace()
                        .strong()
                        .color(palette.window)
                        .background_color(palette.accent),
                );
                let loading = loading(state, status);
                if let Some(what) = loading {
                    ui.label(
                        RichText::new(format!("{} loading {what}", widgets::spinner_frame(ui)))
                            .monospace()
                            .color(palette.secondary),
                    );
                } else if let Some(notice) = &state.notice {
                    ui.label(RichText::new(notice).monospace().color(palette.text));
                    if widgets::bracket_button(ui, &palette, "x", widgets::ButtonStyle::Normal)
                        .clicked()
                    {
                        actions.push(Action::DismissNotice);
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::bracket_button(ui, &palette, "?", widgets::ButtonStyle::Normal)
                        .clicked()
                    {
                        state.keys_open = !state.keys_open;
                    }
                    let parts = [
                        session_label(state),
                        quality_label(state),
                        theme_label(state),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · ");
                    ui.label(RichText::new(parts).monospace().color(palette.secondary));
                });
            });
        });
}

/// What the status line spins about, if anything.
fn loading<'a>(state: &'a State, status: Option<&'a Status>) -> Option<&'static str> {
    if matches!(state.account, AccountState::SigningIn) {
        return Some("signing in");
    }

    match state.view {
        View::Home if matches!(state.feed, Listing::Loading) => Some("feed"),
        View::Results if matches!(state.results, Listing::Loading) => Some("results"),
        View::Watch if state.watching.is_some() && status.is_none_or(|s| s.idle) => Some("video"),
        View::Subscriptions if matches!(state.account_subscriptions, Listing::Loading) => {
            Some("subscriptions")
        }
        View::History if matches!(state.account_history, Listing::Loading) => Some("history"),
        View::WatchLater if matches!(state.account_watch_later, Listing::Loading) => {
            Some("watch later")
        }
        View::Liked if matches!(state.liked, Listing::Loading) => Some("liked videos"),
        View::Playlists if matches!(state.playlists, Listing::Loading) => Some("playlists"),
        View::Playlist | View::Channel if matches!(state.page_videos, Listing::Loading) => {
            Some("videos")
        }
        _ => None,
    }
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
        View::Home => tag(&state.feed),
        View::Results => tag(&state.results),
        View::Subscriptions => tag(&state.account_subscriptions) + source,
        View::History => tag(&state.account_history) + source,
        View::WatchLater => tag(&state.account_watch_later) + source,
        View::Liked => tag(&state.liked),
        View::Playlists => tag(&state.playlists),
        View::Playlist | View::Channel => tag(&state.page_videos),
        _ => 0,
    }
}

/// How many rows the current view's list has (0 = no cursor there).
fn cursor_count(state: &State) -> usize {
    match state.view {
        View::Home => state.feed.items().len(),
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
        View::Playlist | View::Channel => state.page_videos.items().len(),
        _ => 0,
    }
}

/// What Enter does on the cursor's row.
fn open_at(state: &State, index: usize) -> Option<Action> {
    let video = |videos: &[crate::model::Video]| videos.get(index).cloned().map(Action::Open);
    match state.view {
        View::Home => video(state.feed.items()),
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
        View::Playlist | View::Channel => video(state.page_videos.items()),
        _ => None,
    }
}

/// A name for the status line's inverted block.
fn view_name(view: View) -> &'static str {
    match view {
        View::Home => "home",
        View::Results => "results",
        View::Watch => "watch",
        View::Subscriptions => "subscriptions",
        View::History => "history",
        View::WatchLater => "watch later",
        View::Liked => "liked",
        View::Playlists => "playlists",
        View::Playlist => "playlist",
        View::Channel => "channel",
        View::Account => "account",
        View::Settings => "settings",
    }
}

/// "signed in", or the local library's mode.
fn session_label(state: &State) -> Option<String> {
    match &state.account {
        AccountState::SignedIn(_) => Some("signed in".to_owned()),
        AccountState::SigningIn => None,
        AccountState::SignedOut | AccountState::Failed(_) => Some("local".to_owned()),
    }
}

/// The chosen quality: "auto" or "1080p".
fn quality_label(state: &State) -> Option<String> {
    Some(match state.settings.quality {
        crate::model::Quality::Auto => "auto".to_owned(),
        crate::model::Quality::Max(lines) => format!("{lines}p"),
    })
}

/// The theme's display name: the picked palette's, "auto", dark, light.
fn theme_label(state: &State) -> Option<String> {
    Some(match &state.settings.theme {
        crate::settings::ThemeChoice::Auto => {
            if state.follows_omarchy {
                state
                    .themes
                    .iter()
                    .find(|(filename, _)| filename == fastframe_theme::omarchy::FILENAME)
                    .map(|(_, name)| name.clone())
                    .unwrap_or_else(|| "omarchy".to_owned())
            } else {
                "auto".to_owned()
            }
        }
        crate::settings::ThemeChoice::Dark => "dark".to_owned(),
        crate::settings::ThemeChoice::Light => "light".to_owned(),
        crate::settings::ThemeChoice::Custom(filename) => state
            .themes
            .iter()
            .find(|(file, _)| file == filename)
            .map(|(_, name)| name.clone())
            .unwrap_or_else(|| filename.clone()),
    })
}

/// The `?` overlay: every binding in two columns.
fn keys_overlay(ctx: &egui::Context, state: &State) {
    egui::Area::new(egui::Id::new("keys"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            // The overlay needs a body: panes are transparent otherwise.
            let fill = if state.palette.overlay == state.palette.window {
                state.palette.panel
            } else {
                state.palette.overlay
            };
            widgets::pane(ui, &state.palette, "keys", None, true, Some(fill), |ui| {
                let p = &state.palette;
                let column = |ui: &mut egui::Ui, bindings: &[(&str, &str)]| {
                    for (key, what) in bindings {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{key:<7}"))
                                    .monospace()
                                    .color(p.magenta),
                            );
                            ui.label(RichText::new(*what).monospace().color(p.text));
                        });
                    }
                };
                ui.columns(2, |columns| {
                    column(
                        &mut columns[0],
                        &[
                            ("/", "the command line"),
                            ("enter", "search, or open a link"),
                            ("1-9", "the views"),
                            ("?", "this list"),
                            ("esc", "close, or go back"),
                            ("r", "load the page again"),
                            ("j / ↓", "next row"),
                            ("k / ↑", "the row before"),
                            ("g / G", "first / last row"),
                            ("enter", "open the row"),
                        ],
                    );
                    column(
                        &mut columns[1],
                        &[
                            ("space / k", "play or pause"),
                            ("← / →", "5 s back or ahead"),
                            ("j / l", "10 s back or ahead"),
                            ("f", "full screen"),
                            ("esc", "leave full screen, back"),
                        ],
                    );
                });
            });
        });
}

/// The shell's keys, while nobody is typing.
fn keys(ctx: &egui::Context, state: &mut State, actions: &mut Vec<Action>) {
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
        // The number keys jump to the views.
        let digit = [
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num8,
            Key::Num9,
        ]
        .into_iter()
        .position(|key| input.key_pressed(key));
        if let Some(digit) = digit {
            let views = [
                View::Home,
                View::Subscriptions,
                View::History,
                View::WatchLater,
                View::Liked,
                View::Playlists,
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
        // The cursor: j/k or the arrows move it, g/G jump, Enter opens.
        let count = cursor_count(state);
        if count > 0 && state.view != View::Watch {
            if input.key_pressed(Key::J) || input.key_pressed(Key::ArrowDown) {
                state.cursor = (state.cursor + 1).min(count - 1);
            }
            if input.key_pressed(Key::K) || input.key_pressed(Key::ArrowUp) {
                state.cursor = state.cursor.saturating_sub(1);
            }
            if input.key_pressed(Key::G) && input.modifiers.shift {
                state.cursor = count - 1;
            } else if input.key_pressed(Key::G) {
                state.cursor = 0;
            }
            if input.key_pressed(Key::Enter)
                && let Some(action) = open_at(state, state.cursor)
            {
                actions.push(action);
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
        if input.key_pressed(Key::Escape) {
            actions.push(if state.fullscreen {
                Action::ExitFullscreen
            } else {
                Action::Back
            });
        }
    });
}
