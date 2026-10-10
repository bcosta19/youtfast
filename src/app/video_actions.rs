//! The watch page's actions on the account: rating the video and saving
//! it to the account's playlists. Both show the change at once and put it
//! back if YouTube refuses it.

use super::*;

impl App {
    /// The rating of the video being watched: from the cache while fresh,
    /// otherwise asked for. Only an account has one.
    pub(super) fn load_rating(&mut self) {
        self.state.rating = Fetch::Idle;
        self.state.rating_base = Rating::None;
        let Some(id) = self.state.watching.as_ref().map(|video| video.id.clone()) else {
            return;
        };
        if !self.state.signed_in() {
            return;
        }
        match self.ratings.get(&id, util::now(), RATING_FRESH) {
            Some(rating) => {
                self.state.rating = Fetch::Ready(*rating);
                self.state.rating_base = *rating;
            }
            None => {
                self.state.rating = Fetch::Loading;
                self.backend.send(Command::LoadRating(id));
            }
        }
    }

    pub(super) fn on_rating(&mut self, id: String, result: Result<Rating, String>) {
        if let Ok(rating) = &result {
            self.ratings.put(id.clone(), util::now(), *rating);
        }
        if self.watching_id() != Some(id.as_str()) {
            return;
        }
        match result {
            Ok(rating) => {
                self.state.rating = Fetch::Ready(rating);
                self.state.rating_base = rating;
            }
            Err(error) => self.state.rating = Fetch::Failed(error),
        }
    }

    /// Rates the video being watched, showing the new rating at once.
    pub(super) fn rate(&mut self, rating: Rating) {
        let Some(id) = self.watching_id().map(str::to_owned) else {
            return;
        };
        if !self.state.signed_in() {
            return;
        }
        let previous = self.state.rating.ready().copied().unwrap_or_default();
        if previous == rating {
            return;
        }
        self.state.rating = Fetch::Ready(rating);
        self.ratings.put(id.clone(), util::now(), rating);
        // The Liked list changes with it.
        self.state.liked = Listing::Idle;
        self.backend.send(Command::Rate {
            id,
            rating,
            previous,
        });
    }

    pub(super) fn on_rate_failed(&mut self, id: String, previous: Rating, error: String) {
        self.ratings.put(id.clone(), util::now(), previous);
        if self.watching_id() == Some(id.as_str()) {
            self.state.rating = Fetch::Ready(previous);
        }
        self.state.notice = Some(format!("Could not rate the video: {error}"));
    }

    /// Opening the save menu asks for the account's playlists, once per
    /// video.
    pub(super) fn load_save_targets(&mut self) {
        if !self.state.signed_in() || !matches!(self.state.save_targets, Fetch::Idle) {
            return;
        }
        let Some(id) = self.watching_id().map(str::to_owned) else {
            return;
        };
        self.state.save_targets = Fetch::Loading;
        self.backend.send(Command::LoadSaveTargets(id));
    }

    pub(super) fn on_save_targets(&mut self, id: String, result: Result<Vec<SaveTarget>, String>) {
        if self.watching_id() == Some(id.as_str()) {
            self.state.save_targets = match result {
                Ok(targets) => Fetch::Ready(targets),
                Err(error) => Fetch::Failed(error),
            };
        }
    }

    /// Adds the video being watched to an account playlist, or takes it
    /// out, showing it at once.
    pub(super) fn set_saved(&mut self, playlist: String, saved: bool) {
        let Some(id) = self.watching_id().map(str::to_owned) else {
            return;
        };
        if !self.mark_saved(&playlist, saved) {
            return;
        }
        self.forget_playlist(&playlist);
        self.backend.send(Command::SetSaved {
            id,
            playlist,
            saved,
        });
    }

    pub(super) fn on_save_failed(
        &mut self,
        id: String,
        playlist: String,
        saved: bool,
        error: String,
    ) {
        if self.watching_id() == Some(id.as_str()) {
            self.mark_saved(&playlist, !saved);
        }
        self.forget_playlist(&playlist);
        self.state.notice = Some(format!("Could not change the playlist: {error}"));
    }

    /// Sets a save menu row; false when the menu has no such row.
    fn mark_saved(&mut self, playlist: &str, saved: bool) -> bool {
        let Fetch::Ready(targets) = &mut self.state.save_targets else {
            return false;
        };
        match targets.iter_mut().find(|target| target.id == playlist) {
            Some(target) => {
                target.saved = saved;
                true
            }
            None => false,
        }
    }

    /// A changed playlist's listing loads again on its next visit.
    fn forget_playlist(&mut self, playlist: &str) {
        if playlist == "WL" {
            self.state.account_watch_later = Listing::Idle;
        }
        if matches!(&self.state.page, Some(Page::Playlist { id, .. }) if id == playlist) {
            self.state.page_videos = Listing::Idle;
        }
    }

    /// An account change forgets the ratings kept, and reads the video's
    /// own again.
    pub(super) fn forget_account_marks(&mut self) {
        self.ratings = util::Recent::new(RATINGS_KEPT);
        self.state.save_targets = Fetch::Idle;
        self.load_rating();
    }

    fn watching_id(&self) -> Option<&str> {
        self.state.watching.as_ref().map(|video| video.id.as_str())
    }
}
