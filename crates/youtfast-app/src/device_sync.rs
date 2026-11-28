//! Peer-to-peer device library sync actions, debouncing, and merge application.

use super::*;
use youtfast_storage::sync::{
    MAX_PAIRING_KEY_CHARS, MAX_PEER_ADDRESS_CHARS, MIN_SYNC_PORT, SyncSnapshot,
    generate_pairing_key, local_sync_endpoints, normalize_pairing_key,
};

pub(super) const SYNC_DEBOUNCE: Duration = Duration::from_secs(2);

impl State {
    /// Refreshes the list of reachable VPN and local IPv4 addresses shown for
    /// this computer in Settings.
    pub fn refresh_sync_addresses(&mut self) {
        self.sync_addresses = local_sync_endpoints();
    }
}

impl App {
    pub(super) fn set_device_sync_enabled(&mut self, enabled: bool) {
        if self.state.settings.device_sync.enabled == enabled {
            return;
        }
        self.state.settings.device_sync.enabled = enabled;
        if enabled {
            if normalize_pairing_key(&self.state.settings.device_sync.pairing_key).is_none() {
                self.state.settings.device_sync.pairing_key = generate_pairing_key();
            }
            self.state.refresh_sync_addresses();
            self.backend
                .send(Command::SaveLibrary(Box::new(self.state.library.clone())));
        } else {
            self.sync_dirty_since = None;
        }
        self.reconfigure_device_sync();
    }

    pub(super) fn set_device_sync_peer(&mut self, peer: String) {
        self.state.settings.device_sync.peer_address =
            truncate_chars(&peer, MAX_PEER_ADDRESS_CHARS).to_owned();
        self.reconfigure_device_sync();
    }

    pub(super) fn set_device_sync_key(&mut self, key: String) {
        self.state.settings.device_sync.pairing_key =
            truncate_chars(&key, MAX_PAIRING_KEY_CHARS).to_owned();
        self.reconfigure_device_sync();
    }

    pub(super) fn generate_device_sync_key(&mut self) {
        self.state.settings.device_sync.pairing_key = generate_pairing_key();
        self.reconfigure_device_sync();
    }

    pub(super) fn set_device_sync_port(&mut self, port: u16) {
        if port < MIN_SYNC_PORT || self.state.settings.device_sync.port == port {
            self.state.sync_port_draft = self.state.settings.device_sync.port.to_string();
            return;
        }
        self.state.settings.device_sync.port = port;
        self.state.sync_port_draft = port.to_string();
        self.reconfigure_device_sync();
    }

    pub(super) fn set_device_sync_every(&mut self, minutes: u32) {
        let minutes = youtfast_storage::settings::clamp_alerts_every(minutes);
        if self.state.settings.device_sync.every_minutes != minutes {
            self.state.settings.device_sync.every_minutes = minutes;
            self.reconfigure_device_sync();
        }
    }

    fn reconfigure_device_sync(&mut self) {
        self.device_sync_generation = self.device_sync_generation.wrapping_add(1);
        self.state.sync_status = SyncStatus::Idle;
        self.save_settings();
        self.configure_device_sync();
        // Sync shortly after the setup settles, so a new computer pulls the
        // other devices' library as soon as its key matches.
        if self.state.settings.device_sync.enabled {
            self.sync_dirty_since = Some(Instant::now());
            self.backend.context().request_repaint_after(SYNC_DEBOUNCE);
        }
    }

    pub(super) fn configure_device_sync(&self) {
        self.backend.send(Command::ConfigureDeviceSync {
            settings: self.state.settings.device_sync.clone(),
            generation: self.device_sync_generation,
        });
    }

    pub(super) fn device_sync_is_current(&self, generation: u64) -> bool {
        self.state.settings.device_sync.enabled && self.device_sync_generation == generation
    }

