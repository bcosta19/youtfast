//! The watch page: the video playing, its details and its comments.

use super::*;

impl State {
    fn renew_comment_image(&mut self, comment: &str, manual: bool) -> bool {
        // Bound the entire renewal chain, even if each refused request produces
        // a different signed URL. A deliberate retry starts one new attempt.
        let first = self.comment_image_refreshes.insert(comment.to_owned());
        manual || first
    }
    fn fill_comments(
        &mut self,
        key: CommentsKey,
        result: Result<(Vec<CommentRow>, Option<String>), String>,
    ) {
        match (key, result) {
            (CommentsKey::Top, Ok((rows, next))) => {
                let mut comments = match std::mem::take(&mut self.comments) {
                    Fetch::Ready(comments) => comments,
                    _ => Vec::new(),
                };
                let mut known: HashSet<String> =
                    comments.iter().map(|row| row.id.clone()).collect();
                comments.extend(rows.into_iter().filter(|row| known.insert(row.id.clone())));
                self.comments = Fetch::Ready(comments);
                self.comments_next = next;
                self.comments_more = Fetch::Idle;
            }
            (CommentsKey::Top, Err(error)) if matches!(self.comments_more, Fetch::Loading) => {
                self.comments_more = Fetch::Failed(error);
            }
            (CommentsKey::Top, Err(error)) => self.comments = Fetch::Failed(error),
            (CommentsKey::Replies(id), Ok((rows, _))) => {
                self.replies.insert(id, Fetch::Ready(rows));
            }
            (CommentsKey::Replies(id), Err(error)) => {
                self.replies.insert(id, Fetch::Failed(error));
            }
            (CommentsKey::ImageRefresh { comment, old_url }, result) => {
                let fresh = result
                    .ok()
                    .and_then(|(rows, _)| rows.into_iter().find(|row| row.id == comment))
                    .and_then(|row| row.image);
                let Some(fresh) = fresh.filter(|url| url != &old_url) else {
                    log::warn!("image: source refresh failed");
                    return;
                };
                let replace = |rows: &mut Fetch<Vec<CommentRow>>| {
                    if let Fetch::Ready(rows) = rows {
                        for row in rows.iter_mut().filter(|row| {
                            row.id == comment && row.image.as_deref() == Some(&old_url)
                        }) {
                            row.image = Some(fresh.clone());
                        }
                    }
                };
                replace(&mut self.comments);
                for replies in self.replies.values_mut() {
                    replace(replies);
                }
                log::debug!("image: source refreshed");
            }
        }
    }
}

impl App {
    /// Takes in what mpv has to say.
    pub(super) fn poll_player(&mut self) {
        let Some(player) = &mut self.player else {
            return;
        };
        let elapsed = self.playback_tick.elapsed();
        self.playback_tick = Instant::now();
        let events = player.poll();
        // yt-dlp's answer is there once a video has loaded.
        let json = events
            .contains(&mpv::Event::Loaded)
            .then(|| player.ytdl_json())
            .flatten();
        for event in events {
            match event {
                mpv::Event::Started => {
                    self.playback_report.started(
                        self.backend.session_epoch(),
                        self.state.signed_in(),
                        self.state.settings.history_sync,
                    );
                }
                mpv::Event::Loaded => {
                    self.state.player_error = None;
                    self.captions_pending = self.state.settings.captions.is_some();
                }
                mpv::Event::Failed(failure) => {
                    // What the session can fix is tried again with it.
                    let fixable = matches!(
                        failure,
                        mpv::Failure::BotCheck
                            | mpv::Failure::AgeRestricted
                            | mpv::Failure::Unavailable
                    );
                    let retried = fixable
                        && self.player.as_mut().is_some_and(|player| {
                            player.retry_signed_in().unwrap_or_else(|error| {
                                log::warn!("mpv: {error}");
                                false
                            })
                        });
                    if retried {
                        log::info!("refused signed out ({failure:?}); trying signed in");
                    } else {
                        self.playback_report.cancel();
                        self.state.player_error = Some(failure_message(failure).into());
                    }
                }
                mpv::Event::Finished => {
                    if let Some(video) = &self.state.watching {
                        self.state.library.mark_finished(&video.id);
                        self.save_library();
                    }
                    if (self.state.settings.autoplay || !self.state.queue.is_empty())
                        && !self.await_playlist_page(PlaylistAdvance::Autoplay)
                    {
                        self.start_autoplay();
                    }
                }
            }
        }
        if let Some(player) = &self.player {
            let playing = player.is_playing() && self.state.player_error.is_none();
            if self.playback_report.advance(
                self.backend.session_epoch(),
                self.state.signed_in(),
                self.state.settings.history_sync,
                playing,
                elapsed,
            ) && let Some(video) = &self.state.watching
            {
                self.backend.send(Command::MarkWatched {
                    id: video.id.clone(),
                    session: self.backend.session_epoch(),
                });
            }
        }
        if let Some(json) = json {
            // Kept from before a quality change until there is news.
            let (details, resolutions) = ytdlp::ytdl_loaded(&json);
            self.state.resolutions = resolutions;
            if let (Some(details), Some(watching)) = (details, self.state.watching.as_mut())
                && details.id == watching.id
            {
                watching.complete_from(&details);
                // Whether it is live now: a list may still call a finished
                // stream live.
                watching.live = details.live;
                let watching = watching.clone();
                self.state.library.update_video(&watching);
                self.save_library();
            }
        }
    }

