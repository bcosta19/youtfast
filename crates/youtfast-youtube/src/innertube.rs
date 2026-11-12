//! The account and the feeds: YouTube's own API (InnerTube) with the
//! signed-in session, and channel RSS without one. Everything here runs on
//! the backend runtime and returns the app's types.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use youtfast_core::model::{
    Channel, ChannelHeader, ChannelTab, HistoryRow, PlaylistSummary, Video,
};
use youtfast_core::util;

pub mod channel;
pub mod client;
mod discovery;
mod edit;
mod history_sync;
mod home;
pub mod items;
mod playlist;
pub mod rss;
pub mod watch;

pub use channel::{
    ChannelVideoTab, channel_about, channel_header_counts, channel_order_token,
    channel_playlists_page, channel_tab, channel_tab_params,
};
pub use client::{AuthError, Client, ContinuationEndpoint, Error};
pub use discovery::{
    ChannelOrder, PlaylistDetails, SearchDate, SearchKind, SearchLength, SearchOptions,
    SearchOrder, playlist_details, search_filtered, suggestions,
};
pub use edit::{rate, rating, save_targets, set_saved};
pub use history_sync::mark_watched;
pub use home::recommended;
pub use items::absolute;
pub use watch::{comments, details};

/// Channel feeds fetched at once: RSS is cheap, but hundreds of
/// subscriptions should not open hundreds of connections.
const RSS_CONCURRENCY: usize = 8;
/// How long a channel's RSS answer is kept: uploads take minutes to
/// reach RSS anyway, so re-fetching sooner buys nothing.
const RSS_TTL: i64 = 10 * 60;
/// Channel RSS answers by channel id (when, videos), shared by the feed
/// and the alerts check so neither re-fetches what the other just got.
pub type RssCache = std::sync::Mutex<HashMap<String, (i64, Vec<Video>)>>;

/// Builds the InnerTube client. Session cookies live only in memory and are
/// never persisted to disk by this client.
pub fn client() -> Result<Client, Error> {
    Client::new(http())
}

/// The HTTP client under InnerTube. Certificates are checked by the
/// operating system's verifier rather than against its roots read once: a
/// fresh Windows keeps a few roots and downloads the others only when its
/// own verifier needs them, so until something else (a browser) had
/// reached Google, every request failed. Without the platform verifier,
/// reqwest's own (the native roots) is kept.
pub fn http() -> reqwest::ClientBuilder {
    let builder = reqwest::ClientBuilder::new();
    match platform_tls() {
        Some(config) => builder.use_preconfigured_tls(config),
        None => builder,
    }
}

fn platform_tls() -> Option<rustls::ClientConfig> {
    use rustls_platform_verifier::BuilderVerifierExt;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    Some(
        rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .ok()?
            .with_platform_verifier()
            .ok()?
            .with_no_client_auth(),
    )
}

/// Whether the failure means the session is gone (signed out elsewhere,
/// expired, or rotated away by the browser it came from). A 403 is not
/// always that: YouTube's bot check and rate limits answer 403 too, and
/// say so in the message — calling those an expired session sends the
/// user to sign in again over a session that is fine.
pub fn is_signed_out(error: &Error) -> bool {
    match error {
        Error::Auth(_) | Error::HttpStatus(401, _) => true,
        Error::HttpStatus(403, message) => {
            let message = message.to_lowercase();
            !(message.contains("bot") || message.contains("rate") || message.contains("quota"))
        }
        _ => false,
    }
}

/// An error as text for the user and the log: reqwest's message can end in
/// the request's URL ("... for url (...)"), which helps nobody.
pub fn text(error: Error) -> String {
    youtfast_core::privacy::safe_error(&error.to_string())
}

/// Starts using a session, checking it with YouTube first.
pub async fn sign_in(rp: &Client, cookie: String) -> Result<(), Error> {
    rp.user_auth_set_cookie(cookie).await
}

pub async fn sign_out(rp: &Client) {
    // Fails only when there was no session, which is the goal anyway.
    rp.user_auth_remove_cookie().await;
}

