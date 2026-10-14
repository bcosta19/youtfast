//! The watch page: the video playing, its details and its comments.

use super::*;

impl State {
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
        }
    }
}

impl App {
    /// Takes in what mpv has to say.
    pub(super) fn poll_player(&mut self) {
        let Some(player) = &mut self.player else {
            return;
        };
        let events = player.poll();
        // yt-dlp's answer is there once a video has loaded.
        let json = events
            .contains(&mpv::Event::Loaded)
            .then(|| player.ytdl_json())
            .flatten();
        for event in events {
            match event {
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
                        self.state.player_error = Some(failure_message(failure).into());
                    }
                }
                mpv::Event::Finished => {
                    if let Some(video) = &self.state.watching {
                        self.state.library.mark_finished(&video.id);
                        self.save_library();
                    }
                    self.start_autoplay();
                }
            }
        }
        if let Some(json) = json {
            // Kept from before a quality change until there is news.
            let (details, resolutions) = youtube::ytdl_loaded(&json);
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
        self.remember_position();
        if self.state.view != View::Watch {
            // A Short from Home's shelf goes on to the shelf's next ones.
            self.state.up_next = if video.short && self.state.view == View::Home {
                self.state.shorts.items().to_vec()
            } else {
                self.visible_videos()
            };
            self.back.push(self.state.view);
        }
        // What plays now leaves the queue.
        self.state.queue.retain(|queued| queued.id != video.id);
        self.state.autoplay = None;
        self.state.mini_closed = false;
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
            self.state.player_error = player
                .load(&video.url(), start)
                .err()
                .map(|error| error.to_string());
        }
        let id = video.id.clone();
        self.state.watching = Some(video);
        self.state.save_targets = Fetch::Idle;
        self.state.share_at_time = false;
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
        self.state.details = Fetch::Loading;
        self.state.comments = Fetch::Idle;
        self.state.comments_next = None;
        self.state.comments_more = Fetch::Idle;
        self.more_comments_token = None;
        self.state.replies.clear();
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
        result: Result<(Vec<CommentRow>, Option<String>), String>,
    ) {
        if generation != self.details_generation {
            return;
        }
        if key == CommentsKey::Top && result.is_ok() {
            self.more_comments_token = None;
        }
        self.state.fill_comments(key, result);
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
            View::History if account => state
                .account_history
                .items()
                .iter()
                .map(|row| row.video.clone())
                .collect(),
            View::History => state
                .library
                .history
                .iter()
                .map(|watched| watched.video.clone())
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
            self.back.push(self.state.view);
            self.state.view = View::Watch;
        }
        self.state.mini_closed = false;
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
}