    pub(super) fn open(&mut self, mut video: Video) {
        log::debug!("opening video");
        self.playback_report = history_sync::PlaybackReport::default();
        self.playback_tick = Instant::now();
        self.remember_position();
        if self.state.view != View::Watch {
            self.playing_generation += 1;
            self.state.playing_playlist = match &self.state.page {
                Some(Page::Playlist { id, .. }) if self.state.view == View::Playlist => {
                    Some(id.clone())
                }
                _ => None,
            };
            self.state.playing_more = if self.state.playing_playlist.is_some() {
                self.state.page_more.clone()
            } else {
                None
            };
            self.state.playing_more_state = Fetch::Idle;
        }
        self.state.playing_waiting = None;
        if self.state.view != View::Watch {
            // A Short from Home's shelf goes on to the shelf's next ones.
            self.state.up_next = if video.short && self.state.view == View::Home {
                self.state.shorts.items().to_vec()
            } else {
                self.visible_videos()
            };
            self.push_location();
        }
        // What plays now leaves the queue.
        self.state.queue.retain(|queued| queued.id != video.id);
        self.state.autoplay = None;
        self.state.playback_loop = false;
        self.state.mini_closed = false;
        self.state.watch_scroll_to_video = true;
        self.state.player_menu = crate::app::PlayerMenu::Closed;
        self.state.segments.clear();
        self.state.skipped.clear();
        self.captions_pending = false;
        // A video opened from a link knows only its id; the history may know
        // more until yt-dlp answers.
        if let Some(known) = self
            .state
            .library
            .history
            .iter()
            .find(|watched| watched.video.id == video.id)
        {
            video.complete_from(&known.video);
        }
        let start = self.state.library.resume_position(&video.id);
        self.state.library.record_watch(video.clone(), util::now());
        self.save_library();

        self.state.view = View::Watch;
        self.state.seek_preview = None;
        self.state.resolutions.clear();
        // The watch page's other panes start fresh and fill as they load.
        self.reset_watch_panes();
        self.state.comments_visitor_data = None;
        self.state.comment_pages.clear();
        self.state.comment_image_refreshes.clear();
        self.state.description_open = false;
        self.state.comments_open.clear();
        if self.state.settings.sponsorblock && !video.live {
            self.backend.send(Command::LoadSegments(video.id.clone()));
        }
        // A live stream starts at its live edge, not where it was left.
        let start = start.filter(|_| !video.live);
        if let Some(player) = &mut self.player {
            if let Err(error) = player.set_looping(video.short) {
                log::warn!("mpv: {error}");
            }
            let _ = player.set_looping(video.short || self.state.playback_loop);
            let _ = player.set_caption_style(
                self.state.settings.caption_scale,
                self.state.settings.caption_background,
            );
            self.state.player_error = player
                .load(&video.url(), start)
                .err()
                .map(|error| error.to_string());
        }
        let id = video.id.clone();
        self.state.watching = Some(video);
        self.state.save_targets = Fetch::Idle;
        self.state.share_at_time = false;
        self.state.extras = Fetch::Idle;
        self.state.transcript_open = false;
        self.state.transcript_query.clear();
        self.load_rating();
        match self.details_cache.get(&id, util::now(), DETAILS_FRESH) {
            Some(details) => self.show_details(details.clone()),
            None => self.backend.send(Command::LoadDetails {
                generation: self.details_generation,
                id,
            }),
        }
    }

