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
            View::Home if force || stale(&state.feed) => {
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
    }

    /// Asks the backend for a list, marking it loading.
    pub(super) fn request(&mut self, slot: Slot, request: Request) {
        let state = &mut self.state;
        match slot {
            Slot::Search => state.results = Listing::Loading,
            Slot::Feed => state.feed = Listing::Loading,
            Slot::Subscriptions => state.account_subscriptions = Listing::Loading,
            Slot::History => state.account_history = Listing::Loading,
            Slot::WatchLater => state.account_watch_later = Listing::Loading,
            Slot::Liked => state.liked = Listing::Loading,
            Slot::Playlists => state.playlists = Listing::Loading,
            Slot::Page => state.page_videos = Listing::Loading,
            Slot::Shorts => state.shorts = Listing::Loading,
            Slot::Alerts => {}
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

    /// Shows more of Home's feed or of the channel tab, asking for the
    /// next page when the rows here run out.
    pub(super) fn load_more(&mut self) {
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
        // A running continuation cannot be asked twice.
        if self.state.view == View::Channel
            && !matches!(self.state.page_more_state, Fetch::Loading)
            && let Some(more) = self.state.page_more.clone()
        {
            self.state.page_more_state = Fetch::Loading;
            let playlists = self.state.channel_tab == ChannelTab::Playlists;
            self.send_continuation(Slot::Page, Request::More { more, playlists });
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
            (Slot::Search, Ok(Payload::Videos(videos))) => put(&mut state.results, Ok(videos)),
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
            (Slot::WatchLater, Ok(Payload::Videos(videos))) => {
                put(&mut state.account_watch_later, Ok(videos));
            }
            (Slot::Liked, Ok(Payload::Videos(videos))) => put(&mut state.liked, Ok(videos)),
            (Slot::Page, Ok(Payload::Videos(videos))) => put(&mut state.page_videos, Ok(videos)),
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
                }
            }
            (Slot::Subscriptions, Ok(Payload::Channels(channels))) => {
                put(&mut state.account_subscriptions, Ok(channels));
            }
            (Slot::History, Ok(Payload::History(rows))) => {
                put(&mut state.account_history, Ok(rows))
            }
            (Slot::Playlists, Ok(Payload::Playlists(lists))) => {
                put(&mut state.playlists, Ok(lists))
            }
            (Slot::Search, Err(error)) => put(&mut state.results, Err(error)),
            (Slot::Feed, Err(error)) => {
                // A continuation failing keeps the feed already shown.
                if matches!(state.feed_more_state, Fetch::Loading) {
                    state.feed_more_state = Fetch::Failed(error);
                } else {
                    put(&mut state.feed, Err(error));
                }
            }
            (Slot::Shorts, Err(_)) => {}
            (Slot::Alerts, Ok(Payload::Feed { videos, .. })) => {
                feed_page = Some(videos);
            }
            (Slot::Alerts, Err(error)) => {
                log::warn!("the alerts check failed: {error}");
            }
            (Slot::WatchLater, Err(error)) => put(&mut state.account_watch_later, Err(error)),
            (Slot::Liked, Err(error)) => put(&mut state.liked, Err(error)),
            (Slot::Page, Err(error)) => {
                // A continuation failing keeps the rows already shown.
                if matches!(state.page_more_state, Fetch::Loading) {
                    state.page_more_state = Fetch::Failed(error);
                } else {
                    match state.channel_tab {
                        ChannelTab::About => state.channel_about = Fetch::Failed(error),
                        ChannelTab::Playlists => put(&mut state.page_playlists, Err(error)),
                        _ => put(&mut state.page_videos, Err(error)),
                    }
                }
            }
            (Slot::Subscriptions, Err(error)) => put(&mut state.account_subscriptions, Err(error)),
            (Slot::History, Err(error)) => put(&mut state.account_history, Err(error)),
            (Slot::Playlists, Err(error)) => put(&mut state.playlists, Err(error)),
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

    /// After the feed arrives, ask the Shorts tabs of the device
    /// subscriptions it shows (up to 12, ordered by their newest video),
    /// for Home's shelf.
    fn ask_shelf(&mut self) {
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
