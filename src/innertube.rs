//! The account and the feeds, through rustypipe: YouTube's own API
//! (InnerTube) with the signed-in session, and channel RSS without one.
//! Everything here runs on the backend runtime and returns the app's types.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rustypipe::client::{RustyPipe, RustyPipeQuery};
use rustypipe::error::Error;
use rustypipe::model::{
    Channel as RpChannel, ChannelItem, ChannelRss, Comment, HistoryItem, Playlist, PlaylistItem,
    Thumbnail, UrlTarget, VideoDetails, VideoItem, paginator::ContinuationEndpoint,
    paginator::Paginator, richtext::TextComponent,
};
use rustypipe::param::ChannelVideoTab;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::account::CacheFile;
use crate::model::{
    Channel, ChannelAbout, ChannelHeader, ChannelTab, Chapter, CommentRow, Details, HistoryRow,
    PlaylistSummary, Span, Video,
};
use crate::paths::Dirs;
use crate::util;

mod edit;
mod home;
mod playlist;
pub use edit::{rate, rating, save_targets, set_saved};
pub use home::recommended;

/// Channel feeds fetched at once: RSS is cheap, but hundreds of
/// subscriptions should not open hundreds of connections.
const RSS_CONCURRENCY: usize = 8;
/// How long a channel's RSS answer is kept: uploads take minutes to
/// reach RSS anyway, so re-fetching sooner buys nothing.
const RSS_TTL: i64 = 10 * 60;
/// Channel RSS answers by channel id (when, videos), shared by the feed
/// and the alerts check so neither re-fetches what the other just got.
pub type RssCache = std::sync::Mutex<HashMap<String, (i64, Vec<Video>)>>;

/// The rustypipe client. Its cache keeps client versions and the player's
/// deobfuscation data between runs, never the session (see [`CacheFile`]),
/// and it writes no error reports: those hold whole responses. Botguard
/// stays off: playback is mpv's, so the binary rustypipe would find on
/// PATH has nothing to do here.
pub fn client(dirs: &Dirs) -> Result<RustyPipe, Error> {
    RustyPipe::builder()
        .storage(Box::new(CacheFile {
            path: dirs.youtube_cache(),
        }))
        .storage_dir(dirs.cache.clone())
        .no_reporter()
        .no_botguard()
        .build()
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
    let text = error.to_string();
    match text.find(" for url (") {
        Some(at) => text[..at].to_owned(),
        None => text,
    }
}

/// Starts using a session, checking it with YouTube first.
pub async fn sign_in(rp: &RustyPipe, cookie: String) -> Result<(), Error> {
    rp.user_auth_set_cookie(cookie).await
}

pub async fn sign_out(rp: &RustyPipe) {
    // Fails only when there was no session, which is the goal anyway.
    let _ = rp.user_auth_remove_cookie().await;
}

