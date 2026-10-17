//! Bounded in-memory browser navigation. Never restores a library snapshot or
//! account state, and pending requests are invalidated when a location returns.
use super::*;

#[derive(Clone)]
pub(super) struct Location {
    view: View,
    page: Option<Page>,
    query: String,
    search_query: String,
    search_options: innertube::SearchOptions,
    results: Results,
    channels: Listing<Channel>,
    playlists: Listing<PlaylistSummary>,
    results_more: Option<innertube::More>,
    channel_tab: ChannelTab,
    channel_order: innertube::ChannelOrder,
    channel_query: String,
    header: Option<ChannelHeader>,
    videos: Listing<Video>,
    page_playlists: Listing<PlaylistSummary>,
    about: Fetch<ChannelAbout>,
    page_more: Option<innertube::More>,
    playlist_details: Option<innertube::PlaylistDetails>,
    source: Source,
    filter: FeedFilter,
    cursor: usize,
    watching: Option<Video>,
    up_next: Vec<Video>,
    playing_playlist: Option<String>,
    playing_more: Option<innertube::More>,
}

impl Location {
    pub(super) fn capture(s: &State) -> Self {
        Self {
            view: s.view,
            page: s.page.clone(),
            query: s.query.clone(),
            search_query: s.search_query.clone(),
            search_options: s.search_options,
            results: s.results.clone(),
            channels: s.results_channels.clone(),
            playlists: s.results_playlists.clone(),
            results_more: s.results_more.clone(),
            channel_tab: s.channel_tab.clone(),
            channel_order: s.channel_order,
            channel_query: s.channel_query.clone(),
            header: s.channel_header.clone(),
            videos: s.page_videos.clone(),
            page_playlists: s.page_playlists.clone(),
            about: s.channel_about.clone(),
            page_more: s.page_more.clone(),
            playlist_details: s.playlist_details.clone(),
            source: s.source,
            filter: s.feed_filter,
            cursor: s.cursor,
            watching: s.watching.clone(),
            up_next: s.up_next.clone(),
            playing_playlist: s.playing_playlist.clone(),
            playing_more: s.playing_more.clone(),
        }
    }
    fn restore(self, s: &mut State) -> Option<Video> {
        fn settled<T>(v: Listing<T>) -> Listing<T> {
            if matches!(v, Listing::Loading) {
                Listing::Idle
            } else {
                v
            }
        }
        s.view = self.view;
        s.page = self.page;
        s.query = self.query;
        s.search_query = self.search_query;
        s.search_options = self.search_options;
        s.results = settled(self.results);
        s.results_channels = settled(self.channels);
        s.results_playlists = settled(self.playlists);
        s.results_more = self.results_more;
        s.results_more_state = Fetch::Idle;
        s.channel_tab = self.channel_tab;
        s.channel_order = self.channel_order;
        s.channel_query = self.channel_query;
        s.channel_header = self.header;
        s.page_videos = settled(self.videos);
        s.page_playlists = settled(self.page_playlists);
        s.channel_about = if matches!(self.about, Fetch::Loading) {
            Fetch::Idle
        } else {
            self.about
        };
        s.page_more = self.page_more;
        s.page_more_state = Fetch::Idle;
        s.playlist_details = self.playlist_details;
        s.source = self.source;
        s.feed_filter = self.filter;
        s.cursor = self.cursor;
        s.cursor_moved = false;
        let watching = self.watching.filter(|_| self.view == View::Watch);
        if watching.is_some() {
            s.up_next = self.up_next;
            s.playing_playlist = self.playing_playlist;
            s.playing_more = self.playing_more;
            s.playing_more_state = Fetch::Idle;
            s.playing_waiting = None;
        }
        watching
    }
}

