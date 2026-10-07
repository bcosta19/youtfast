//! Signing in and out, and bringing a library over without an account,
//! in the Account pane.

use egui::{CornerRadius, Frame, Margin, RichText, Stroke, Ui};

use super::widgets;
use crate::app::{AccountState, State};
use crate::model::{Action, browser_label};

/// The text column's width: long lines of explanation read badly wider.
const WIDTH: f32 = 640.0;

pub fn show(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ui, |ui| {
        widgets::pane(ui, &state.palette, "Account", None, true, None, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    // set_max_width also widens: a narrow window would cut
                    // the text off at its edge instead of wrapping it.
                    ui.set_max_width(WIDTH.min(ui.available_width()));
                    match &state.account {
                        AccountState::SignedIn(source) => {
                            signed_in(ui, state, &source.label(), actions)
                        }
                        AccountState::SigningIn => {
                            ui.label(
                                RichText::new(format!("{} signing in", widgets::spinner_frame(ui)))
                                    .monospace()
                                    .color(state.palette.secondary),
                            );
                        }
                        AccountState::SignedOut | AccountState::Failed(_) => {
                            if let AccountState::Failed(error) = &state.account {
                                ui.label(RichText::new(error).monospace().color(state.palette.red));
                                ui.add_space(widgets::line(ui));
                            }
                            sign_in(ui, state, actions);
                        }
                    }
                    ui.add_space(widgets::line(ui));
                    without_account(ui, state, actions);
                });
        });
    });
}

fn signed_in(ui: &mut Ui, state: &State, source: &str, actions: &mut Vec<Action>) {
    ui.label(
        RichText::new(format!("signed in with the session from {source}"))
            .monospace()
            .color(state.palette.text),
    );
    ui.label(
        RichText::new(
            "Your feed, subscriptions, history, Watch later, liked videos and playlists \
             come from your account. The session is kept in the system keyring, never \
             in a file.",
        )
        .color(state.palette.secondary),
    );
    ui.add_space(widgets::line(ui) / 2.0);
    if widgets::bracket_button(ui, &state.palette, "sign out", widgets::ButtonStyle::Normal)
        .on_hover_text("Forgets the session on this computer. The browser stays signed in.")
        .clicked()
    {
        actions.push(Action::SignOut);
    }
}

fn sign_in(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    risk(ui, state);
    ui.add_space(widgets::line(ui));

    ui.label(
        RichText::new("use your browser's YouTube login")
            .monospace()
            .strong()
            .color(state.palette.text),
    );
    if state.browsers.is_empty() {
        ui.label(
            RichText::new("no supported browser was found on this computer")
                .monospace()
                .color(state.palette.dim),
        );
    } else {
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("browser")
                .selected_text(RichText::new(browser_label(&state.browser)).monospace())
                .show_ui(ui, |ui| {
                    for browser in &state.browsers {
                        if ui
                            .selectable_label(state.browser == *browser, browser_label(browser))
                            .clicked()
                        {
                            actions.push(Action::SetBrowser((*browser).to_owned()));
                        }
                    }
                });
            if widgets::bracket_button(ui, &state.palette, "sign in", widgets::ButtonStyle::Primary)
                .clicked()
            {
                actions.push(Action::SignInWithBrowser(state.browser.clone()));
            }
        });
        ui.label(
            RichText::new(
                "Sign in on youtube.com in that browser first. YouTFast reads its YouTube \
                 cookies through yt-dlp, and again whenever they expire. On Linux, Chromium, \
                 Chrome, Brave and the like keep them encrypted with the desktop keyring, \
                 which needs Python's secretstorage module (python-secretstorage on Arch).",
            )
            .color(state.palette.secondary)
            .small(),
        );
    }
    ui.add_space(widgets::line(ui));

    ui.label(
        RichText::new("or a cookies.txt file")
            .monospace()
            .strong()
            .color(state.palette.text),
    );
    ui.label(
        RichText::new(
            "Safer for the browser's own session, which YouTube otherwise keeps changing:\n\
             1. Open a private window and sign in to youtube.com.\n\
             2. Export the cookies with an extension such as \"Get cookies.txt LOCALLY\".\n\
             3. Close the private window without signing out.",
        )
        .monospace()
        .color(state.palette.secondary),
    );
    if widgets::bracket_button(
        ui,
        &state.palette,
        "choose cookies.txt",
        widgets::ButtonStyle::Normal,
    )
    .clicked()
    {
        actions.push(Action::SignInWithFile);
    }
}

/// What signing in risks, said before it is done: a square warning rule.
fn risk(ui: &mut Ui, state: &State) {
    let warning = state.palette.yellow;
    Frame::new()
        .stroke(Stroke::new(1.0, warning))
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.label(
                RichText::new("before you sign in")
                    .monospace()
                    .strong()
                    .color(warning),
            );
            ui.label(
                RichText::new(
                    "YouTube offers no sign-in for apps like this one, so YouTFast uses the \
                     session your browser has, as yt-dlp does. Google may treat that as \
                     automated access: the account could be asked to verify itself, signed \
                     out, or in rare cases suspended. A secondary account is the safe choice.",
                )
                .monospace()
                .color(state.palette.text),
            );
        });
}

fn without_account(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    ui.label(
        RichText::new("without an account")
            .monospace()
            .strong()
            .color(state.palette.text),
    );
    ui.label(
        RichText::new(
            "Subscriptions, history and Watch later also work on this computer alone, \
             with no risk to any account. Bring them over from Google Takeout: choose \
             YouTube and YouTube Music, with JSON as the history format, then import \
             subscriptions.csv and watch-history.json.",
        )
        .monospace()
        .color(state.palette.secondary),
    );
    if widgets::bracket_button(
        ui,
        &state.palette,
        "import from Google Takeout",
        widgets::ButtonStyle::Normal,
    )
    .clicked()
    {
        actions.push(Action::ImportTakeout);
    }
}
