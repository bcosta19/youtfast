//! Lists: asking the backend for them, and putting their answers in place.

use super::*;

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
                            self.send_request(Slot::Page, Request::ChannelAbout(id));
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
                state.results = Listing::Loading;
                state.results_more = None;
                state.results_more_state = Fetch::Idle;
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
            View::Results => state
                .results_more
                .clone()
                .filter(|_| in_flight(&state.results_more_state))
                .map(|more| (Slot::Search, Request::MoreVideos(more))),
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
        let (subscriptions, history) = self.state.library.merge(import);
        self.state.notice = Some(format!(
            "Imported {subscriptions} new subscriptions and {history} new videos of history."
        ));
        self.save_library();
        self.state.feed = Listing::Idle;
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
            (Slot::Search, Ok(Payload::VideoList { items, more })) => {
                state.results_more = more;
                state.results_more_state = Fetch::Idle;
                put(&mut state.results, Ok(items));
            }
            (Slot::Search, Ok(Payload::More { items, more })) => {
                state.results_more = more;
                state.results_more_state = Fetch::Idle;
                if let PageItems::Videos(videos) = items {
                    state.results.items_mut().extend(videos);
                }
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
                state.channel_header = Some(header);
                state.page_more = more;
                state.page_more_state = Fetch::Idle;
                match items {
                    PageItems::Videos(videos) => put(&mut state.page_videos, Ok(videos)),
                    PageItems::Playlists(items) => put(&mut state.page_playlists, Ok(items)),
                    _ => {}
                }
            }
            (Slot::Page, Ok(Payload::About(about))) => {
                state.channel_about = Fetch::Ready(about);
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
            (Slot::Search, Err(error)) => {
                if matches!(state.results_more_state, Fetch::Loading) {
                    state.results_more_state = Fetch::Failed(error);
                } else {
                    put(&mut state.results, Err(error));
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

    pub(super) fn open_page(&mut self, ctx: &egui::Context, view: View, page: Page) {
        if self.state.page.as_ref() != Some(&page) {
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
        if self.state.view != view {
            self.back.push(self.state.view);
        }
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
                ChannelTab::About => Request::ChannelAbout(channel.id.clone()),
                tab => Request::ChannelTab {
                    id: channel.id.clone(),
                    tab: tab.clone(),
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
