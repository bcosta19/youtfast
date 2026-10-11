//! Accounts: adding, switching, renaming and removing them, and signing
//! in and out.

use super::*;

/// `name`, or `name 2`, `name 3`… when the library already has it.
fn unique_name(library: &Library, name: &str) -> String {
    let name = name.trim();
    let name = if name.is_empty() {
        "YouTube account"
    } else {
        name
    };
    if !library.account_name_taken(name) {
        return name.to_owned();
    }
    (2..)
        .map(|number| format!("{name} {number}"))
        .find(|candidate| !library.account_name_taken(candidate))
        .expect("some number is free")
}

impl App {
    /// The account the add form describes, named as typed or, left
    /// blank, after `suggested` (made unique with a number); its id is the
    /// first free one. A typed name already used makes none.
    fn new_account(&self, source: AccountSource, suggested: &str) -> Option<Account> {
        let typed = self.state.account_name.trim();
        let name = if typed.is_empty() {
            unique_name(&self.state.library, suggested)
        } else if self.state.library.account_name_taken(typed) {
            return None;
        } else {
            typed.to_owned()
        };
        let id = self
            .state
            .library
            .accounts
            .iter()
            .map(|account| account.id)
            .max()
            .unwrap_or(0)
            + 1;
        Some(Account { id, name, source })
    }

    /// Adds the form's account from a cookies.txt file the user picks.
    pub(super) fn add_account_file(&mut self) {
        // The name makes the account; the pick happens on the way.
        let Some(account) = self.new_account(AccountSource::File, "YouTube account") else {
            return;
        };
        let picked = rfd::AsyncFileDialog::new()
            .set_title("Choose the cookies.txt exported from YouTube")
            .add_filter("cookies.txt", &["txt"])
            .pick_file();
        self.backend.send(Command::AddAccount {
            account,
            method: SignInMethod::File(Box::pin(picked)),
        });
    }

    /// Adds the form's account from the chosen browser and profile.
    pub(super) fn add_account_browser(&mut self) {
        let mut spec = self.state.browser.clone();
        let profile = self.state.account_profile.trim();
        if !profile.is_empty() {
            spec = format!("{spec}:{profile}");
        }
        let suggested = crate::model::browser_label(&self.state.browser);
        let Some(account) = self.new_account(AccountSource::Browser(spec), &suggested) else {
            return;
        };
        self.backend.send(Command::AddAccount {
            account,
            method: SignInMethod::Browser,
        });
    }

    /// Adds an account from a detected browser profile: one click, the
    /// name from the profile unless one was typed.
    pub(super) fn add_account_profile(&mut self, index: usize) {
        let Some(profile) = self.state.browser_profiles.get(index).cloned() else {
            return;
        };
        let source = AccountSource::Browser(profile.spec());
        let Some(account) = self.new_account(source, &profile.suggested_name()) else {
            return;
        };
        self.state.account_name.clear();
        self.backend.send(Command::AddAccount {
            account,
            method: SignInMethod::Browser,
        });
    }

    /// Signs in as another kept account.
    pub(super) fn switch_account(&mut self, id: u32) {
        if self.state.library.active_account != Some(id)
            && let Some(account) = self.state.library.account(id).cloned()
        {
            self.state.account = AccountState::SigningIn;
            self.backend.send(Command::Switch(account));
        }
    }

    /// Renames an account, unless the new name is blank, and closes the
    /// rename field.
    pub(super) fn rename_account(&mut self, id: u32, name: &str) {
        let name = name.trim();
        if !name.is_empty()
            && let Some(account) = self.state.library.account_mut(id)
        {
            account.name = name.to_owned();
            self.save_library();
        }
        self.state.renaming = None;
    }

    /// Forgets an account and its session, signing out first when it is
    /// the active one.
    pub(super) fn remove_account(&mut self, id: u32) {
        if self.state.library.active_account == Some(id) {
            self.backend.send(Command::SignOut);
        }
        self.state
            .library
            .accounts
            .retain(|account| account.id != id);
        self.save_library();
        self.backend.send(Command::RemoveAccount(id));
        self.state.removing = None;
    }

    /// Makes a signed-in account the source of the account pages and
    /// reloads them.
    pub(super) fn on_signed_in(&mut self, id: u32) {
        // The browser for playback follows once it is known.
        self.set_playback_cookies(None);
        self.state.account = AccountState::SignedIn(id);
        self.state.library.active_account = Some(id);
        self.save_library();
        self.state.source = Source::Account;
        // The account's subscriptions keep their channels out of the
        // RSS half of the feed, so they come first.
        self.request(Slot::Subscriptions, Request::Subscriptions);
        self.state.feed = Listing::Idle;
        self.state.shorts = Listing::Idle;
        self.load(self.state.view, true);
        self.forget_account_marks();
    }

    /// Drops the account's lists and leaves its pages.
    pub(super) fn on_signed_out(&mut self) {
        self.set_playback_cookies(None);
        self.state.account = AccountState::SignedOut;
        // The account stays listed: its session is kept to switch
        // back.
        self.state.library.active_account = None;
        self.save_library();
        self.state.account_subscriptions = Listing::Idle;
        self.state.subscriptions_more = None;
        self.state.subscriptions_more_state = Fetch::Idle;
        self.state.account_history = Listing::Idle;
        self.state.history_more = None;
        self.state.history_more_state = Fetch::Idle;
        self.state.account_watch_later = Listing::Idle;
        self.state.watch_later_more = None;
        self.state.watch_later_more_state = Fetch::Idle;
        self.state.liked = Listing::Idle;
        self.state.liked_more = None;
        self.state.liked_more_state = Fetch::Idle;
        self.state.playlists = Listing::Idle;
        self.state.playlists_more = None;
        self.state.playlists_more_state = Fetch::Idle;
        self.state.feed = Listing::Idle;
        if matches!(self.state.view, View::Liked | View::Playlists) {
            self.state.view = View::Home;
        }
        self.load(self.state.view, true);
        self.forget_account_marks();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_name_becomes_a_free_one() {
        let mut library = Library::default();
        assert_eq!(unique_name(&library, "ana"), "ana");
        assert_eq!(unique_name(&library, "  "), "YouTube account");
        for name in ["ana", "ana 2"] {
            library.accounts.push(Account {
                id: library.accounts.len() as u32 + 1,
                name: name.to_owned(),
                source: AccountSource::File,
            });
        }
        assert_eq!(unique_name(&library, "ana"), "ana 3");
    }
}
