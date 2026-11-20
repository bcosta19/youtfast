//! Bounded in-memory cache and application wiring for DeArrow branding.
//! Responses are never written to disk and are cleared when DeArrow is turned off.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};

use youtfast_backend::DeArrowConsent;
use youtfast_core::model::Video;
use youtfast_youtube::dearrow::{Branding, THUMBNAIL_BASE};

use super::*;

/// Maximum number of DeArrow branding results retained in memory.
pub const DEARROW_CACHE_LIMIT: usize = 1_000;
const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(5);

/// Bounded in-memory cache of DeArrow branding results (`DEARROW_CACHE_LIMIT` entries).
#[derive(Default)]
pub struct DeArrowCache {
    entries: HashMap<String, Branding>,
    order: VecDeque<String>,
    pending: HashSet<String>,
    wanted: RefCell<Vec<String>>,
    retry_at: Option<std::time::Instant>,
    branding_consent: Option<DeArrowConsent>,
    thumbnail_consent: Option<DeArrowConsent>,
}

impl DeArrowCache {
    pub fn get(&self, video_id: &str) -> Option<&Branding> {
        self.entries.get(video_id)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Stores `branding` for `video_id`, evicting the oldest entry past
    /// [`DEARROW_CACHE_LIMIT`].
    pub fn insert(&mut self, video_id: String, branding: Branding) {
        self.pending.remove(&video_id);
        match self.entries.entry(video_id) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.insert(branding);
                return;
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                self.order.push_back(entry.key().clone());
                entry.insert(branding);
            }
        }
        while self.order.len() > DEARROW_CACHE_LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }

    /// Marks `video_id` for lookup on the next `App::logic` pass when not yet
    /// cached or in flight.
    pub fn queue(&self, video_id: &str) {
        if video_id.is_empty()
            || self.entries.contains_key(video_id)
            || self.pending.contains(video_id)
            || self
                .retry_at
                .is_some_and(|retry| retry > std::time::Instant::now())
        {
            return;
        }
        let mut wanted = self.wanted.borrow_mut();
        if !wanted.iter().any(|existing| existing == video_id) {
            wanted.push(video_id.to_owned());
        }
    }

    /// Drains queued video IDs and records them as in flight.
    pub fn take_wanted(&mut self) -> Vec<String> {
        let wanted = std::mem::take(&mut *self.wanted.borrow_mut());
        wanted
            .into_iter()
            .filter(|id| !self.entries.contains_key(id) && self.pending.insert(id.clone()))
            .collect()
    }

    /// Releases a failed lookup without caching it as a miss.
    fn failed(&mut self, video_id: &str) {
        self.pending.remove(video_id);
        self.retry_at = Some(std::time::Instant::now() + RETRY_DELAY);
    }

    /// Drops every cached, pending, and queued entry.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.pending.clear();
        self.wanted.borrow_mut().clear();
        self.retry_at = None;
        if let Some(consent) = self.branding_consent.take() {
            consent.revoke();
        }
    }
}

fn granted_consent(token: &mut Option<DeArrowConsent>) -> DeArrowConsent {
    token.get_or_insert_with(DeArrowConsent::granted).clone()
}

impl State {
    /// Whether either DeArrow feature is enabled in settings.
    pub fn dearrow_enabled(&self) -> bool {
        self.settings.dearrow_titles || self.settings.dearrow_thumbnails
    }

    /// Queues a DeArrow lookup for `video_id` when DeArrow is enabled.
    pub fn request_dearrow(&self, video_id: &str) {
        if self.dearrow_enabled() {
            self.dearrow.queue(video_id);
        }
    }

    /// Returns the community replacement title for `video` when DeArrow titles
    /// are enabled and a distinct replacement title is cached.
    pub fn dearrow_replacement_title(&self, video: &Video) -> Option<&str> {
        if !self.settings.dearrow_titles {
            return None;
        }
        self.request_dearrow(&video.id);
        self.dearrow
            .get(&video.id)
            .and_then(|branding| branding.title.as_deref())
            .filter(|title| *title != video.title)
    }

    /// Returns the title to display for `video` (DeArrow title when enabled and
    /// available, otherwise `video.title`).
    pub fn display_title<'a>(&'a self, video: &'a Video) -> &'a str {
        self.dearrow_replacement_title(video)
            .unwrap_or(video.title.as_str())
    }

