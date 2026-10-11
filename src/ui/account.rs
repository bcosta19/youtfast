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
            heading(ui, palette, "Sign in with your browser");
            note(
                ui,
                palette,
                "Sign in on youtube.com in the browser first, then pick its profile here. \
                 YouTFast reads that profile's YouTube login through yt-dlp, and again \
                 whenever it expires.",
            );
            ui.add_space(8.0);
            if state.browser_profiles.is_empty() {
                note(
                    ui,
                    palette,
                    "No supported browser was found on this computer.",
                );
            }
            let busy = matches!(state.account, AccountState::SigningIn);
            for (index, profile) in state.browser_profiles.iter().enumerate() {
                profile_row(ui, state, index, profile, busy, actions);
            }
            ui.add_space(12.0);

            // The name is optional: a blank one comes from the profile.
            let taken = state.library.account_name_taken(state.account_name.trim());
            ui.horizontal(|ui| {
                let mut name = state.account_name.clone();
                let field = field(
                    ui,
                    palette,
                    &mut name,
                    "Name (optional)",
                    egui::Id::new("account name"),
                    240.0,
                );
                if field.changed() {
                    actions.push(Action::SetAccountName(name));
                }
                let (text, color) = if taken {
                    ("That name is already used", palette.red)
                } else {
                    ("Left blank, the profile names it", palette.dim)
                };
                ui.label(RichText::new(text).font(theme::regular(13.0)).color(color));
            });
            ui.add_space(12.0);
            let style = if taken {
                widgets::ButtonStyle::Normal
            } else {
                widgets::ButtonStyle::Primary
            };

            let open = state.account_advanced;
            let chevron = if open {
                Icon::ChevronUp
            } else {
                Icon::ChevronDown
            };
            if widgets::icon_text_button(
                ui,
                &raised,
                Some(chevron),
                "Another browser or profile",
                widgets::ButtonStyle::Normal,
            )
            .clicked()
            {
                actions.push(Action::ToggleAccountAdvanced);
            }
            if open {
                ui.add_space(8.0);
                advanced(ui, state, style, actions);
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

/// A browser profile found on this computer: its initial, its name and
/// where it is (browser, e-mail), and Sign in, or Added once an account
/// signs in from it.
fn profile_row(
    ui: &mut Ui,
    state: &State,
    index: usize,
    profile: &crate::account::BrowserProfile,
    busy: bool,
    actions: &mut Vec<Action>,
) {
    let palette = &state.palette;
    let raised = widgets::on_card(palette);
    let spec = profile.spec();
    let added = state
        .library
        .accounts
        .iter()
        .any(|account| account.source == crate::model::AccountSource::Browser(spec.clone()));
    let title = if profile.name.is_empty() {
        browser_label(profile.browser)
    } else {
        profile.name.clone()
    };
    let mut detail = browser_label(profile.browser);
    if let Some(email) = &profile.email {
        detail.push_str(" · ");
        detail.push_str(email);
    }
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
        widgets::avatar(ui, palette, None, &title, rect);
        ui.add_space(4.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(
                RichText::new(&title)
                    .font(theme::semibold(14.0))
                    .color(palette.text),
            );
            ui.label(
                RichText::new(detail)
                    .font(theme::regular(13.0))
                    .color(palette.secondary),
            );
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if added {
                ui.label(
                    RichText::new("Added")
                        .font(theme::medium(13.0))
                        .color(palette.dim),
                );
            } else if ui
                .add_enabled_ui(!busy, |ui| {
                    widgets::button(ui, &raised, "Sign in", widgets::ButtonStyle::Primary)
                })
                .inner
                .clicked()
            {
                actions.push(Action::AddAccountProfile(index));
            }
        });
    });
    ui.add_space(6.0);
}

/// A browser and profile folder by hand, for a profile the lists missed.
fn advanced(ui: &mut Ui, state: &State, style: widgets::ButtonStyle, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let raised = widgets::on_card(palette);
    if state.browsers.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("browser")
            .selected_text(RichText::new(browser_label(&state.browser)).font(theme::regular(14.0)))
            .show_ui(ui, |ui| {
                for browser in &state.browsers {
                    if ui
                        .selectable_label(
                            state.browser == *browser,
                            RichText::new(browser_label(browser)).font(theme::regular(14.0)),
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
            "Profile folder",
            egui::Id::new("account profile"),
            160.0,
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
        "The folder is the profile's own (\"Profile 2\" for Chromium and the like, \
         \"xxxx.default-release\" for Firefox); blank is the default profile. On Linux, \
         Chromium, Chrome, Brave and the like keep cookies encrypted with the desktop \
         keyring, which needs Python's secretstorage module (python-secretstorage on Arch).",
    );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::BrowserProfile;
    use crate::model::AccountSource;

    /// The form with profiles found, one already added, and Advanced open
    /// lays out without a NaN or a negative rect.
    #[test]
    fn the_accounts_page_draws_the_profiles_found() {
        let profile =
            |browser, folder: Option<&str>, name: &str, email: Option<&str>| BrowserProfile {
                browser,
                profile: folder.map(str::to_owned),
                name: name.to_owned(),
                email: email.map(str::to_owned),
            };
        let mut state = State {
            browsers: vec!["chromium", "firefox"],
            browser: "chromium".to_owned(),
            browser_profiles: vec![
                profile("chromium", None, "Person 1", Some("ana@example.com")),
                profile(
                    "firefox",
                    Some("x.default-release"),
                    "default-release",
                    None,
                ),
            ],
            ..State::default()
        };
        state.library.accounts.push(Account {
            id: 1,
            name: "ana".to_owned(),
            source: AccountSource::Browser("chromium".to_owned()),
        });
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        for (width, advanced) in [(1200.0, false), (420.0, true)] {
            state.account_advanced = advanced;
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(width, 1400.0))),
                ..Default::default()
            };
            for _ in 0..2 {
                let mut output = ctx.run_ui(input.clone(), |ui| {
                    let mut actions = Vec::new();
                    show(ui, &state, &mut actions);
                });
                output.textures_delta.clear();
            }
        }
    }
}
