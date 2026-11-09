//! Alerts: checking the subscriptions for new uploads, and the
//! Notifications page's visit.

use super::*;

/// How fresh the feed must be for a check to reuse it instead of asking
/// its sources again: the timer's first check fires a minute after the
/// feed landed.
const ALERTS_REUSE_FEED: i64 = 15 * 60;

impl State {
    pub(super) fn alerts_failed(&mut self, error: String) {
        self.alerts_checking = false;
        self.notice = Some(format!("Could not check for uploads: {error}"));
    }

    fn visit_alerts(&mut self, now: i64) -> bool {
        self.alerts_visit = std::mem::take(&mut self.alerts);
        let previous = self.library.alerts_seen_at.unwrap_or(now);
        let newest = self
            .alerts_visit
            .iter()
            .filter_map(|video| video.published_at)
            .max();
        let seen_at = previous.max(newest.unwrap_or(previous).min(now));
        let changed = self.library.alerts_seen_at != Some(seen_at);
        self.library.alerts_seen_at = Some(seen_at);
        changed
    }

    pub(super) fn visit_alerts_popup(&mut self, now: i64) -> bool {
        if self.view == View::Notifications {
            return false;
        }
        self.visit_alerts(now)
    }

    /// Keep arrivals visible during a visit; otherwise leave them unread.
    fn receive_alerts(&mut self, mut new: Vec<Video>, now: i64) -> bool {
        if new.is_empty() || (!self.notifications_popup_open && self.view != View::Notifications) {
            self.alerts = new;
            return false;
        }

        let newest = new.iter().filter_map(|video| video.published_at).max();
        let ids: HashSet<&str> = new.iter().map(|video| video.id.as_str()).collect();
        self.alerts_visit
            .retain(|video| !ids.contains(video.id.as_str()));
        new.append(&mut self.alerts_visit);
        self.alerts_visit = new;
        self.alerts.clear();
        let previous = self.library.alerts_seen_at.unwrap_or(now);
        let seen_at = previous.max(newest.unwrap_or(previous).min(now));
        let changed = self.library.alerts_seen_at != Some(seen_at);
        self.library.alerts_seen_at = Some(seen_at);
        changed
    }
}

impl App {
    /// The check the timer runs: the feed's sources again, on its own
    /// slot so Home's list is undisturbed — unless the feed itself just
    /// loaded, which already is that answer.
    pub(super) fn check_alerts(&mut self) {
        if !self.state.settings.alerts || self.state.alerts_checking {
            return;
        }
        let fresh = self
            .state
            .feed_loaded_at
            .is_some_and(|at| util::now() - at < ALERTS_REUSE_FEED);
        if fresh && matches!(self.state.feed, Listing::Ready(_)) {
            let mut videos = self.state.feed.items().to_vec();
            videos.extend(self.state.shorts.items().iter().cloned());
            self.note_alerts(videos);
            return;
        }
        self.state.alerts_checking = true;
        self.ask_alerts();
    }

    /// `[ check now ]`'s check: always asks the sources again.
    pub(super) fn check_alerts_now(&mut self) {
        if !self.state.settings.alerts || self.state.alerts_checking {
            return;
        }
        self.state.alerts_checking = true;
        self.ask_alerts();
    }

    /// Asks the feed's sources for a check, on the alerts' own slot.
    fn ask_alerts(&mut self) {
        let account = self.state.signed_in();
        let channels = self.state.device_only_subscriptions();
        self.send_request(Slot::Alerts, Request::Alerts { channels, account });
    }