/// The home feed: the account's subscription feed when signed in, and the
/// latest videos of every channel subscribed on this device. Newest first.
/// Channels whose RSS `cache` still holds fresh answer from it.
pub async fn feed(
    rp: &Client,
    channels: Vec<Channel>,
    account: bool,
    cache: &RssCache,
) -> Result<(Vec<Video>, Option<More>), String> {
    let mut videos = Vec::new();
    let mut problems = Vec::new();
    let mut more = None;
    if account {
        match rp
            .post_raw(
                "browse",
                &json!({ "browseId": "FEsubscriptions" }),
                true,
                None,
            )
            .await
        {
            Ok(page) => {
                let visitor_data = page
                    .pointer("/responseContext/visitorData")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let (sub_videos, ctoken) = items::collect_videos_and_ctoken(&page);
                more = ctoken.map(|ctoken| More {
                    ctoken,
                    endpoint: ContinuationEndpoint::Browse,
                    visitor_data,
                    authenticated: true,
                    ours: false,
                    owner: None,
                });
                videos.extend(sub_videos);
            }
            Err(error) => problems.push(format!("your account's feed: {}", text(error))),
        }
    }

    let now = util::now();
    // Channels whose RSS is still fresh answer from the cache; the rest
    // are fetched and kept there.
    let mut fresh = Vec::new();
    {
        let cache = cache.lock().unwrap();
        for channel in channels {
            match cache.get(&channel.id) {
                Some((at, hit)) if now - at < RSS_TTL => videos.extend(hit.iter().cloned()),
                _ => fresh.push(channel),
            }
        }
    }
    let mut tasks = spawn_limited(fresh, |channel| {
        let rp = rp.clone();
        async move {
            let result = rss::fetch(&rp, &channel.id).await;
            (channel, result)
        }
    });
    let mut failed_channels = 0;
    while let Some(joined) = tasks.join_next().await {
        match joined {
            Ok((channel, Ok(videos_of_channel))) => {
                cache
                    .lock()
                    .unwrap()
                    .insert(channel.id, (now, videos_of_channel.clone()));
                videos.extend(videos_of_channel);
            }
            Ok((channel, Err(error))) => {
                log::warn!(
                    "could not read the feed of {}: {}",
                    channel.name,
                    text(error)
                );
                failed_channels += 1;
            }
            Err(error) => {
                log::warn!("a feed task failed: {error}");
                failed_channels += 1;
            }
        }
    }
    if failed_channels > 0 {
        problems.push(format!("{failed_channels} channels could not be read"));
    }
    if videos.is_empty() && !problems.is_empty() {
        return Err(problems.join("; "));
    }

    let mut seen = HashSet::new();
    videos.retain(|video| seen.insert(video.id.clone()));
    // Undated videos go last; the sort is stable, so they keep the order
    // their source gave them.
    videos.sort_by_key(|video| Reverse(video.published_at));
    Ok((videos, more))
}

/// The newest Shorts of the given channels, up to `per` each, interleaved
/// so no channel floods the shelf. Channels that fail are skipped.
pub async fn channel_shorts(rp: &Client, channels: Vec<String>, per: usize) -> Vec<Video> {
    let mut tasks = spawn_limited(channels, |id| {
        let rp = rp.clone();
        async move {
            let result = channel_tab(&rp, &id, &ChannelTab::Shorts, ChannelOrder::Latest).await;
            (id, result)
        }
    });
    let mut each = Vec::new();
    while let Some(joined) = tasks.join_next().await {
        match joined {
            Ok((_, Ok((_, videos, _)))) => {
                let shorts: Vec<Video> = videos
                    .into_iter()
                    .take(per)
                    .map(|mut video| {
                        video.short = true;
                        video
                    })
                    .collect();
                each.push(shorts);
            }
            Ok((id, Err(error))) => {
                log::warn!("could not read the shorts of {id}: {error}");
            }
            Err(error) => {
                log::warn!("a shorts task failed: {error}");
            }
        }
    }
    interleave(each)
}

/// Spawns `fetch` for every item, [`RSS_CONCURRENCY`] at a time.
fn spawn_limited<T, F, R>(items: Vec<T>, fetch: impl Fn(T) -> F) -> JoinSet<R>
where
    F: Future<Output = R> + Send + 'static,
    R: Send + 'static,
{
    let semaphore = Arc::new(Semaphore::new(RSS_CONCURRENCY));
    let mut tasks = JoinSet::new();
    for item in items {
        let semaphore = Arc::clone(&semaphore);
        let task = fetch(item);
        tasks.spawn(async move {
            let _permit = semaphore.acquire_owned().await;
            task.await
        });
    }
    tasks
}

