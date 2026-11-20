//! The player's own controls: speed, mute, captions, the queue and what
//! plays next, live streams' edge, SponsorBlock's skips, the mini player,
//! and the search field's suggestions.

use super::*;

/// How long a note stays over the video.
const OSD_FOR: Duration = Duration::from_millis(900);
/// The countdown before the next video plays.
const AUTOPLAY_AFTER: Duration = Duration::from_secs(8);
/// How long typing pauses before suggestions are asked for.
const SUGGEST_AFTER: Duration = Duration::from_millis(250);
/// Further behind its edge than this, a live stream offers to go back to
/// it. mpv plays some 10 to 25 seconds behind the newest segment.
pub const LIVE_BEHIND: f64 = 40.0;
/// The speeds the menu and `<`/`>` step through.
pub const SPEEDS: [f64; 8] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];

/// The languages yt-dlp is asked for captions in: the kept one, the
/// desktop's, then English, as base codes ("pt").
pub fn caption_languages(settings: &Settings) -> Vec<String> {
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()));
    languages(settings.captions.as_deref(), locale.as_deref())
}

fn languages(kept: Option<&str>, locale: Option<&str>) -> Vec<String> {
    let mut languages = Vec::new();
    // "pt_BR.UTF-8" and "pt-BR" both come down to "pt"; "C" is no language.
    let locale = locale.filter(|locale| !matches!(*locale, "C" | "POSIX"));
    for code in [kept, locale, Some("en")].into_iter().flatten() {
        let base = base_language(code);
        if !base.is_empty() && !languages.contains(&base) {
            languages.push(base);
        }
    }
    languages
}

/// "pt" for "pt-BR", "pt_BR.UTF-8", "pt-en" (Portuguese from English) and
/// "pt-orig".
pub fn base_language(code: &str) -> String {
    code.split(['-', '_', '.'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// The captions track to show for `language`: the uploader's own first,
/// then the automatic ones in the video's language, then a translation.
pub fn pick_subtitle(subtitles: &[mpv::Subtitle], language: &str) -> Option<i64> {
    let base = base_language(language);
    subtitles
        .iter()
        .filter(|subtitle| base_language(&subtitle.lang) == base)
        .min_by_key(|subtitle| {
            if subtitle.lang.ends_with("-orig") {
                1
            } else if subtitle.title.contains(" from ") {
                2
            } else {
                0
            }
        })
        .map(|subtitle| subtitle.id)
}

/// The speed after `speed` in [`SPEEDS`], one step `up` or down.
pub fn step_speed(speed: f64, up: bool) -> f64 {
    let index = SPEEDS
        .iter()
        .position(|step| (step - speed).abs() < 0.01)
        .unwrap_or(3);
    let index = if up {
        (index + 1).min(SPEEDS.len() - 1)
    } else {
        index.saturating_sub(1)
    };
    SPEEDS[index]
}

/// "1×", "1.25×".
pub fn speed_label(speed: f64) -> String {
    let text = format!("{speed:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text}×")
}

/// Whether `status` is currently at the live edge of a live stream.
pub fn at_live_edge(watching_live: bool, status: &mpv::Status) -> bool {
    if !watching_live {
        return false;
    }
    let edge = status.duration.max(status.cached_until);
    edge <= 0.0 || edge - status.position <= LIVE_BEHIND
}

/// What plays after `watching`: the queue's first, else the video after it
/// in Up next (or Up next's first when it is not there), else the first
/// related video.
pub fn next_video<'a>(
    watching: Option<&Video>,
    queue: &'a [Video],
    up_next: &'a [Video],
    related: &'a [Video],
) -> Option<&'a Video> {
    if let Some(first) = queue.first() {
        return Some(first);
    }
    let id = watching.map(|video| video.id.as_str());
    let shorts = watching.is_some_and(|video| video.short);
    let suitable = |video: &&Video| !shorts || video.short;
    let after = match up_next
        .iter()
        .position(|video| Some(video.id.as_str()) == id)
    {
        Some(index) => up_next.iter().skip(index + 1).find(suitable),
        None => up_next.iter().find(suitable),
    };
    after.or_else(|| {
        related
            .iter()
            .find(|video| Some(video.id.as_str()) != id && (!shorts || video.short))
    })
}

/// The item before the current video in its source list. All controls use
/// this same lookup, including the desktop's media controls.
pub fn previous_video<'a>(watching: Option<&Video>, up_next: &'a [Video]) -> Option<&'a Video> {
    let watching = watching?;
    let id = &watching.id;
    let index = up_next.iter().position(|video| &video.id == id)?;
    up_next[..index]
        .iter()
        .rev()
        .find(|video| !watching.short || video.short)
}

/// Move one queued item without duplicating or dropping any other item.
fn reorder_queue(queue: &mut Vec<Video>, from: usize, to: usize) {
    if from < queue.len() && to < queue.len() && from != to {
        let video = queue.remove(from);
        queue.insert(to, video);
    }
}

impl App {
    pub(super) fn move_queue(&mut self, from: usize, to: usize) {
        reorder_queue(&mut self.state.queue, from, to);
        self.state.refresh_autoplay();
    }

    /// Retry the same video without making a second history entry or
    /// replacing its queue/source list.
    pub(super) fn retry_playback(&mut self) {
        self.local_playback_generation += 1;
        let Some(video) = &self.state.watching else {
            return;
        };
        if let Some(surface) = &self.surface {
            surface.forget();
        }
        if let Some(player) = &mut self.player {
            let position = player.status().position.max(0.0);
            self.state.player_error = player
                .reload(&video.url(), position, false)
                .err()
                .map(|error| error.to_string());
        }
    }

    pub(super) fn toggle_loop(&mut self) {
        self.state.playback_loop = !self.state.playback_loop;
        let looping = self.state.playback_loop || self.state.watching_short();
        self.with_player(|player| player.set_looping(looping));
    }

