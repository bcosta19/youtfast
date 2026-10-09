//! The Account page: the `accounts` pane listing each account with
//! switch/rename/remove, and the `add account` form below it.

use egui::{Align, CornerRadius, Frame, Layout, Margin, RichText, Stroke, TextEdit, Ui};

use super::widgets;
use crate::app::{AccountState, State};
use crate::library::Account;
use crate::model::{Action, browser_label};

/// The text column's width: long lines of explanation read badly wider.
const WIDTH: f32 = 640.0;

pub fn show(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                // set_max_width also widens: a narrow window would cut
                // the text off at its edge instead of wrapping it.
                ui.set_max_width(WIDTH.min(ui.available_width()));
                // Room for the title on the first pane's top rule.
                ui.add_space(widgets::line(ui) / 2.0);
                accounts_pane(ui, state, actions);
                ui.add_space(widgets::line(ui));
                add_pane(ui, state, actions);
                ui.add_space(widgets::line(ui));
                without_account(ui, state, actions);
            });
    });
}

/// One row per account; the active one is green and gets `[ sign out ]`.
fn accounts_pane(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    widgets::pane(
        ui,
        &state.palette,
        "accounts",
        Some(state.library.accounts.len()),
        true,
        None,
        |ui| {
            if matches!(state.account, AccountState::SigningIn) {
                ui.label(
                    RichText::new(format!("{} signing in", widgets::spinner_frame(ui)))
                        .monospace()
                        .color(state.palette.secondary),
                );
            }
            if let AccountState::Failed(error) = &state.account {
                ui.label(RichText::new(error).monospace().color(state.palette.red));
            }
            if state.library.accounts.is_empty()
                && !matches!(state.account, AccountState::SigningIn)
            {
                ui.label(
                    RichText::new("-- no accounts --")
                        .monospace()
                        .color(state.palette.dim),
                );
                ui.label(
                    RichText::new("add one below")
                        .monospace()
                        .color(state.palette.secondary),
                );
            }
            for account in state.library.accounts.clone() {
                account_row(ui, state, &account, actions);
            }
        },
    );
}

/// `● name · source` with switch/rename/remove/sign-out brackets.
fn account_row(ui: &mut Ui, state: &State, account: &Account, actions: &mut Vec<Action>) {
    let active = state.library.active_account == Some(account.id);
    ui.horizontal(|ui| {
        ui.label(RichText::new("●").monospace().color(if active {
            state.palette.green
        } else {
            state.palette.dim
        }));
        // Rename edits the name inline, in the same spot.
        if let Some((renaming, text)) = &state.renaming
            && *renaming == account.id
        {
            let mut name = text.clone();
            let field = ui.add(
                TextEdit::singleline(&mut name)
                    .desired_width(160.0)
                    .font(egui::FontId::monospace(14.0))
                    .frame(egui::Frame::NONE),
            );
            if field.changed() {
                actions.push(Action::RenameAccount {
                    id: account.id,
                    name,
                });
            }
            ui.painter().hline(
                field.rect.left()..=field.rect.right(),
                field.rect.bottom() - 1.0,
                Stroke::new(1.0, state.palette.accent),
            );
            field.request_focus();
            if field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                actions.push(Action::RenameAccount {
                    id: account.id,
                    name: String::new(),
                });
            }
        } else {
            ui.label(
                RichText::new(&account.name)
                    .monospace()
                    .strong()
                    .color(state.palette.text),
            );
        }
        ui.label(
            RichText::new(format!("· {}", account.source.label()))
                .monospace()
                .color(state.palette.secondary),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // The inline confirm: `remove "name"? [ yes ] [ no ]`.
            if state.removing == Some(account.id) {
                if widgets::bracket_button(ui, &state.palette, "yes", widgets::ButtonStyle::Normal)
                    .clicked()
                {
                    actions.push(Action::RemoveAccount(account.id));
                }
                if widgets::bracket_button(ui, &state.palette, "no", widgets::ButtonStyle::Normal)
                    .clicked()
                {
                    actions.push(Action::CancelRemoving);
                }
                ui.label(
                    RichText::new(format!("remove \"{}\"?", account.name))
                        .monospace()
                        .color(state.palette.red),
                );
                return;
            }
            if active {
                if widgets::bracket_button(
                    ui,
                    &state.palette,
                    "sign out",
                    widgets::ButtonStyle::Normal,
                )
                .on_hover_text("Forgets the session on this computer. The account stays listed.")
                .clicked()
                {
                    actions.push(Action::SignOut);
                }
            } else if widgets::bracket_button(
                ui,
                &state.palette,
                "switch",
                widgets::ButtonStyle::Normal,
            )
            .clicked()
            {
                actions.push(Action::SwitchAccount(account.id));
            }
            if widgets::bracket_button(ui, &state.palette, "remove", widgets::ButtonStyle::Normal)
                .clicked()
            {
                actions.push(Action::MarkRemoving(account.id));
            }
            if widgets::bracket_button(ui, &state.palette, "rename", widgets::ButtonStyle::Normal)
                .clicked()
            {
                actions.push(Action::MarkRenaming {
                    id: account.id,
                    name: account.name.clone(),
                });
            }
        });
    });
}

