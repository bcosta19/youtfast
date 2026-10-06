//! The account and the feeds, through rustypipe: YouTube's own API
//! (InnerTube) with the signed-in session, and channel RSS without one.
//! Everything here runs on the backend runtime and returns the app's types.

use std::cmp::Reverse;
use std::collections::HashSet;
use std::sync::Arc;

use rustypipe::client::RustyPipe;
use rustypipe::error::Error;
use rustypipe::model::{ChannelItem, ChannelRss, PlaylistItem, Thumbnail, VideoItem};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::account::CacheFile;
use crate::model::{Channel, HistoryRow, PlaylistSummary, Video};
use crate::paths::Dirs;
use crate::util;

/// Channel feeds fetched at once: RSS is cheap, but hundreds of
/// subscriptions should not open hundreds of connections.
const RSS_CONCURRENCY: usize = 8;
/// The most videos the home feed shows.
const FEED_LIMIT: usize = 200;
/// The most items an account list (subscriptions, history, a playlist)
/// pages through.
const LIST_LIMIT: usize = 500;

/// The rustypipe client. Its cache keeps client versions and the player's
/// deobfuscation data between runs, never the session (see [`CacheFile`]),
/// and it writes no error reports: those hold whole responses.
pub fn client(dirs: &Dirs) -> Result<RustyPipe, Error> {
    RustyPipe::builder()
        .storage(Box::new(CacheFile {
            path: dirs.youtube_cache(),
        }))
        .storage_dir(dirs.cache.clone())
        .no_reporter()
        .build()
}

/// Whether the failure means the session is gone (signed out elsewhere,
/// expired, or rotated away by the browser it came from).
pub fn is_signed_out(error: &Error) -> bool {
    matches!(error, Error::Auth(_)) || matches!(error, Error::HttpStatus(401 | 403, _))
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
pub async fn feed(
    rp: &RustyPipe,
    channels: Vec<Channel>,
    account: bool,
) -> Result<Vec<Video>, String> {
    let mut videos = Vec::new();
    let mut problems = Vec::new();
    if account {
        match rp.query().subscription_feed().await {
            Ok(page) => videos.extend(page.items.into_iter().map(video)),
            Err(error) => problems.push(format!("your account's feed: {error}")),
        }
    }

    let semaphore = Arc::new(Semaphore::new(RSS_CONCURRENCY));
    let mut tasks = JoinSet::new();
    for channel in channels {
        let rp = rp.clone();
        let semaphore = Arc::clone(&semaphore);
        tasks.spawn(async move {
            let _permit = semaphore.acquire_owned().await;
            let result = rp.query().channel_rss(&channel.id).await;
            (channel.name, result)
        });
    }
    let now = util::now();
    let mut failed_channels = 0;
    while let Some(joined) = tasks.join_next().await {
        match joined {
            Ok((_, Ok(feed))) => videos.extend(rss_videos(feed, now)),
            Ok((name, Err(error))) => {
                log::warn!("could not read the feed of {name}: {error}");
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
    videos.truncate(FEED_LIMIT);
    Ok(videos)
}

/// A channel's 15 latest videos, from its RSS.
pub async fn channel_videos(rp: &RustyPipe, channel_id: &str) -> Result<Vec<Video>, String> {
    let feed = rp
        .query()
        .channel_rss(channel_id)
        .await
        .map_err(|error| error.to_string())?;
    Ok(rss_videos(feed, util::now()))
}

pub async fn subscriptions(rp: &RustyPipe) -> Result<Vec<Channel>, Error> {
    let query = rp.query();
    let mut page = query.subscriptions().await?;
    page.extend_limit(&query, LIST_LIMIT).await?;
    let mut channels: Vec<Channel> = page.items.into_iter().map(channel).collect();
    channels.sort_by_cached_key(|channel| channel.name.to_lowercase());
    Ok(channels)
}

pub async fn history(rp: &RustyPipe) -> Result<Vec<HistoryRow>, Error> {
    let query = rp.query();
    let mut page = query.history().await?;
    page.extend_limit(&query, LIST_LIMIT).await?;
    Ok(page
        .items
        .into_iter()
        .map(|entry| HistoryRow {
            video: video(entry.item),
            when: entry.playback_date_txt,
        })
        .collect())
}

pub async fn watch_later(rp: &RustyPipe) -> Result<Vec<Video>, Error> {
    let query = rp.query();
    let mut playlist = query.watch_later().await?;
    playlist.videos.extend_limit(&query, LIST_LIMIT).await?;
    Ok(playlist.videos.items.into_iter().map(video).collect())
}

pub async fn liked(rp: &RustyPipe) -> Result<Vec<Video>, Error> {
    let query = rp.query();
    let mut playlist = query.liked_videos().await?;
    playlist.videos.extend_limit(&query, LIST_LIMIT).await?;
    Ok(playlist.videos.items.into_iter().map(video).collect())
}

pub async fn playlists(rp: &RustyPipe) -> Result<Vec<PlaylistSummary>, Error> {
    let query = rp.query();
    let mut page = query.saved_playlists().await?;
    page.extend_limit(&query, LIST_LIMIT).await?;
    Ok(page.items.into_iter().map(playlist).collect())
}

/// A playlist's videos. Private playlists need the session, which rustypipe
/// adds by itself when one is set.
pub async fn playlist_videos(rp: &RustyPipe, id: &str) -> Result<Vec<Video>, Error> {
    let query = rp.query();
    let mut playlist = query.playlist(id).await?;
    playlist.videos.extend_limit(&query, LIST_LIMIT).await?;
    Ok(playlist.videos.items.into_iter().map(video).collect())
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

/// The largest of a set of thumbnails, as an absolute URL (YouTube gives
/// some as `//yt3.ggpht.com/…`).
fn largest(thumbnails: &[Thumbnail]) -> Option<String> {
    let url = &thumbnails
        .iter()
        .max_by_key(|thumbnail| thumbnail.width)?
        .url;
    Some(if url.starts_with("//") {
        format!("https:{url}")
    } else {
        url.clone()
    })
}