/// Round-robin over the lists so every channel takes its turn: the first
/// item of each, then the second of each, until all are spent.
pub fn interleave<T>(lists: Vec<Vec<T>>) -> Vec<T> {
    let mut lists: Vec<_> = lists.into_iter().map(Vec::into_iter).collect();
    let mut out = Vec::new();
    loop {
        let mut any = false;
        for list in &mut lists {
            if let Some(item) = list.next() {
                out.push(item);
                any = true;
            }
        }
        if !any {
            return out;
        }
    }
}

/// How to continue a paged list: YouTube's continuation token, the
/// endpoint it came from, and whether the list it continues needed the
/// signed-in session (the account's lists always do).
#[derive(Clone, Debug)]
pub struct More {
    pub ctoken: String,
    pub endpoint: ContinuationEndpoint,
    pub visitor_data: Option<String>,
    pub authenticated: bool,
    /// A playlist continuation handled by `playlist::more`.
    pub ours: bool,
    /// The channel whose own tab the list is: YouTube leaves the channel out
    /// of those videos, and a continuation page has no header to take it from.
    pub owner: Option<Owner>,
}

/// The channel a channel tab's videos belong to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owner {
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
}

impl Owner {
    pub(crate) fn of(header: &ChannelHeader) -> Self {
        Self {
            id: header.id.clone(),
            name: header.name.clone(),
            avatar: header.avatar.clone(),
        }
    }

    /// Fills what the video lacks from its channel. A video of another
    /// channel (a collaboration) keeps its own name and gets no avatar.
    pub fn fill(&self, video: &mut Video) {
        let channel_id = video.channel_id.get_or_insert_with(|| self.id.clone());
        if *channel_id != self.id {
            return;
        }
        video.channel.get_or_insert_with(|| self.name.clone());
        if video.channel_avatar.is_none() {
            video.channel_avatar.clone_from(&self.avatar);
        }
    }
}

async fn post_continuation(rp: &Client, more: &More) -> Result<Value, String> {
    rp.post_raw(
        more.endpoint.as_str(),
        &json!({ "continuation": more.ctoken }),
        more.authenticated,
        more.visitor_data.as_deref(),
    )
    .await
    .map_err(text)
}

fn next_more(more: &More, ctoken: Option<String>, page: &Value) -> Option<More> {
    ctoken.map(|ctoken| More {
        ctoken,
        endpoint: more.endpoint,
        visitor_data: page
            .pointer("/responseContext/visitorData")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| more.visitor_data.clone()),
        authenticated: more.authenticated,
        ours: more.ours,
        owner: more.owner.clone(),
    })
}

/// The next page of videos.
pub async fn more_videos(rp: &Client, more: &More) -> Result<(Vec<Video>, Option<More>), String> {
    if more.ours {
        return playlist::more(rp, more).await;
    }
    let page = post_continuation(rp, more).await?;
    let (mut videos, ctoken) = items::collect_videos_and_ctoken(&page);
    if let Some(owner) = &more.owner {
        videos.iter_mut().for_each(|video| owner.fill(video));
    }
    let next = next_more(more, ctoken, &page);
    Ok((videos, next))
}

/// The next page of playlists.
pub async fn more_playlists(
    rp: &Client,
    more: &More,
) -> Result<(Vec<PlaylistSummary>, Option<More>), String> {
    let page = post_continuation(rp, more).await?;
    let (playlists, ctoken) = items::collect_playlists_and_ctoken(&page);
    let next = next_more(more, ctoken, &page);
    Ok((playlists, next))
}

/// The next page of channels (the account's subscriptions).
pub async fn more_channels(
    rp: &Client,
    more: &More,
) -> Result<(Vec<Channel>, Option<More>), String> {
    let page = post_continuation(rp, more).await?;
    let (channels, ctoken) = items::collect_channels_and_ctoken(&page);
    let next = next_more(more, ctoken, &page);
    Ok((channels, next))
}

/// The next page of the account's history.
pub async fn more_history(
    rp: &Client,
    more: &More,
) -> Result<(Vec<HistoryRow>, Option<More>), String> {
    let page = rp
        .post_raw(
            "browse",
            &json!({ "continuation": more.ctoken }),
            true,
            more.visitor_data.as_deref(),
        )
        .await
        .map_err(text)?;
    let (rows, ctoken) = items::collect_history_and_ctoken(&page);
    let next = next_more(more, ctoken, &page);
    Ok((rows, next))
}

