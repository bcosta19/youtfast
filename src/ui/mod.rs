//! The views. They draw from [`State`] and the player's status and push
//! [`Action`]s; `App::apply` carries them out after the frame. The only
//! state a view changes itself is its own (the search text, the seek bar
//! while it is dragged).

mod account;
mod library;
mod results;
mod watch;
mod widgets;

use egui::{
    Button, Color32, CornerRadius, Frame, Key, Margin, RichText, Sense, Shape, Stroke, TextEdit,
    vec2,
};

use crate::app::{AccountState, State};
use crate::icons::Icon;
use crate::model::{Action, View};
use crate::mpv::{Status, VideoSurface};

pub const ACCENT: Color32 = Color32::from_rgb(255, 0, 51);

pub fn show(
    ui: &mut egui::Ui,
    state: &mut State,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
    actions: &mut Vec<Action>,
) {
    shortcuts(ui.ctx(), state, actions);
    if state.view == View::Watch && state.fullscreen {
        watch::fullscreen(ui, state, status, surface, actions);
        return;
    }
    top_bar(ui, state, actions);
    if let Some(notice) = &state.notice {
        egui::Panel::bottom("notice").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(notice);
                if ui.button("Dismiss").clicked() {
                    actions.push(Action::DismissNotice);
                }
            });
        });
    }
    // The video gets the whole width, as on youtube.com.
    if state.view != View::Watch {
        sidebar(ui, state, actions);
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
        View::Playlist | View::Channel => library::page(ui, state, actions),
        View::Account => account::show(ui, state, actions),
    }
}

fn sidebar(ui: &mut egui::Ui, state: &State, actions: &mut Vec<Action>) {
    let fill = ui.visuals().panel_fill;
    egui::Panel::left("sidebar")
        .exact_size(220.0)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(fill)
                .inner_margin(Margin::symmetric(12, 12)),
        )
        .show(ui, |ui| {
            let mut item = |ui: &mut egui::Ui, view: View, label: &str| {
                let selected = state.view == view
                    || (view == View::Playlists && state.view == View::Playlist)
                    || (view == View::Subscriptions && state.view == View::Channel);
                let button = Button::selectable(selected, RichText::new(label).size(15.0));
                if ui.add_sized([ui.available_width(), 34.0], button).clicked() {
                    actions.push(Action::Navigate(view));
                }
            };
            item(ui, View::Home, "Home");
            item(ui, View::Subscriptions, "Subscriptions");
            item(ui, View::History, "History");
            item(ui, View::WatchLater, "Watch later");
            if state.signed_in() {
                item(ui, View::Liked, "Liked videos");
                item(ui, View::Playlists, "Playlists");
            }
            ui.separator();
            let account = match &state.account {
                AccountState::SignedIn(_) => "Account",
                AccountState::SigningIn => "Signing in…",
                AccountState::SignedOut | AccountState::Failed(_) => "Sign in",
            };
            item(ui, View::Account, account);
        });
}

fn top_bar(ui: &mut egui::Ui, state: &mut State, actions: &mut Vec<Action>) {
    let fill = ui.visuals().panel_fill;
    egui::Panel::top("top-bar")
        .exact_size(56.0)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(fill)
                .inner_margin(Margin::symmetric(16, 0)),
        )
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                let deeper = matches!(state.view, View::Watch | View::Channel | View::Playlist);
                if deeper && widgets::icon_button(ui, Icon::Back, 18.0, "Back (Esc)").clicked() {
                    actions.push(Action::Back);
                }
                logo(ui);
                ui.add_space(24.0);
                let width = (ui.available_width() - 100.0).clamp(160.0, 640.0);
                let field = ui.add(
                    TextEdit::singleline(&mut state.query)
                        .hint_text("Search, or paste a YouTube link")
                        .desired_width(width)
                        .margin(Margin::symmetric(12, 8)),
                );
                let submitted =
                    field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
                if ui.button("Search").clicked() || submitted {
                    actions.push(Action::Search(state.query.clone()));
                }
            });
        });
}

fn logo(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(30.0, 21.0), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(6), ACCENT);
    let center = rect.center();
    painter.add(Shape::convex_polygon(
        vec![
            center + vec2(-4.0, -5.5),
            center + vec2(6.0, 0.0),
            center + vec2(-4.0, 5.5),
        ],
        Color32::WHITE,
        Stroke::NONE,
    ));
    ui.label(RichText::new("YouTFast").strong().size(18.0));
}

/// The player's keys, while nobody is typing.
fn shortcuts(ctx: &egui::Context, state: &State, actions: &mut Vec<Action>) {
    if ctx.text_edit_focused() {
        return;
    }
    if matches!(state.view, View::Channel | View::Playlist) {
        if ctx.input(|input| input.key_pressed(Key::Escape)) {
            actions.push(Action::Back);
        }
        return;
    }
    if state.view != View::Watch {
        return;
    }
    ctx.input(|input| {
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
