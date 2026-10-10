//! The Accounts page: the accounts as rows in a card (round avatar, name,
//! where it signs in from, the actions as pills), the add-account form in
//! its own card, and what works without an account.

use egui::{
    Align, Color32, CornerRadius, Frame, Layout, Margin, Rect, Response, RichText, Sense, Stroke,
    TextEdit, Ui, pos2, vec2,
};

use super::{results, widgets};
use crate::app::{AccountState, State};
use crate::icons::Icon;
use crate::library::Account;
use crate::model::{Action, browser_label};
use crate::theme::{self, Palette, RADIUS};

/// The text column's width: long lines of explanation read badly wider.
const WIDTH: f32 = 640.0;

pub fn show(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    results::panel(ui, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                // set_max_width also widens: a narrow window would cut
                // the text off at its edge instead of wrapping it.
                ui.set_max_width(WIDTH.min(ui.available_width()));
                results::title(ui, state, "Accounts", Some(state.library.accounts.len()));
                accounts(ui, state, actions);
                ui.add_space(24.0);
                add_account(ui, state, actions);
                ui.add_space(24.0);
                without_account(ui, state, actions);
                ui.add_space(24.0);
            });
    });
}

/// The accounts in a card, one row each; signing in and its failure above
/// them.
fn accounts(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    widgets::card(ui, palette, |ui| {
        let signing_in = matches!(state.account, AccountState::SigningIn);
        if signing_in {
            ui.horizontal(|ui| {
                widgets::spinner(ui, 16.0);
                ui.label(
                    RichText::new("Signing in…")
                        .font(theme::regular(13.0))
                        .color(palette.secondary),
                );
            });
        }
        if let AccountState::Failed(error) = &state.account {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                Icon::Alert.image(palette.red, 16.0).paint_at(ui, rect);
                ui.add(
                    egui::Label::new(
                        RichText::new(error)
                            .font(theme::regular(14.0))
                            .color(palette.red),
                    )
                    .wrap(),
                );
            });
        }
        if state.library.accounts.is_empty() && !signing_in {
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);
                ui.label(
                    RichText::new("No accounts")
                        .font(theme::semibold(16.0))
                        .color(palette.text),
                );
                ui.label(
                    RichText::new("Add one below")
                        .font(theme::regular(14.0))
                        .color(palette.secondary),
                );
                ui.add_space(8.0);
            });
        }
        for (index, account) in state.library.accounts.iter().enumerate() {
            if index > 0 {
                ui.add_space(8.0);
            }
            account_row(ui, state, account, actions);
        }
    });
}

/// The round avatar with the initial, the name in semibold over where it
/// signs in from, the active one marked, and its actions at the end.
fn account_row(ui: &mut Ui, state: &State, account: &Account, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let raised = widgets::on_card(palette);
    let active = state.library.active_account == Some(account.id);
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
        widgets::avatar(ui, palette, None, &account.name, rect);
        ui.add_space(4.0);
        ui.vertical(|ui| {
            // Rename edits the name inline, in the same spot.
            if let Some((renaming, text)) = &state.renaming
                && *renaming == account.id
            {
                let mut name = text.clone();
                let field = field(
                    ui,
                    palette,
                    &mut name,
                    "Account name",
                    egui::Id::new("rename account"),
                    180.0,
                );
                if field.changed() {
                    actions.push(Action::RenameAccount {
                        id: account.id,
                        name,
                    });
                }
                field.request_focus();
                if field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                    actions.push(Action::RenameAccount {
                        id: account.id,
                        name: String::new(),
                    });
                }
            } else {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&account.name)
                            .font(theme::semibold(14.0))
                            .color(palette.text),
                    );
                    if active {
                        active_pill(ui, palette);
                    }
                });
            }
            ui.label(
                RichText::new(account.source.label())
                    .font(theme::regular(13.0))
                    .color(palette.secondary),
            );
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // The inline confirm: `Remove "name"?  Cancel  Remove`.
            if state.removing == Some(account.id) {
                if widgets::button(ui, &raised, "Remove", widgets::ButtonStyle::Primary).clicked() {
                    actions.push(Action::RemoveAccount(account.id));
                }
                if widgets::button(ui, &raised, "Cancel", widgets::ButtonStyle::Normal).clicked() {
                    actions.push(Action::CancelRemoving);
                }
                ui.label(
                    RichText::new(format!("Remove \u{201c}{}\u{201d}?", account.name))
                        .font(theme::medium(14.0))
                        .color(palette.red),
                );
                return;
            }
            if active {
                if widgets::button(ui, &raised, "Sign out", widgets::ButtonStyle::Normal)
                    .on_hover_text(
                        "Forgets the session on this computer. The account stays listed.",
                    )
                    .clicked()
                {
                    actions.push(Action::SignOut);
                }
            } else if widgets::button(ui, &raised, "Switch", widgets::ButtonStyle::Normal).clicked()
            {
                actions.push(Action::SwitchAccount(account.id));
            }
            if widgets::button(ui, &raised, "Remove", widgets::ButtonStyle::Normal).clicked() {
                actions.push(Action::MarkRemoving(account.id));
            }
            if widgets::button(ui, &raised, "Rename", widgets::ButtonStyle::Normal).clicked() {
                actions.push(Action::MarkRenaming {
                    id: account.id,
                    name: account.name.clone(),
                });
            }
        });
    });
}