/// The first page of the account's subscriptions and its continuation;
/// the "Load more" rows ask for the next page.
pub async fn subscriptions(rp: &Client) -> Result<(Vec<Channel>, Option<More>), Error> {
    let page = rp
        .post_raw("browse", &json!({ "browseId": "FEchannels" }), true, None)
        .await?;
    let visitor_data = page
        .pointer("/responseContext/visitorData")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let (mut channels, ctoken) = items::collect_channels_and_ctoken(&page);
    channels.sort_by_cached_key(|channel| channel.name.to_lowercase());
    let more = ctoken.map(|ctoken| More {
        ctoken,
        endpoint: ContinuationEndpoint::Browse,
        visitor_data,
        authenticated: true,
        ours: false,
        owner: None,
    });
    Ok((channels, more))
}

/// The first page of the account's history and its continuation.
pub async fn history(rp: &Client) -> Result<(Vec<HistoryRow>, Option<More>), Error> {
    let page = rp
        .post_raw("browse", &json!({ "browseId": "FEhistory" }), true, None)
        .await?;
    let visitor_data = page
        .pointer("/responseContext/visitorData")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let (rows, ctoken) = items::collect_history_and_ctoken(&page);
    let more = ctoken.map(|ctoken| More {
        ctoken,
        endpoint: ContinuationEndpoint::Browse,
        visitor_data,
        authenticated: true,
        ours: false,
        owner: None,
    });
    Ok((rows, more))
}

pub async fn watch_later(rp: &Client) -> Result<(Vec<Video>, Option<More>), Error> {
    playlist::videos(rp, "WL")
        .await
        .map_err(|text| Error::Other(text.into()))
}

pub async fn liked(rp: &Client) -> Result<(Vec<Video>, Option<More>), Error> {
    playlist::videos(rp, "LL")
        .await
        .map_err(|text| Error::Other(text.into()))
}

pub async fn playlists(rp: &Client) -> Result<(Vec<PlaylistSummary>, Option<More>), Error> {
    let page = rp
        .post_raw(
            "browse",
            &json!({ "browseId": "FEplaylist_aggregation" }),
            true,
            None,
        )
        .await?;
    let visitor_data = page
        .pointer("/responseContext/visitorData")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let (mut list, mut ctoken) = items::collect_playlists_and_ctoken(&page);
    if list.is_empty()
        && let Ok(fallback) = rp
            .post_raw("browse", &json!({ "browseId": "FElibrary" }), true, None)
            .await
    {
        let (fb_list, fb_ctoken) = items::collect_playlists_and_ctoken(&fallback);
        list = fb_list;
        ctoken = fb_ctoken;
    }
    let more = ctoken.map(|ctoken| More {
        ctoken,
        endpoint: ContinuationEndpoint::Browse,
        visitor_data,
        authenticated: true,
        ours: false,
        owner: None,
    });
    Ok((list, more))
}

#[cfg(test)]
mod tests {
    use super::*;
    use youtfast_core::model::Span;

    #[test]
    fn a_channel_tab_fills_its_own_videos_but_not_a_collaborators() {
        let owner = Owner {
            id: "UCowner".into(),
            name: "Owner".into(),
            avatar: Some("https://yt3.ggpht.com/owner".into()),
        };
        let mut own = Video::from_id("aaaaaaaaaaa".into());
        owner.fill(&mut own);
        assert_eq!(own.channel_id.as_deref(), Some("UCowner"));
        assert_eq!(own.channel.as_deref(), Some("Owner"));
        assert_eq!(
            own.channel_avatar.as_deref(),
            Some("https://yt3.ggpht.com/owner")
        );
        let mut other = Video::from_id("bbbbbbbbbbb".into());
        other.channel_id = Some("UCother".into());
        other.channel = Some("Other".into());
        owner.fill(&mut other);
        assert_eq!(other.channel.as_deref(), Some("Other"));
        assert_eq!(other.channel_avatar, None);
    }

    #[tokio::test]
    async fn a_failed_visitor_data_fetch_is_an_error_not_a_crash() {
        // Requests go through a proxy nothing listens on, so the fetch fails
        // without leaving this machine. The release build aborts on any
        // panic, so the failure has to come back as an error.
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy = format!("http://{}", closed.local_addr().unwrap());
        drop(closed);
        let client = reqwest::ClientBuilder::new().proxy(reqwest::Proxy::all(proxy).unwrap());
        let rp = Client::new(client).unwrap();
        assert!(rp.get_visitor_data().await.is_err());
    }

