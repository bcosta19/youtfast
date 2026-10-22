//! Lists: asking the backend for them, and putting their answers in place.

use super::*;

impl State {
    fn search_request(&self) -> Option<Request> {
        (!self.search_query.trim().is_empty()).then(|| Request::Search {
            query: self.search_query.clone(),
            options: self.search_options,
        })
    }

    fn start_search(&mut self) {
        self.results = Listing::Idle;
        self.results_channels = Listing::Idle;
        self.results_playlists = Listing::Idle;
        match self.search_options.kind {
            innertube::SearchKind::Videos => self.results = Listing::Loading,
            innertube::SearchKind::Channels => self.results_channels = Listing::Loading,
            innertube::SearchKind::Playlists => self.results_playlists = Listing::Loading,
        }
        self.results_more = None;
        self.results_more_state = Fetch::Idle;
    }

    fn search_continuation(&self) -> Option<(Slot, Request)> {
        self.results_more
            .clone()
            .filter(|_| !matches!(self.results_more_state, Fetch::Loading))
            .map(|more| {
                (
                    Slot::Search,
                    Request::MoreSearch {
                        more,
                        kind: self.search_options.kind,
                    },
                )
            })
    }

    fn fill_search(&mut self, result: Result<Payload, String>) {
        match result {
            Ok(Payload::Search { items, more }) => {
                self.results_more = more;
                self.results_more_state = Fetch::Idle;
                match items {
                    PageItems::Videos(items) => self.results = Listing::Ready(items),
                    PageItems::Channels(items) => self.results_channels = Listing::Ready(items),
                    PageItems::Playlists(items) => self.results_playlists = Listing::Ready(items),
                    _ => {}
                }
            }
            Ok(Payload::More { items, more }) => {
                self.results_more = more;
                self.results_more_state = Fetch::Idle;
                match items {
                    PageItems::Videos(items) => self.results.items_mut().extend(items),
                    PageItems::Channels(items) => self.results_channels.items_mut().extend(items),
                    PageItems::Playlists(items) => self.results_playlists.items_mut().extend(items),
                    _ => {}
                }
            }
            Err(error) if matches!(self.results_more_state, Fetch::Loading) => {
                self.results_more_state = Fetch::Failed(error)
            }
            Err(error) => match self.search_options.kind {
                innertube::SearchKind::Videos => self.results = Listing::Failed(error),
                innertube::SearchKind::Channels => self.results_channels = Listing::Failed(error),
                innertube::SearchKind::Playlists => self.results_playlists = Listing::Failed(error),
            },
            _ => {}
        }
    }

    fn fill_playlist(
        &mut self,
        details: innertube::PlaylistDetails,
        items: Vec<Video>,
        more: Option<innertube::More>,
    ) {
        self.playlist_details = Some(details);
        self.page_more = more;
        self.page_more_state = Fetch::Idle;
        self.page_videos = Listing::Ready(items);
    }

    fn fill_channel_page(
        &mut self,
        header: ChannelHeader,
        items: PageItems,
        more: Option<innertube::More>,
    ) {
        self.fill_channel_header(header);
        self.page_more = more;
        self.page_more_state = Fetch::Idle;
        match items {
            PageItems::Videos(videos) => self.page_videos = Listing::Ready(videos),
            PageItems::Playlists(items) => self.page_playlists = Listing::Ready(items),
            _ => {}
        }
    }

    fn channel_about_request(&self, id: String) -> Request {
        Request::ChannelAbout {
            id,
            needs_header: self.channel_header.is_none(),
        }
    }

    fn fill_channel_about(&mut self, header: Option<ChannelHeader>, about: ChannelAbout) {
        if let Some(header) = header {
            self.fill_channel_header(header);
        }
        self.channel_about = Fetch::Ready(about);
    }

    fn fill_channel_header(&mut self, header: ChannelHeader) {
        // A channel opened from a link only gets its UC… id, real name and
        // avatar once the header arrives, including through About.
        if let Some(Page::Channel(channel)) = &mut self.page {
            channel.id = header.id.clone();
            channel.name = header.name.clone();
            if header.avatar.is_some() {
                channel.avatar = header.avatar.clone();
            }
        }
        self.channel_header = Some(header);
    }

    fn accept_import(
        &mut self,
        library: Library,
        (subscriptions, history): (usize, usize),
        revision: u64,
        current: u64,
    ) -> bool {
        if revision != current {
            self.notice = Some("The library changed while importing. Retry the import to keep your latest changes.".into());
            return false;
        }
        self.library = library;
        self.notice = Some(crate::i18n::format(
            self.settings.language,
            "Imported {subscriptions} new subscriptions and {history} new videos of history.",
            &[
                ("subscriptions", subscriptions.to_string()),
                ("history", history.to_string()),
            ],
        ));
        true
    }
}

