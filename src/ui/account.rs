//! The Accounts page: the accounts as rows in a card (round avatar, name,
//! where it signs in from, the actions as pills), the add-account form in
//! its own card, and what works without an account.

use egui::{
    Align, Color32, CornerRadius, Frame, Margin, Rect, Response, RichText, Sense, Stroke, TextEdit,
    Ui, pos2, vec2,
};

use super::{results, widgets};
use crate::app::{AccountState, State};
use crate::appearance::i18n::{Locale, format as trf, gettext as tr};
use crate::appearance::icons::Icon;
use crate::appearance::theme::{self, Palette, RADIUS};
use crate::model::{Action, browser_label};
use crate::storage::library::Account;

/// The text column's width: long lines of explanation read badly wider.
const WIDTH: f32 = 640.0;

pub fn show(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    results::panel(ui, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                // set_max_width also widens: a narrow window would cut
                // the text off at its edge instead of wrapping it.
                ui.set_max_width(WIDTH.min(ui.available_width()));
                results::title(
                    ui,
                    state,
                    &tr(locale, "Accounts"),
                    Some(state.library.accounts.len()),
                );
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
    let locale = state.settings.language;
    let palette = &state.palette;
    widgets::card(ui, palette, |ui| {
        let signing_in = matches!(state.account, AccountState::SigningIn);
        if signing_in {
            ui.horizontal(|ui| {
                widgets::spinner(ui, 16.0);
                ui.label(
                    RichText::new(tr(locale, "Signing in…"))
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
                        RichText::new(crate::appearance::i18n::message(locale, error))
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
                    RichText::new(tr(locale, "No accounts"))
                        .font(theme::semibold(16.0))
                        .color(palette.text),
                );
                ui.label(
                    RichText::new(tr(locale, "Add an account below"))
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

/// The account's identity and actions, on separate lines so both remain usable
/// at narrow window widths.
fn account_row(ui: &mut Ui, state: &State, account: &Account, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;
    let raised = widgets::on_card(palette);
    let active = state.account == AccountState::SignedIn(account.id);
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
                    &tr(locale, "Account name"),
                    egui::Id::new("rename account"),
                    180.0,
                );
                let escape = ui.input(|input| input.key_pressed(egui::Key::Escape));
                if escape && (field.has_focus() || field.lost_focus()) {
                    actions.push(Action::RenameAccount {
                        id: account.id,
                        name: String::new(),
                    });
                } else if field.lost_focus() {
                    actions.push(Action::RenameAccount {
                        id: account.id,
                        name,
                    });
                } else if field.changed() {
                    actions.push(Action::SetRenameDraft {
                        id: account.id,
                        name,
                    });
                }
            } else {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(&account.name)
                            .font(theme::semibold(14.0))
                            .color(palette.text),
                    );
                    if active {
                        active_pill(ui, palette, locale);
                    }
                });
            }
            ui.label(
                RichText::new(match &account.source {
                    crate::model::AccountSource::File => {
                        tr(locale, "a cookies.txt file").into_owned()
                    }
                    crate::model::AccountSource::Browser(_) => account.source.label(),
                })
                .font(theme::regular(13.0))
                .color(palette.secondary),
            );
        });
    });
    if state.removing == Some(account.id) {
        ui.label(
            RichText::new(trf(
                locale,
                "Remove “{name}”?",
                &[("name", account.name.clone())],
            ))
            .font(theme::medium(14.0))
            .color(palette.red),
        );
        ui.horizontal_wrapped(|ui| {
            if widgets::button(
                ui,
                &raised,
                &tr(locale, "Cancel"),
                widgets::ButtonStyle::Normal,
            )
            .clicked()
            {
                actions.push(Action::CancelRemoving);
            }
            if widgets::button(
                ui,
                &raised,
                &tr(locale, "Remove"),
                widgets::ButtonStyle::Primary,
            )
            .clicked()
            {
                actions.push(Action::RemoveAccount(account.id));
            }
        });
    } else {
        ui.horizontal_wrapped(|ui| {
            if widgets::button(
                ui,
                &raised,
                &tr(locale, "Rename"),
                widgets::ButtonStyle::Normal,
            )
            .clicked()
            {
                actions.push(Action::MarkRenaming {
                    id: account.id,
                    name: account.name.clone(),
                });
            }
            if widgets::button(
                ui,
                &raised,
                &tr(locale, "Remove"),
                widgets::ButtonStyle::Normal,
            )
            .clicked()
            {
                actions.push(Action::MarkRemoving(account.id));
            }
            if active {
                if widgets::button(
                    ui,
                    &raised,
                    &tr(locale, "Sign out"),
                    widgets::ButtonStyle::Normal,
                )
                .on_hover_text(tr(
                    locale,
                    "Forgets the session on this computer. The account stays listed.",
                ))
                .clicked()
                {
                    actions.push(Action::SignOut);
                }
            } else if widgets::button(
                ui,
                &raised,
                &tr(locale, "Switch"),
                widgets::ButtonStyle::Normal,
            )
            .clicked()
            {
                actions.push(Action::SwitchAccount(account.id));
            }
        });
    }
}

