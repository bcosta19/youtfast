//! Signing in and out, and bringing a library over without an account.

use egui::{Color32, CornerRadius, Frame, Margin, RichText, Stroke, Ui};

use crate::app::{AccountState, State};
use crate::model::{Action, browser_label};

/// The column's width: long lines of explanation read badly wider.
const WIDTH: f32 = 640.0;

pub fn show(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                // set_max_width also widens: a narrow window would cut the text
                // off at its edge instead of wrapping it.
                ui.set_max_width(WIDTH.min(ui.available_width()));
                ui.heading("Account");
                ui.add_space(12.0);
                match &state.account {
                    AccountState::SignedIn(source) => signed_in(ui, &source.label(), actions),
                    AccountState::SigningIn => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Signing in…");
                        });
                    }
                    AccountState::SignedOut | AccountState::Failed(_) => {
                        if let AccountState::Failed(error) = &state.account {
                            ui.label(RichText::new(error).color(ui.visuals().error_fg_color));
                            ui.add_space(12.0);
                        }
                        sign_in(ui, state, actions);
                    }
                }
                ui.add_space(24.0);
                ui.separator();
                ui.add_space(12.0);
                without_account(ui, actions);
            });
    });
}

fn signed_in(ui: &mut Ui, source: &str, actions: &mut Vec<Action>) {
    ui.label(format!(
        "Signed in to YouTube with the session from {source}."
    ));
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Your feed, subscriptions, history, Watch later, liked videos and playlists \
             come from your account. The session is kept in the system keyring, never \
             in a file.",
        )
        .weak(),
    );
    ui.add_space(12.0);
    if ui
        .button("Sign out")
        .on_hover_text("Forgets the session on this computer. The browser stays signed in.")
        .clicked()
    {
        actions.push(Action::SignOut);
    }
}

fn sign_in(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    risk(ui);
    ui.add_space(16.0);

    ui.label(
        RichText::new("Use your browser's YouTube login")
            .strong()
            .size(16.0),
    );
    ui.add_space(4.0);
    if state.browsers.is_empty() {
        ui.label(RichText::new("No supported browser was found on this computer.").weak());
    } else {
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("browser")
                .selected_text(browser_label(&state.browser))
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
            if ui.button("Sign in").clicked() {
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
            .weak()
            .small(),
        );
    }
    ui.add_space(16.0);

    ui.label(
        RichText::new("Or use a cookies.txt file")
            .strong()
            .size(16.0),
    );
    ui.add_space(4.0);
    ui.label(
        "Safer for the browser's own session, which YouTube otherwise keeps changing:\n\
         1. Open a private window and sign in to youtube.com.\n\
         2. Export the cookies with an extension such as \"Get cookies.txt LOCALLY\".\n\
         3. Close the private window without signing out.",
    );
    ui.add_space(4.0);
    if ui.button("Choose cookies.txt…").clicked() {
        actions.push(Action::SignInWithFile);
    }
}

/// What signing in risks, said before it is done.
fn risk(ui: &mut Ui) {
    let warning = Color32::from_rgb(230, 170, 60);
    Frame::new()
        .stroke(Stroke::new(1.0, warning))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.label(RichText::new("Before you sign in").strong().color(warning));
            ui.add_space(4.0);
            ui.label(
                "YouTube offers no sign-in for apps like this one, so YouTFast uses the \
                 session your browser has, as yt-dlp does. Google may treat that as \
                 automated access: the account could be asked to verify itself, signed \
                 out, or in rare cases suspended. A secondary account is the safe choice.",
            );
        });
}

fn without_account(ui: &mut Ui, actions: &mut Vec<Action>) {
    ui.label(RichText::new("Without an account").strong().size(16.0));
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Subscriptions, history and Watch later also work on this computer alone, \
             with no risk to any account. Bring them over from Google Takeout: choose \
             YouTube and YouTube Music, with JSON as the history format, then import \
             subscriptions.csv and watch-history.json.",
        )
        .weak(),
    );
    ui.add_space(8.0);
    if ui.button("Import from Google Takeout").clicked() {
        actions.push(Action::ImportTakeout);
    }
}