    /// Empties the details and comments panes, and drops the answers to
    /// their requests in flight.
    fn reset_watch_panes(&mut self) {
        self.state.reset_watch_fetches();
        self.more_comments_token = None;
        self.generation += 1;
        self.details_generation = self.generation;
    }

    /// Asks again for the details of the video being watched.
    pub(super) fn refresh_details(&mut self, video: Video) {
        self.reset_watch_panes();
        self.backend.send(Command::LoadDetails {
            generation: self.details_generation,
            id: video.id,
        });
    }

    /// Takes in a video's details, unless another video was opened since.
    pub(super) fn on_details(&mut self, generation: u64, result: Result<Details, String>) {
        if generation != self.details_generation {
            return;
        }
        match result {
            Ok(details) => {
                self.details_cache
                    .put(details.id.clone(), util::now(), details.clone());
                self.show_details(details);
            }
            Err(error) => self.state.details = Fetch::Failed(error),
        }
    }

    /// Takes in a comments or replies page, unless another video was opened
    /// since.
    pub(super) fn on_comments(
        &mut self,
        generation: u64,
        key: CommentsKey,
        token: String,
        result: Result<(Vec<CommentRow>, Option<String>), String>,
    ) {
        if generation != self.details_generation {
            return;
        }
        if !matches!(key, CommentsKey::ImageRefresh { .. })
            && let Ok((rows, _)) = &result
        {
            for row in rows.iter().filter(|row| row.image.is_some()) {
                self.state
                    .comment_pages
                    .insert(row.id.clone(), token.clone());
            }
        }
        if key == CommentsKey::Top && result.is_ok() {
            self.more_comments_token = None;
        }
        self.state.fill_comments(key, result);
    }

    /// Re-request exactly the page which produced this attachment. A failed or
    /// unchanged source leaves the comment intact and stops automatic renewal.
    pub(super) fn refresh_comment_image(&mut self, old_url: String, manual: bool) {
        let row = self
            .state
            .comments
            .ready()
            .into_iter()
            .flatten()
            .chain(
                self.state
                    .replies
                    .values()
                    .filter_map(|rows| rows.ready())
                    .flatten(),
            )
            .find(|row| row.image.as_deref() == Some(old_url.as_str()))
            .map(|row| row.id.clone());
        if let Some(comment) = row
            && self.state.renew_comment_image(&comment, manual)
            && let Some(token) = self.state.comment_pages.get(&comment)
        {
            self.backend.send(Command::LoadComments {
                generation: self.details_generation,
                key: CommentsKey::ImageRefresh { comment, old_url },
                token: token.clone(),
                visitor_data: self.state.comments_visitor_data.clone(),
            });
        }
    }

    /// Asks for the next comments page.
    pub(super) fn load_more_comments(&mut self) {
        // Take the token so a second click asks for the same page
        // again only after a retry.
        if matches!(self.state.comments_more, Fetch::Loading) {
            return;
        }
        if let Some(token) = self
            .state
            .comments_next
            .take()
            .or_else(|| self.more_comments_token.clone())
        {
            self.more_comments_token = Some(token.clone());
            self.state.comments_more = Fetch::Loading;
            self.backend.send(Command::LoadComments {
                generation: self.details_generation,
                key: CommentsKey::Top,
                token,
                visitor_data: self.state.comments_visitor_data.clone(),
            });
        }
    }