    /// Returns the DeArrow community title for `video_id` when enabled and
    /// available, even if the original title is not yet populated.
    pub fn dearrow_title_by_id(&self, video_id: &str) -> Option<&str> {
        if !self.settings.dearrow_titles {
            return None;
        }
        self.request_dearrow(video_id);
        self.dearrow
            .get(video_id)
            .and_then(|branding| branding.title.as_deref())
    }

    /// Returns the DeArrow thumbnail URL for `video` when DeArrow thumbnails are
    /// enabled and a community thumbnail exists for it.
    pub fn dearrow_thumbnail_url(&self, video: &Video) -> Option<String> {
        if !self.settings.dearrow_thumbnails {
            return None;
        }
        self.request_dearrow(&video.id);
        self.dearrow
            .get(&video.id)
            .and_then(|branding| branding.thumbnail_url(&video.id))
    }

    pub(super) fn accept_dearrow(&mut self, video_id: String, branding: Branding) {
        if self.dearrow_enabled() {
            self.dearrow.insert(video_id, branding);
        }
    }

    pub(super) fn set_dearrow_titles_choice(&mut self, on: bool) {
        self.settings.dearrow_titles = on;
        if !self.dearrow_enabled() {
            self.dearrow.clear();
        }
    }

    pub(super) fn set_dearrow_thumbnails_choice(&mut self, on: bool) {
        self.settings.dearrow_thumbnails = on;
        if !on && let Some(consent) = self.dearrow.thumbnail_consent.take() {
            consent.revoke();
        }
        if !self.dearrow_enabled() {
            self.dearrow.clear();
        }
    }
}

impl App {
    pub(super) fn set_dearrow_titles(&mut self, ctx: &egui::Context, on: bool) {
        self.state.set_dearrow_titles_choice(on);
        self.save_settings();
        if on && let Some(video) = &self.state.watching {
            self.state.request_dearrow(&video.id);
        }
        ctx.request_repaint();
    }

    pub(super) fn set_dearrow_thumbnails(&mut self, ctx: &egui::Context, on: bool) {
        self.state.set_dearrow_thumbnails_choice(on);
        self.save_settings();
        if !on {
            images::evict_prefix(ctx, THUMBNAIL_BASE);
        } else if let Some(video) = &self.state.watching {
            self.state.request_dearrow(&video.id);
        }
        ctx.request_repaint();
    }

    pub(super) fn complete_dearrow(
        &mut self,
        video_id: String,
        branding: Option<Branding>,
        consent: DeArrowConsent,
    ) {
        if !consent.is_current() || !self.state.dearrow_enabled() {
            return;
        }
        if let Some(branding) = branding {
            self.state.accept_dearrow(video_id, branding);
        } else {
            self.state.dearrow.failed(&video_id);
            self.backend.context().request_repaint_after(RETRY_DELAY);
        }
    }

    pub(super) fn drain_dearrow(&mut self) {
        if !self.state.dearrow_enabled() {
            self.state.dearrow.clear();
            return;
        }
        let wanted = self.state.dearrow.take_wanted();
        if !wanted.is_empty() {
            let consent = granted_consent(&mut self.state.dearrow.branding_consent);
            for video_id in wanted {
                self.backend.send(Command::DeArrow {
                    video_id,
                    consent: consent.clone(),
                });
            }
        }
    }

