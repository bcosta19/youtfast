//! Alerts: checking the subscriptions for new uploads, and the
//! Notifications page's visit.

use super::*;

impl App {
    /// The check the timer (or `[ check now ]`) runs: the feed's sources
    /// again, on its own slot so Home's list is undisturbed.
    pub(super) fn check_alerts(&mut self) {
        if !self.state.settings.alerts || self.state.alerts_checking {
            return;
        }
        self.state.alerts_checking = true;
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
        let changed = self.state.library.alerts_seen_at != Some(found.seen_at)
            || self.state.library.alerts_notified_at != found.notified_at;
        self.state.library.alerts_seen_at = Some(found.seen_at);
        self.state.library.alerts_notified_at = found.notified_at;
        if changed {
            self.save_library();
        }
        // `new` is everything unseen since the last visit; replace, so a
        // second check does not repeat the badge's rows.
        self.state.alerts = found.new;
        if self.state.settings.desktop_notifications
            && self.state.settings.alerts
            && !found.to_notify.is_empty()
        {
            self.backend.send(Command::Notify {
                videos: found.to_notify,
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
        self.state.settings.alerts_every_minutes = crate::settings::clamp_alerts_every(minutes);
        self.save_settings();
        self.set_alerts_timer();
    }

    /// Opening the Notifications page marks it all seen, but the rows stay
    /// drawn for this visit.
    pub(super) fn mark_notifications_visited(&mut self) {
        self.state.alerts_visit = self.state.alerts.clone();
        self.state.alerts.clear();
        if self.state.library.alerts_seen_at != Some(util::now()) {
            self.state.library.alerts_seen_at = Some(util::now());
            self.save_library();
        }
    }
}