    /// Syncs with the address in Settings or, without one, with the devices
    /// found on Tailscale and NetBird.
    pub(super) fn sync_devices_now(&mut self, manual: bool) {
        if !self.state.settings.device_sync.enabled
            || matches!(self.state.sync_status, SyncStatus::Syncing)
        {
            return;
        }
        if !manual && normalize_pairing_key(&self.state.settings.device_sync.pairing_key).is_none()
        {
            return;
        }
        self.sync_dirty_since = None;
        self.state.sync_status = SyncStatus::Syncing;
        self.backend
            .send(Command::SaveLibrary(Box::new(self.state.library.clone())));
        self.backend.send(Command::SyncDevices {
            generation: self.device_sync_generation,
            manual,
        });
    }

    pub(super) fn device_sync_tick(&mut self) {
        if self
            .sync_dirty_since
            .is_some_and(|since| since.elapsed() >= SYNC_DEBOUNCE)
            && !matches!(self.state.sync_status, SyncStatus::Syncing)
        {
            self.sync_dirty_since = None;
            self.sync_devices_now(false);
        }
    }

    pub(super) fn on_device_sync_completed(
        &mut self,
        result: Result<Vec<SyncSnapshot>, String>,
        manual: bool,
    ) {
        match result {
            Ok(snapshots) => {
                for snapshot in snapshots {
                    self.apply_sync_snapshot(snapshot);
                }
                self.state.sync_status = SyncStatus::Synced(util::now());
                if manual {
                    self.state.notice = Some("Devices synced.".to_owned());
                }
            }
            Err(error) => {
                self.state.sync_status = SyncStatus::Failed(error.clone());
                if manual {
                    self.state.notice = Some(error);
                }
            }
        }
    }

    pub(super) fn apply_sync_snapshot(&mut self, snapshot: SyncSnapshot) {
        let outcome = self.state.library.merge_sync(snapshot);
        if !outcome.changed {
            return;
        }
        self.recheck_topic_search_history();
        self.library_revision.set(self.library_revision.get() + 1);
        // Save merged library directly without re-marking sync_dirty_since.
        self.backend
            .send(Command::SaveLibrary(Box::new(self.state.library.clone())));

        if outcome.subscriptions_changed {
            self.state.feed = Listing::Idle;
        }
        if outcome.subscriptions_changed
            || outcome.filters_changed
            || (outcome.history_changed && self.state.for_you_source != Some(Source::Account))
        {
            self.state.invalidate_local_for_you(&mut self.generations);
            self.load_visible_for_you();
        }
        if outcome.subscriptions_changed && self.state.view == View::Home {
            self.load(View::Home, false);
        }
        if outcome.playlists_changed
            && let Some(Page::Playlist { id, .. }) = &self.state.page
            && DevicePlaylist::is_local_id(id)
        {
            let id = id.clone();
            self.sync_local_playlist_page(&id);
        }
    }
}

fn truncate_chars(input: &str, max_chars: usize) -> &str {
    input
        .char_indices()
        .nth(max_chars)
        .map_or(input, |(end, _)| &input[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_status_and_library_merge_update_state_and_local_playlist_page() {
        let mut remote = Library::default();
        let pl_id = remote.create_playlist("Synced Playlist").unwrap();
        let video = Video {
            title: "Synced Video".into(),
            ..Video::from_id("syncvid0001".into())
        };
        remote.add_to_playlist(&pl_id, video.clone());
        remote.record_watch(video.clone(), 200);
        remote.set_position(&video.id, 95.0, 600.0);

        let mut state = State {
            view: View::Playlist,
            page: Some(Page::Playlist {
                id: pl_id.clone(),
                name: "Old".into(),
            }),
            ..Default::default()
        };

        let outcome = state.library.merge_sync(remote.sync_snapshot(38452));
        assert!(outcome.changed);
        assert!(outcome.history_changed);
        assert!(outcome.playlists_changed);
        assert!(state.library.watched(&video.id));
        assert_eq!(state.library.resume_position(&video.id), Some(95.0));
        assert!(state.library.in_playlist(&pl_id, &video.id));
    }
}