    #[test]
    fn certificates_are_checked_by_the_platform_verifier() {
        assert!(platform_tls().is_some());
    }

    #[tokio::test]
    #[ignore = "manual public network check of a refused certificate"]
    async fn a_self_signed_certificate_is_named_as_such() {
        let error = http()
            .build()
            .unwrap()
            .get("https://self-signed.badssl.com/")
            .send()
            .await
            .unwrap_err();
        assert_eq!(
            text(Error::from(error)),
            youtfast_core::privacy::UNTRUSTED_CERTIFICATE
        );
    }

    #[tokio::test]
    #[ignore = "manual public network check through the platform verifier"]
    async fn youtube_answers_through_the_platform_verifier() {
        let rp = Client::new(http()).unwrap();
        assert!(!suggestions(&rp, "rust").await.unwrap().is_empty());
    }

    fn from_json(json: &str) -> Value {
        serde_json::from_str(json).unwrap()
    }

    fn span(component: &Value, watching_id: &str) -> Span {
        watch::span_from_synthetic_component(component, watching_id).unwrap()
    }

    #[test]
    fn a_timestamp_on_the_video_being_watched_seeks() {
        let component = from_json(
            r#"{"YouTube":{"text":"12:34","target":{"Video":{"id":"aaaaaaaaaaa","start_time":754}}}}"#,
        );
        assert_eq!(
            span(&component, "aaaaaaaaaaa"),
            Span::Seek {
                text: "12:34".to_owned(),
                seconds: 754
            }
        );
    }

    #[test]
    fn a_video_link_without_a_start_time_opens_the_video() {
        let component = from_json(
            r#"{"YouTube":{"text":"watch it","target":{"Video":{"id":"bbbbbbbbbbb","start_time":0}}}}"#,
        );
        assert_eq!(
            span(&component, "aaaaaaaaaaa"),
            Span::Video {
                text: "watch it".to_owned(),
                id: "bbbbbbbbbbb".to_owned()
            }
        );
    }

    #[test]
    fn a_timestamp_at_the_start_seeks_without_reopening() {
        let component = from_json(
            r#"{"YouTube":{"text":"0:00","target":{"Video":{"id":"aaaaaaaaaaa","start_time":0}}}}"#,
        );
        assert_eq!(
            span(&component, "aaaaaaaaaaa"),
            Span::Seek {
                text: "0:00".to_owned(),
                seconds: 0,
            }
        );
    }