    pub(super) fn set_caption_scale(&mut self, scale: f64) {
        self.state.settings.caption_scale = scale.clamp(0.5, 2.0);
        self.apply_caption_style();
    }

    pub(super) fn set_caption_background(&mut self, background: bool) {
        self.state.settings.caption_background = background;
        self.apply_caption_style();
    }

    fn apply_caption_style(&mut self) {
        let scale = self.state.settings.caption_scale;
        let background = self.state.settings.caption_background;
        self.with_player(|player| player.set_caption_style(scale, background));
        self.save_settings();
    }

    pub(super) fn show_osd(&mut self, text: String) {
        self.state.osd = Some((text, Instant::now() + OSD_FOR));
    }

    pub(super) fn set_speed(&mut self, speed: f64) {
        let live_edge = match &self.player {
            Some(player) => at_live_edge(self.state.watching_live(), player.status()),
            None => self.state.watching_live(),
        };
        let (applied, save) = self.state.record_speed(speed, live_edge);
        self.speed_manual = true;
        self.with_player(|player| player.set_speed(applied));
        self.show_osd(speed_label(applied));
        if save {
            self.save_settings();
        }
    }

    pub(super) fn set_temporary_speed(&mut self, speed: f64) {
        let live_edge = self
            .player
            .as_ref()
            .is_some_and(|player| at_live_edge(self.state.watching_live(), player.status()));
        let speed = if live_edge {
            1.0
        } else {
            youtfast_storage::settings::sanitize_speed(speed)
        };
        self.with_player(|player| player.set_speed(speed));
    }

    pub(super) fn set_remember_channel_speed(&mut self, remember: bool) {
        let current_speed = self
            .player
            .as_ref()
            .map_or(self.state.settings.playback_speed, |player| {
                player.status().speed
            });
        if self
            .state
            .set_remember_channel_speed(remember, current_speed)
        {
            self.save_settings();
        }
    }

    pub(super) fn toggle_mute(&mut self) {
        let Some(status) = self.player.as_ref().map(|player| player.status()) else {
            return;
        };
        let unmute = self.state.prepare_manual_unmute(status.position, status.muted);
        self.with_player(|player| player.set_muted(!unmute));
        self.show_osd(if unmute { "Unmuted" } else { "Muted" }.to_owned());
    }

    pub(super) fn seek_fraction(&mut self, fraction: f64) {
        let Some(status) = self.player.as_ref().map(Mpv::status) else {
            return;
        };
        if status.duration > 0.0 && !self.state.watching_live() {
            let seconds = status.duration * fraction.clamp(0.0, 1.0);
            self.apply_seek(seconds);
        }
    }

    fn apply_seek(&mut self, seconds: f64) {
        if self
            .player
            .as_ref()
            .is_some_and(|player| player.status().seekable)
        {
            self.with_player(|player| player.seek_to(seconds, true));
            self.media.seeked(seconds);
        }
    }

    /// Shows a captions track and keeps its language for the next videos.
    pub(super) fn set_subtitle(&mut self, id: Option<i64>) {
        self.with_player(|player| player.set_subtitle(id));
        let lang = id.and_then(|id| {
            self.player.as_ref().and_then(|player| {
                player
                    .status()
                    .subtitles
                    .iter()
                    .find(|subtitle| subtitle.id == id)
                    .map(|subtitle| base_language(&subtitle.lang))
            })
        });
        self.show_osd(match &lang {
            Some(_) => "Captions on".to_owned(),
            None => "Captions off".to_owned(),
        });
        if lang.is_some() || self.state.settings.captions.is_some() {
            self.state.settings.captions = lang;
            self.save_settings();
            let languages = caption_languages(&self.state.settings);
            if let Some(player) = &mut self.player
                && let Err(error) = player.set_caption_languages(languages)
            {
                log::warn!("mpv: {error}");
            }
        }
    }

    /// `c` and the captions button: off when shown, else the kept language
    /// (or the first track offered).
    pub(super) fn toggle_captions(&mut self) {
        let Some(subtitles) = self
            .player
            .as_ref()
            .map(|player| player.status().subtitles.clone())
        else {
            return;
        };
        if subtitles.iter().any(|subtitle| subtitle.selected) {
            self.set_subtitle(None);
            return;
        }
        let kept = self
            .state
            .settings
            .captions
            .as_deref()
            .and_then(|lang| pick_subtitle(&subtitles, lang));
        let desktop = caption_languages(&self.state.settings)
            .iter()
            .find_map(|lang| pick_subtitle(&subtitles, lang));
        let any = subtitles.first().map(|subtitle| subtitle.id);
        match kept.or(desktop).or(any) {
            Some(id) => self.set_subtitle(Some(id)),
            None => self.show_osd("No captions".to_owned()),
        }
    }

    /// A live stream, back to the newest part mpv has.
    pub(super) fn go_live(&mut self) {
        let Some(status) = self.player.as_ref().map(Mpv::status) else {
            return;
        };
        // mpv's duration of a live stream is its edge, as far as loaded.
        let edge = status.duration.max(status.cached_until);
        if edge > 0.0 {
            let reset_speed = (status.speed - 1.0).abs() >= 0.01;
            self.with_player(|player| player.seek_to((edge - 3.0).max(0.0), false));
            if reset_speed {
                self.with_player(|player| player.set_speed(1.0));
            }
            self.with_player(|player| player.set_paused(false));
            self.show_osd("Live".to_owned());
        }
    }

    /// The related videos of the watch page, when they are here.
    fn related_videos(&self) -> &[Video] {
        self.state
            .details
            .ready()
            .map_or(&[], |details| details.related.as_slice())
    }

    pub(super) fn play_next(&mut self) {
        let next = next_video(
            self.state.watching.as_ref(),
            &self.state.queue,
            &self.state.up_next,
            self.related_videos(),
        )
        .cloned();
        match next {
            Some(video) => self.advance(video),
            None => self.show_osd("No video up next".to_owned()),
        }
    }