/// The `accent` pill with a check that marks the account in use.
fn active_pill(ui: &mut Ui, palette: &Palette, locale: Locale) {
    let galley = ui.painter().layout_no_wrap(
        tr(locale, "Active").into_owned(),
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
    let locale = state.settings.language;
    let palette = &state.palette;
    let raised = widgets::on_card(palette);
    widgets::section(ui, palette, &tr(locale, "Add an account"), None, |ui| {
        widgets::card(ui, palette, |ui| {
            risk(ui, palette, locale);
            ui.add_space(16.0);
            heading(ui, palette, &tr(locale, "Sign in with your browser"));
            note(
                ui,
                palette,
                &tr(
                    locale,
                    "Sign in to youtube.com in your browser, then select its profile here. YouTFast reads that profile's YouTube session through yt-dlp and reads it again when it expires.",
                ),
            );
            ui.add_space(8.0);
            if state.browser_profiles.is_empty() {
                note(
                    ui,
                    palette,
                    &tr(locale, "No supported browser was found on this computer."),
                );
            }
            let busy = matches!(state.account, AccountState::SigningIn);
            for (index, profile) in state.browser_profiles.iter().enumerate() {
                profile_row(ui, state, index, profile, busy, actions);
            }
            ui.add_space(12.0);

            // The name is optional: a blank one comes from the profile.
            let taken = state.library.account_name_taken(state.account_name.trim());
            ui.vertical(|ui| {
                let mut name = state.account_name.clone();
                let field = field(
                    ui,
                    palette,
                    &mut name,
                    &tr(locale, "Name (optional)"),
                    egui::Id::new("account name"),
                    240.0,
                );
                if field.changed() {
                    actions.push(Action::SetAccountName(name));
                }
                let (text, color) = if taken {
                    (tr(locale, "That name is already used"), palette.red)
                } else {
                    (
                        tr(locale, "Leave blank to use the profile name"),
                        palette.secondary,
                    )
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
                &tr(locale, "Another browser or profile"),
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

            heading(ui, palette, &tr(locale, "Or a cookies.txt file"));
            note(
                ui,
                palette,
                &tr(
                    locale,
                    "Helps preserve your browser session, which YouTube can change:\n1. Open a private window and sign in to youtube.com.\n2. Export cookies with an extension such as \"Get cookies.txt LOCALLY\".\n3. Close the private window without signing out.",
                ),
            );
            ui.add_space(8.0);
            if widgets::button(ui, &raised, &tr(locale, "Sign in with cookies.txt"), style)
                .clicked()
            {
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
    profile: &crate::storage::account::BrowserProfile,
    busy: bool,
    actions: &mut Vec<Action>,
) {
    let locale = state.settings.language;
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
    });
    if added {
        ui.label(
            RichText::new(tr(locale, "Added"))
                .font(theme::medium(13.0))
                .color(palette.secondary),
        );
    } else if ui
        .add_enabled_ui(!busy, |ui| {
            widgets::button(
                ui,
                &raised,
                &tr(locale, "Sign in"),
                widgets::ButtonStyle::Primary,
            )
        })
        .inner
        .clicked()
    {
        actions.push(Action::AddAccountProfile(index));
    }
    ui.add_space(6.0);
}

/// A browser and profile folder by hand, for a profile the lists missed.
fn advanced(ui: &mut Ui, state: &State, style: widgets::ButtonStyle, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
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
            &tr(locale, "Profile folder"),
            egui::Id::new("account profile"),
            160.0,
        );
        if field.changed() {
            actions.push(Action::SetAccountProfile(profile));
        }
        let label = trf(
            locale,
            "Sign in with {browser}",
            &[("browser", browser_label(&state.browser))],
        );
        if widgets::button(ui, &raised, &label, style).clicked() {
            actions.push(Action::AddAccountBrowser);
        }
    });
    ui.add_space(4.0);
    note(
        ui,
        palette,
        &tr(
            locale,
            "Enter the profile folder (\"Profile 2\" in Chromium and similar browsers, \"xxxx.default-release\" in Firefox). Leave blank for the default profile. On Linux, Chromium, Chrome, Brave and similar browsers encrypt cookies using the desktop keyring, which needs Python secretstorage (python-secretstorage on Arch).",
        ),
    );
}

/// What signing in risks, said before it is done: the info icon, then
/// the note in secondary.
fn risk(ui: &mut Ui, palette: &Palette, locale: Locale) {
    ui.horizontal_top(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
        Icon::Info.image(palette.secondary, 18.0).paint_at(ui, rect);
        ui.vertical(|ui| {
            ui.label(
                RichText::new(tr(locale, "Before you sign in"))
                    .font(theme::semibold(14.0))
                    .color(palette.text),
            );
            note(
                ui,
                palette,
                &tr(locale, "YouTube offers no sign-in for apps like this, so YouTFast uses your browser session, as yt-dlp does. Google may treat this as automated access and ask for verification, end the session or, in rare cases, suspend the account. A secondary account reduces this risk."),
            );
        });
    });
}

/// What works on this computer alone, and the Takeout import.
fn without_account(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;
    widgets::section(ui, palette, &tr(locale, "Without an account"), None, |ui| {
        widgets::card(ui, palette, |ui| {
            note(
                ui,
                palette,
                &tr(
                    locale,
                    "Subscriptions, history and Watch later also work on this computer alone. To import them from Google Takeout, select YouTube and YouTube Music, choose JSON for history, then import subscriptions.csv and watch-history.json.",
                ),
            );
            ui.add_space(8.0);
            let raised = widgets::on_card(palette);
            if widgets::button(
                ui,
                &raised,
                &tr(locale, "Import from Google Takeout"),
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
    use crate::model::AccountSource;
    use crate::storage::account::BrowserProfile;

    #[test]
    fn account_row_actions_fit_narrow_window_in_every_state() {
        for mode in ["normal", "renaming", "removing"] {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            crate::appearance::theme::install_fonts(&ctx);
            let account = Account {
                id: 1,
                name: "Ana".into(),
                source: AccountSource::File,
            };
            let mut state = State {
                account: AccountState::SignedIn(1),
                ..State::default()
            };
            if mode == "renaming" {
                state.renaming = Some((1, account.name.clone()));
            }
            if mode == "removing" {
                state.removing = Some(1);
            }
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(336.0, 700.0))),
                    ..Default::default()
                },
                |ui| account_row(ui, &state, &account, &mut Vec::new()),
            );
            output.textures_delta.clear();
            let identity_bottom = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == "a cookies.txt file" => {
                        Some(text.pos.y + text.galley.size().y)
                    }
                    _ => None,
                })
                .expect("account source is visible");
            let tree = output
                .platform_output
                .accesskit_update
                .expect("accessibility tree");
            let labels: &[&str] = if mode == "removing" {
                &["Cancel", "Remove"]
            } else {
                &["Rename", "Remove", "Sign out"]
            };
            for label in labels {
                let node = tree
                    .nodes
                    .iter()
                    .find(|(_, node)| node.label() == Some(*label))
                    .unwrap_or_else(|| panic!("missing {mode} {label}"));
                let bounds = node.1.bounds().expect("button bounds");
                assert!(
                    bounds.x0 >= 0.0 && bounds.x1 <= 336.0,
                    "{mode} {label} outside window: {bounds:?}"
                );
                assert!(
                    bounds.y0 >= f64::from(identity_bottom),
                    "{mode} {label} overlaps account identity: {bounds:?}"
                );
            }
        }
    }

    #[test]
    fn add_account_form_text_fits_narrow_window() {
        let ctx = egui::Context::default();
        crate::appearance::theme::install_fonts(&ctx);
        let state = State::default();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(360.0, 1400.0))),
                ..Default::default()
            },
            |ui| show(ui, &state, &mut Vec::new()),
        );
        output.textures_delta.clear();
        let mut cookie_note_found = false;
        for shape in &output.shapes {
            if let egui::Shape::Text(text) = &shape.shape {
                cookie_note_found |= text
                    .galley
                    .text()
                    .contains("Helps preserve your browser session");
                assert!(
                    text.pos.x >= 0.0 && text.pos.x + text.galley.size().x <= 360.0,
                    "text outside narrow window: {:?}",
                    text.galley.text()
                );
            }
        }
        assert!(cookie_note_found, "cookies instructions should be shown");
    }

    #[test]
    fn rename_keeps_a_draft_until_enter_and_escape_cancels() {
        let ctx = egui::Context::default();
        crate::appearance::theme::install_fonts(&ctx);
        let account = Account {
            id: 1,
            name: "Ana".into(),
            source: AccountSource::File,
        };
        let mut state = State {
            renaming: Some((1, account.name.clone())),
            ..State::default()
        };
        ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("rename account")));
        let frame = |state: &mut State, events| {
            let mut actions = Vec::new();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 400.0))),
                    events,
                    ..Default::default()
                },
                |ui| account_row(ui, state, &account, &mut actions),
            );
            output.textures_delta.clear();
            actions
        };
        assert!(frame(&mut state, vec![]).is_empty());
        for character in ["x", "y"] {
            let actions = frame(&mut state, vec![egui::Event::Text(character.into())]);
            let [Action::SetRenameDraft { id: 1, name }] = actions.as_slice() else {
                panic!("typing must update only the draft: {actions:?}");
            };
            state.renaming = Some((1, name.clone()));
        }
        let draft = state.renaming.as_ref().expect("still editing").1.clone();
        assert!(draft.contains('x') && draft.contains('y'));
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        assert_eq!(
            frame(&mut state, vec![key(egui::Key::Enter)]),
            vec![Action::RenameAccount { id: 1, name: draft }]
        );

        state.renaming = Some((1, "discard me".into()));
        ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("rename account")));
        frame(&mut state, vec![]);
        assert_eq!(
            frame(&mut state, vec![key(egui::Key::Escape)]),
            vec![Action::RenameAccount {
                id: 1,
                name: String::new(),
            }]
        );
    }

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
        crate::appearance::theme::install_fonts(&ctx);
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