impl State {
    pub(super) fn extras_need_request(&self) -> bool {
        match &self.extras {
            Fetch::Loading => false,
            Fetch::Ready(extras) => extras.retry_needed(),
            _ => true,
        }
    }
    pub(super) fn reset_watch_fetches(&mut self) {
        self.extras = Fetch::Idle;
        self.details = Fetch::Loading;
        self.comments = Fetch::Idle;
        self.comments_next = None;
        self.comments_more = Fetch::Idle;
        self.replies.clear();
    }
    pub(super) fn set_autoplay_choice(&mut self, on: bool) {
        self.settings.autoplay = on;
        if !on {
            self.autoplay = None;
            if self.playing_waiting == Some(PlaylistAdvance::Autoplay) {
                self.playing_waiting = None;
            }
        }
    }
    pub(super) fn finish_playlist_page(
        &mut self,
        current: u64,
        generation: u64,
        id: &str,
        result: Result<(Vec<Video>, Option<innertube::More>), String>,
    ) -> Option<PlaylistAdvance> {
        if current != generation || self.playing_playlist.as_deref() != Some(id) {
            return None;
        }
        match result {
            Ok((videos, more)) => {
                let mut known: HashSet<String> =
                    self.up_next.iter().map(|v| v.id.clone()).collect();
                self.up_next
                    .extend(videos.into_iter().filter(|v| known.insert(v.id.clone())));
                self.playing_more = more;
                self.playing_more_state = Fetch::Idle;
                self.playing_waiting.take()
            }
            Err(error) => {
                self.playing_more_state = Fetch::Failed(error);
                self.notice =
                    Some("Could not load the next playlist videos. Use Next to retry.".into());
                None
            }
        }
    }
    pub(super) fn needs_playlist_page(&self) -> bool {
        self.watching.as_ref().is_some_and(|watching| {
            self.up_next
                .iter()
                .position(|v| v.id == watching.id)
                .is_some_and(|i| {
                    !self.up_next[i + 1..]
                        .iter()
                        .any(|v| !watching.short || v.short)
                })
        }) && self.queue.is_empty()
            && self.playing_more.is_some()
    }
    /// Stable widget scope keeps each page's own scroll position and focus IDs.
    pub fn navigation_key(&self) -> egui::Id {
        let page = match &self.page {
            Some(Page::Channel(c)) => c.id.as_str(),
            Some(Page::Playlist { id, .. }) => id.as_str(),
            None => "",
        };
        egui::Id::new((
            self.view as u8,
            page,
            format!("{:?}", self.channel_tab),
            self.search_query.as_str(),
            self.source as u8,
        ))
    }
}