/// The home feed: the account's subscription feed when signed in, and the
/// latest videos of every channel subscribed on this device. Newest first.
/// Channels whose RSS `cache` still holds fresh answer from it.
pub async fn feed(
    rp: &RustyPipe,
    channels: Vec<Channel>,
    account: bool,
    cache: &RssCache,
) -> Result<(Vec<Video>, Option<More>), String> {
    let mut videos = Vec::new();
    let mut problems = Vec::new();
    let mut more = None;
    if account {
        match rp.query().subscription_feed().await {
            Ok(page) => {
                more = More::of(&page);
                videos.extend(page.items.into_iter().map(video));
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
            let result = rp.query().channel_rss(&channel.id).await;
            (channel, result)
        }
    });
    let mut failed_channels = 0;
    while let Some(joined) = tasks.join_next().await {
        match joined {
            Ok((channel, Ok(feed))) => {
                let videos_of_channel: Vec<Video> = rss_videos(feed, now);
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
pub async fn channel_shorts(rp: &RustyPipe, channels: Vec<String>, per: usize) -> Vec<Video> {
    let mut tasks = spawn_limited(channels, |id| {
        let rp = rp.clone();
        async move {
            let result = rp
                .query()
                .channel_videos_tab(&id, ChannelVideoTab::Shorts)
                .await;
            (id, result)
        }
    });
    let mut each = Vec::new();
    while let Some(joined) = tasks.join_next().await {
        match joined {
            Ok((_, Ok(channel))) => {
                let shorts: Vec<Video> = channel
                    .content
                    .items
                    .into_iter()
                    .take(per)
                    .map(|item| {
                        let mut video = video(item);
                        video.short = true;
                        video
                    })
                    .collect();
                each.push(shorts);
            }
            Ok((id, Err(error))) => {
                log::warn!("could not read the shorts of {id}: {}", text(error));
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
    /// The page is read here, not by rustypipe: a playlist in the layout
    /// rustypipe does not know yet (`playlist.rs`).
    pub ours: bool,
}

impl More {
    /// The next page's directions, when the paginator has one.
    pub fn of<T>(paginator: &Paginator<T>) -> Option<More> {
        paginator.ctoken.as_ref().map(|ctoken| More {
            ctoken: ctoken.clone(),
            endpoint: paginator.endpoint,
            visitor_data: paginator.visitor_data.clone(),
            authenticated: paginator.authenticated,
            ours: false,
        })
    }
}

/// The query a continuation runs through: with the session when the
/// list it continues needed one.
fn continuation_query(rp: &RustyPipe, more: &More) -> RustyPipeQuery {
    let query = rp.query();
    if more.authenticated {
        query.authenticated()
    } else {
        query
    }
}

/// The next page of videos.
pub async fn more_videos(
    rp: &RustyPipe,
    more: &More,
) -> Result<(Vec<Video>, Option<More>), String> {
    if more.ours {
        return playlist::more(rp, more).await;
    }
    let page = continuation_query(rp, more)
        .continuation::<VideoItem, _>(&more.ctoken, more.endpoint, more.visitor_data.as_deref())
        .await
        .map_err(text)?;
    let more = More::of(&page);
    Ok((page.items.into_iter().map(video).collect(), more))
}

/// The next page of playlists.
pub async fn more_playlists(
    rp: &RustyPipe,
    more: &More,
) -> Result<(Vec<PlaylistSummary>, Option<More>), String> {
    let page = continuation_query(rp, more)
        .continuation::<PlaylistItem, _>(&more.ctoken, more.endpoint, more.visitor_data.as_deref())
        .await
        .map_err(text)?;
    let more = More::of(&page);
    Ok((page.items.into_iter().map(playlist).collect(), more))
}

/// The next page of channels (the account's subscriptions).
pub async fn more_channels(
    rp: &RustyPipe,
    more: &More,
) -> Result<(Vec<Channel>, Option<More>), String> {
    let page = continuation_query(rp, more)
        .continuation::<ChannelItem, _>(&more.ctoken, more.endpoint, more.visitor_data.as_deref())
        .await
        .map_err(text)?;
    let more = More::of(&page);
    Ok((page.items.into_iter().map(channel).collect(), more))
}

/// The first page of videos matching `query`, and how to ask for the
/// next: the same continuation every other list uses.
pub async fn search(rp: &RustyPipe, query: &str) -> Result<(Vec<Video>, Option<More>), String> {
    let result = rp
        .query()
        .search::<VideoItem, _>(query)
        .await
        .map_err(text)?;
    let more = More::of(&result.items);
    Ok((result.items.items.into_iter().map(video).collect(), more))
}

/// The next page of the account's history.
pub async fn more_history(
    rp: &RustyPipe,
    more: &More,
) -> Result<(Vec<HistoryRow>, Option<More>), String> {
    let page = rp
        .query()
        .history_continuation(&more.ctoken, more.visitor_data.as_deref())
        .await
        .map_err(text)?;
    let more = More::of(&page);
    Ok((page.items.into_iter().map(history_row).collect(), more))
}

/// A channel's Videos, Shorts or Live tab, or its videos matching `query`.
/// Returns the header every call sends along.
pub async fn channel_tab(
    rp: &RustyPipe,
    id: &str,
    tab: &ChannelTab,
) -> Result<(ChannelHeader, Vec<Video>, Option<More>), String> {
    let query = rp.query();
    let result = match tab {
        ChannelTab::Search(text) => query.channel_search(id, text).await,
        ChannelTab::Videos => query.channel_videos_tab(id, ChannelVideoTab::Videos).await,
        ChannelTab::Shorts => query.channel_videos_tab(id, ChannelVideoTab::Shorts).await,
        ChannelTab::Live => query.channel_videos_tab(id, ChannelVideoTab::Live).await,
        _ => unreachable!("channel_tab is for the video tabs"),
    }
    .map_err(text)?;
    let header = channel_header(&result);
    let more = More::of(&result.content);
    let shorts = *tab == ChannelTab::Shorts;
    let videos = result
        .content
        .items
        .into_iter()
        .map(|item| {
            let mut video = video(item);
            // The Shorts tab does not mark its items.
            video.short |= shorts;
            video
        })
        .collect();
    Ok((header, videos, more))
}

/// A channel's playlists tab.
pub async fn channel_playlists_page(
    rp: &RustyPipe,
    id: &str,
) -> Result<(ChannelHeader, Vec<PlaylistSummary>, Option<More>), String> {
    let result = rp.query().channel_playlists(id).await.map_err(text)?;
    let header = channel_header(&result);
    let more = More::of(&result.content);
    Ok((
        header,
        result.content.items.into_iter().map(playlist).collect(),
        more,
    ))
}

/// A channel's About tab.
pub async fn channel_about(rp: &RustyPipe, id: &str) -> Result<ChannelAbout, String> {
    let info = rp.query().channel_info(id).await.map_err(text)?;
    Ok(ChannelAbout {
        url: info.url,
        description: info.description,
        links: info.links,
        views: info.view_count,
        created: info
            .create_date
            .map(|date| iso_date(date.year(), date.month(), date.day())),
        country: info.country.map(|country| country.name().to_owned()),
        subscribers: info.subscriber_count,
        videos: info.video_count,
    })
}

/// The header every channel response carries.
fn channel_header<T>(channel: &RpChannel<T>) -> ChannelHeader {
    ChannelHeader {
        id: channel.id.clone(),
        name: channel.name.clone(),
        handle: channel.handle.clone(),
        avatar: largest(&channel.avatar),
        subscribers: channel.subscriber_count,
        videos: channel.video_count,
        has_shorts: channel.has_shorts,
        has_live: channel.has_live,
    }
}

/// A date as `YYYY-MM-DD`.
fn iso_date(year: i32, month: impl Into<u8>, day: u8) -> String {
    format!("{year:04}-{:02}-{day:02}", month.into())
}

/// The first page of the account's subscriptions and its continuation;
/// the "Load more" rows ask for the next page.
pub async fn subscriptions(rp: &RustyPipe) -> Result<(Vec<Channel>, Option<More>), Error> {
    let page = rp.query().subscriptions().await?;
    let more = More::of(&page);
    let mut channels: Vec<Channel> = page.items.into_iter().map(channel).collect();
    channels.sort_by_cached_key(|channel| channel.name.to_lowercase());
    Ok((channels, more))
}

/// The first page of the account's history and its continuation.
pub async fn history(rp: &RustyPipe) -> Result<(Vec<HistoryRow>, Option<More>), Error> {
    let page = rp.query().history().await?;
    let more = More::of(&page);
    Ok((page.items.into_iter().map(history_row).collect(), more))
}

fn history_row(entry: HistoryItem<VideoItem>) -> HistoryRow {
    HistoryRow {
        video: video(entry.item),
        when: entry.playback_date_txt,
    }
}

pub async fn watch_later(rp: &RustyPipe) -> Result<(Vec<Video>, Option<More>), Error> {
    or_ours(rp, "WL", rp.query().watch_later().await).await
}

pub async fn liked(rp: &RustyPipe) -> Result<(Vec<Video>, Option<More>), Error> {
    or_ours(rp, "LL", rp.query().liked_videos().await).await
}

/// rustypipe's reading of a playlist, or this app's when YouTube sent the
/// newer layout rustypipe does not know: the videos as `lockupViewModel`s
/// straight in the section, which rustypipe fails to read.
async fn or_ours(
    rp: &RustyPipe,
    id: &str,
    result: Result<Playlist, Error>,
) -> Result<(Vec<Video>, Option<More>), Error> {
    match result {
        Ok(page) => Ok(playlist_page(page)),
        Err(error) if newer_layout(&error) => playlist::videos(rp, id)
            .await
            .map_err(|text| Error::Other(text.into())),
        Err(error) => Err(error),
    }
}

fn newer_layout(error: &Error) -> bool {
    // "itemSectionRenderer empty" for the account's lists, a missing
    // field for public ones: any reading error is the layout.
    matches!(error, Error::Extraction(_))
}

pub async fn playlists(rp: &RustyPipe) -> Result<(Vec<PlaylistSummary>, Option<More>), Error> {
    let page = rp.query().saved_playlists().await?;
    let more = More::of(&page);
    Ok((page.items.into_iter().map(playlist).collect(), more))
}

/// A playlist's first page of videos. Private playlists need the session,
/// which rustypipe adds by itself when one is set.
pub async fn playlist_videos(
    rp: &RustyPipe,
    id: &str,
) -> Result<(Vec<Video>, Option<More>), Error> {
    or_ours(rp, id, rp.query().playlist(id).await).await
}

/// A playlist's first page of videos and its continuation.
fn playlist_page(playlist: Playlist) -> (Vec<Video>, Option<More>) {
    let more = More::of(&playlist.videos);
    (playlist.videos.items.into_iter().map(video).collect(), more)
}

/// The watch page's data: metadata, description, chapters, recommended
/// videos and the comments' continuation token. Works signed in or out.
/// What YouTube suggests for a search being typed. Signed out: the request
/// carries no session.
pub async fn suggestions(rp: &RustyPipe, query: &str) -> Result<Vec<String>, String> {
    rp.query().search_suggestion(query).await.map_err(text)
}

pub async fn details(rp: &RustyPipe, id: &str) -> Result<Details, String> {
    let details = rp.query().video_details(id).await.map_err(text)?;
    Ok(video_details(details, id))
}

fn video_details(item: VideoDetails, watching_id: &str) -> Details {
    Details {
        id: item.id,
        title: item.name,
        channel: Channel {
            id: item.channel.id,
            name: item.channel.name,
            avatar: largest(&item.channel.avatar),
        },
        subscribers: item.channel.subscriber_count,
        views: item.view_count,
        likes: item.like_count,
        published: item.publish_date_txt,
        published_date: item
            .publish_date
            .map(|date| iso_date(date.year(), date.month(), date.day())),
        live: item.is_live,
        description: item
            .description
            .0
            .iter()
            .map(|component| span(component, watching_id))
            .collect(),
        chapters: item
            .chapters
            .into_iter()
            .map(|chapter| Chapter {
                name: chapter.name,
                seconds: chapter.position,
            })
            .collect(),
        related: item.recommended.items.into_iter().map(video).collect(),
        comments_token: item.top_comments.ctoken,
        comment_count: item.top_comments.count,
        visitor_data: item.visitor_data,
    }
}

/// A page of comments (or of a comment's replies), and the next page's
/// token when there is one.
pub async fn comments(
    rp: &RustyPipe,
    ctoken: &str,
    visitor_data: Option<&str>,
) -> Result<(Vec<CommentRow>, Option<String>), String> {
    let page = rp
        .query()
        .video_comments(ctoken, visitor_data)
        .await
        .map_err(text)?;
    Ok((page.items.iter().map(comment).collect(), page.ctoken))
}

fn comment(item: &Comment) -> CommentRow {
    CommentRow {
        id: item.id.clone(),
        author: item.author.as_ref().map(|author| author.name.clone()),
        author_id: item.author.as_ref().map(|author| author.id.clone()),
        author_avatar: item
            .author
            .as_ref()
            .and_then(|author| at_least(&author.avatar, COMMENT_AVATAR)),
        text: item
            .text
            .0
            .iter()
            .map(|component| span(component, ""))
            .collect(),
        published: item.publish_date_txt.clone(),
        likes: item.like_count,
        replies: item.reply_count,
        replies_token: item.replies.ctoken.clone(),
        pinned: item.pinned,
        by_owner: item.by_owner,
        hearted: item.hearted,
    }
}

/// One text component as a span. A link to the video being watched with a
/// start time is a timestamp that seeks; to another video or a channel it
/// opens there; anything else is a browser link.
fn span(component: &TextComponent, watching_id: &str) -> Span {
    let text = component.get_text().to_owned();
    match component {
        TextComponent::Text { .. } => Span::Text(text),
        TextComponent::Web { url, .. } => Span::Link {
            text,
            url: url.clone(),
        },
        TextComponent::YouTube { target, .. } => match target {
            UrlTarget::Video { id, start_time } if id == watching_id && *start_time > 0 => {
                Span::Seek {
                    text,
                    seconds: *start_time,
                }
            }
            UrlTarget::Video { id, .. } => Span::Video {
                text,
                id: id.clone(),
            },
            UrlTarget::Channel { id } => Span::Channel {
                text,
                id: id.clone(),
            },
            _ => Span::Link {
                text,
                url: target.to_url(),
            },
        },
        // A variant this rustypipe version does not know yet.
        _ => Span::Text(text),
    }
}

fn video(item: VideoItem) -> Video {
    let channel = item.channel.as_ref().map(|channel| channel.name.clone());
    Video {
        id: item.id,
        title: item.name,
        channel,
        channel_id: item.channel.map(|channel| channel.id),
        duration: item.duration.map(f64::from),
        views: item.view_count,
        live: item.is_live,
        published_at: item.publish_date.map(|date| date.unix_timestamp()),
        published: item.publish_date_txt,
        short: item.is_short,
    }
}

fn rss_videos(feed: ChannelRss, now: i64) -> Vec<Video> {
    feed.videos
        .into_iter()
        .map(|entry| {
            let at = entry.publish_date.unix_timestamp();
            Video {
                id: entry.id,
                title: entry.name,
                channel: Some(feed.name.clone()),
                channel_id: Some(feed.id.clone()),
                duration: None,
                // Zero while a premiere has not started.
                views: (entry.view_count > 0).then_some(entry.view_count),
                live: false,
                published_at: Some(at),
                published: Some(util::ago(at, now)),
                short: false,
            }
        })
        .collect()
}

fn channel(item: ChannelItem) -> Channel {
    Channel {
        avatar: largest(&item.avatar),
        id: item.id,
        name: item.name,
    }
}

fn playlist(item: PlaylistItem) -> PlaylistSummary {
    PlaylistSummary {
        thumbnail: largest(&item.thumbnail),
        channel: item.channel.map(|channel| channel.name),
        videos: item.video_count,
        id: item.id,
        name: item.name,
    }
}

/// The largest of a set of thumbnails, as an absolute URL.
fn largest(thumbnails: &[Thumbnail]) -> Option<String> {
    let thumbnail = thumbnails.iter().max_by_key(|thumbnail| thumbnail.width)?;
    Some(absolute(&thumbnail.url))
}

/// The width a comment's avatar is fetched at: it is drawn about 36 px
/// square, and a page shows dozens.
const COMMENT_AVATAR: u32 = 64;

/// The smallest of a set of thumbnails at least `width` pixels wide (the
/// largest when none is), as an absolute URL.
fn at_least(thumbnails: &[Thumbnail], width: u32) -> Option<String> {
    let thumbnail = thumbnails
        .iter()
        .filter(|thumbnail| thumbnail.width >= width)
        .min_by_key(|thumbnail| thumbnail.width)
        .or_else(|| thumbnails.iter().max_by_key(|thumbnail| thumbnail.width))?;
    Some(absolute(&thumbnail.url))
}

/// YouTube gives some image URLs as `//yt3.ggpht.com/…`.
fn absolute(url: &str) -> String {
    if url.starts_with("//") {
        format!("https:{url}")
    } else {
        url.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TextComponent is non_exhaustive, so tests build it from JSON.
    fn from_json(json: &str) -> TextComponent {
        serde_json::from_str(json).unwrap()
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
        // And the same video at its start is not a timestamp.
        let component = from_json(
            r#"{"YouTube":{"text":"0:00","target":{"Video":{"id":"aaaaaaaaaaa","start_time":0}}}}"#,
        );
        assert_eq!(
            span(&component, "aaaaaaaaaaa"),
            Span::Video {
                text: "0:00".to_owned(),
                id: "aaaaaaaaaaa".to_owned()
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
        use rustypipe::error::Error;
        let status = |code, msg: &str| Error::HttpStatus(code, msg.to_owned().into());
        assert!(is_signed_out(&Error::Auth(
            rustypipe::error::AuthError::NoLogin
        )));
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
        // Thumbnail is non_exhaustive too.
        let thumbnail = |width: u32| -> Thumbnail {
            serde_json::from_str(&format!(
                r#"{{"url":"//yt3.ggpht.com/{width}","width":{width},"height":{width}}}"#
            ))
            .unwrap()
        };
        let set = [thumbnail(176), thumbnail(48), thumbnail(88)];
        assert_eq!(
            at_least(&set, 64).as_deref(),
            Some("https://yt3.ggpht.com/88")
        );
        // None wide enough: the largest there is.
        let small = [thumbnail(32), thumbnail(48)];
        assert_eq!(
            at_least(&small, 64).as_deref(),
            Some("https://yt3.ggpht.com/48")
        );
        assert_eq!(at_least(&[], 64), None);
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