    pub(super) fn play_previous(&mut self) {
        let previous = previous_video(self.state.watching.as_ref(), &self.state.up_next).cloned();
        if let Some(video) = previous {
            self.advance(video);
        }
    }

    /// The video ended: the next one starts after a countdown, unless
    /// autoplay is off or nothing comes next. A queued video plays even
    /// with autoplay off, as YouTube's queue does.
    pub(super) fn start_autoplay(&mut self) {
        if !self
            .state
            .cast
            .allows_local_autoplay(self.local_playback_generation)
        {
            return;
        }
        if self.state.watching_live() || self.state.watching_short() || self.state.playback_loop {
            return;
        }
        if !self.state.settings.autoplay && self.state.queue.is_empty() {
            return;
        }
        let next = next_video(
            self.state.watching.as_ref(),
            &self.state.queue,
            &self.state.up_next,
            self.related_videos(),
        )
        .cloned();
        if let Some(video) = next {
            self.state.autoplay = Some((video, Instant::now() + AUTOPLAY_AFTER));
        }
    }

    pub(super) fn enqueue(&mut self, video: Video, next: bool) {
        self.state.queue.retain(|queued| queued.id != video.id);
        if next {
            self.state.queue.insert(0, video);
            self.state.notice = Some("Playing next".to_owned());
        } else {
            self.state.queue.push(video);
            self.state.notice = Some("Added to queue".to_owned());
        }
        self.state.refresh_autoplay();
    }

    pub(super) fn clear_sponsor_mute(&mut self) {
        if self.state.sponsor_muted.take().is_some() {
            self.with_player(|player| player.set_muted(false));
        }
    }

    pub(super) fn set_sponsorblock(&mut self, on: bool) {
        self.state.settings.sponsorblock = on;
        self.state
            .cast
            .sponsor_enabled
            .store(on, std::sync::atomic::Ordering::Relaxed);
        self.save_settings();
        if on
            && let Some(session) = &self.state.cast.session
            && !session.video.live
        {
            self.backend
                .send(Command::LoadSegments(session.video.id.clone()));
        }
        if !on {
            self.clear_sponsor_mute();
            self.state.segments.clear();
            self.state.skipped.clear();
        } else if let Some(video) = &self.state.watching
            && !video.live
        {
            self.backend.send(Command::LoadSegments(video.id.clone()));
        }
    }

    pub(super) fn set_sponsor_category(
        &mut self,
        category: SponsorCategory,
        behavior: SponsorBehavior,
    ) {
        self.state
            .settings
            .sponsorblock_categories
            .set(category, behavior);
        self.save_settings();
        if behavior != SponsorBehavior::AutoSkip
            && let Some(index) = self.state.sponsor_muted
            && self
                .state
                .segments
                .get(index)
                .is_some_and(|segment| segment.category == category.as_str())
        {
            self.clear_sponsor_mute();
        }
    }

    /// The skip button: past the segment playing.
    pub(super) fn skip_segment(&mut self) {
        let Some(position) = self.player.as_ref().map(|player| player.status().position) else {
            return;
        };
        if let Some(index) = self.state.current_skippable_segment(position) {
            let end = self.state.segments[index].end;
            self.state.skipped.insert(index);
            if let Some(muted_index) = self.state.sponsor_muted
                && (muted_index == index
                    || self
                        .state
                        .segments
                        .get(muted_index)
                        .is_none_or(|segment| !within_mute_segment(segment, end)))
            {
                self.clear_sponsor_mute();
            }
            self.apply_seek(end);
        }
    }

    pub(super) fn close_mini_player(&mut self) {
        self.remember_position();
        self.with_player(|player| player.set_paused(true));
        self.state.mini_closed = true;
        self.state.playing_waiting = None;
        self.state.autoplay = None;
    }

    /// Each frame: the countdown to the next video, SponsorBlock's skips,
    /// the kept captions language, and the note over the video fading.
    pub(super) fn playback_tick(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        if let Some((_, at)) = &self.state.osd {
            if *at <= now {
                self.state.osd = None;
            } else {
                ctx.request_repaint_after(*at - now);
            }
        }
        if let Some((_, at)) = &self.state.autoplay {
            if *at <= now {
                // A source may have changed while the countdown was visible.
                self.state.refresh_autoplay();
                if let Some((video, _)) = self.state.autoplay.take() {
                    self.advance(video);
                }
            } else {
                // The countdown's seconds tick down on screen.
                ctx.request_repaint_after(Duration::from_millis(250));
            }
        }
        // Only what the checks below read: this runs every frame, and the
        // status carries strings and the captions' list.
        let Some((paused, seekable, muted, position, speed, live_edge)) =
            self.player.as_ref().map(|player| {
                let status = player.status();
                (
                    status.paused,
                    status.seekable,
                    status.muted,
                    status.position,
                    status.speed,
                    at_live_edge(self.state.watching_live(), status),
                )
            })
        else {
            return;
        };
        if live_edge && (speed - 1.0).abs() >= 0.01 {
            self.with_player(|player| player.set_speed(1.0));
        }
        if self.captions_pending
            && let Some(subtitles) = self
                .player
                .as_ref()
                .map(|player| player.status().subtitles.clone())
                .filter(|subtitles| !subtitles.is_empty())
        {
            self.captions_pending = false;
            if let Some(id) = self
                .state
                .settings
                .captions
                .as_deref()
                .and_then(|lang| pick_subtitle(&subtitles, lang))
            {
                self.with_player(|player| player.set_subtitle(Some(id)));
            }
        }
        let tick = self
            .state
            .tick_sponsorblock(paused, seekable, muted, position);
        if let Some(mute) = tick.set_muted {
            self.with_player(|player| player.set_muted(mute));
        }
        if let Some(end) = tick.seek_to {
            self.apply_seek(end);
        }
        if let Some(osd) = tick.osd {
            self.show_osd(osd.to_owned());
        }
    }