    /// Opens a comment's replies, asking for them, or closes them.
    pub(super) fn toggle_replies(&mut self, id: String) {
        if self.state.replies.contains_key(&id) {
            self.state.replies.remove(&id);
        } else {
            let token = self
                .state
                .comments
                .ready()
                .and_then(|comments| comments.iter().find(|row| row.id == id))
                .and_then(|row| row.replies_token.clone());
            self.state.replies.insert(id.clone(), Fetch::Loading);
            if let Some(token) = token {
                self.backend.send(Command::LoadComments {
                    generation: self.details_generation,
                    key: CommentsKey::Replies(id),
                    token,
                    visitor_data: self.state.comments_visitor_data.clone(),
                });
            } else {
                self.state.replies.insert(
                    id,
                    Fetch::Failed("the replies' token is missing".to_owned()),
                );
            }
        }
    }

    /// Shows a video's details on the watch page, fresh or from the cache,
    /// and asks for its first comments page.
    fn show_details(&mut self, details: Details) {
        // Complete what the watch page and the library know.
        if let Some(watching) = self.state.watching.as_mut()
            && watching.id == details.id
        {
            let filled = Video {
                id: details.id.clone(),
                title: details.title.clone(),
                channel: Some(details.channel.name.clone()),
                channel_id: Some(details.channel.id.clone()),
                channel_avatar: details.channel.avatar.clone(),
                duration: None,
                views: Some(details.views),
                live: details.live,
                published_at: None,
                published: details.published.clone(),
                short: false,
            };
            watching.complete_from(&filled);
            let watching = watching.clone();
            self.state.library.update_video(&watching);
            self.save_library();
        }
        // The first comments page comes right behind it.
        if let Some(token) = details.comments_token.clone() {
            self.state.comments = Fetch::Loading;
            self.state.comments_visitor_data = details.visitor_data.clone();
            self.backend.send(Command::LoadComments {
                generation: self.details_generation,
                key: CommentsKey::Top,
                token,
                visitor_data: details.visitor_data.clone(),
            });
        }
        self.state.details = Fetch::Ready(details);
    }

