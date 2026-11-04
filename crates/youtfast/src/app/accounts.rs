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

impl State {
    fn forget_account_data(&mut self, ctx: &egui::Context) {
        egui::Popup::close_all(ctx);
        ctx.request_repaint();
        self.cursor = 0;
        self.account_subscriptions = Listing::Idle;
        self.subscriptions_more = None;
        self.subscriptions_more_state = Fetch::Idle;
        self.account_history = Listing::Idle;
        self.account_history_generation += 1;
        self.history_needs_refresh = false;
        self.history_requested_at = None;
        self.history_filter = crate::logic::history::HistoryFilter::All;
        self.history_more = None;
        self.history_more_state = Fetch::Idle;
        self.account_watch_later = Listing::Idle;
        self.watch_later_more = None;
        self.watch_later_more_state = Fetch::Idle;
        self.liked = Listing::Idle;
        self.liked_more = None;
        self.liked_more_state = Fetch::Idle;
        self.playlists = Listing::Idle;
        self.playlists_more = None;
        self.playlists_more_state = Fetch::Idle;
        self.feed = Listing::Idle;
        self.feed_more = None;
        self.feed_more_state = Fetch::Idle;
        self.feed_loaded_at = None;
        self.feed_shown = 40;
        self.shorts = Listing::Idle;
        self.for_you = Listing::Idle;
        self.for_you_more = None;
        self.for_you_more_state = Fetch::Idle;
        self.for_you_from_account = false;
        self.page_videos = Listing::Idle;
        self.page_playlists = Listing::Idle;
        self.channel_header = None;
        self.channel_about = Fetch::Idle;
        self.page_more = None;
        self.page_more_state = Fetch::Idle;
        if matches!(self.page, Some(Page::Playlist { .. })) {
            self.page = None;
            if self.view == View::Playlist {
                self.view = View::Home;
            }
        }
        self.details = Fetch::Idle;
        self.comments = Fetch::Idle;
        self.comments_next = None;
        self.comments_more = Fetch::Idle;
        self.comments_visitor_data = None;
        self.comment_pages.clear();
        self.comment_image_refreshes.clear();
        self.replies.clear();
        self.comments_open.clear();
        self.rating = Fetch::Idle;
        self.rating_base = Rating::None;
        self.rating_pending.clear();
        self.save_pending.clear();
        self.save_targets = Fetch::Idle;
        self.up_next.clear();
        self.queue.clear();
        self.autoplay = None;
        self.alerts.clear();
        self.alerts_visit.clear();
        self.alerts_checking = false;
        self.last_checked = None;
    }
}

impl App {
    pub(super) fn begin_account_change(&mut self) {
        self.playback_report.cancel();
        self.playing_generation += 1;
        self.back.clear();
        self.forward.clear();
        self.state.extras = Fetch::Idle;
        self.state.transcript_open = false;
        self.state.results_channels = Listing::Idle;
        self.state.results_playlists = Listing::Idle;
        self.state.playing_playlist = None;
        self.state.playing_more = None;
        self.state.playing_more_state = Fetch::Idle;
        self.state.playing_waiting = None;
        self.set_playback_cookies(None);
        self.state.forget_account_data(self.backend.context());
        self.generations.clear();
        self.generation += 1;
        self.details_generation = self.generation;
        self.details_cache = util::Recent::new(DETAILS_KEPT);
        self.ratings = util::Recent::new(RATINGS_KEPT);
        self.more_comments_token = None;
        self.shelf_asked = false;
        self.state.account = AccountState::SigningIn;
    }

