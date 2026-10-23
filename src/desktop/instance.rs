//! One running YouTFast per user, through fastframe-instance.
//!
//! The crate holds a lock in the per-user runtime directory and serves the
//! private channel a later launch hands its request over. The handler runs
//! on the crate's thread, so it only parses and queues; `App` drains the
//! queue and acts.

use std::sync::{Arc, Mutex, OnceLock};

use fastframe_instance::Slot;

/// What another launch asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Remote {
    /// Shows the window.
    Show,
    /// Opens a link or runs a search.
    Open(String),
    /// Lists the palettes again (the Omarchy hook sends this).
    ReloadThemes,
}

/// Requests queued by later launches, drained by the app.
pub type Queue = Arc<Mutex<Vec<Remote>>>;

/// What a launch found when it asked for the slot.
pub enum Launch {
    /// This process is the running copy; keep the guard until it exits.
    Only(Guard),
    /// The running copy took the request.
    Handed,
    /// The running copy declined the request, or did not answer.
    Declined,
}

/// Marks this process as the running copy until it exits.
pub struct Guard {
    commands: Queue,
    /// The egui context, set once the app exists; requests that arrive
    /// earlier only wait in the queue.
    wake: Arc<OnceLock<egui::Context>>,
    _claim: fastframe_instance::Guard,
}

impl Guard {
    pub fn commands(&self) -> Queue {
        Arc::clone(&self.commands)
    }

    /// Called once the egui context exists, so later requests repaint.
    pub fn set_context(&self, ctx: &egui::Context) {
        let _ = self.wake.set(ctx.clone());
    }
}

/// The verbs another launch may send: `show`, `open <text>`,
/// `reload-themes`. Anything else is declined.
fn parse(request: &str) -> Option<Remote> {
    match request.split_once(' ') {
        None => match request {
            "show" => Some(Remote::Show),
            "reload-themes" => Some(Remote::ReloadThemes),
            _ => None,
        },
        Some(("open", text)) if !text.trim().is_empty() => {
            Some(Remote::Open(text.trim().to_owned()))
        }
        _ => None,
    }
}

/// Becomes the running copy, or hands `request` to the one already
/// running.
pub fn launch(request: &str) -> Launch {
    let commands = Queue::default();
    let wake = Arc::new(OnceLock::<egui::Context>::new());
    let (queue, waker) = (Arc::clone(&commands), Arc::clone(&wake));
    match Slot::new("youtfast").claim(request, move |request| {
        parse(request).map(|remote| {
            queue
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(remote);
            if let Some(ctx) = waker.get() {
                ctx.request_repaint();
            }
            "ok".to_owned()
        })
    }) {
        fastframe_instance::Claim::First(claim) => Launch::Only(Guard {
            commands,
            wake,
            _claim: claim,
        }),
        fastframe_instance::Claim::Running(_) => Launch::Handed,
        fastframe_instance::Claim::Declined | fastframe_instance::Claim::Unanswered => {
            Launch::Declined
        }
    }
}

/// Sends `request` to the running copy. Fails quietly when none runs.
pub fn send(request: &str) {
    match Slot::new("youtfast").send(request) {
        Ok(_) => {}
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
            ) =>
        {
            log::info!("no running copy answered '{request}'")
        }
        Err(error) => log::warn!("could not reach the running copy: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_our_own_verbs_are_understood() {
        assert_eq!(parse("show"), Some(Remote::Show));
        assert_eq!(parse("reload-themes"), Some(Remote::ReloadThemes));
        assert_eq!(
            parse("open https://youtu.be/aaaaaaaaaaa"),
            Some(Remote::Open("https://youtu.be/aaaaaaaaaaa".into()))
        );
        // A search keeps its spaces.
        assert_eq!(
            parse("open lofi beats to relax"),
            Some(Remote::Open("lofi beats to relax".into()))
        );
        assert_eq!(parse("open  padded  "), Some(Remote::Open("padded".into())));
        for unknown in ["", "open", "open ", "frobnicate", "show extra"] {
            assert_eq!(parse(unknown), None, "{unknown}");
        }
    }
}