    /// Asks for suggestions once the search text has stayed the same a
    /// moment; an empty field or one already searched has none.
    pub(super) fn suggest_tick(&mut self, ctx: &egui::Context) {
        let query = self.state.query.trim().to_owned();
        if query.is_empty()
            || youtfast_core::links::video_id_from_link(&query).is_some()
            || youtfast_core::links::channel_from_link(&query).is_some()
        {
            self.state.suggestions.clear();
            self.query_changed = None;
            self.suggested = query;
            return;
        }
        if query == self.suggested {
            return;
        }
        match &self.query_changed {
            Some((text, at)) if *text == query => {
                let waited = at.elapsed();
                if waited >= SUGGEST_AFTER {
                    self.query_changed = None;
                    self.suggested = query.clone();
                    self.backend.send(Command::Suggest(query));
                } else {
                    ctx.request_repaint_after(SUGGEST_AFTER - waited);
                }
            }
            _ => {
                self.query_changed = Some((query, Instant::now()));
                ctx.request_repaint_after(SUGGEST_AFTER);
            }
        }
    }
}

impl State {
    /// The playback speed a newly started `video` should use:
    /// live streams stay at 1×; otherwise channel speed -> global speed -> 1.0.
    pub fn target_speed_for(&self, video: &Video) -> f64 {
        if video.live {
            return 1.0;
        }
        if let Some(channel_id) = video.channel_id.as_deref()
            && !channel_id.is_empty()
            && let Some(speed) = self.settings.channel_speed(channel_id)
        {
            return speed;
        }
        youtfast_storage::settings::sanitize_speed(self.settings.playback_speed)
    }

    /// Whether the currently watching video's channel has a remembered speed.
    pub fn remembering_channel_speed(&self) -> bool {
        self.watching
            .as_ref()
            .and_then(|video| video.channel_id.as_deref())
            .filter(|id| !id.is_empty())
            .is_some_and(|channel_id| self.settings.channel_speed(channel_id).is_some())
    }

    /// Records a user-chosen speed change (`<`/`>` or the speed menu) and
    /// returns `(speed_to_apply, settings_changed)`. Live streams do not
    /// overwrite stored speeds, and at the live edge they stay at 1×.
    pub fn record_speed(&mut self, speed: f64, live_edge: bool) -> (f64, bool) {
        if live_edge {
            return (1.0, false);
        }
        let speed = youtfast_storage::settings::sanitize_speed(speed);
        if self.watching_live() {
            return (speed, false);
        }
        if let Some(channel_id) = self
            .watching
            .as_ref()
            .and_then(|video| video.channel_id.as_deref())
            .filter(|id| !id.is_empty())
            && self.settings.channel_speed(channel_id).is_some()
        {
            self.settings
                .set_channel_speed(channel_id.to_owned(), speed);
            return (speed, true);
        }
        self.settings.playback_speed = speed;
        (speed, true)
    }

    /// Enables or disables remembering `current_speed` for the playing video's
    /// channel. Returns whether `Settings` changed.
    pub fn set_remember_channel_speed(&mut self, remember: bool, current_speed: f64) -> bool {
        if self.watching_live() {
            return false;
        }
        let Some(channel_id) = self
            .watching
            .as_ref()
            .and_then(|video| video.channel_id.as_deref())
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
        else {
            return false;
        };
        if remember {
            self.settings.set_channel_speed(channel_id, current_speed);
            true
        } else {
            self.settings.remove_channel_speed(&channel_id)
        }
    }

    /// Keep an active countdown in sync with the queue and current sources.
    /// Changes to the choice do not restart the countdown.
    pub(super) fn refresh_autoplay(&mut self) {
        let Some((_, deadline)) = self.autoplay.take() else {
            return;
        };
        if self.watching_live()
            || self.watching_short()
            || self.playback_loop
            || (!self.settings.autoplay && self.queue.is_empty())
        {
            return;
        }
        let related = self
            .details
            .ready()
            .map_or(&[][..], |details| details.related.as_slice());
        self.autoplay = next_video(self.watching.as_ref(), &self.queue, &self.up_next, related)
            .cloned()
            .map(|video| (video, deadline));
    }

    /// Returns whether a mute toggle or volume change should explicitly unmute mpv.
    pub(super) fn prepare_manual_unmute(&mut self, position: f64, polled_muted: bool) -> bool {
        // The mpv status snapshot may not yet reflect this frame's SponsorBlock mute.
        let owned_mute = self.sponsor_muted.take();
        if owned_mute.is_none() && !polled_muted {
            return false;
        }
        self.skipped.extend(owned_mute);
        if self.settings.sponsorblock {
            let categories = self.settings.sponsorblock_categories;
            self.skipped.extend(
                self.segments
                    .iter()
                    .enumerate()
                    .filter(|(_, segment)| {
                        segment.action == SegmentAction::Mute
                            && categories.behavior_for(&segment.category)
                                == SponsorBehavior::AutoSkip
                            && within_mute_segment(segment, position)
                    })
                    .map(|(index, _)| index),
            );
        }
        true
    }

    /// Replaces the segments for this video; returns whether mpv needs to be unmuted.
    pub(super) fn accept_segments(&mut self, id: &str, segments: Vec<Segment>) -> bool {
        if !self.settings.sponsorblock
            || self.watching.as_ref().is_none_or(|video| video.id != id)
            || self.segments == segments
        {
            return false;
        }
        let previous_mute = self.sponsor_muted;
        self.sponsor_muted = previous_mute.and_then(|index| {
            let active = self.segments.get(index)?;
            segments.iter().position(|segment| segment == active)
        });
        self.segments = segments;
        self.skipped.clear();
        previous_mute.is_some() && self.sponsor_muted.is_none()
    }