    /// The videos on screen, to offer as Up next beside one of them.
    fn visible_videos(&self) -> Vec<Video> {
        let state = &self.state;
        let account = state.source() == Source::Account;
        match state.view {
            View::Home if state.feed_filter == crate::model::FeedFilter::ForYou => {
                state.for_you.items().to_vec()
            }
            View::Home => state.feed.items().to_vec(),
            View::Results => state.results.items().to_vec(),
            View::Liked => state.liked.items().to_vec(),
            View::Channel | View::Playlist => state.page_videos.items().to_vec(),
            View::WatchLater if account => state.account_watch_later.items().to_vec(),
            View::WatchLater => state.library.watch_later.clone(),
            View::History => state
                .history_rows()
                .iter()
                .map(|row| row.video.clone())
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Remembers where the video being watched was left; a live stream has
    /// no such place.
    pub(super) fn remember_position(&mut self) {
        if let (Some(video), Some(player)) = (&self.state.watching, &self.player) {
            let status = player.status();
            if !status.idle && !video.live {
                self.state
                    .library
                    .set_position(&video.id, status.position, status.duration);
                self.save_library();
            }
        }
    }

    /// Which browser's session mpv's yt-dlp falls back on for a video
    /// YouTube refuses signed out; `None` forgets it.
    pub(super) fn set_playback_cookies(&mut self, browser: Option<String>) {
        let Some(player) = &mut self.player else {
            return;
        };
        if browser.is_some() {
            log::debug!("signed-in playback retry enabled");
        }
        player.set_cookies_from_browser(browser);
    }

    /// Bounds the streams from now on, and reloads the video being watched
    /// where it was, so the new quality shows at once.
    pub(super) fn set_quality(&mut self, quality: Quality) {
        if quality == self.state.settings.quality {
            return;
        }
        log::info!("quality set to {quality:?}");
        self.state.settings.quality = quality;
        self.save_settings();
        let Some(player) = &mut self.player else {
            return;
        };
        if let Err(error) = player.set_max_resolution(quality.limit()) {
            log::warn!("mpv: {error}");
            return;
        }
        let (idle, position, paused) = {
            let status = player.status();
            (status.idle, status.position, status.paused)
        };
        if let Some(video) = &self.state.watching
            && !idle
            && let Err(error) = player.reload(&video.url(), position, paused)
        {
            log::warn!("mpv: {error}");
        }
    }

    /// Back to the watch page of the video playing, from the mini player
    /// or the desktop's media controls.
    pub(super) fn return_to_watch(&mut self) {
        if self.state.watching.is_some() && self.state.view != View::Watch {
            self.push_location();
            self.state.view = View::Watch;
        }
        self.state.mini_closed = false;
        self.state.watch_scroll_to_video = true;
    }

    /// A video playing must be drawn: the mini player does it, and once it
    /// was closed the watch page does.
    pub(super) fn show_playing(&mut self) {
        if self.state.mini_closed {
            self.return_to_watch();
        }
    }

    /// Leaving the watch page hands the video to the mini player, which
    /// keeps drawing it (an undrawn render-API video output stalls), and
    /// remembers where it was left.
    pub(super) fn leave_video(&mut self, ctx: &egui::Context) {
        self.set_fullscreen(ctx, false);
        self.state.player_menu = crate::app::PlayerMenu::Closed;
        if self.state.view == View::Watch {
            self.remember_position();
            if self.state.mini_closed {
                self.with_player(|player| player.set_paused(true));
            }
        }
    }

    pub(super) fn set_fullscreen(&mut self, ctx: &egui::Context, fullscreen: bool) {
        if self.state.fullscreen != fullscreen {
            self.state.fullscreen = fullscreen;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(fullscreen));
        }
    }

    pub(super) fn with_player(&self, command: impl FnOnce(&Mpv) -> Result<(), mpv::Error>) {
        if let Some(player) = &self.player
            && let Err(error) = command(player)
        {
            log::warn!("mpv: {error}");
        }
    }
}

/// What the watch page says when a video could not play.
fn failure_message(failure: mpv::Failure) -> &'static str {
    match failure {
        mpv::Failure::BotCheck => {
            "YouTube is limiting requests from this connection. Try again later."
        }
        mpv::Failure::AgeRestricted => "This video is age-restricted and needs signing in.",
        mpv::Failure::Unavailable => "This video is private or unavailable.",
        mpv::Failure::Forbidden => {
            "YouTube refused access to the stream (HTTP 403). The stream link may have expired or access may be restricted. Try reopening the video."
        }
        mpv::Failure::RateLimited => {
            "YouTube is limiting requests (HTTP 429). Wait a while before trying again."
        }
        mpv::Failure::Extraction => {
            "yt-dlp could not extract the stream signature. Try updating yt-dlp and reopening the video."
        }
        mpv::Failure::Loading => {
            "mpv could not open the video stream. Check your connection and try reopening the video."
        }
        mpv::Failure::Interrupted => {
            "Playback was interrupted after the video loaded. The player did not identify the cause; the connection or stream link may no longer be valid. Try reopening the video."
        }
        mpv::Failure::AudioOutput => {
            "mpv could not initialize audio output. Check your audio device and try again."
        }
        mpv::Failure::VideoOutput => {
            "mpv could not initialize video output. Try restarting the app."
        }
        mpv::Failure::UnknownFormat => "mpv could not recognize the stream format.",
        mpv::Failure::Other => {
            "Could not play this video. The player did not identify the cause. Try reopening the video."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interruption_feedback_does_not_recommend_an_extractor_update() {
        let message = failure_message(mpv::Failure::Interrupted);
        assert!(message.contains("after the video loaded"));
        assert!(message.contains("did not identify the cause"));
        assert!(!message.contains("yt-dlp"));
        assert!(failure_message(mpv::Failure::Forbidden).contains("HTTP 403"));
        assert!(failure_message(mpv::Failure::Extraction).contains("updating yt-dlp"));
    }

    fn comment(id: &str) -> CommentRow {
        CommentRow {
            id: id.into(),
            author: None,
            author_id: None,
            author_avatar: None,
            text: Vec::new(),
            image: None,
            published: String::new(),
            likes: None,
            replies: 0,
            replies_token: None,
            pinned: false,
            by_owner: false,
            hearted: false,
        }
    }

    #[test]
    fn comment_continuations_finish_loading_preserve_rows_on_failure_and_retry() {
        let mut state = State::default();
        for page in 0..3 {
            state.comments_more = Fetch::Loading;
            state.fill_comments(
                CommentsKey::Top,
                Ok((
                    vec![comment(&page.to_string())],
                    Some(format!("page-{}", page + 1)),
                )),
            );
            assert_eq!(state.comments.ready().unwrap().len(), page + 1);
            assert!(matches!(state.comments_more, Fetch::Idle));
        }
        state.comments_more = Fetch::Loading;
        state.fill_comments(CommentsKey::Top, Err("offline".into()));
        assert_eq!(state.comments.ready().unwrap().len(), 3);
        assert!(matches!(state.comments_more, Fetch::Failed(_)));
        state.comments_more = Fetch::Loading;
        state.fill_comments(
            CommentsKey::Top,
            Ok((vec![comment("2"), comment("3")], None)),
        );
        assert_eq!(state.comments.ready().unwrap().len(), 4);
        assert!(matches!(state.comments_more, Fetch::Idle));
        assert!(state.comments_next.is_none());
    }

    #[test]
    fn a_failed_alert_check_can_be_tried_again() {
        let mut state = State {
            alerts_checking: true,
            ..State::default()
        };
        state.alerts_failed("offline".into());
        assert!(!state.alerts_checking);
        assert!(state.notice.unwrap().contains("offline"));
    }
    #[test]
    fn refreshing_an_attachment_preserves_text_paging_and_newer_sources() {
        let mut original = comment("top");
        original.image = Some("expired".into());
        let mut reply = comment("reply");
        reply.image = Some("already-new".into());
        let mut state = State {
            comments: Fetch::Ready(vec![original]),
            comments_next: Some("next-page".into()),
            ..Default::default()
        };
        state
            .replies
            .insert("top".into(), Fetch::Ready(vec![reply]));
        let mut fresh = comment("top");
        fresh.image = Some("renewed".into());
        fresh.author = Some("must not replace author".into());
        state.fill_comments(
            CommentsKey::ImageRefresh {
                comment: "top".into(),
                old_url: "expired".into(),
            },
            Ok((vec![fresh], None)),
        );
        assert_eq!(
            state.comments.ready().unwrap()[0].image.as_deref(),
            Some("renewed")
        );
        assert_ne!(
            state.comments.ready().unwrap()[0].author.as_deref(),
            Some("must not replace author")
        );
        assert_eq!(state.comments_next.as_deref(), Some("next-page"));
        let mut old = comment("reply");
        old.image = Some("stale".into());
        state.fill_comments(
            CommentsKey::ImageRefresh {
                comment: "reply".into(),
                old_url: "expired".into(),
            },
            Ok((vec![old], None)),
        );
        assert_eq!(
            state.replies["top"].ready().unwrap()[0].image.as_deref(),
            Some("already-new")
        );
        state.fill_comments(
            CommentsKey::ImageRefresh {
                comment: "top".into(),
                old_url: "renewed".into(),
            },
            Err("offline".into()),
        );
        assert_eq!(
            state.comments.ready().unwrap()[0].image.as_deref(),
            Some("renewed")
        );
    }
    #[test]
    fn renewed_urls_cannot_create_an_automatic_refresh_loop() {
        let mut state = State::default();
        assert!(state.renew_comment_image("comment", false));
        assert!(!state.renew_comment_image("comment", false));
        assert!(state.renew_comment_image("comment", true));
        assert!(!state.renew_comment_image("comment", false));
        assert!(state.renew_comment_image("other", false));
    }
}