/// The `accent` pill with a check that marks the account in use.
fn active_pill(ui: &mut Ui, palette: &Palette) {
    let galley = ui.painter().layout_no_wrap(
        "Active".to_owned(),
        theme::semibold(12.0),
        palette.on_accent,
    );
    let icon = 12.0;
    let height = 20.0;
    let width = 8.0 + icon + 4.0 + galley.size().x + 10.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    ui.painter().rect_filled(
        rect,
        CornerRadius::same((height / 2.0) as u8),
        palette.accent,
    );
    Icon::Check.image(palette.on_accent, icon).paint_at(
        ui,
        Rect::from_min_size(
            pos2(rect.left() + 8.0, rect.center().y - icon / 2.0),
            vec2(icon, icon),
        ),
    );
    let at = pos2(
        rect.left() + 8.0 + icon + 4.0,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(at, galley, palette.on_accent);
}

/// The add-account form in a card: the risk first, the name, then a
/// browser's login or a cookies.txt file.
fn add_account(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let raised = widgets::on_card(palette);
    widgets::section(ui, palette, "Add an account", None, |ui| {
        widgets::card(ui, palette, |ui| {
            risk(ui, palette);
            ui.add_space(16.0);
            // The name is required and must be new.
            let taken = state.library.account_name_taken(state.account_name.trim());
            let named = !state.account_name.trim().is_empty();
            heading(ui, palette, "Name");
            ui.horizontal(|ui| {
                let mut name = state.account_name.clone();
                let field = field(
                    ui,
                    palette,
                    &mut name,
                    "A name for this account",
                    egui::Id::new("account name"),
                    240.0,
                );
                if field.changed() {
                    actions.push(Action::SetAccountName(name));
                }
                if taken {
                    ui.label(
                        RichText::new("That name is already used")
                            .font(theme::regular(13.0))
                            .color(palette.red),
                    );
                } else if !named {
                    ui.label(
                        RichText::new("Name the account first")
                            .font(theme::regular(13.0))
                            .color(palette.dim),
                    );
                }
            });
            ui.add_space(16.0);

            let ready = named && !taken;
            let style = if ready {
                widgets::ButtonStyle::Primary
            } else {
                widgets::ButtonStyle::Normal
            };
            heading(ui, palette, "Use your browser's YouTube login");
            if state.browsers.is_empty() {
                note(
                    ui,
                    palette,
                    "No supported browser was found on this computer.",
                );
            } else {
                ui.horizontal_wrapped(|ui| {
                    egui::ComboBox::from_id_salt("browser")
                        .selected_text(
                            RichText::new(browser_label(&state.browser)).font(theme::regular(14.0)),
                        )
                        .show_ui(ui, |ui| {
                            for browser in &state.browsers {
                                if ui
                                    .selectable_label(
                                        state.browser == *browser,
                                        RichText::new(browser_label(browser))
                                            .font(theme::regular(14.0)),
                                    )
                                    .clicked()
                                {
                                    actions.push(Action::SetBrowser((*browser).to_owned()));
                                }
                            }
                        });
                    let mut profile = state.account_profile.clone();
                    let field = field(
                        ui,
                        palette,
                        &mut profile,
                        "Profile",
                        egui::Id::new("account profile"),
                        140.0,
                    );
                    if field.changed() {
                        actions.push(Action::SetAccountProfile(profile));
                    }
                    let label = format!("Sign in with {}", browser_label(&state.browser));
                    if widgets::button(ui, &raised, &label, style).clicked() {
                        actions.push(Action::AddAccountBrowser);
                    }
                });
                ui.add_space(4.0);
                note(
                    ui,
                    palette,
                    "Sign in on youtube.com in that browser first. YouTFast reads its YouTube \
                     cookies through yt-dlp, and again whenever they expire. On Linux, \
                     Chromium, Chrome, Brave and the like keep them encrypted with the \
                     desktop keyring, which needs Python's secretstorage module \
                     (python-secretstorage on Arch).",
                );
            }
            ui.add_space(16.0);

            heading(ui, palette, "Or a cookies.txt file");
            note(
                ui,
                palette,
                "Safer for the browser's own session, which YouTube otherwise keeps \
                 changing:\n\
                 1. Open a private window and sign in to youtube.com.\n\
                 2. Export the cookies with an extension such as \"Get cookies.txt LOCALLY\".\n\
                 3. Close the private window without signing out.",
            );
            ui.add_space(8.0);
            if widgets::button(ui, &raised, "Sign in with cookies.txt", style).clicked() {
                actions.push(Action::AddAccountFile);
            }
        });
    });
}

/// What signing in risks, said before it is done: the info icon, then
/// the note in secondary.
fn risk(ui: &mut Ui, palette: &Palette) {
    ui.horizontal_top(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
        Icon::Info.image(palette.secondary, 18.0).paint_at(ui, rect);
        ui.vertical(|ui| {
            ui.label(
                RichText::new("Before you sign in")
                    .font(theme::semibold(14.0))
                    .color(palette.text),
            );
            note(
                ui,
                palette,
                "YouTube offers no sign-in for apps like this one, so YouTFast uses the \
                 session your browser has, as yt-dlp does. Google may treat that as \
                 automated access: the account could be asked to verify itself, signed \
                 out, or in rare cases suspended. A secondary account is the safe choice.",
            );
        });
    });
}