    pub(super) fn sign_out(&mut self) {
        self.begin_account_change();
        self.backend.send(Command::SignOut);
    }

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
            .set_title(crate::appearance::i18n::gettext(
                self.state.settings.language,
                "Choose the cookies.txt exported from YouTube",
            ))
            .add_filter("cookies.txt", &["txt"])
            .pick_file();
        self.begin_account_change();
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
        self.begin_account_change();
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
        self.begin_account_change();
        self.backend.send(Command::AddAccount {
            account,
            method: SignInMethod::Browser,
        });
    }

    /// Signs in as another kept account.
    pub(super) fn switch_account(&mut self, id: u32) {
        if (self.state.library.active_account != Some(id)
            || self.state.account != AccountState::SignedIn(id))
            && let Some(account) = self.state.library.account(id).cloned()
        {
            self.begin_account_change();
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
            self.sign_out();
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
        self.begin_account_change();
        self.state.account = AccountState::SignedIn(id);
        self.state.library.active_account = Some(id);
        self.save_library();
        self.state.source = Source::Account;
        self.state.history_filter = crate::logic::history::HistoryFilter::All;
        self.state.history_needs_refresh = false;
        // The account's subscriptions keep their channels out of the
        // RSS half of the feed, so they come first.
        self.request(Slot::Subscriptions, Request::Subscriptions);
        self.state.feed = Listing::Idle;
        self.state.shorts = Listing::Idle;
        self.load(self.state.view, true);
        self.forget_account_marks();
        if let Some(video) = self.state.watching.clone() {
            self.refresh_details(video);
        }
    }

    /// Drops the account's lists and leaves its pages.
    pub(super) fn on_signed_out(&mut self) {
        self.finish_signed_out(None);
    }

    pub(super) fn on_session_unavailable(&mut self, id: u32, error: String) {
        self.finish_signed_out(Some(id));
        self.state.account = AccountState::Failed(error);
    }

    fn finish_signed_out(&mut self, retry_account: Option<u32>) {
        let account = if matches!(self.state.account, AccountState::Failed(_)) {
            self.state.account.clone()
        } else {
            AccountState::SignedOut
        };
        self.begin_account_change();
        self.state.account = account;
        // The account stays listed: its session is kept to switch
        // back.
        self.state.library.active_account = retry_account;
        self.save_library();
        if matches!(self.state.view, View::Liked | View::Playlists) {
            self.state.view = View::Home;
        }
        self.load(self.state.view, true);
        self.forget_account_marks();
        if let Some(video) = self.state.watching.clone() {
            self.refresh_details(video);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_changes_forget_private_lists_without_touching_the_device() {
        let private = Video::from_id("private0001".into());
        let mut state = State {
            account_history: Listing::Ready(vec![HistoryRow {
                video: private.clone(),
                when: None,
            }]),
            account_watch_later: Listing::Ready(vec![private.clone()]),
            liked: Listing::Ready(vec![private.clone()]),
            for_you: Listing::Ready(vec![private.clone()]),
            for_you_from_account: true,
            page: Some(Page::Playlist {
                id: "private-playlist".into(),
                name: "Private".into(),
            }),
            page_videos: Listing::Ready(vec![private.clone()]),
            queue: vec![private.clone()],
            up_next: vec![private.clone()],
            alerts: vec![private.clone()],
            save_targets: Fetch::Ready(vec![SaveTarget {
                id: "private-playlist".into(),
                name: "Private".into(),
                saved: true,
            }]),
            channel_header: Some(crate::model::ChannelHeader {
                name: "private-channel".into(),
                ..Default::default()
            }),
            cursor: 99,
            view: View::Playlist,
            ..State::default()
        };
        state
            .library
            .record_watch(Video::from_id("device00001".into()), 1);
        let ctx = egui::Context::default();
        let menu = egui::Id::new("private-playlist-menu");
        egui::Popup::open_id(&ctx, menu);
        assert!(egui::Popup::is_id_open(&ctx, menu));
        state.forget_account_data(&ctx);
        assert!(!egui::Popup::is_id_open(&ctx, menu));
        assert!(state.channel_header.is_none());
        assert_eq!(state.cursor, 0);
        assert!(matches!(state.account_history, Listing::Idle));
        assert!(matches!(state.account_watch_later, Listing::Idle));
        assert!(matches!(state.liked, Listing::Idle));
        assert!(matches!(state.for_you, Listing::Idle));
        assert!(!state.for_you_from_account);
        assert!(state.page.is_none());
        assert!(matches!(state.page_videos, Listing::Idle));
        assert!(matches!(state.save_targets, Fetch::Idle));
        assert!(state.queue.is_empty() && state.up_next.is_empty() && state.alerts.is_empty());
        assert_eq!(state.view, View::Home);
        assert_eq!(state.library.history[0].video.id, "device00001");
    }

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
