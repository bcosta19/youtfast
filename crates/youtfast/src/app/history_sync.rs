//! A report belongs to one playback and the session in which it started.
use std::time::Duration;

use crate::storage::settings::HistorySync;

#[derive(Default)]
pub(super) struct PlaybackReport {
    session: Option<u64>,
    elapsed: Duration,
    sent: bool,
}

impl PlaybackReport {
    pub fn cancel(&mut self) {
        self.session = None;
        self.sent = true;
    }

    pub fn started(&mut self, session: u64, signed_in: bool, mode: HistorySync) {
        // Restarts after seeking, buffering, quality changes or a Short's loop
        // do not create another playback.
        if self.session.is_none() && !self.sent {
            self.session = (signed_in && mode == HistorySync::SendToYoutube).then_some(session);
            self.sent = self.session.is_none();
        }
    }

    pub fn advance(
        &mut self,
        session: u64,
        signed_in: bool,
        mode: HistorySync,
        playing: bool,
        elapsed: Duration,
    ) -> bool {
        if self.sent || self.session.is_none() {
            return false;
        }
        if self.session != Some(session) || !signed_in || mode != HistorySync::SendToYoutube {
            self.sent = true;
            return false;
        }
        if playing {
            // Two seconds of active playback excludes Shorts passed over
            // quickly. Cap frame gaps rather than counting suspension as play.
            self.elapsed += elapsed.min(Duration::from_millis(250));
        }
        if self.elapsed >= Duration::from_secs(2) {
            self.sent = true;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(
        report: &mut PlaybackReport,
        session: u64,
        signed_in: bool,
        mode: HistorySync,
        playing: bool,
    ) -> usize {
        (0..12)
            .filter(|_| {
                report.advance(
                    session,
                    signed_in,
                    mode,
                    playing,
                    Duration::from_millis(250),
                )
            })
            .count()
    }

    #[test]
    fn only_started_signed_in_opted_in_playback_reports_once() {
        let mut report = PlaybackReport::default();
        assert_eq!(
            run(&mut report, 1, true, HistorySync::SendToYoutube, true),
            0
        );
        for mode in [
            HistorySync::Separate,
            HistorySync::Merged,
            HistorySync::SendToYoutube,
        ] {
            for signed_in in [false, true] {
                let mut report = PlaybackReport::default();
                report.started(1, signed_in, mode);
                assert_eq!(run(&mut report, 1, signed_in, mode, false), 0);
                assert_eq!(
                    run(&mut report, 1, signed_in, mode, true),
                    usize::from(signed_in && mode == HistorySync::SendToYoutube)
                );
                report.started(1, signed_in, mode);
                assert_eq!(run(&mut report, 1, signed_in, mode, true), 0);
            }
        }
    }

    #[test]
    fn canceled_playback_cannot_bind_to_a_new_account_even_before_started() {
        let mode = HistorySync::SendToYoutube;
        for already_started in [false, true] {
            let mut report = PlaybackReport::default();
            if already_started {
                report.started(1, true, mode);
            }
            report.cancel();
            report.started(2, true, mode);
            assert_eq!(run(&mut report, 2, true, mode, true), 0);
        }
        let mut next = PlaybackReport::default();
        next.started(2, true, mode);
        assert_eq!(run(&mut next, 2, true, mode, true), 1);
    }

    #[test]
    fn scroll_by_session_change_or_opting_out_cannot_report() {
        let mode = HistorySync::SendToYoutube;
        let mut report = PlaybackReport::default();
        report.started(1, true, mode);
        assert!(!report.advance(1, true, mode, true, Duration::from_secs(1)));
        assert_eq!(run(&mut report, 2, true, mode, true), 0);
        let mut report = PlaybackReport::default();
        report.started(1, true, mode);
        assert_eq!(run(&mut report, 1, true, HistorySync::Merged, true), 0);
        assert_eq!(run(&mut report, 1, true, mode, true), 0);
    }
}