impl App {
    pub(super) fn push_location(&mut self) {
        if self.back.len() >= 30 {
            self.back.remove(0);
        }
        self.back.push(Location::capture(&self.state));
        self.forward.clear();
    }
    pub(super) fn travel(&mut self, ctx: &egui::Context, forward: bool) {
        self.leave_video(ctx);
        let target = if forward {
            self.forward.pop()
        } else {
            self.back.pop()
        };
        let Some(target) = target else {
            return;
        };
        let current = Location::capture(&self.state);
        let stack = if forward {
            &mut self.back
        } else {
            &mut self.forward
        };
        if stack.len() >= 30 {
            stack.remove(0);
        }
        stack.push(current);
        let video = target.restore(&mut self.state);
        self.generation += 1;
        self.generations.insert(Slot::Search, self.generation);
        self.generations.insert(Slot::Page, self.generation);
        self.open_channel = match &self.state.page {
            Some(Page::Channel(c)) => Some(c.id.clone()),
            _ => None,
        };
        if let Some(video) = video {
            self.playing_generation += 1;
            if self
                .state
                .watching
                .as_ref()
                .is_none_or(|v| v.id != video.id)
            {
                self.open(video);
            } else {
                self.return_to_watch();
            }
        } else {
            self.load(self.state.view, false);
        }
        self.cursor_key = (self.state.view, ui::cursor_key(&self.state));
    }
    /// Advancing a playing list must not replace its origin with the page
    /// currently browsed underneath the mini player.
    pub(super) fn advance(&mut self, video: Video) {
        let browsing =
            (self.state.view != View::Watch && !self.state.mini_closed).then_some(self.state.view);
        self.state.view = View::Watch;
        self.open(video);
        if let Some(view) = browsing {
            self.state.view = view;
        }
    }
    pub(super) fn prefetch_playlist(&mut self) {
        let near_end = self
            .state
            .watching
            .as_ref()
            .and_then(|v| self.state.up_next.iter().position(|item| item.id == v.id))
            .is_some_and(|i| i + 3 >= self.state.up_next.len());
        if (near_end || self.state.needs_playlist_page())
            && self.state.playing_more_state == Fetch::Idle
        {
            self.request_playlist_page();
        }
    }
    fn request_playlist_page(&mut self) {
        if matches!(self.state.playing_more_state, Fetch::Loading) {
            return;
        }
        if let (Some(id), Some(more)) = (&self.state.playing_playlist, &self.state.playing_more) {
            self.backend.send(Command::LoadPlaylistMore {
                generation: self.playing_generation,
                id: id.clone(),
                more: more.clone(),
            });
            self.state.playing_more_state = Fetch::Loading;
        }
    }
    pub(super) fn await_playlist_page(&mut self, reason: PlaylistAdvance) -> bool {
        if self.state.needs_playlist_page() {
            self.state.playing_waiting = Some(reason);
            self.request_playlist_page();
            self.show_osd("Loading next videos…".into());
            true
        } else {
            false
        }
    }
    pub(super) fn load_extras(&mut self) {
        if !self.state.extras_need_request() {
            return;
        }
        let Some(video) = self.state.watching.as_ref() else {
            return;
        };
        let language = player::caption_languages(&self.state.settings)
            .into_iter()
            .next()
            .unwrap_or_else(|| "en".into());
        self.state.extras = Fetch::Loading;
        self.backend.send(Command::LoadExtras {
            generation: self.details_generation,
            id: video.id.clone(),
            language,
        });
    }
}