    #[test]
    fn channel_and_other_targets_become_their_spans() {
        let component =
            from_json(r#"{"YouTube":{"text":"the channel","target":{"Channel":{"id":"UCx"}}}}"#);
        assert_eq!(
            span(&component, ""),
            Span::Channel {
                text: "the channel".to_owned(),
                id: "UCx".to_owned()
            }
        );
        let component =
            from_json(r#"{"YouTube":{"text":"a playlist","target":{"Playlist":{"id":"PLx"}}}}"#);
        match span(&component, "") {
            Span::Link { url, .. } => {
                assert_eq!(url, "https://www.youtube.com/playlist?list=PLx")
            }
            other => panic!("a playlist link is a web link, got {other:?}"),
        }
    }

    #[test]
    fn a_bot_check_is_not_a_dead_session() {
        let status = |code, msg: &str| Error::HttpStatus(code, msg.to_owned().into());
        assert!(is_signed_out(&Error::Auth(AuthError::NoLogin)));
        assert!(is_signed_out(&status(401, "")));
        // An empty or plainly credential 403 still means the session.
        assert!(is_signed_out(&status(403, "")));
        assert!(is_signed_out(&status(403, "Invalid credentials")));
        // YouTube's own words for a bot check or rate limit do not.
        assert!(!is_signed_out(&status(
            403,
            "Sign in to confirm you're not a bot"
        )));
        assert!(!is_signed_out(&status(
            403,
            "Too many requests; rate limited"
        )));
        assert!(!is_signed_out(&status(500, "")));
    }

    #[test]
    fn interleaving_gives_every_channel_a_turn() {
        assert_eq!(
            interleave(vec![vec!["a1", "a2", "a3"], vec!["b1"], vec!["c1", "c2"],]),
            vec!["a1", "b1", "c1", "a2", "c2", "a3"]
        );
        assert!(interleave::<i32>(vec![vec![], vec![]]).is_empty());
    }

    #[test]
    fn a_small_avatar_is_the_smallest_wide_enough() {
        let thumbnail = |width: u32| -> Value {
            serde_json::from_str(&format!(
                r#"{{"url":"//yt3.ggpht.com/{width}","width":{width},"height":{width}}}"#
            ))
            .unwrap()
        };
        let set = Value::Array(vec![thumbnail(176), thumbnail(48), thumbnail(88)]);
        assert_eq!(
            items::at_least_value(Some(&set), 64).as_deref(),
            Some("https://yt3.ggpht.com/88")
        );
        // None wide enough: the largest there is.
        let small = Value::Array(vec![thumbnail(32), thumbnail(48)]);
        assert_eq!(
            items::at_least_value(Some(&small), 64).as_deref(),
            Some("https://yt3.ggpht.com/48")
        );
        let empty = Value::Array(vec![]);
        assert_eq!(items::at_least_value(Some(&empty), 64), None);
    }

    #[test]
    fn comment_timestamps_seek_only_within_the_watched_video() {
        let item = serde_json::json!({
            "id": "synthetic-comment",
            "text": [
                {"YouTube": {"text": "0:47", "target": {"Video": {
                    "id": "aaaaaaaaaaa", "start_time": 47
                }}}},
                {"Text": {"text": " great moment\n"}},
                {"YouTube": {"text": "0:00", "target": {"Video": {
                    "id": "aaaaaaaaaaa", "start_time": 0
                }}}},
                {"YouTube": {"text": "another video", "target": {"Video": {
                    "id": "bbbbbbbbbbb", "start_time": 47
                }}}}
            ],
            "author": null, "publish_date": null, "publish_date_txt": "1 day ago",
            "like_count": 0, "reply_count": 0,
            "replies": {"items": [], "ctoken": null, "count": 0, "endpoint": "browse"},
            "by_owner": false, "pinned": false, "hearted": false
        });
        assert_eq!(
            watch::parse_legacy_comment(&item, None, "aaaaaaaaaaa")
                .unwrap()
                .text,
            [
                Span::Seek {
                    text: "0:47".into(),
                    seconds: 47
                },
                Span::Text(" great moment\n".into()),
                Span::Seek {
                    text: "0:00".into(),
                    seconds: 0
                },
                Span::Video {
                    text: "another video".into(),
                    id: "bbbbbbbbbbb".into()
                },
            ]
        );
    }

    #[test]
    fn comment_attachments_keep_text_and_choose_a_small_display_source() {
        let mut json = serde_json::json!({
            "id": "synthetic-comment", "text": [{"Text": {"text": "hello"}}],
            "author": null, "publish_date": null, "publish_date_txt": "1 day ago",
            "like_count": 0, "reply_count": 0,
            "replies": {"items": [], "ctoken": null, "count": 0, "endpoint": "browse"},
            "by_owner": false, "pinned": false, "hearted": false
        });
        assert_eq!(
            watch::parse_legacy_comment(&json, None, "aaaaaaaaaaa")
                .unwrap()
                .image,
            None
        );
        json["image"] = serde_json::json!([
            {"url": "//example.invalid/large.gif", "width": 1280, "height": 720},
            {"url": "//example.invalid/display.gif", "width": 320, "height": 180},
            {"url": "//example.invalid/tiny.gif", "width": 100, "height": 56}
        ]);
        let mapped = watch::parse_legacy_comment(&json, None, "aaaaaaaaaaa").unwrap();
        assert_eq!(mapped.text, [Span::Text("hello".into())]);
        assert_eq!(
            mapped.image.as_deref(),
            Some("https://example.invalid/display.gif")
        );
        json["text"] = serde_json::json!([]);
        let image_only = watch::parse_legacy_comment(&json, None, "aaaaaaaaaaa").unwrap();
        assert!(image_only.text.is_empty());
        assert!(image_only.image.is_some());
    }

    #[test]
    fn text_and_web_stay_themselves() {
        let component = from_json(r#"{"Text":{"text":"hello"}}"#);
        assert_eq!(span(&component, ""), Span::Text("hello".to_owned()));
        let component = from_json(r#"{"Web":{"text":"a page","url":"https://example.com"}}"#);
        assert_eq!(
            span(&component, ""),
            Span::Link {
                text: "a page".to_owned(),
                url: "https://example.com".to_owned()
            }
        );
    }
}
