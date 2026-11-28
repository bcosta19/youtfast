//! One circuit for YouTube requests from this process, independent of account.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::watch;

const INITIAL: Duration = Duration::from_secs(2 * 60);
const MAX: Duration = Duration::from_secs(60 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestIntent {
    Background,
    UserInitiated,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Ready,
    PausedUntil(Instant),
    Probing,
}

#[derive(Debug)]
pub struct Paused;

#[derive(Default)]
struct Circuit {
    status: Status,
    generation: u64,
    delay: Duration,
    expired_deadline: Option<Instant>,
}

impl Circuit {
    fn begin(&mut self, now: Instant, intent: RequestIntent) -> Result<u64, Paused> {
        match self.status {
            Status::Ready => (),
            Status::PausedUntil(until)
                if now >= until && intent == RequestIntent::UserInitiated =>
            {
                self.generation += 1;
                self.expired_deadline = Some(until);
                self.status = Status::Probing;
            }
            _ => return Err(Paused),
        }
        Ok(self.generation)
    }

    fn limited(&mut self, request: u64, now: Instant, retry_after: Option<Duration>) {
        if request != self.generation {
            return;
        }
        let retry_until = retry_after.and_then(|delay| now.checked_add(delay));
        if let Status::PausedUntil(until) = self.status {
            // In-flight requests can report the same block; only a longer
            // server delay matters, not how many requests were in flight.
            if let Some(retry_until) = retry_until.filter(|retry| *retry > until) {
                self.status = Status::PausedUntil(retry_until);
            }
            return;
        }
        self.delay = (self.delay * 2).clamp(INITIAL, MAX);
        self.status = Status::PausedUntil((now + self.delay).max(retry_until.unwrap_or(now)));
        self.expired_deadline = None;
    }

    fn finish(&mut self, request: u64, success: bool) {
        if request != self.generation || self.status != Status::Probing {
            return;
        }
        self.status = if success {
            self.delay = Duration::ZERO;
            Status::Ready
        } else {
            Status::PausedUntil(self.expired_deadline.expect("probe has a prior deadline"))
        };
        self.expired_deadline = None;
    }
}

#[derive(Default)]
struct Shared {
    circuit: Mutex<Circuit>,
    status: watch::Sender<Status>,
}

#[derive(Clone, Default)]
pub struct RequestGate(Arc<Shared>);

impl RequestGate {
    fn update<T>(&self, f: impl FnOnce(&mut Circuit) -> T) -> T {
        let mut circuit = self.0.circuit.lock().unwrap();
        let before = circuit.status;
        let result = f(&mut circuit);
        if before != circuit.status {
            self.0.status.send_replace(circuit.status);
        }
        result
    }

    pub fn status(&self) -> Status {
        *self.0.status.borrow()
    }

    pub fn subscribe(&self) -> watch::Receiver<Status> {
        self.0.status.subscribe()
    }

    pub fn begin(&self, intent: RequestIntent) -> Result<RequestPermit, Paused> {
        self.begin_at(intent, Instant::now())
    }

    pub fn begin_at(&self, intent: RequestIntent, now: Instant) -> Result<RequestPermit, Paused> {
        let (generation, probe) = self.update(|circuit| {
            let generation = circuit.begin(now, intent)?;
            Ok((generation, circuit.status == Status::Probing))
        })?;
        Ok(RequestPermit {
            gate: self.clone(),
            generation,
            pending: true,
            probe,
        })
    }
}

/// A half-open slot is released on cancellation without pretending the request succeeded.
#[must_use]
pub struct RequestPermit {
    gate: RequestGate,
    generation: u64,
    pending: bool,
    probe: bool,
}

impl RequestPermit {
    pub fn is_probe(&self) -> bool {
        self.probe
    }

    pub fn success(mut self) {
        self.mark_success();
    }

    /// An already loaded stream can still report a limiting failure.
    pub fn mark_success(&mut self) {
        self.gate
            .update(|circuit| circuit.finish(self.generation, true));
        self.pending = false;
    }

    pub fn limited(mut self, retry_after: Option<Duration>) {
        self.gate.update(|circuit| {
            circuit.limited(self.generation, Instant::now(), retry_after);
        });
        self.pending = false;
    }
}

impl Drop for RequestPermit {
    fn drop(&mut self) {
        if self.pending {
            self.gate
                .update(|circuit| circuit.finish(self.generation, false));
        }
    }
}

/// Accepts delta seconds or an HTTP-date (IMF-fixdate).
pub(crate) fn retry_after(value: &str, now: jiff::Timestamp) -> Option<Duration> {
    let value = value.trim();
    if let Ok(seconds) = value.parse() {
        return Some(Duration::from_secs(seconds));
    }
    let date = jiff::civil::DateTime::strptime("%a, %d %b %Y %H:%M:%S GMT", value).ok()?;
    let at = date.to_zoned(jiff::tz::TimeZone::UTC).ok()?.timestamp();
    Some(Duration::from_secs(
        (at.as_second() - now.as_second()).max(0) as u64,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use RequestIntent::{Background, UserInitiated};

    #[test]
    fn only_one_explicit_request_probes_after_the_cooldown_and_success_resets_it() {
        let mut circuit = Circuit::default();
        let now = Instant::now();
        let first = circuit.begin(now, Background).unwrap();
        circuit.limited(first, now, None);
        let until = now + INITIAL;
        assert_eq!(circuit.status, Status::PausedUntil(until));
        assert!(
            circuit
                .begin(until - Duration::from_secs(1), UserInitiated)
                .is_err()
        );
        assert!(circuit.begin(until, Background).is_err());
        let probe = circuit.begin(until, UserInitiated).unwrap();
        assert!(circuit.begin(until, UserInitiated).is_err());
        circuit.finish(first, true);
        assert_eq!(circuit.status, Status::Probing);
        circuit.finish(probe, true);
        assert_eq!(circuit.status, Status::Ready);
        let next = circuit.begin(until, Background).unwrap();
        circuit.limited(next, until, None);
        assert_eq!(circuit.status, Status::PausedUntil(until + INITIAL));
    }

    #[test]
    fn probe_limits_double_to_an_hour_and_retry_after_can_extend_the_deadline() {
        let mut circuit = Circuit::default();
        let mut now = Instant::now();
        for minutes in [2, 4, 8, 16, 32, 60, 60] {
            let first = circuit.begin(now, UserInitiated).unwrap();
            circuit.limited(first, now, Some(Duration::from_secs(30)));
            let until = now + Duration::from_secs(60 * minutes);
            assert_eq!(circuit.status, Status::PausedUntil(until));
            now = until;
        }
        let probe = circuit.begin(now, UserInitiated).unwrap();
        let longer = Duration::from_secs(90 * 60);
        circuit.limited(probe, now, Some(longer));
        assert_eq!(circuit.status, Status::PausedUntil(now + longer));
    }

    #[test]
    fn cancelled_and_stale_requests_do_not_close_or_wedge_the_circuit() {
        let mut circuit = Circuit::default();
        let now = Instant::now();
        let first = circuit.begin(now, Background).unwrap();
        let parallel = circuit.begin(now, Background).unwrap();
        circuit.limited(first, now, None);
        circuit.limited(parallel, now, None);
        assert_eq!(circuit.delay, INITIAL);
        let longer = Duration::from_secs(5 * 60);
        circuit.limited(parallel, now, Some(longer));
        let until = now + longer;
        assert_eq!(circuit.status, Status::PausedUntil(until));
        let probe = circuit.begin(until, UserInitiated).unwrap();
        circuit.limited(parallel, until, None);
        assert_eq!(circuit.status, Status::Probing);
        circuit.finish(probe, false);
        assert!(circuit.begin(until, Background).is_err());
        assert!(circuit.begin(until, UserInitiated).is_ok());
    }
}