pub(super) fn chapter_target(
    chapters: &[crate::model::Chapter],
    position: f64,
    forward: bool,
) -> Option<f64> {
    if forward {
        chapters
            .iter()
            .find(|c| f64::from(c.seconds) > position + 0.1)
            .map(|c| f64::from(c.seconds))
    } else {
        chapters
            .iter()
            .rev()
            .find(|c| f64::from(c.seconds) < position - 3.0)
            .map(|c| f64::from(c.seconds))
            .or(Some(0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_playlist_pages_continue_before_falling_back_to_related() {
        let video = Video::from_id("aaaaaaaaaaa".into());
        let mut s = State {
            watching: Some(video.clone()),
            up_next: vec![video],
            playing_playlist: Some("PLsame".into()),
            playing_waiting: Some(PlaylistAdvance::Manual),
            ..State::default()
        };
        let more = innertube::More {
            ctoken: "next".into(),
            endpoint: rustypipe::model::paginator::ContinuationEndpoint::Browse,
            visitor_data: None,
            authenticated: false,
            ours: true,
        };
        assert_eq!(
            s.finish_playlist_page(2, 2, "PLsame", Ok((Vec::new(), Some(more)))),
            Some(PlaylistAdvance::Manual)
        );
        assert!(s.needs_playlist_page());
        s.watching.as_mut().unwrap().short = true;
        s.up_next
            .extend((0..5).map(|i| Video::from_id(format!("long{i}"))));
        assert!(
            s.needs_playlist_page(),
            "Shorts must continue past long videos in a mixed playlist"
        );
        s.playing_more = None;
        assert!(!s.needs_playlist_page());
    }
    #[test]
    fn restoring_a_watch_location_restores_its_playlist_but_preserves_global_queue() {
        let a = Video::from_id("aaaaaaaaaaa".into());
        let b = Video::from_id("bbbbbbbbbbb".into());
        let mut s = State {
            view: View::Watch,
            watching: Some(a.clone()),
            up_next: vec![a.clone(), b.clone()],
            playing_playlist: Some("PLfirst".into()),
            ..State::default()
        };
        let location = Location::capture(&s);
        s.watching = Some(b);
        s.up_next.clear();
        s.playing_playlist = None;
        s.queue.push(a.clone());
        assert_eq!(location.restore(&mut s), Some(a));
        assert_eq!(s.up_next.len(), 2);
        assert_eq!(s.playing_playlist.as_deref(), Some("PLfirst"));
        assert_eq!(s.queue.len(), 1);
    }
    #[test]
    fn stale_pages_cannot_restore_an_old_playlist_or_advance_after_cancelled_autoplay() {
        let mut s = State {
            playing_playlist: Some("PLsame".into()),
            playing_waiting: Some(PlaylistAdvance::Autoplay),
            ..State::default()
        };
        let video = Video::from_id("aaaaaaaaaaa".into());
        assert_eq!(
            s.finish_playlist_page(2, 1, "PLsame", Ok((vec![video.clone()], None))),
            None
        );
        assert!(s.up_next.is_empty());
        s.set_autoplay_choice(false);
        assert_eq!(
            s.finish_playlist_page(2, 2, "PLsame", Ok((vec![video], None))),
            None
        );
        s.playing_waiting = Some(PlaylistAdvance::Manual);
        s.set_autoplay_choice(false);
        assert_eq!(
            s.finish_playlist_page(2, 2, "PLsame", Ok((Vec::new(), None))),
            Some(PlaylistAdvance::Manual)
        );
    }
    #[test]
    fn transcript_failures_retry_and_refresh_does_not_strand_an_inflight_request() {
        let mut s = State {
            extras: Fetch::Ready(crate::video_extras::Extras {
                preview: None,
                language: "en".into(),
                transcript: Err("unavailable".into()),
            }),
            ..State::default()
        };
        assert!(s.extras_need_request());
        s.extras = Fetch::Loading;
        assert!(!s.extras_need_request());
        s.reset_watch_fetches();
        assert!(s.extras_need_request());
        s.extras = Fetch::Ready(crate::video_extras::Extras {
            preview: None,
            language: "en".into(),
            transcript: Ok(Vec::new()),
        });
        assert!(!s.extras_need_request());
    }
    #[test]
    fn returning_to_a_channel_restores_its_identity_without_rolling_back_library() {
        let mut s = State {
            view: View::Channel,
            page: Some(Page::Channel(Channel {
                id: "UCfirst".into(),
                name: "First".into(),
                avatar: None,
            })),
            channel_tab: ChannelTab::Shorts,
            cursor: 4,
            ..State::default()
        };
        let id = s.navigation_key();
        let page = Location::capture(&s);
        s.page = Some(Page::Channel(Channel {
            id: "UCsecond".into(),
            ..Channel::default()
        }));
        s.library.history.clear();
        s.library
            .watch_later
            .push(Video::from_id("ccccccccccc".into()));
        page.restore(&mut s);
        assert_eq!(s.navigation_key(), id);
        assert_eq!(s.cursor, 4);
        assert_eq!(s.library.watch_later.len(), 1);
    }
    #[test]
    fn chapters_go_forward_and_restart_a_chapter_before_going_back() {
        let c = [
            crate::model::Chapter {
                name: "a".into(),
                seconds: 0,
            },
            crate::model::Chapter {
                name: "b".into(),
                seconds: 60,
            },
        ];
        assert_eq!(chapter_target(&c, 20.0, true), Some(60.0));
        assert_eq!(chapter_target(&c, 70.0, false), Some(60.0));
        assert_eq!(chapter_target(&c, 61.0, false), Some(0.0));
    }
}
