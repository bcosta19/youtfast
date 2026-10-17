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
    }

    /// Retry the same video without making a second history entry or
    /// replacing its queue/source list.
    pub(super) fn retry_playback(&mut self) {
        let Some(video) = &self.state.watching else {
            return;
        };
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
        self.with_player(|player| player.set_speed(speed));
        self.show_osd(speed_label(speed));
    }

    pub(super) fn toggle_mute(&mut self) {
        let Some(muted) = self.player.as_ref().map(|player| player.status().muted) else {
            return;
        };
        self.with_player(|player| player.set_muted(!muted));
        self.show_osd(
            if muted {
                "Som ativado"
            } else {
                "Som desativado"
            }
            .to_owned(),
        );
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
            Some(_) => "Legendas ativadas".to_owned(),
            None => "Legendas desativadas".to_owned(),
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
            None => self.show_osd("Sem legendas".to_owned()),
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
            self.with_player(|player| player.seek_to((edge - 3.0).max(0.0), false));
            self.with_player(|player| player.set_paused(false));
            self.show_osd("Ao vivo".to_owned());
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
            None => self.show_osd("Nenhum vídeo a seguir".to_owned()),
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
            self.state.notice = Some("Será reproduzido a seguir".to_owned());
        } else {
            self.state.queue.push(video);
            self.state.notice = Some("Adicionado à fila".to_owned());
        }
    }

    pub(super) fn set_sponsorblock(&mut self, on: bool) {
        self.state.settings.sponsorblock = on;
        self.save_settings();
        if !on {
            self.state.segments.clear();
        } else if let Some(video) = &self.state.watching
            && !video.live
        {
            self.backend.send(Command::LoadSegments(video.id.clone()));
        }
    }

    /// The segment being played, if any, by index.
    fn current_segment(&self, position: f64) -> Option<usize> {
        self.state
            .segments
            .iter()
            .position(|segment| position >= segment.start && position < segment.end - 0.3)
    }

    /// The skip button: past the segment playing.
    pub(super) fn skip_segment(&mut self) {
        let Some(position) = self.player.as_ref().map(|player| player.status().position) else {
            return;
        };
        if let Some(index) = self.current_segment(position) {
            let end = self.state.segments[index].end;
            self.state.skipped.insert(index);
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
        if let Some((video, at)) = &self.state.autoplay {
            if *at <= now {
                let video = video.clone();
                self.state.autoplay = None;
                self.advance(video);
            } else {
                // The countdown's seconds tick down on screen.
                ctx.request_repaint_after(Duration::from_millis(250));
            }
        }
        let Some(status) = self.player.as_ref().map(|player| player.status().clone()) else {
            return;
        };
        if self.captions_pending && !status.subtitles.is_empty() {
            self.captions_pending = false;
            if let Some(id) = self
                .state
                .settings
                .captions
                .as_deref()
                .and_then(|lang| pick_subtitle(&status.subtitles, lang))
            {
                self.with_player(|player| player.set_subtitle(Some(id)));
            }
        }
        if !status.paused
            && status.seekable
            && let Some(index) = self.current_segment(status.position)
            && !self.state.skipped.contains(&index)
            && self.state.segments[index].skipped_at_once()
        {
            let segment = self.state.segments[index].clone();
            self.state.skipped.insert(index);
            self.apply_seek(segment.end);
            self.show_osd(
                match segment.category.as_str() {
                    "sponsor" => "Patrocínio ignorado",
                    "selfpromo" => "Autopromoção ignorada",
                    "interaction" => "Pedido de interação ignorado",
                    _ => "Trecho ignorado",
                }
                .to_owned(),
            );
        }
    }

    /// Asks for suggestions once the search text has stayed the same a
    /// moment; an empty field or one already searched has none.
    pub(super) fn suggest_tick(&mut self, ctx: &egui::Context) {
        let query = self.state.query.trim().to_owned();
        if query.is_empty() || youtube::video_id_from_link(&query).is_some() {
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
}
