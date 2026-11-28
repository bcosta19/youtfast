//! Render the backend's connection-wide pause without owning request state.

use std::time::{Duration, Instant};

use egui::{Context, Frame, Margin, Ui};
use youtfast_app::{State, YouTubeRequestStatus};
use youtfast_i18n::{Locale, format as trf, gettext as tr};

const PAUSE: &str = "YouTube requests are paused to avoid a longer block.";
const RETRY_IN: &str = "Try again in {minutes} min";
const RETRY_TO_CHECK: &str = "Retry to check the connection.";
const CHECKING: &str = "Checking whether YouTube requests can resume…";

fn minutes_left(status: YouTubeRequestStatus, now: Instant) -> Option<(u64, Duration)> {
    let YouTubeRequestStatus::PausedUntil(until) = status else {
        return None;
    };
    let remaining = until
        .checked_duration_since(now)
        .filter(|time| !time.is_zero())?;
    let seconds = remaining
        .as_secs()
        .saturating_add(u64::from(remaining.subsec_nanos() != 0));
    let minutes = seconds.div_ceil(60);
    Some((
        minutes,
        remaining.saturating_sub(Duration::from_secs((minutes - 1) * 60)),
    ))
}

pub(super) fn retry_enabled(status: YouTubeRequestStatus, now: Instant) -> bool {
    status != YouTubeRequestStatus::Probing && minutes_left(status, now).is_none()
}

pub(super) fn retry_hint(ctx: &Context, state: &State, now: Instant) -> Option<String> {
    let locale = state.settings.language;
    if state.youtube_requests == YouTubeRequestStatus::Probing {
        return Some(tr(locale, CHECKING).into_owned());
    }
    let (minutes, repaint_after) = minutes_left(state.youtube_requests, now)?;
    ctx.request_repaint_after(repaint_after);
    Some(countdown(locale, minutes))
}

fn countdown(locale: Locale, minutes: u64) -> String {
    trf(locale, RETRY_IN, &[("minutes", minutes.to_string())])
}

pub(super) fn show(ui: &mut Ui, state: &State) {
    let locale = state.settings.language;
    let message = match state.youtube_requests {
        YouTubeRequestStatus::Ready => return,
        YouTubeRequestStatus::Probing => tr(locale, CHECKING).into_owned(),
        YouTubeRequestStatus::PausedUntil(_) => {
            let hint = retry_hint(ui.ctx(), state, Instant::now());
            let ending = hint
                .as_deref()
                .map_or_else(|| tr(locale, RETRY_TO_CHECK), std::borrow::Cow::Borrowed);
            format!("{} {ending}", tr(locale, PAUSE))
        }
    };
    egui::Panel::top("youtube-request-pause")
        .resizable(false)
        .frame(
            Frame::new()
                .fill(state.palette.panel)
                .inner_margin(Margin::symmetric(16, 6)),
        )
        .show(ui, |ui| {
            ui.label(message);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cooldown_countdown_rounds_up_and_enables_retry_at_the_deadline() {
        let until = Instant::now() + Duration::from_secs(120);
        let status = YouTubeRequestStatus::PausedUntil(until);
        let now = until - Duration::from_millis(60_001);
        assert_eq!(
            minutes_left(status, now),
            Some((2, Duration::from_millis(1)))
        );
        assert_eq!(
            minutes_left(status, until - Duration::from_nanos(1)),
            Some((1, Duration::from_nanos(1)))
        );
        assert!(!retry_enabled(status, now));
        assert!(retry_enabled(status, until));
        assert!(!retry_enabled(YouTubeRequestStatus::Probing, until));
    }

    #[test]
    fn every_interface_language_translates_the_pause_and_countdown() {
        for &locale in Locale::ALL
            .iter()
            .filter(|&&locale| locale != Locale::English)
        {
            for key in [PAUSE, RETRY_IN, RETRY_TO_CHECK, CHECKING] {
                assert_ne!(tr(locale, key), key, "missing {key} for {locale:?}");
            }
            let text = countdown(locale, 17);
            assert!(text.contains("17") && !text.contains("{minutes}"));
        }
    }
}
