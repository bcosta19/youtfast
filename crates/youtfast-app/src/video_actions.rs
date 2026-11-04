//! The watch page's actions on the account: rating the video and saving
//! it to the account's playlists. Both show the change at once and read the
//! account again if the write fails or its outcome is unknown.

use super::*;

impl State {
    fn begin_rating(&mut self, id: &str, rating: Rating) -> Option<Rating> {
        let previous = self.rating.ready().copied()?;
        if previous == rating || !self.rating_pending.insert(id.to_owned()) {
            return None;
        }
        self.rating = Fetch::Ready(rating);
        Some(previous)
    }

    fn begin_save(&mut self, id: &str, playlist: &str, saved: bool) -> bool {
        let key = (id.to_owned(), playlist.to_owned());
        if self.save_pending.contains(&key) {
            return false;
        }
        let Fetch::Ready(targets) = &mut self.save_targets else {
            return false;
        };
        let Some(target) = targets.iter_mut().find(|target| target.id == playlist) else {
            return false;
        };
        if target.saved == saved {
            return false;
        }
        target.saved = saved;
        self.save_pending.insert(key);
        true
    }
}

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
        if self.state.rating_pending.contains(&id) {
            return;
        }
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
        let Some(previous) = self.state.begin_rating(&id, rating) else {
            return;
        };
        self.ratings.put(id.clone(), util::now(), rating);
        // The Liked list changes with it.
        self.state.liked = Listing::Idle;
        self.state.liked_more = None;
        self.state.liked_more_state = Fetch::Idle;
        self.backend.send(Command::Rate {
            id,
            rating,
            previous,
        });
    }

    pub(super) fn on_rate_failed(&mut self, id: String, _previous: Rating, error: String) {
        self.state.rating_pending.remove(&id);
        // A timeout can occur after the server applied the write. Read the
        // actual rating instead of caching an assumed rollback.
        self.ratings.remove(&id);
        if self.watching_id() == Some(id.as_str()) {
            self.load_rating();
        }
        self.state.notice = Some(format!("Could not rate the video: {error}"));
    }

    pub(super) fn on_rate_succeeded(&mut self, id: String, rating: Rating) {
        self.state.rating_pending.remove(&id);
        self.ratings.put(id.clone(), util::now(), rating);
        if self.watching_id() == Some(id.as_str()) {
            self.state.rating = Fetch::Ready(rating);
        }
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
        if self
            .state
            .save_pending
            .iter()
            .any(|(video, _)| video == &id)
        {
            return;
        }
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
        if !self.state.begin_save(&id, &playlist, saved) {
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
        _saved: bool,
        error: String,
    ) {
        self.state
            .save_pending
            .remove(&(id.clone(), playlist.clone()));
        if self.watching_id() == Some(id.as_str()) {
            self.state.save_targets = Fetch::Idle;
        }
        self.forget_playlist(&playlist);
        self.state.notice = Some(format!("Could not change the playlist: {error}"));
    }

    pub(super) fn on_save_succeeded(&mut self, id: String, playlist: String, saved: bool) {
        self.state
            .save_pending
            .remove(&(id.clone(), playlist.clone()));
        if self.watching_id() == Some(id.as_str()) {
            self.mark_saved(&playlist, saved);
        }
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
            self.state.watch_later_more = None;
            self.state.watch_later_more_state = Fetch::Idle;
        }
        if matches!(&self.state.page, Some(Page::Playlist { id, .. }) if id == playlist) {
            self.state.page_videos = Listing::Idle;
            self.state.page_more = None;
            self.state.page_more_state = Fetch::Idle;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_rating_rejects_a_second_change_until_the_first_finishes() {
        let mut state = State {
            rating: Fetch::Ready(Rating::None),
            ..Default::default()
        };
        assert_eq!(
            state.begin_rating("video", Rating::Like),
            Some(Rating::None)
        );
        assert_eq!(state.begin_rating("video", Rating::Dislike), None);
        assert_eq!(state.rating.ready(), Some(&Rating::Like));
        state.rating_pending.remove("video");
        assert_eq!(
            state.begin_rating("video", Rating::Dislike),
            Some(Rating::Like)
        );
    }

    #[test]
    fn pending_playlist_change_rejects_a_second_change_on_the_same_row() {
        let mut state = State {
            save_targets: Fetch::Ready(vec![SaveTarget {
                id: "p".into(),
                name: "List".into(),
                saved: false,
            }]),
            ..Default::default()
        };
        assert!(state.begin_save("video", "p", true));
        assert!(!state.begin_save("video", "p", false));
        assert!(state.save_targets.ready().unwrap()[0].saved);
        state.save_pending.remove(&("video".into(), "p".into()));
        assert!(state.begin_save("video", "p", false));
    }
}