impl App {
    /// Loads what a view shows: always when `force`d (Refresh, a new
    /// sign-in), otherwise only what is not there yet or failed.
    pub(super) fn load(&mut self, view: View, force: bool) {
        fn stale<T>(listing: &Listing<T>) -> bool {
            matches!(listing, Listing::Idle | Listing::Failed(_))
        }
        fn stale_fetch<T>(fetch: &Fetch<T>) -> bool {
            matches!(fetch, Fetch::Idle | Fetch::Failed(_))
        }
        let account = self.state.source() == Source::Account;
        let state = &self.state;
        match view {
            View::Results
                if force
                    || match state.search_options.kind {
                        innertube::SearchKind::Videos => stale(&state.results),
                        innertube::SearchKind::Channels => stale(&state.results_channels),
                        innertube::SearchKind::Playlists => stale(&state.results_playlists),
                    } =>
            {
                if let Some(request) = state.search_request() {
                    self.request(Slot::Search, request);
                }
            }
            View::Home if force || stale(&state.feed) || stale(&state.for_you) => {
                if force || stale(&state.for_you) {
                    self.load_for_you();
                }
                let state = &self.state;
                if !force && !stale(&state.feed) {
                    self.ask_shelf();
                    return;
                }
                let signed_in = state.signed_in();
                let channels = state.device_only_subscriptions();
                self.state.feed_shown = 40;
                self.state.feed_more = None;
                self.state.feed_more_state = Fetch::Idle;
                self.state.shorts = Listing::Idle;
                if channels.is_empty() && !signed_in {
                    self.state.feed = Listing::Ready(Vec::new());
                } else {
                    self.request(
                        Slot::Feed,
                        Request::Feed {
                            channels,
                            account: signed_in,
                        },
                    );
                }
            }
            View::Subscriptions if account && (force || stale(&state.account_subscriptions)) => {
                self.request(Slot::Subscriptions, Request::Subscriptions);
            }
            View::History if account && (force || stale(&state.account_history)) => {
                self.request(Slot::History, Request::History);
            }
            View::WatchLater if account && (force || stale(&state.account_watch_later)) => {
                self.request(Slot::WatchLater, Request::WatchLater);
            }
            View::Liked if force || stale(&state.liked) => {
                self.request(Slot::Liked, Request::Liked)
            }
            View::Playlists if force || stale(&state.playlists) => {
                self.request(Slot::Playlists, Request::Playlists);
            }
            View::Playlist if force || stale(&state.page_videos) => {
                if let Some(Page::Playlist { id, .. }) = &state.page {
                    let request = Request::Playlist(id.clone());
                    self.request(Slot::Page, request);
                }
            }
            View::Channel => {
                let Some(Page::Channel(channel)) = &state.page else {
                    return;
                };
                let id = channel.id.clone();
                match &self.state.channel_tab {
                    // The About pane has no rows to refresh.
                    ChannelTab::About => {
                        if force || stale_fetch(&self.state.channel_about) {
                            self.state.channel_about = Fetch::Loading;
                            let request = self.state.channel_about_request(id);
                            self.send_request(Slot::Page, request);
                        }
                    }
                    ChannelTab::Playlists => {
                        if force || stale(&self.state.page_playlists) {
                            self.state.page_playlists = Listing::Loading;
                            self.send_request(
                                Slot::Page,
                                Request::ChannelTab {
                                    id,
                                    tab: ChannelTab::Playlists,
                                    order: self.state.channel_order,
                                },
                            );
                        }
                    }
                    tab if force || stale(&self.state.page_videos) => {
                        self.request(
                            Slot::Page,
                            Request::ChannelTab {
                                id,
                                tab: tab.clone(),
                                order: self.state.channel_order,
                            },
                        );
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        // Back on Home with the feed already here, its shelf is due.
        self.ask_shelf();
    }

    /// Asks for Home's For you: the account's when signed in, with this
    /// computer's history as the seeds either way, in case it has none.
    fn load_for_you(&mut self) {
        let state = &self.state;
        let signed_in = state.signed_in();
        let seeds = crate::recommend::seeds(&state.library.history);
        if seeds.is_empty() && !signed_in {
            self.state.for_you = Listing::Ready(Vec::new());
            return;
        }
        let taste = crate::recommend::taste(&state.library.history, &state.library.subscriptions);
        self.request(
            Slot::ForYou,
            Request::ForYou {
                account: signed_in,
                seeds,
                taste,
            },
        );
    }

    /// Asks the backend for a list, marking it loading.
    pub(super) fn request(&mut self, slot: Slot, request: Request) {
        let state = &mut self.state;
        // A fresh first page drops the list's continuation too.
        match slot {
            Slot::Search => {
                state.start_search();
            }
            Slot::Feed => {
                state.feed = Listing::Loading;
                self.shelf_asked = false;
            }
            Slot::Subscriptions => {
                state.account_subscriptions = Listing::Loading;
                state.subscriptions_more = None;
                state.subscriptions_more_state = Fetch::Idle;
            }
            Slot::History => {
                state.account_history = Listing::Loading;
                state.history_more = None;
                state.history_more_state = Fetch::Idle;
            }
            Slot::WatchLater => {
                state.account_watch_later = Listing::Loading;
                state.watch_later_more = None;
                state.watch_later_more_state = Fetch::Idle;
            }
            Slot::Liked => {
                state.liked = Listing::Loading;
                state.liked_more = None;
                state.liked_more_state = Fetch::Idle;
            }
            Slot::Playlists => {
                state.playlists = Listing::Loading;
                state.playlists_more = None;
                state.playlists_more_state = Fetch::Idle;
            }
            Slot::Page => {
                state.page_videos = Listing::Loading;
                state.page_more = None;
                state.page_more_state = Fetch::Idle;
            }
            Slot::Shorts => state.shorts = Listing::Loading,
            Slot::Alerts => {}
            Slot::ForYou => {
                state.for_you = Listing::Loading;
                state.for_you_more = None;
                state.for_you_more_state = Fetch::Idle;
            }
        }
        self.send_request(slot, request);
    }

    /// `request` without touching the listing: a request whose answer
    /// augments what is already shown (the shelf's shorts).
    pub(super) fn send_request(&mut self, slot: Slot, request: Request) {
        self.generation += 1;
        self.generations.insert(slot, self.generation);
        self.backend.send(Command::Load {
            slot,
            generation: self.generation,
            request,
        });
    }

    /// Asks for the next page of what a slot shows.
    fn send_continuation(&mut self, slot: Slot, request: Request) {
        // The slot's generation, not the latest anywhere: a request on
        // another slot must not strand this one.
        let generation = self.generations.get(&slot).copied().unwrap_or(0);
        self.backend.send(Command::Load {
            slot,
            generation,
            request,
        });
    }

    /// Shows more of Home's feed or of the page's list, asking for the
    /// next page when the rows here run out.
    pub(super) fn load_more(&mut self) {
        if self.state.view == View::Home && self.state.feed_filter == FeedFilter::ForYou {
            if !matches!(self.state.for_you_more_state, Fetch::Loading)
                && let Some(more) = self.state.for_you_more.clone()
            {
                self.state.for_you_more_state = Fetch::Loading;
                self.send_continuation(Slot::ForYou, Request::MoreVideos(more));
            }
            return;
        }
        if self.state.view == View::Home {
            let len = self.state.feed.items().len();
            if self.state.feed_shown < len {
                // More rows are already here; draw them.
                self.state.feed_shown += 40;
            } else if !matches!(self.state.feed_more_state, Fetch::Loading)
                && let Some(more) = self.state.feed_more.clone()
            {
                self.state.feed_more_state = Fetch::Loading;
                self.send_continuation(Slot::Feed, Request::MoreFeed(more));
            }
            return;
        }
        let Some((slot, request)) = self.continuation() else {
            return;
        };
        // Mark the continuation on its way, so a double click on its
        // button cannot send the same page twice.
        let state = match slot {
            Slot::Search => &mut self.state.results_more_state,
            Slot::Subscriptions => &mut self.state.subscriptions_more_state,
            Slot::History => &mut self.state.history_more_state,
            Slot::WatchLater => &mut self.state.watch_later_more_state,
            Slot::Liked => &mut self.state.liked_more_state,
            Slot::Playlists => &mut self.state.playlists_more_state,
            Slot::Page => &mut self.state.page_more_state,
            _ => return,
        };
        *state = Fetch::Loading;
        self.send_continuation(slot, request);
    }

    /// The continuation of the list the page on screen shows: the slot
    /// its next page fills, and the request that asks for it. `None`
    /// when the page's list is not paged or its next page is already on
    /// its way.
    fn continuation(&self) -> Option<(Slot, Request)> {
        let in_flight = |fetch: &Fetch<()>| !matches!(fetch, Fetch::Loading);
        let state = &self.state;
        // The channel page's tabs and a playlist's page all continue
        // through the page slot: the playlists tab asks for playlists,
        // every other list videos.
        let page = match state.view {
            View::Channel => Some(matches!(state.channel_tab, ChannelTab::Playlists)),
            View::Playlist => Some(false),
            _ => None,
        };
        if let Some(playlists) = page {
            return state
                .page_more
                .clone()
                .filter(|_| in_flight(&state.page_more_state))
                .map(|more| (Slot::Page, Request::More { more, playlists }));
        }
        match state.view {
            View::Results => state.search_continuation(),
            View::Subscriptions => state
                .subscriptions_more
                .clone()
                .filter(|_| in_flight(&state.subscriptions_more_state))
                .map(|more| (Slot::Subscriptions, Request::MoreSubscriptions(more))),
            View::History if state.source() == Source::Account => state
                .history_more
                .clone()
                .filter(|_| in_flight(&state.history_more_state))
                .map(|more| (Slot::History, Request::MoreHistory(more))),
            View::WatchLater if state.source() == Source::Account => state
                .watch_later_more
                .clone()
                .filter(|_| in_flight(&state.watch_later_more_state))
                .map(|more| (Slot::WatchLater, Request::MoreVideos(more))),
            View::Liked => state
                .liked_more
                .clone()
                .filter(|_| in_flight(&state.liked_more_state))
                .map(|more| (Slot::Liked, Request::MoreVideos(more))),
            View::Playlists => state
                .playlists_more
                .clone()
                .filter(|_| in_flight(&state.playlists_more_state))
                .map(|more| (Slot::Playlists, Request::MorePlaylists(more))),
            _ => None,
        }
    }

    /// Takes in a Takeout import and reloads the view with it.
    pub(super) fn on_imported(&mut self, import: Import) {
        self.backend.send(Command::MergeImport {
            library: Box::new(self.state.library.clone()),
            import,
            revision: self.library_revision.get(),
        });
    }

    pub(super) fn on_import_merged(
        &mut self,
        library: Library,
        counts: (usize, usize),
        revision: u64,
    ) {
        if !self
            .state
            .accept_import(library, counts, revision, self.library_revision.get())
        {
            return;
        }
        self.save_library();
        self.state.feed = Listing::Idle;
        self.state.for_you = Listing::Idle;
        self.load(self.state.view, false);
    }

    /// Puts a loaded list where its slot's view reads it.
    pub(super) fn fill(&mut self, slot: Slot, result: Result<Payload, String>) {
        fn put<T>(listing: &mut Listing<T>, result: Result<Vec<T>, String>) {
            *listing = match result {
                Ok(items) => Listing::Ready(items),
                Err(error) => Listing::Failed(error),
            };
        }
        let state = &mut self.state;
        let mut feed_page = None;
        match (slot, result) {
            (Slot::Search, result) => state.fill_search(result),
            (
                Slot::Page,
                Ok(Payload::PlaylistPage {
                    details,
                    items,
                    more,
                }),
            ) => {
                state.fill_playlist(details, items, more);
            }
            (Slot::Feed, Ok(Payload::Feed { videos, more })) => {
                feed_page = Some(videos.clone());
                // Shorts leave the list for the shelf.
                let (shorts, videos): (Vec<Video>, Vec<Video>) =
                    videos.into_iter().partition(|video| video.short);
                if !shorts.is_empty() {
                    state.shorts = Listing::Ready(shorts);
                }
                state.feed_more = more;
                state.feed_more_state = Fetch::Idle;
                state.feed_shown = 40;
                put(&mut state.feed, Ok(videos));
                state.feed_loaded_at = Some(util::now());
                self.ask_shelf();
            }
            (Slot::Feed, Ok(Payload::More { items, more })) => {
                state.feed_more = more;
                state.feed_more_state = Fetch::Idle;
                if let PageItems::Videos(videos) = items {
                    state.feed_shown +=
                        merge_feed_page(state.feed.items_mut(), state.shorts.items_mut(), videos);
                }
            }
            (Slot::Shorts, Ok(Payload::Videos(videos))) => {
                // RSS shorts leave the list and join the shelf, after the
                // account's, deduped.
                let known: HashSet<String> = state
                    .shorts
                    .items()
                    .iter()
                    .map(|video| video.id.clone())
                    .collect();
                state.shorts.items_mut().extend(
                    videos
                        .into_iter()
                        .filter(|video| !known.contains(&video.id)),
                );
                let on_shelf: HashSet<String> = state
                    .shorts
                    .items()
                    .iter()
                    .map(|video| video.id.clone())
                    .collect();
                if let Listing::Ready(rows) = &mut state.feed {
                    rows.retain(|video| !on_shelf.contains(&video.id));
                }
            }
            (Slot::WatchLater, Ok(Payload::VideoList { items, more })) => {
                state.watch_later_more = more;
                state.watch_later_more_state = Fetch::Idle;
                put(&mut state.account_watch_later, Ok(items));
            }
            (Slot::Liked, Ok(Payload::VideoList { items, more })) => {
                state.liked_more = more;
                state.liked_more_state = Fetch::Idle;
                put(&mut state.liked, Ok(items));
            }
            (Slot::Page, Ok(Payload::VideoList { items, more })) => {
                state.page_more = more;
                state.page_more_state = Fetch::Idle;
                put(&mut state.page_videos, Ok(items));
            }
            (
                Slot::Page,
                Ok(Payload::ChannelPage {
                    header,
                    items,
                    more,
                }),
            ) => {
                self.open_channel = Some(header.id.clone());
                state.fill_channel_page(header, items, more);
            }
            (Slot::Page, Ok(Payload::About { header, about })) => {
                if let Some(header) = &header {
                    self.open_channel = Some(header.id.clone());
                }
                state.fill_channel_about(header.map(|header| *header), about);
            }
            (Slot::Page, Ok(Payload::More { items, more })) => {
                state.page_more = more;
                state.page_more_state = Fetch::Idle;
                match items {
                    PageItems::Videos(videos) => {
                        state.page_videos.items_mut().extend(videos);
                    }
                    PageItems::Playlists(items) => {
                        state.page_playlists.items_mut().extend(items);
                    }
                    _ => {}
                }
            }
            (Slot::Subscriptions, Ok(Payload::Channels { items, more })) => {
                state.subscriptions_more = more;
                state.subscriptions_more_state = Fetch::Idle;
                put(&mut state.account_subscriptions, Ok(items));
            }
            (Slot::Subscriptions, Ok(Payload::More { items, more })) => {
                state.subscriptions_more = more;
                state.subscriptions_more_state = Fetch::Idle;
                if let PageItems::Channels(channels) = items {
                    let subscriptions = state.account_subscriptions.items_mut();
                    subscriptions.extend(channels);
                    // The list stays by name over the added page.
                    subscriptions.sort_by_cached_key(|channel| channel.name.to_lowercase());
                }
            }
            (Slot::History, Ok(Payload::History { items, more })) => {
                state.history_more = more;
                state.history_more_state = Fetch::Idle;
                put(&mut state.account_history, Ok(items));
            }
            (Slot::History, Ok(Payload::More { items, more })) => {
                state.history_more = more;
                state.history_more_state = Fetch::Idle;
                if let PageItems::History(rows) = items {
                    state.account_history.items_mut().extend(rows);
                }
            }
            (Slot::WatchLater, Ok(Payload::More { items, more })) => {
                state.watch_later_more = more;
                state.watch_later_more_state = Fetch::Idle;
                if let PageItems::Videos(videos) = items {
                    state.account_watch_later.items_mut().extend(videos);
                }
            }
            (Slot::Liked, Ok(Payload::More { items, more })) => {
                state.liked_more = more;
                state.liked_more_state = Fetch::Idle;
                if let PageItems::Videos(videos) = items {
                    state.liked.items_mut().extend(videos);
                }
            }
            (Slot::Playlists, Ok(Payload::Playlists { items, more })) => {
                state.playlists_more = more;
                state.playlists_more_state = Fetch::Idle;
                put(&mut state.playlists, Ok(items));
            }
            (Slot::Playlists, Ok(Payload::More { items, more })) => {
                state.playlists_more = more;
                state.playlists_more_state = Fetch::Idle;
                if let PageItems::Playlists(lists) = items {
                    state.playlists.items_mut().extend(lists);
                }
            }
            (Slot::Feed, Err(error)) => {
                // A continuation failing keeps the feed already shown.
                if matches!(state.feed_more_state, Fetch::Loading) {
                    state.feed_more_state = Fetch::Failed(error);
                } else {
                    put(&mut state.feed, Err(error));
                }
            }
            (Slot::Shorts, Err(_)) => {}
            (
                Slot::ForYou,
                Ok(Payload::ForYou {
                    videos,
                    more,
                    from_account,
                }),
            ) => {
                state.for_you_more = more;
                state.for_you_more_state = Fetch::Idle;
                state.for_you_from_account = from_account;
                put(&mut state.for_you, Ok(videos));
            }
            (Slot::ForYou, Ok(Payload::More { items, more })) => {
                state.for_you_more = more;
                state.for_you_more_state = Fetch::Idle;
                if let PageItems::Videos(videos) = items {
                    let list = state.for_you.items_mut();
                    for video in videos {
                        if !video.short && !list.iter().any(|known| known.id == video.id) {
                            list.push(video);
                        }
                    }
                }
            }
            (Slot::ForYou, Err(error)) => {
                if matches!(state.for_you_more_state, Fetch::Loading) {
                    state.for_you_more_state = Fetch::Failed(error);
                } else {
                    put(&mut state.for_you, Err(error));
                }
            }
            (Slot::Alerts, Ok(Payload::Feed { videos, .. })) => {
                feed_page = Some(videos);
            }
            (Slot::Alerts, Err(error)) => {
                log::warn!("the alerts check failed: {error}");
                state.alerts_failed(error);
            }
            // A continuation failing keeps the rows already shown; its
            // error lands on the continuation's own state.
            (Slot::WatchLater, Err(error)) => {
                if matches!(state.watch_later_more_state, Fetch::Loading) {
                    state.watch_later_more_state = Fetch::Failed(error);
                } else {
                    put(&mut state.account_watch_later, Err(error));
                }
            }
            (Slot::Liked, Err(error)) => {
                if matches!(state.liked_more_state, Fetch::Loading) {
                    state.liked_more_state = Fetch::Failed(error);
                } else {
                    put(&mut state.liked, Err(error));
                }
            }
            (Slot::Page, Err(error)) => {
                if matches!(state.page_more_state, Fetch::Loading) {
                    state.page_more_state = Fetch::Failed(error);
                } else if state.view == View::Playlist {
                    put(&mut state.page_videos, Err(error));
                } else {
                    match state.channel_tab {
                        ChannelTab::About => state.channel_about = Fetch::Failed(error),
                        ChannelTab::Playlists => put(&mut state.page_playlists, Err(error)),
                        _ => put(&mut state.page_videos, Err(error)),
                    }
                }
            }
            (Slot::Subscriptions, Err(error)) => {
                if matches!(state.subscriptions_more_state, Fetch::Loading) {
                    state.subscriptions_more_state = Fetch::Failed(error);
                } else {
                    put(&mut state.account_subscriptions, Err(error));
                }
            }
            (Slot::History, Err(error)) => {
                if matches!(state.history_more_state, Fetch::Loading) {
                    state.history_more_state = Fetch::Failed(error);
                } else {
                    put(&mut state.account_history, Err(error));
                }
            }
            (Slot::Playlists, Err(error)) => {
                if matches!(state.playlists_more_state, Fetch::Loading) {
                    state.playlists_more_state = Fetch::Failed(error);
                } else {
                    put(&mut state.playlists, Err(error));
                }
            }
            (slot, Ok(_)) => log::error!("{slot:?} received another kind of list"),
        }
        // The feed's own load doubles as a check, only while alerts are on.
        if let Some(videos) = feed_page {
            if self.state.settings.alerts {
                self.note_alerts(videos);
            } else {
                self.state.alerts_checking = false;
            }
        }
    }

    /// While Home shows it, ask the Shorts tabs of the device
    /// subscriptions the feed shows (up to 12, ordered by their newest
    /// video), once per feed load, for the shelf.
    fn ask_shelf(&mut self) {
        if self.shelf_asked || self.state.view != View::Home {
            return;
        }
        let mut seen: HashSet<String> = HashSet::new();
        let channels: Vec<String> = self
            .state
            .feed
            .items()
            .iter()
            .filter_map(|video| video.channel_id.clone())
            .filter(|id| {
                self.state
                    .library
                    .subscriptions
                    .iter()
                    .any(|channel| channel.id == *id)
            })
            .filter(|id| seen.insert(id.clone()))
            .take(12)
            .collect();
        if channels.is_empty() {
            return;
        }
        self.shelf_asked = true;
        // The shelf keeps what the feed already put there.
        self.send_request(Slot::Shorts, Request::Shorts { channels });
    }

    pub(super) fn open_page(&mut self, ctx: &egui::Context, view: View, mut page: Page) {
        if let Page::Channel(channel) = &mut page
            && channel.name.is_empty()
            && let Some(known) = self
                .state
                .library
                .subscriptions
                .iter()
                .find(|subscribed| subscribed.id == channel.id)
        {
            channel.name = known.name.clone();
            channel.avatar = known.avatar.clone();
        }
        if self.state.view != view || self.state.page.as_ref() != Some(&page) {
            self.push_location();
        }
        if self.state.page.as_ref() != Some(&page) {
            self.state.playlist_details = None;
            self.state.page_videos = Listing::Idle;
            self.state.page_more = None;
            self.state.page_more_state = Fetch::Idle;
        }
        // Another channel restarts its page: tabs, header, search field.
        let channel_id = match &page {
            Page::Channel(channel) => Some(channel.id.clone()),
            _ => None,
        };
        if view == View::Channel && self.open_channel.as_deref() != channel_id.as_deref() {
            self.state.channel_header = None;
            self.state.channel_tab = ChannelTab::default();
            self.state.page_playlists = Listing::Idle;
            self.state.channel_about = Fetch::Idle;
            self.state.channel_query.clear();
        }
        self.open_channel = channel_id;
        self.leave_video(ctx);
        self.state.page = Some(page);
        self.state.view = view;
        self.load(view, false);
    }

    /// Switches the channel page's tab and loads the tab's list.
    pub(super) fn set_channel_tab(&mut self, tab: ChannelTab) {
        if self.state.channel_tab == tab {
            return;
        }
        self.state.channel_tab = tab;
        self.state.page_more = None;
        self.state.page_more_state = Fetch::Idle;
        // The current tab's list reloads; the other tab keeps what
        // it has but draws nothing until picked.
        match &self.state.channel_tab {
            ChannelTab::About => self.state.channel_about = Fetch::Loading,
            ChannelTab::Playlists => self.state.page_playlists = Listing::Loading,
            _ => self.state.page_videos = Listing::Loading,
        }
        if let Some(Page::Channel(channel)) = &self.state.page {
            let request = match &self.state.channel_tab {
                ChannelTab::About => self.state.channel_about_request(channel.id.clone()),
                tab => Request::ChannelTab {
                    id: channel.id.clone(),
                    tab: tab.clone(),
                    order: self.state.channel_order,
                },
            };
            self.send_request(Slot::Page, request);
        }
    }
}

/// A feed continuation page: shorts to the shelf, the rest appended
/// without repeats. Returns how many rows landed in the feed.
fn merge_feed_page(feed: &mut Vec<Video>, shorts: &mut Vec<Video>, page: Vec<Video>) -> usize {
    let (shelf, rest): (Vec<Video>, Vec<Video>) = page.into_iter().partition(|video| video.short);
    let on_shelf: HashSet<String> = shorts.iter().map(|video| video.id.clone()).collect();
    shorts.extend(
        shelf
            .into_iter()
            .filter(|video| !on_shelf.contains(&video.id)),
    );
    let known: HashSet<String> = feed.iter().map(|video| video.id.clone()).collect();
    let fresh: Vec<Video> = rest
        .into_iter()
        .filter(|video| !known.contains(&video.id))
        .collect();
    let landed = fresh.len();
    feed.extend(fresh);
    landed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn more(token: &str) -> innertube::More {
        innertube::More {
            ctoken: token.into(),
            endpoint: rustypipe::model::paginator::ContinuationEndpoint::Search,
            visitor_data: None,
            authenticated: false,
            ours: false,
        }
    }

    fn search_items(kind: innertube::SearchKind, id: &str) -> PageItems {
        match kind {
            innertube::SearchKind::Videos => PageItems::Videos(vec![Video::from_id(id.into())]),
            innertube::SearchKind::Channels => PageItems::Channels(vec![Channel {
                id: id.into(),
                name: format!("Canal {id}"),
                avatar: None,
            }]),
            innertube::SearchKind::Playlists => PageItems::Playlists(vec![PlaylistSummary {
                id: id.into(),
                name: format!("Playlist {id}"),
                thumbnail: None,
                channel: None,
                videos: Some(2),
            }]),
        }
    }

    fn search_ids(state: &State) -> Vec<&str> {
        match state.search_options.kind {
            innertube::SearchKind::Videos => state
                .results
                .items()
                .iter()
                .map(|row| row.id.as_str())
                .collect(),
            innertube::SearchKind::Channels => state
                .results_channels
                .items()
                .iter()
                .map(|row| row.id.as_str())
                .collect(),
            innertube::SearchKind::Playlists => state
                .results_playlists
                .items()
                .iter()
                .map(|row| row.id.as_str())
                .collect(),
        }
    }

    #[test]
    fn retry_search_keeps_the_submitted_query_and_all_filter_choices() {
        let options = innertube::SearchOptions {
            kind: innertube::SearchKind::Videos,
            order: innertube::SearchOrder::Views,
            date: innertube::SearchDate::Week,
            length: innertube::SearchLength::Long,
            live: true,
        };
        let mut state = State {
            query: "Texto ainda não enviado".into(),
            search_query: "Rust".into(),
            search_options: options,
            ..Default::default()
        };
        state.start_search();
        state.fill_search(Err("Falha temporária".into()));
        assert!(matches!(state.results, Listing::Failed(_)));
        let Some(Request::Search {
            query,
            options: sent,
        }) = state.search_request()
        else {
            panic!("retry must search again")
        };
        assert_eq!(query, "Rust");
        assert_eq!(sent, options);
        state.start_search();
        assert!(matches!(state.results, Listing::Loading));
        state.search_query.clear();
        assert!(state.search_request().is_none());
    }

    #[test]
    fn changing_search_kind_discards_old_rows_and_continuations_before_loading() {
        for &kind in innertube::SearchKind::ALL {
            let mut state = State {
                results: Listing::Ready(vec![Video::from_id("old".into())]),
                results_channels: Listing::Ready(vec![Channel::default()]),
                results_playlists: Listing::Ready(vec![PlaylistSummary {
                    id: "old".into(),
                    name: "Old".into(),
                    thumbnail: None,
                    channel: None,
                    videos: None,
                }]),
                results_more: Some(more("old-page")),
                results_more_state: Fetch::Failed("old error".into()),
                ..Default::default()
            };
            state.search_options.kind = kind;
            state.start_search();
            assert!(
                state.results.items().is_empty()
                    && state.results_channels.items().is_empty()
                    && state.results_playlists.items().is_empty()
            );
            assert!(state.results_more.is_none());
            assert!(matches!(state.results_more_state, Fetch::Idle));
            match kind {
                innertube::SearchKind::Videos => assert!(matches!(state.results, Listing::Loading)),
                innertube::SearchKind::Channels => {
                    assert!(matches!(state.results_channels, Listing::Loading))
                }
                innertube::SearchKind::Playlists => {
                    assert!(matches!(state.results_playlists, Listing::Loading))
                }
            }
        }
    }

    #[test]
    fn every_search_kind_keeps_loaded_rows_when_paging_fails_and_can_retry() {
        for &kind in innertube::SearchKind::ALL {
            let mut state = State::default();
            state.search_options.kind = kind;
            state.start_search();
            state.fill_search(Ok(Payload::Search {
                items: search_items(kind, "first"),
                more: Some(more("page-2")),
            }));
            assert_eq!(search_ids(&state), ["first"]);
            let Some((
                Slot::Search,
                Request::MoreSearch {
                    more: next,
                    kind: sent,
                },
            )) = state.search_continuation()
            else {
                panic!("missing typed continuation")
            };
            assert_eq!(sent, kind);
            assert_eq!(next.ctoken, "page-2");
            state.results_more_state = Fetch::Loading;
            assert!(state.search_continuation().is_none());
            state.fill_search(Err("Falha na próxima página".into()));
            assert!(matches!(state.results_more_state, Fetch::Failed(_)));
            assert_eq!(search_ids(&state), ["first"]);
            assert!(state.search_continuation().is_some());
            state.results_more_state = Fetch::Loading;
            state.fill_search(Ok(Payload::More {
                items: search_items(kind, "second"),
                more: None,
            }));
            assert_eq!(search_ids(&state), ["first", "second"]);
            assert!(state.search_continuation().is_none());
            assert!(matches!(state.results_more_state, Fetch::Idle));
        }
    }

    #[test]
    fn first_page_failure_targets_the_selected_search_kind() {
        for &kind in innertube::SearchKind::ALL {
            let mut state = State::default();
            state.search_options.kind = kind;
            state.start_search();
            state.fill_search(Err("Serviço indisponível".into()));
            match kind {
                innertube::SearchKind::Videos => {
                    assert!(matches!(state.results, Listing::Failed(_)))
                }
                innertube::SearchKind::Channels => {
                    assert!(matches!(state.results_channels, Listing::Failed(_)))
                }
                innertube::SearchKind::Playlists => {
                    assert!(matches!(state.results_playlists, Listing::Failed(_)))
                }
            }
            assert!(matches!(state.results_more_state, Fetch::Idle));
        }
    }

    #[test]
    fn playlist_metadata_and_video_order_land_together_without_replacing_the_page_identity() {
        let page = Page::Playlist {
            id: "playlist-id".into(),
            name: "Nome conhecido".into(),
        };
        let mut state = State {
            view: View::Playlist,
            page: Some(page.clone()),
            ..Default::default()
        };
        let details = innertube::PlaylistDetails {
            name: "Nome atualizado".into(),
            description: "Descrição".into(),
            channel: Some("Canal".into()),
            videos: Some(30),
            ..Default::default()
        };
        state.fill_playlist(
            details.clone(),
            vec![
                Video::from_id("first".into()),
                Video::from_id("second".into()),
            ],
            Some(more("next")),
        );
        assert_eq!(state.page, Some(page));
        assert_eq!(state.playlist_details.as_ref(), Some(&details));
        assert_eq!(
            state
                .page_videos
                .items()
                .iter()
                .map(|video| video.id.as_str())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert_eq!(state.page_more.as_ref().unwrap().ctoken, "next");
        assert!(matches!(state.page_more_state, Fetch::Idle));
    }

    #[test]
    fn channel_header_completes_a_channel_opened_from_a_link() {
        let linked = youtube::channel_from_link("https://www.youtube.com/@BlenderStudio").unwrap();
        let mut state = State {
            view: View::Channel,
            page: Some(Page::Channel(linked)),
            ..Default::default()
        };
        let header = ChannelHeader {
            id: "UCz75RVbH8q2jdBJ4SnwuZZQ".into(),
            name: "Blender Studio".into(),
            handle: Some("@BlenderStudio".into()),
            avatar: Some("https://yt3.ggpht.com/avatar".into()),
            ..Default::default()
        };
        state.fill_channel_page(
            header,
            PageItems::Videos(vec![Video::from_id("aaaaaaaaaaa".into())]),
            None,
        );
        assert_eq!(
            state.page,
            Some(Page::Channel(Channel {
                id: "UCz75RVbH8q2jdBJ4SnwuZZQ".into(),
                name: "Blender Studio".into(),
                avatar: Some("https://yt3.ggpht.com/avatar".into()),
            }))
        );
    }

    #[test]
    fn about_completes_channel_links_when_the_initial_videos_response_is_superseded() {
        for link in [
            "https://youtube.com/@ExampleChannel",
            "https://youtube.com/channel/UCaaaaaaaaaaaaaaaaaaaaaa",
            "https://youtube.com/c/ExampleChannel",
            "https://youtube.com/user/ExampleChannel",
        ] {
            let linked = youtube::channel_from_link(link).unwrap();
            let mut state = State {
                view: View::Channel,
                page: Some(Page::Channel(linked.clone())),
                channel_tab: ChannelTab::About,
                channel_about: Fetch::Loading,
                page_videos: Listing::Loading,
                ..Default::default()
            };
            assert!(matches!(
                state.channel_about_request(linked.id.clone()),
                Request::ChannelAbout { id, needs_header: true } if id == linked.id
            ));
            let header = ChannelHeader {
                id: "UCaaaaaaaaaaaaaaaaaaaaaa".into(),
                name: "Example Channel".into(),
                avatar: Some("https://example.com/avatar.png".into()),
                has_shorts: true,
                has_live: true,
                ..Default::default()
            };
            let about = ChannelAbout {
                description: "Channel description".into(),
                ..Default::default()
            };
            // Only About's response arrives: the initial Videos generation
            // is obsolete, so it cannot supply the missing header.
            state.fill_channel_about(Some(header.clone()), about.clone());
            assert_eq!(state.channel_tab, ChannelTab::About);
            assert_eq!(state.channel_header, Some(header.clone()));
            assert_eq!(
                state.page,
                Some(Page::Channel(Channel {
                    id: header.id,
                    name: header.name,
                    avatar: header.avatar,
                })),
                "{link}"
            );
            assert!(matches!(state.channel_about, Fetch::Ready(ref value) if value == &about));
            let header = state.channel_header.as_ref().unwrap();
            assert!(header.has_shorts && header.has_live);
        }
    }

    #[test]
    fn about_reuses_an_existing_header_and_keeps_tab_rows_and_continuations() {
        let header = ChannelHeader {
            id: "UCaaaaaaaaaaaaaaaaaaaaaa".into(),
            name: "Example Channel".into(),
            ..Default::default()
        };
        let mut state = State {
            view: View::Channel,
            page: Some(Page::Channel(Channel {
                id: header.id.clone(),
                name: header.name.clone(),
                avatar: None,
            })),
            channel_tab: ChannelTab::About,
            ..Default::default()
        };
        state.fill_channel_page(
            header.clone(),
            PageItems::Videos(vec![Video::from_id("aaaaaaaaaaa".into())]),
            Some(more("next")),
        );
        assert!(matches!(
            state.channel_about_request(header.id.clone()),
            Request::ChannelAbout {
                needs_header: false,
                ..
            }
        ));
        state.fill_channel_about(None, ChannelAbout::default());
        assert_eq!(state.channel_header, Some(header));
        assert_eq!(state.page_videos.items()[0].id, "aaaaaaaaaaa");
        assert_eq!(state.page_more.as_ref().unwrap().ctoken, "next");
        assert!(matches!(state.channel_about, Fetch::Ready(_)));
    }

    #[test]
    fn an_import_cannot_restore_history_cleared_while_it_was_merging() {
        let mut state = State::default();
        let mut old = Library::default();
        old.record_watch(Video::from_id("old-history".into()), 1);
        assert!(!state.accept_import(old, (0, 1), 1, 2));
        assert!(state.library.history.is_empty());
        assert!(state.notice.unwrap().contains("Retry"));
    }

    #[test]
    fn a_continuation_page_splits_shorts_and_skips_repeats() {
        let mut feed = vec![Video::from_id("a".to_owned())];
        let mut shorts = Vec::new();
        let mut new = Video::from_id("b".to_owned());
        new.short = true;
        let page = vec![
            Video::from_id("a".to_owned()),
            new,
            Video::from_id("c".to_owned()),
        ];
        assert_eq!(merge_feed_page(&mut feed, &mut shorts, page), 1);
        assert_eq!(
            feed.iter()
                .map(|video| video.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "c"]
        );
        assert_eq!(
            shorts
                .iter()
                .map(|video| video.id.as_str())
                .collect::<Vec<_>>(),
            ["b"]
        );
    }
}