    /// First active segment that can be skipped via the skip button (`AutoSkip` or `ManualSkip`).
    pub(super) fn current_skippable_segment(&self, position: f64) -> Option<usize> {
        self.segments.iter().position(|segment| {
            matches!(
                self.settings
                    .sponsorblock_categories
                    .behavior_for(&segment.category),
                SponsorBehavior::AutoSkip | SponsorBehavior::ManualSkip
            ) && position >= segment.start
                && position < segment.end - 0.3
        })
    }

    /// Advances SponsorBlock's auto-skip and auto-mute state for the current playback position.
    pub(super) fn tick_sponsorblock(
        &mut self,
        paused: bool,
        seekable: bool,
        muted: bool,
        position: f64,
    ) -> SponsorTick {
        let mut tick = SponsorTick::default();
        if let Some(muted_index) = self.sponsor_muted {
            let still_active = self.settings.sponsorblock
                && self.segments.get(muted_index).is_some_and(|segment| {
                    self.settings
                        .sponsorblock_categories
                        .behavior_for(&segment.category)
                        == SponsorBehavior::AutoSkip
                        && segment.action == SegmentAction::Mute
                        && within_mute_segment(segment, position)
                });
            if !still_active {
                self.sponsor_muted = None;
                tick.set_muted = Some(false);
            }
        }
        if !self.settings.sponsorblock || paused || !seekable {
            return tick;
        }
        if let Some((index, segment)) = self.segments.iter().enumerate().find(|(index, segment)| {
            !self.skipped.contains(index)
                && self
                    .settings
                    .sponsorblock_categories
                    .behavior_for(&segment.category)
                    == SponsorBehavior::AutoSkip
                && segment.action == SegmentAction::Skip
                && position >= segment.start
                && position < segment.end - 0.3
        }) {
            self.skipped.insert(index);
            if self
                .sponsor_muted
                .is_some_and(|index| !within_mute_segment(&self.segments[index], segment.end))
            {
                self.sponsor_muted = None;
                tick.set_muted = Some(false);
            }
            tick.seek_to = Some(segment.end);
            tick.osd = Some(skipped_osd(&segment.category));
            return tick;
        }
        if self.sponsor_muted.is_none()
            && let Some((index, segment)) =
                self.segments.iter().enumerate().find(|(index, segment)| {
                    !self.skipped.contains(index)
                        && self
                            .settings
                            .sponsorblock_categories
                            .behavior_for(&segment.category)
                            == SponsorBehavior::AutoSkip
                        && segment.action == SegmentAction::Mute
                        && within_mute_segment(segment, position)
                })
        {
            let already_user_muted = muted && tick.set_muted != Some(false);
            if already_user_muted {
                self.skipped.insert(index);
            } else {
                self.sponsor_muted = Some(index);
                tick.set_muted = Some(true);
                tick.osd = Some(muted_osd(&segment.category));
            }
        }
        tick
    }
}

fn within_mute_segment(segment: &Segment, position: f64) -> bool {
    position >= segment.start && position < segment.end
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct SponsorTick {
    pub set_muted: Option<bool>,
    pub seek_to: Option<f64>,
    pub osd: Option<&'static str>,
}

fn skipped_osd(category: &str) -> &'static str {
    SponsorCategory::parse(category).map_or("Segment skipped", SponsorCategory::skipped_osd)
}