/// The `add account` pane: name, browser (and profile) or cookies.txt.
fn add_pane(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    widgets::pane(ui, &state.palette, "add account", None, false, None, |ui| {
        risk(ui, state);
        ui.add_space(widgets::line(ui) / 2.0);
        // The name is required and must be new.
        let taken = state.library.account_name_taken(state.account_name.trim());
        let named = !state.account_name.trim().is_empty();
        ui.horizontal(|ui| {
            let mut name = state.account_name.clone();
            let field = ui.add(
                TextEdit::singleline(&mut name)
                    .desired_width(200.0)
                    .hint_text(RichText::new("name").color(state.palette.dim))
                    .font(egui::FontId::monospace(14.0))
                    .frame(egui::Frame::NONE),
            );
            if field.changed() {
                actions.push(Action::SetAccountName(name));
            }
            ui.painter().hline(
                field.rect.left()..=field.rect.right(),
                field.rect.bottom() - 1.0,
                Stroke::new(1.0, state.palette.outline),
            );
            if taken {
                ui.label(
                    RichText::new("that name is already used")
                        .monospace()
                        .color(state.palette.red),
                );
            } else if !named {
                ui.label(
                    RichText::new("name the account first")
                        .monospace()
                        .color(state.palette.dim),
                );
            }
        });
        ui.add_space(widgets::line(ui) / 2.0);

        let ready = named && !taken;
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
                let mut profile = state.account_profile.clone();
                let field = ui.add(
                    TextEdit::singleline(&mut profile)
                        .desired_width(140.0)
                        .hint_text(RichText::new("profile").color(state.palette.dim))
                        .font(egui::FontId::monospace(14.0))
                        .frame(egui::Frame::NONE),
                );
                if field.changed() {
                    actions.push(Action::SetAccountProfile(profile));
                }
                ui.painter().hline(
                    field.rect.left()..=field.rect.right(),
                    field.rect.bottom() - 1.0,
                    Stroke::new(1.0, state.palette.outline),
                );
                let style = if ready {
                    widgets::ButtonStyle::Primary
                } else {
                    widgets::ButtonStyle::Normal
                };
                if widgets::bracket_button(
                    ui,
                    &state.palette,
                    &format!(
                        "sign in with {}",
                        browser_label(&state.browser).to_lowercase()
                    ),
                    style,
                )
                .clicked()
                {
                    actions.push(Action::AddAccountBrowser);
                }
            });
            ui.label(
                RichText::new(
                    "Sign in on youtube.com in that browser first. YouTFast reads its \
                     YouTube cookies through yt-dlp, and again whenever they expire. On \
                     Linux, Chromium, Chrome, Brave and the like keep them encrypted with \
                     the desktop keyring, which needs Python's secretstorage module \
                     (python-secretstorage on Arch).",
                )
                .color(state.palette.secondary)
                .small(),
            );
        }
        ui.add_space(widgets::line(ui) / 2.0);

        ui.label(
            RichText::new("or a cookies.txt file")
                .monospace()
                .strong()
                .color(state.palette.text),
        );
        ui.label(
            RichText::new(
                "Safer for the browser's own session, which YouTube otherwise keeps \
                 changing:\n\
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
            "sign in with cookies.txt",
            if ready {
                widgets::ButtonStyle::Primary
            } else {
                widgets::ButtonStyle::Normal
            },
        )
        .clicked()
        {
            actions.push(Action::AddAccountFile);
        }
    });
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
    widgets::pane(
        ui,
        &state.palette,
        "without an account",
        None,
        false,
        None,
        |ui| {
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
        },
    );
}