    /// What a check (or the feed's own load) found: keep the unseen,
    /// notify the rest, stamp the library.
    pub(super) fn note_alerts(&mut self, videos: Vec<Video>) {
        self.state.alerts_checking = false;
        self.state.last_checked = Some(util::now());
        let Some(found) = alerts::compute(
            videos,
            self.state.library.alerts_seen_at,
            self.state.library.alerts_notified_at,
            util::now(),
        ) else {
            return;
        };
        let mut changed = self.state.library.alerts_seen_at != Some(found.seen_at)
            || self.state.library.alerts_notified_at != found.notified_at;
        self.state.library.alerts_seen_at = Some(found.seen_at);
        self.state.library.alerts_notified_at = found.notified_at;
        changed |= self.state.receive_alerts(found.new, util::now());
        if changed {
            self.save_library();
        }
        if self.state.settings.desktop_notifications
            && self.state.settings.alerts
            && !found.to_notify.is_empty()
        {
            self.backend.send(Command::Notify {
                videos: found.to_notify,
                language: self.state.settings.language,
            });
        }
    }

    /// Starts or stops the alerts timer to match the settings.
    pub(super) fn set_alerts_timer(&self) {
        let minutes = if self.state.settings.alerts {
            self.state.settings.alerts_every_minutes
        } else {
            0
        };
        self.backend.send(Command::SetAlertsEvery(minutes));
    }

    /// Marks every alert seen and empties the Notifications page.
    pub(super) fn mark_alerts_seen(&mut self) {
        self.state.alerts.clear();
        self.state.alerts_visit.clear();
        self.state.library.alerts_seen_at = Some(util::now());
        self.save_library();
    }

    /// Turns the alerts check on or off.
    pub(super) fn set_alerts(&mut self, on: bool) {
        self.state.settings.alerts = on;
        self.save_settings();
        self.set_alerts_timer();
    }

    /// Sets how often the alerts check runs, clamped to the offered choices.
    pub(super) fn set_alerts_every(&mut self, minutes: u32) {
        self.state.settings.alerts_every_minutes =
            youtfast_storage::settings::clamp_alerts_every(minutes);
        self.save_settings();
        self.set_alerts_timer();
    }

    /// Opening the page or popup marks alerts seen, but keeps this visit's rows.
    pub(super) fn mark_notifications_visited(&mut self) {
        if self.state.visit_alerts(util::now()) {
            self.save_library();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(id: &str) -> Video {
        Video::from_id(id.to_owned())
    }

    #[test]
    fn popup_on_notifications_page_preserves_the_current_visit() {
        let mut state = State {
            view: View::Notifications,
            alerts_visit: vec![video("already-seen")],
            ..State::default()
        };

        assert!(!state.visit_alerts_popup(100));
        assert_eq!(state.alerts_visit[0].id, "already-seen");
        assert_eq!(state.library.alerts_seen_at, None);
    }

    #[test]
    fn opening_before_a_check_does_not_discard_unfetched_uploads() {
        let mut state = State {
            notifications_popup_open: true,
            ..State::default()
        };
        state.library.alerts_seen_at = Some(50);
        assert!(!state.visit_alerts_popup(100));
        let mut upload = video("not-yet-fetched");
        upload.published_at = Some(90);
        let found = alerts::compute(vec![upload], state.library.alerts_seen_at, None, 110)
            .expect("the check has an existing watermark");

        assert!(state.receive_alerts(found.new, 110));
        assert_eq!(state.alerts_visit[0].id, "not-yet-fetched");
        assert_eq!(state.library.alerts_seen_at, Some(90));
    }

    #[test]
    fn arrivals_update_an_open_visit_without_duplicates() {
        let mut state = State {
            notifications_popup_open: true,
            alerts_visit: vec![video("old"), video("duplicate")],
            ..State::default()
        };

        assert!(state.receive_alerts(vec![video("new"), video("duplicate")], 100));
        let ids: Vec<_> = state
            .alerts_visit
            .iter()
            .map(|video| video.id.as_str())
            .collect();
        assert_eq!(ids, ["new", "duplicate", "old"]);
        assert!(state.alerts.is_empty());
        assert_eq!(state.library.alerts_seen_at, Some(100));

        state.notifications_popup_open = false;
        assert!(!state.receive_alerts(vec![video("unread")], 200));
        assert_eq!(state.alerts[0].id, "unread");
        assert_eq!(state.library.alerts_seen_at, Some(100));
    }
}