fn muted_osd(category: &str) -> &'static str {
    SponsorCategory::parse(category).map_or("Segment muted", SponsorCategory::muted_osd)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(id: &str) -> Video {
        Video::from_id(id.to_owned())
    }

    fn subtitle(id: i64, lang: &str, title: &str) -> mpv::Subtitle {
        mpv::Subtitle {
            id,
            lang: lang.to_owned(),
            title: title.to_owned(),
            selected: false,
        }
    }

    #[test]
    fn previous_stays_inside_the_source_list() {
        let source = [video("a"), video("b"), video("c")];
        assert_eq!(previous_video(Some(&source[1]), &source), Some(&source[0]));
        assert_eq!(previous_video(Some(&source[0]), &source), None);
        assert_eq!(previous_video(Some(&video("elsewhere")), &source), None);
        assert_eq!(previous_video(None, &source), None);
    }

    #[test]
    fn short_navigation_keeps_shorts_unless_the_user_explicitly_queued_a_video() {
        let mut a = video("short-a");
        a.short = true;
        let mut b = video("short-b");
        b.short = true;
        let long = video("long");
        let source = [a.clone(), long.clone(), b.clone()];
        assert_eq!(next_video(Some(&a), &[], &source, &[]), Some(&source[2]));
        assert_eq!(previous_video(Some(&b), &source), Some(&source[0]));
        assert_eq!(
            next_video(Some(&b), &[], &source, std::slice::from_ref(&long)),
            None
        );
        assert_eq!(
            next_video(Some(&a), std::slice::from_ref(&long), &source, &[]),
            Some(&long)
        );
    }

    #[test]
    fn queue_reordering_keeps_items_and_ignores_stale_indices() {
        let mut queue = vec![video("a"), video("b"), video("c")];
        reorder_queue(&mut queue, 0, 2);
        assert_eq!(
            queue
                .iter()
                .map(|video| video.id.as_str())
                .collect::<Vec<_>>(),
            ["b", "c", "a"]
        );
        reorder_queue(&mut queue, 2, 0);
        let original = vec![video("a"), video("b"), video("c")];
        assert_eq!(queue, original);
        reorder_queue(&mut queue, 8, 0);
        reorder_queue(&mut queue, 0, 8);
        assert_eq!(queue, original);
    }

    #[test]
    fn countdown_follows_queue_changes_and_cancels_when_autoplay_is_off() {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut state = State {
            watching: Some(video("current")),
            queue: vec![video("a"), video("b")],
            autoplay: Some((video("a"), deadline)),
            ..Default::default()
        };
        state.settings.autoplay = false;
        reorder_queue(&mut state.queue, 1, 0);
        state.refresh_autoplay();
        assert_eq!(
            state.autoplay.as_ref().map(|(v, at)| (v.id.as_str(), *at)),
            Some(("b", deadline))
        );

        state.queue.retain(|v| v.id != "b");
        state.refresh_autoplay();
        assert_eq!(
            state.autoplay.as_ref().map(|(v, _)| v.id.as_str()),
            Some("a")
        );

        state.queue.clear();
        state.refresh_autoplay();
        assert!(state.autoplay.is_none());
    }

    #[test]
    fn countdown_uses_source_when_queue_cleared_and_autoplay_is_on() {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut state = State {
            watching: Some(video("current")),
            queue: vec![video("queued")],
            up_next: vec![video("current"), video("next")],
            autoplay: Some((video("queued"), deadline)),
            ..Default::default()
        };
        state.settings.autoplay = true;
        state.queue.clear();
        state.refresh_autoplay();
        assert_eq!(
            state.autoplay.as_ref().map(|(v, at)| (v.id.as_str(), *at)),
            Some(("next", deadline))
        );
    }

    #[test]
    fn late_segments_are_ignored_after_sponsorblock_is_disabled() {
        let mut state = State {
            watching: Some(video("current")),
            ..Default::default()
        };
        let segment = Segment {
            category: "sponsor".into(),
            start: 1.0,
            end: 3.0,
            action: SegmentAction::Skip,
        };
        state.settings.sponsorblock = false;
        state.accept_segments("current", vec![segment.clone()]);
        assert!(state.segments.is_empty());
        state.settings.sponsorblock = true;
        state.accept_segments("other", vec![segment.clone()]);
        assert!(state.segments.is_empty());
        state.accept_segments("current", vec![segment.clone()]);
        assert_eq!(state.segments, vec![segment]);
    }

    #[test]
    fn sponsorblock_refresh_keeps_mute_ownership_until_exit() {
        let mut state = State {
            watching: Some(video("current")),
            ..Default::default()
        };
        state.settings.sponsorblock = true;
        let mute = Segment {
            category: "sponsor".into(),
            start: 10.0,
            end: 20.0,
            action: SegmentAction::Mute,
        };
        assert!(!state.accept_segments("current", vec![mute.clone()]));
        assert_eq!(
            state.tick_sponsorblock(false, true, false, 12.0).set_muted,
            Some(true)
        );
        assert!(!state.accept_segments("current", vec![mute.clone()]));
        assert_eq!(
            state.tick_sponsorblock(false, true, true, 20.0).set_muted,
            Some(false)
        );
        assert_eq!(
            state.tick_sponsorblock(false, true, false, 12.0).set_muted,
            Some(true)
        );
        let mut earlier = mute.clone();
        earlier.start = 1.0;
        earlier.end = 2.0;
        assert!(!state.accept_segments("current", vec![earlier, mute]));
        assert_eq!(state.sponsor_muted, Some(1));
        assert!(state.accept_segments("current", vec![]));
    }

    #[test]
    fn sponsorblock_categories_drive_skip_mute_manual_seekbar_and_ignore() {
        let make_seg = |category: &str, start: f64, end: f64, action: SegmentAction| Segment {
            category: category.to_owned(),
            start,
            end,
            action,
        };

        // Default behaviors: sponsor/selfpromo/interaction AutoSkip;
        // intro/outro/preview/music_offtopic/filler ManualSkip.
        let mut state = State::default();
        state.settings.sponsorblock = true;
        state.segments = vec![
            make_seg("sponsor", 10.0, 20.0, SegmentAction::Skip),
            make_seg("intro", 30.0, 40.0, SegmentAction::Skip),
        ];

        // AutoSkip + Skip triggers seek + OSD; ManualSkip does not auto-skip.
        let tick = state.tick_sponsorblock(false, true, false, 12.0);
        assert_eq!(
            tick,
            SponsorTick {
                set_muted: None,
                seek_to: Some(20.0),
                osd: Some("Sponsor skipped"),
            }
        );
        assert!(state.skipped.contains(&0));
        // Seeking back into an already-skipped AutoSkip segment does not re-skip.
        assert_eq!(
            state.tick_sponsorblock(false, true, false, 15.0),
            SponsorTick::default()
        );
        // Playing inside ManualSkip intro does not auto-skip, but is skippable by button.
        assert_eq!(
            state.tick_sponsorblock(false, true, false, 32.0),
            SponsorTick::default()
        );
        assert_eq!(state.current_skippable_segment(32.0), Some(1));

        // Configuring intro to AutoSkip skips it and shows "Intro skipped".
        state
            .settings
            .sponsorblock_categories
            .set(SponsorCategory::Intro, SponsorBehavior::AutoSkip);
        assert_eq!(
            state.tick_sponsorblock(false, true, false, 32.0),
            SponsorTick {
                set_muted: None,
                seek_to: Some(40.0),
                osd: Some("Intro skipped"),
            }
        );

        // SeekBarOnly and Ignore never auto-skip and are never skippable by button.
        state
            .settings
            .sponsorblock_categories
            .set(SponsorCategory::Outro, SponsorBehavior::SeekBarOnly);
        state
            .settings
            .sponsorblock_categories
            .set(SponsorCategory::Filler, SponsorBehavior::Ignore);
        state.segments = vec![
            make_seg("outro", 50.0, 60.0, SegmentAction::Skip),
            make_seg("filler", 70.0, 80.0, SegmentAction::Mute),
        ];
        assert_eq!(
            state.tick_sponsorblock(false, true, false, 52.0),
            SponsorTick::default()
        );
        assert_eq!(state.current_skippable_segment(52.0), None);
        assert_eq!(
            state.tick_sponsorblock(false, true, false, 72.0),
            SponsorTick::default()
        );
        assert_eq!(state.current_skippable_segment(72.0), None);
    }

    #[test]
    fn sponsorblock_mutes_short_intervals_from_start_to_end() {
        let mut state = State {
            segments: vec![Segment {
                category: "sponsor".into(),
                start: 10.0,
                end: 10.2,
                action: SegmentAction::Mute,
            }],
            ..Default::default()
        };
        state.settings.sponsorblock = true;
        assert_eq!(
            state.tick_sponsorblock(false, true, false, 9.99).set_muted,
            None
        );
        assert_eq!(
            state.tick_sponsorblock(false, true, false, 10.0).set_muted,
            Some(true)
        );
        assert_eq!(
            state.tick_sponsorblock(false, true, true, 10.19).set_muted,
            None
        );
        assert_eq!(
            state.tick_sponsorblock(false, true, true, 10.2).set_muted,
            Some(false)
        );
    }

    #[test]
    fn sponsorblock_auto_mute_lifecycle_handles_entry_exit_and_user_mute() {
        let mut state = State::default();
        state.settings.sponsorblock = true;
        state.segments = vec![
            Segment {
                category: "sponsor".into(),
                start: 10.0,
                end: 20.0,
                action: SegmentAction::Mute,
            },
            Segment {
                category: "selfpromo".into(),
                start: 30.0,
                end: 40.0,
                action: SegmentAction::Mute,
            },
        ];

        // Entering unmuted mutes mpv and records sponsor_muted.
        let tick = state.tick_sponsorblock(false, true, false, 11.0);
        assert_eq!(
            tick,
            SponsorTick {
                set_muted: Some(true),
                seek_to: None,
                osd: Some("Sponsor muted"),
            }
        );
        assert_eq!(state.sponsor_muted, Some(0));

        // While staying inside, no repeated mute/OSD.
        assert_eq!(
            state.tick_sponsorblock(false, true, true, 19.95),
            SponsorTick::default()
        );

        // Leaving segment unmutes mpv automatically.
        assert_eq!(
            state.tick_sponsorblock(false, true, true, 20.0),
            SponsorTick {
                set_muted: Some(false),
                seek_to: None,
                osd: None,
            }
        );
        assert_eq!(state.sponsor_muted, None);

        // If already muted before entering segment 1, SponsorBlock leaves mute untouched
        // on entry and on exit.
        assert_eq!(
            state.tick_sponsorblock(false, true, true, 32.0),
            SponsorTick::default()
        );
        assert_eq!(state.sponsor_muted, None);
        assert_eq!(
            state.tick_sponsorblock(false, true, true, 41.0),
            SponsorTick::default()
        );
    }

    #[test]
    fn sponsorblock_manual_unmute_overrides_stale_status_and_all_overlapping_mutes() {
        let mut state = State {
            segments: vec![
                Segment {
                    category: "sponsor".into(),
                    start: 10.0,
                    end: 20.0,
                    action: SegmentAction::Mute,
                },
                Segment {
                    category: "selfpromo".into(),
                    start: 15.0,
                    end: 25.0,
                    action: SegmentAction::Mute,
                },
            ],
            ..Default::default()
        };
        state.settings.sponsorblock = true;
        assert_eq!(
            state.tick_sponsorblock(false, true, false, 16.0).set_muted,
            Some(true)
        );
        // The toggle and volume action both use this decision before writing to mpv.
        assert!(state.prepare_manual_unmute(16.0, false));
        assert_eq!(
            state.tick_sponsorblock(false, true, false, 17.0),
            SponsorTick::default()
        );
        assert_eq!(
            state.tick_sponsorblock(false, true, false, 21.0),
            SponsorTick::default()
        );
    }

    #[test]
    fn sponsorblock_disabling_while_paused_restores_audio() {
        let mut state = State {
            segments: vec![Segment {
                category: "sponsor".into(),
                start: 10.0,
                end: 20.0,
                action: SegmentAction::Mute,
            }],
            ..Default::default()
        };
        state.settings.sponsorblock = true;

        assert_eq!(
            state.tick_sponsorblock(false, true, false, 12.0).set_muted,
            Some(true)
        );
        state.settings.sponsorblock = false;
        assert_eq!(
            state.tick_sponsorblock(true, true, true, 12.5),
            SponsorTick {
                set_muted: Some(false),
                seek_to: None,
                osd: None,
            }
        );
    }

    #[test]
    fn every_category_has_distinct_skipped_and_muted_osd_text() {
        for cat in SponsorCategory::ALL {
            assert_eq!(skipped_osd(cat.as_str()), cat.skipped_osd());
            assert_eq!(muted_osd(cat.as_str()), cat.muted_osd());
            assert!(cat.skipped_osd().ends_with(" skipped"));
            assert!(cat.muted_osd().ends_with(" muted"));
        }
        assert_eq!(skipped_osd("unknown"), "Segment skipped");
        assert_eq!(muted_osd("unknown"), "Segment muted");
    }

    #[test]
    fn the_queue_goes_first_then_up_next_then_related() {
        let (a, b, c, r) = (video("a"), video("b"), video("c"), video("r"));
        let up_next = [a.clone(), b.clone(), c.clone()];
        let related = [b.clone(), r.clone()];
        let pick = |watching: &Video, queue: &[Video], up_next: &[Video], related: &[Video]| {
            next_video(Some(watching), queue, up_next, related).map(|video| video.id.clone())
        };
        assert_eq!(
            pick(&b, std::slice::from_ref(&r), &up_next, &related),
            Some("r".into())
        );
        assert_eq!(pick(&b, &[], &up_next, &related), Some("c".into()));
        // The last of Up next goes on to the related videos.
        assert_eq!(pick(&c, &[], &up_next, &related), Some("b".into()));
        assert_eq!(pick(&b, &[], &[], &related), Some("r".into()));
        // Not in Up next (opened from a related video): Up next's first.
        assert_eq!(pick(&r, &[], &up_next, &[]), Some("a".into()));
        assert_eq!(pick(&a, &[], &[], &[]), None);
    }

    #[test]
    fn captions_prefer_the_uploaders_then_the_automatic_then_a_translation() {
        let tracks = [
            subtitle(1, "en", "English"),
            subtitle(2, "pt-en", "Portuguese from English"),
            subtitle(3, "pt-orig", "Portuguese (Original)"),
            subtitle(4, "pt-BR", "Portuguese (Brazil)"),
        ];
        assert_eq!(pick_subtitle(&tracks, "pt"), Some(4));
        assert_eq!(pick_subtitle(&tracks[..3], "pt-BR"), Some(3));
        assert_eq!(pick_subtitle(&tracks[..2], "pt"), Some(2));
        assert_eq!(pick_subtitle(&tracks, "ja"), None);
    }

    #[test]
    fn captions_are_asked_in_the_kept_then_desktop_language() {
        assert_eq!(
            languages(Some("pt-BR"), Some("de_DE.UTF-8")),
            ["pt", "de", "en"]
        );
        assert_eq!(languages(None, Some("C")), ["en"]);
        assert_eq!(languages(Some("en"), Some("en_US.UTF-8")), ["en"]);
    }

    #[test]
    fn speeds_step_through_youtubes() {
        assert_eq!(step_speed(1.0, true), 1.25);
        assert_eq!(step_speed(2.0, true), 2.0);
        assert_eq!(step_speed(0.25, false), 0.25);
        // An odd speed steps from normal.
        assert_eq!(step_speed(1.1, false), 0.75);
        assert_eq!(speed_label(1.0), "1×");
        assert_eq!(speed_label(1.25), "1.25×");
        assert_eq!(speed_label(0.5), "0.5×");
    }

    #[test]
    fn target_speed_follows_channel_then_global_then_normal_and_keeps_live_at_normal() {
        let mut state = State::default();
        let mut ch_a = video("vid_a000001");
        ch_a.channel_id = Some("UC_a".into());
        let mut ch_b = video("vid_b000001");
        ch_b.channel_id = Some("UC_b".into());
        let no_ch = video("vid_c000001");

        // Default is 1.0.
        assert_eq!(state.target_speed_for(&ch_a), 1.0);
        assert_eq!(state.target_speed_for(&no_ch), 1.0);

        // Global speed applies when channel has no override.
        state.settings.playback_speed = 1.5;
        assert_eq!(state.target_speed_for(&ch_a), 1.5);
        assert_eq!(state.target_speed_for(&no_ch), 1.5);

        // Per-channel speed overrides global speed (even back down to 1.0).
        state.settings.set_channel_speed("UC_a".into(), 1.0);
        assert_eq!(state.target_speed_for(&ch_a), 1.0);
        assert_eq!(state.target_speed_for(&ch_b), 1.5);

        // Live streams always start at 1.0 even with global and channel speeds.
        let mut live_a = ch_a.clone();
        live_a.live = true;
        assert_eq!(state.target_speed_for(&live_a), 1.0);
    }

    #[test]
    fn recording_speed_updates_channel_or_global_and_ignores_live_edge() {
        let mut state = State::default();
        let mut ch_a = video("vid_a000001");
        ch_a.channel_id = Some("UC_a".into());
        let mut ch_b = video("vid_b000001");
        ch_b.channel_id = Some("UC_b".into());

        // Without per-channel remember, setting speed persists globally.
        state.watching = Some(ch_a.clone());
        assert!(!state.remembering_channel_speed());
        assert_eq!(state.record_speed(1.5, false), (1.5, true));
        assert_eq!(state.settings.playback_speed, 1.5);
        assert_eq!(state.settings.channel_speed("UC_a"), None);

        // Enabling remember stores the current speed for the channel.
        assert!(state.set_remember_channel_speed(true, 1.5));
        assert!(state.remembering_channel_speed());
        assert_eq!(state.settings.channel_speed("UC_a"), Some(1.5));

        // Changing speed while remembered updates the channel only, leaving global intact.
        assert_eq!(state.record_speed(1.75, false), (1.75, true));
        assert_eq!(state.settings.channel_speed("UC_a"), Some(1.75));
        assert_eq!(state.settings.playback_speed, 1.5);

        // Switching to another channel keeps global speed; changing it updates global only.
        state.watching = Some(ch_b.clone());
        assert!(!state.remembering_channel_speed());
        assert_eq!(state.target_speed_for(&ch_b), 1.5);
        assert_eq!(state.record_speed(1.25, false), (1.25, true));
        assert_eq!(state.settings.playback_speed, 1.25);
        assert_eq!(state.settings.channel_speed("UC_a"), Some(1.75));

        // Turning off per-channel remember removes the entry.
        state.watching = Some(ch_a.clone());
        assert!(state.set_remember_channel_speed(false, 1.75));
        assert!(!state.remembering_channel_speed());
        assert_eq!(state.settings.channel_speed("UC_a"), None);
        assert_eq!(state.target_speed_for(&ch_a), 1.25);

        // Live stream at the live edge stays at 1.0 and never overwrites stored speed.
        let mut live = ch_a.clone();
        live.live = true;
        state.watching = Some(live);
        assert_eq!(state.record_speed(1.75, true), (1.0, false));
        assert_eq!(state.settings.playback_speed, 1.25);
        assert!(!state.set_remember_channel_speed(true, 1.0));

        // Behind the live edge allows temporary speed-up without overwriting stored speed.
        assert_eq!(state.record_speed(1.5, false), (1.5, false));
        assert_eq!(state.settings.playback_speed, 1.25);

        let edge_status = mpv::Status {
            duration: 1000.0,
            position: 980.0,
            ..mpv::Status::default()
        };
        assert!(at_live_edge(true, &edge_status));
        assert!(!at_live_edge(false, &edge_status));
        let behind_status = mpv::Status {
            duration: 1000.0,
            position: 900.0,
            ..mpv::Status::default()
        };
        assert!(!at_live_edge(true, &behind_status));
    }
}