/// What works on this computer alone, and the Takeout import.
fn without_account(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    widgets::section(ui, palette, "Without an account", None, |ui| {
        widgets::card(ui, palette, |ui| {
            note(
                ui,
                palette,
                "Subscriptions, history and Watch later also work on this computer alone, \
                 with no risk to any account. Bring them over from Google Takeout: choose \
                 YouTube and YouTube Music, with JSON as the history format, then import \
                 subscriptions.csv and watch-history.json.",
            );
            ui.add_space(8.0);
            let raised = widgets::on_card(palette);
            if widgets::button(
                ui,
                &raised,
                "Import from Google Takeout",
                widgets::ButtonStyle::Normal,
            )
            .clicked()
            {
                actions.push(Action::ImportTakeout);
            }
        });
    });
}

/// A part of the form's name, in semibold.
fn heading(ui: &mut Ui, palette: &Palette, text: &str) {
    ui.label(
        RichText::new(text)
            .font(theme::semibold(14.0))
            .color(palette.text),
    );
    ui.add_space(4.0);
}

/// Explanation in secondary, wrapped to the column.
fn note(ui: &mut Ui, palette: &Palette, text: &str) {
    ui.add(
        egui::Label::new(
            RichText::new(text)
                .font(theme::regular(13.0))
                .color(palette.secondary),
        )
        .wrap(),
    );
}

/// A filled, rounded text field on a card: a step brighter than the card,
/// brighter again while focused; the hint in `dim`.
fn field(
    ui: &mut Ui,
    palette: &Palette,
    text: &mut String,
    hint: &str,
    id: egui::Id,
    width: f32,
) -> Response {
    let focused = ui.memory(|memory| memory.has_focus(id));
    let (fill, stroke) = if focused {
        (
            palette.surface_active,
            Stroke::new(1.0, palette.text.gamma_multiply(0.6)),
        )
    } else {
        (
            palette.surface_hover,
            Stroke::new(1.0, Color32::TRANSPARENT),
        )
    };
    ui.add(
        TextEdit::singleline(text)
            .id(id)
            .desired_width(width)
            .hint_text(RichText::new(hint).color(palette.dim))
            .font(theme::regular(14.0))
            .vertical_align(Align::Center)
            .frame(
                Frame::new()
                    .fill(fill)
                    .stroke(stroke)
                    .corner_radius(CornerRadius::same(RADIUS))
                    .inner_margin(Margin::symmetric(12, 8)),
            ),
    )
}