    pub(super) fn drain_thumbnails(&mut self) {
        let mut wanted = images::drain_wanted();
        let mut community = Vec::new();
        wanted.retain_mut(|(url, _)| {
            if !url
                .strip_prefix(THUMBNAIL_BASE)
                .is_some_and(|rest| rest.starts_with('/'))
            {
                return true;
            }
            if self.state.settings.dearrow_thumbnails {
                community.push(std::mem::take(url));
            }
            false
        });
        if !community.is_empty() {
            let consent = granted_consent(&mut self.state.dearrow.thumbnail_consent);
            self.backend.send(Command::DeArrowThumbnails {
                urls: community,
                consent,
            });
        }
        if !wanted.is_empty() {
            self.backend.send(Command::CacheThumbnails(wanted));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use youtfast_youtube::dearrow::Thumbnail;

    #[test]
    fn cache_is_bounded_and_evicts_oldest_entries() {
        let mut cache = DeArrowCache::default();
        for index in 0..(DEARROW_CACHE_LIMIT + 5) {
            cache.insert(
                format!("vid_{index:07}"),
                Branding {
                    title: Some(format!("Title {index}")),
                    thumbnail: None,
                },
            );
        }
        assert_eq!(cache.len(), DEARROW_CACHE_LIMIT);
        for index in 0..5 {
            assert!(cache.get(&format!("vid_{index:07}")).is_none());
        }
        assert_eq!(
            cache.get("vid_0000005").and_then(|b| b.title.as_deref()),
            Some("Title 5")
        );
    }

    #[test]
    fn failed_branding_lookup_can_be_queued_again_after_backoff() {
        let mut cache = DeArrowCache::default();
        cache.queue("video");
        assert_eq!(cache.take_wanted(), ["video"]);
        cache.failed("video");
        assert!(cache.get("video").is_none());
        cache.queue("video");
        assert!(cache.take_wanted().is_empty());
        cache.retry_at = Some(std::time::Instant::now());
        cache.queue("video");
        assert_eq!(cache.take_wanted(), ["video"]);
    }

    #[test]
    fn toggling_off_clears_cache_and_ignores_late_events() {
        let mut state = State::default();
        state.set_dearrow_titles_choice(true);
        state.accept_dearrow(
            "abc".into(),
            Branding {
                title: Some("Clean".into()),
                thumbnail: Some(Thumbnail::Timestamp(10.0)),
            },
        );
        assert_eq!(state.dearrow.len(), 1);

        state.set_dearrow_titles_choice(false);
        assert!(state.dearrow.is_empty());

        state.accept_dearrow(
            "late".into(),
            Branding {
                title: Some("Late".into()),
                thumbnail: None,
            },
        );
        assert!(state.dearrow.is_empty());
    }

    #[test]
    fn thumbnail_and_branding_consent_are_revoked_independently() {
        let mut state = State::default();
        state.set_dearrow_titles_choice(true);
        state.set_dearrow_thumbnails_choice(true);
        let branding = granted_consent(&mut state.dearrow.branding_consent);
        let thumbnail = granted_consent(&mut state.dearrow.thumbnail_consent);

        state.set_dearrow_thumbnails_choice(false);
        assert!(!thumbnail.is_current());
        assert!(branding.is_current());
        state.set_dearrow_thumbnails_choice(true);
        let next_thumbnail = granted_consent(&mut state.dearrow.thumbnail_consent);
        assert!(next_thumbnail.is_current());
        assert!(!thumbnail.is_current());

        state.set_dearrow_titles_choice(false);
        assert!(branding.is_current());
        state.set_dearrow_thumbnails_choice(false);
        assert!(!branding.is_current());
        assert!(!next_thumbnail.is_current());
        state.set_dearrow_titles_choice(true);
        assert!(granted_consent(&mut state.dearrow.branding_consent).is_current());
        assert!(!branding.is_current());
    }

    #[test]
    fn display_title_and_thumbnail_url_follow_settings() {
        let mut state = State::default();
        let mut video = Video::from_id("dQw4w9WgXcQ".into());
        video.title = "CLICKBAIT!".into();
        state.settings.dearrow_titles = true;
        state.settings.dearrow_thumbnails = true;
        state.accept_dearrow(
            video.id.clone(),
            Branding {
                title: Some("Informative Title".into()),
                thumbnail: Some(Thumbnail::Timestamp(24.0)),
            },
        );
        assert_eq!(state.display_title(&video), "Informative Title");
        assert_eq!(
            state.dearrow_replacement_title(&video),
            Some("Informative Title")
        );
        assert_eq!(
            state.dearrow_thumbnail_url(&video).as_deref(),
            Some("https://dearrow-thumb.ajay.app/api/v1/getThumbnail?videoID=dQw4w9WgXcQ&time=24")
        );

        state.set_dearrow_thumbnails_choice(false);
        assert!(state.dearrow_thumbnail_url(&video).is_none());
        assert_eq!(state.display_title(&video), "Informative Title");

        state.set_dearrow_titles_choice(false);
        assert_eq!(state.display_title(&video), "CLICKBAIT!");
        assert!(state.dearrow.is_empty());
    }
}
