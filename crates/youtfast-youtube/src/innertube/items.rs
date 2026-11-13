//! Parsing YouTube's JSON items (`videoRenderer`, `lockupViewModel`,
//! `shortsLockupViewModel`, `channelRenderer`, `playlistRenderer`) and rich
//! text spans into `youtfast_core::model` types.

use serde_json::Value;

use super::edit::{collect_listed, find, text};
use youtfast_core::model::{Channel, ChannelMeta, HistoryRow, PlaylistSummary, Span, Video};
use youtfast_core::util;

/// The item renderers the lists below know, by their JSON key. Anything else
/// in an item list is reported: a YouTube layout change must show up in the
/// log instead of silently shortening or emptying a page.
const KNOWN_ITEMS: &[&str] = &[
    "videoRenderer",
    "gridVideoRenderer",
    "compactVideoRenderer",
    "lockupViewModel",
    "shortsLockupViewModel",
    "playlistRenderer",
    "gridPlaylistRenderer",
    "compactPlaylistRenderer",
    "channelRenderer",
    "gridChannelRenderer",
    "continuationItemRenderer",
    "continuationItemViewModel",
    "itemSectionRenderer",
    "richItemRenderer",
    "richSectionRenderer",
    "shelfRenderer",
    "reelShelfRenderer",
    "gridRenderer",
    "sectionListRenderer",
    "messageRenderer",
];

/// The renderer keys of one page that no reader recognised, deduplicated.
#[derive(Default)]
struct UnrecognisedItems(std::collections::BTreeSet<String>);

impl UnrecognisedItems {
    fn note(&mut self, value: &Value, listed: bool) {
        if !listed {
            return;
        }
        let Value::Object(map) = value else {
            return;
        };
        for key in map.keys() {
            if (key.ends_with("Renderer") || key.ends_with("ViewModel"))
                && !KNOWN_ITEMS.contains(&key.as_str())
            {
                self.0.insert(key.clone());
            }
        }
    }

    fn report(self, kind: &str, found: usize) {
        if self.0.is_empty() {
            return;
        }
        let keys = self.0.into_iter().collect::<Vec<_>>().join(", ");
        if found == 0 {
            log::warn!("no {kind} read; unrecognised InnerTube items: {keys}");
        } else {
            log::debug!("unrecognised InnerTube items alongside {found} {kind}: {keys}");
        }
    }
}

/// YouTube gives some image URLs as `//yt3.ggpht.com/…`.
pub fn absolute(url: &str) -> String {
    if url.starts_with("//") {
        format!("https:{url}")
    } else {
        url.to_owned()
    }
}

/// The URL of the widest image in a `thumbnails` or `sources` list.
pub fn widest(images: Option<&Value>) -> Option<String> {
    images?
        .as_array()?
        .iter()
        .filter_map(|image| {
            let width = image.get("width").and_then(Value::as_u64).unwrap_or(0);
            Some((width, image.get("url")?.as_str()?))
        })
        .max_by_key(|(width, _)| *width)
        .map(|(_, url)| absolute(url))
}

/// The smallest of a `thumbnails` or `sources` list at least `min_width`
/// pixels wide (or the largest when none reaches `min_width`).
pub fn at_least_value(images: Option<&Value>, min_width: u32) -> Option<String> {
    let list = images?.as_array()?;
    let pick = list
        .iter()
        .filter_map(|img| {
            let w = img.get("width").and_then(Value::as_u64)? as u32;
            let url = img.get("url")?.as_str()?;
            (w >= min_width).then_some((w, url))
        })
        .min_by_key(|(w, _)| *w)
        .or_else(|| {
            list.iter()
                .filter_map(|img| {
                    let w = img.get("width").and_then(Value::as_u64).unwrap_or(0) as u32;
                    let url = img.get("url")?.as_str()?;
                    Some((w, url))
                })
                .max_by_key(|(w, _)| *w)
        })?;
    Some(absolute(pick.1))
}

/// `"12:34"` or `"1:02:03"` in seconds.
pub fn parse_length(text: &str) -> Option<f64> {
    let mut seconds = 0.0;
    for part in text.trim().split(':') {
        seconds = seconds * 60.0 + part.parse::<u32>().ok()? as f64;
    }
    Some(seconds)
}

/// `"1,234 views"`, `"1.2M views"`, `"35K watching"`, `"572K subscribers"` as a number.
pub fn parse_count(text: &str) -> Option<u64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let word = text
        .split_whitespace()
        .find(|w| w.chars().next().is_some_and(|c| c.is_ascii_digit()))?
        .replace(',', "");
    let (number, scale) = match word.chars().last()? {
        'K' | 'k' => (&word[..word.len() - 1], 1e3),
        'M' | 'm' => (&word[..word.len() - 1], 1e6),
        'B' | 'b' => (&word[..word.len() - 1], 1e9),
        _ => (word.as_str(), 1.0),
    };
    let value: f64 = number.parse().ok()?;
    Some((value * scale).round() as u64)
}

/// Estimates a Unix timestamp from relative age text like `"4 hours ago"` or `"2 days ago"`.
pub fn parse_relative_age(text: &str) -> Option<i64> {
    let lower = text.to_ascii_lowercase();
    let lower = lower
        .strip_prefix("streamed ")
        .or_else(|| lower.strip_prefix("premiered "))
        .unwrap_or(&lower);
    let mut parts = lower.split_whitespace();
    let count: i64 = parts.next()?.parse().ok()?;
    let unit = parts.next()?;
    let seconds = if unit.starts_with("second") || unit.starts_with("sec") {
        count
    } else if unit.starts_with("minute") || unit.starts_with("min") {
        count * 60
    } else if unit.starts_with("hour") || unit.starts_with("hr") {
        count * 3600
    } else if unit.starts_with("day") {
        count * 86_400
    } else if unit.starts_with("week") || unit.starts_with("wk") {
        count * 7 * 86_400
    } else if unit.starts_with("month") || unit.starts_with("mo") {
        count * 30 * 86_400
    } else if unit.starts_with("year") || unit.starts_with("yr") {
        count * 365 * 86_400
    } else {
        return None;
    };
    Some(util::now().saturating_sub(seconds))
}

/// Reads an older `videoRenderer`, `gridVideoRenderer` or `compactVideoRenderer`.
pub fn from_renderer(renderer: &Value) -> Option<Video> {
    let id = renderer.get("videoId")?.as_str()?.to_owned();
    let owner = renderer
        .get("ownerText")
        .or_else(|| renderer.get("longBylineText"))
        .or_else(|| renderer.get("shortBylineText"));
    let live = renderer
        .get("badges")
        .and_then(Value::as_array)
        .is_some_and(|badges| {
            badges.iter().any(|badge| {
                badge
                    .pointer("/metadataBadgeRenderer/style")
                    .and_then(Value::as_str)
                    == Some("BADGE_STYLE_TYPE_LIVE_NOW")
            })
        })
        || renderer
            .get("thumbnailOverlays")
            .and_then(Value::as_array)
            .is_some_and(|overlays| {
                overlays.iter().any(|o| {
                    o.pointer("/thumbnailOverlayTimeStatusRenderer/style")
                        .and_then(Value::as_str)
                        == Some("LIVE")
                })
            });
    let short = renderer
        .pointer("/navigationEndpoint/reelWatchEndpoint")
        .is_some()
        || renderer
            .get("thumbnailOverlays")
            .and_then(Value::as_array)
            .is_some_and(|overlays| {
                overlays.iter().any(|o| {
                    o.pointer("/thumbnailOverlayTimeStatusRenderer/style")
                        .and_then(Value::as_str)
                        == Some("SHORTS")
                })
            });
    let avatar = renderer
        .pointer("/channelThumbnailSupportedRenderers/channelThumbnailWithLinkRenderer/thumbnail/thumbnails")
        .or_else(|| renderer.pointer("/channelThumbnail/thumbnails"));
    let published = text(renderer.get("publishedTimeText"));
    let published_at = published.as_deref().and_then(parse_relative_age);
    let duration = text(renderer.get("lengthText"))
        .or_else(|| {
            renderer
                .get("thumbnailOverlays")
                .and_then(Value::as_array)
                .and_then(|overlays| {
                    overlays
                        .iter()
                        .find_map(|o| text(o.pointer("/thumbnailOverlayTimeStatusRenderer/text")))
                })
        })
        .and_then(|length| parse_length(&length))
        .or_else(|| {
            renderer
                .get("lengthSeconds")
                .and_then(Value::as_str)
                .and_then(|s| s.parse().ok())
        });
    let views = text(renderer.get("viewCountText"))
        .or_else(|| text(renderer.get("shortViewCountText")))
        .and_then(|views| parse_count(&views));
    Some(Video {
        id,
        title: text(renderer.get("title"))
            .or_else(|| text(renderer.get("headline")))
            .unwrap_or_default(),
        channel_avatar: widest(avatar),
        channel: text(owner),
        channel_id: owner.and_then(|owner| {
            find(owner, &|v| {
                v.pointer("/browseEndpoint/browseId")
                    .and_then(Value::as_str)
                    .filter(|id| id.starts_with("UC"))
            })
            .map(str::to_owned)
        }),
        duration,
        views,
        live,
        published_at,
        published,
        short,
    })
}

/// Reads a modern `lockupViewModel` of a video (`YT-6` and `YT-7`).
pub fn from_lockup(lockup: &Value) -> Option<Video> {
    if lockup.get("contentType")?.as_str()? != "LOCKUP_CONTENT_TYPE_VIDEO" {
        return None;
    }
    let id = lockup.get("contentId")?.as_str()?.to_owned();
    let metadata = lockup.pointer("/metadata/lockupMetadataViewModel")?;
    let title = metadata
        .pointer("/title/content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let rows = metadata
        .pointer("/metadata/contentMetadataViewModel/metadataRows")
        .and_then(Value::as_array);
    let part = |row: usize, part: usize| {
        rows.and_then(|rows| rows.get(row))
            .and_then(|row| row.pointer(&format!("/metadataParts/{part}/text/content")))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let first_row_channel_id = rows
        .and_then(|rows| rows.first())
        .and_then(|row| {
            find(row, &|value| {
                value
                    .pointer("/browseEndpoint/browseId")
                    .and_then(Value::as_str)
                    .filter(|id| id.starts_with("UC"))
            })
        })
        .map(str::to_owned);
    let row_count = rows.map_or(0, Vec::len);
    // YT-7: on a channel's own tab there is only one metadata row (`[views, age]`)
    // with no channel link; home/search/related send `[channel, views · age]`.
    let (channel, channel_id, stats_row) = if row_count >= 2 || first_row_channel_id.is_some() {
        (part(0, 0), first_row_channel_id, 1)
    } else {
        (None, None, 0)
    };
    // The badge over the thumbnail: the length (in either overlay shape, YT-7), or LIVE.
    let badge = lockup.get("contentImage").and_then(|image| {
        find(image, &|value| {
            value
                .pointer("/thumbnailBadgeViewModel/text")
                .and_then(Value::as_str)
        })
    });
    let live = badge.is_some_and(|badge| badge.eq_ignore_ascii_case("live"));
    let (views, published) = match (part(stats_row, 0), part(stats_row, 1)) {
        // A live stream's row is `N watching`, with no age.
        (Some(first), second) => (parse_count(&first), second),
        (None, _) => (None, None),
    };
    let published_at = published.as_deref().and_then(parse_relative_age);
    // YT-6: The channel's avatar, left of the title.
    let avatar = metadata
        .pointer("/image/decoratedAvatarViewModel/avatar/avatarViewModel/image/sources")
        .or_else(|| metadata.pointer("/image/avatarViewModel/image/sources"));
    Some(Video {
        id,
        title,
        channel_avatar: widest(avatar),
        channel,
        channel_id,
        duration: badge.and_then(parse_length),
        views,
        live,
        published_at,
        published,
        short: false,
    })
}

/// Reads a `shortsLockupViewModel` (`YT-2`: thumbnail in either `thumbnail`
/// or `thumbnailViewModel.thumbnailViewModel.image`).
pub fn from_short(short: &Value) -> Option<Video> {
    let sources = short
        .pointer("/thumbnailViewModel/thumbnailViewModel/image/sources")
        .or_else(|| short.pointer("/thumbnail/sources"))
        .and_then(Value::as_array)?;
    if sources.is_empty() {
        return None;
    }
    let id = short
        .pointer("/onTap/innertubeCommand/reelWatchEndpoint/videoId")
        .and_then(Value::as_str)
        .or_else(|| {
            short
                .get("entityId")
                .and_then(Value::as_str)
                .and_then(|entity| entity.strip_prefix("shorts-shelf-item-"))
        })?
        .to_owned();
    let title = short
        .pointer("/overlayMetadata/primaryText/content")
        .and_then(Value::as_str)
        .or_else(|| {
            short
                .get("accessibilityText")
                .and_then(Value::as_str)
                .and_then(|acc| acc.split(',').next())
        })
        .unwrap_or_default()
        .to_owned();
    let views = short
        .pointer("/overlayMetadata/secondaryText/content")
        .and_then(Value::as_str)
        .and_then(parse_count)
        .or_else(|| {
            short
                .get("accessibilityText")
                .and_then(Value::as_str)
                .and_then(parse_accessibility_views)
        });
    Some(Video {
        id,
        title,
        channel: None,
        channel_avatar: None,
        channel_id: None,
        duration: None,
        views,
        live: false,
        published_at: None,
        published: None,
        short: true,
    })
}

fn parse_accessibility_views(text: &str) -> Option<u64> {
    for segment in text.split(", ") {
        let segment = segment.trim();
        if segment.contains("view") {
            let lower = segment.to_ascii_lowercase();
            let mut words = lower.split_whitespace();
            let number = words.next()?;
            let scale = match words.next()? {
                "thousand" => 1e3,
                "million" => 1e6,
                "billion" => 1e9,
                _ => return parse_count(segment),
            };
            let num: f64 = number.replace(',', "").parse().ok()?;
            return Some((num * scale).round() as u64);
        }
    }
    None
}

/// Reads any video item (`videoRenderer`, `compactVideoRenderer`,
/// `gridVideoRenderer`, `lockupViewModel`, or `shortsLockupViewModel`).
pub fn video_from_value(value: &Value) -> Option<Video> {
    if let Some(r) = value
        .get("videoRenderer")
        .or_else(|| value.get("gridVideoRenderer"))
        .or_else(|| value.get("compactVideoRenderer"))
    {
        from_renderer(r)
    } else if let Some(lockup) = value.get("lockupViewModel") {
        from_lockup(lockup)
    } else if let Some(short) = value.get("shortsLockupViewModel") {
        from_short(short)
    } else {
        None
    }
}

/// Collects all unique videos on a page and the first continuation token.
pub fn collect_videos_and_ctoken(page: &Value) -> (Vec<Video>, Option<String>) {
    let mut videos: Vec<Video> = Vec::new();
    let mut ctoken = None;
    let mut unknown = UnrecognisedItems::default();
    collect_listed(page, &mut |value, listed| {
        unknown.note(value, listed);
        if let Some(video) = video_from_value(value) {
            if !videos.iter().any(|known| known.id == video.id) {
                videos.push(video);
            }
        } else if ctoken.is_none() {
            ctoken = extract_ctoken(value);
        }
    });
    unknown.report("videos", videos.len());
    (videos, ctoken)
}

pub fn extract_ctoken(value: &Value) -> Option<String> {
    value
        .pointer("/continuationItemRenderer/continuationEndpoint/continuationCommand/token")
        .or_else(|| {
            value.pointer(
                "/continuationItemRenderer/button/buttonRenderer/command/continuationCommand/token",
            )
        })
        .or_else(|| {
            value.pointer(
                "/continuationItemViewModel/continuationCommand/innertubeCommand/continuationCommand/token",
            )
        })
        .or_else(|| {
            value.pointer(
                "/continuationItemRenderer/continuationEndpoint/browseContinuation/continuation",
            )
        })
        .or_else(|| value.pointer("/nextContinuationData/continuation"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// Reads a `PlaylistSummary` from either a `lockupViewModel` (`YT-4`: playlist
/// or podcast) or an older `playlistRenderer` / `gridPlaylistRenderer`.
pub fn playlist_from_value(value: &Value) -> Option<PlaylistSummary> {
    if let Some(lockup) = value.get("lockupViewModel") {
        playlist_from_lockup(lockup)
    } else if let Some(renderer) = value
        .get("playlistRenderer")
        .or_else(|| value.get("gridPlaylistRenderer"))
        .or_else(|| value.get("compactPlaylistRenderer"))
    {
        playlist_from_renderer(renderer)
    } else {
        None
    }
}

pub fn playlist_from_lockup(lockup: &Value) -> Option<PlaylistSummary> {
    // YT-4: podcasts on the Playlists tab use LOCKUP_CONTENT_TYPE_PODCAST.
    let content_type = lockup.get("contentType")?.as_str()?;
    if !matches!(
        content_type,
        "LOCKUP_CONTENT_TYPE_PLAYLIST" | "LOCKUP_CONTENT_TYPE_PODCAST"
    ) {
        return None;
    }
    let id = lockup.get("contentId")?.as_str()?.to_owned();
    let metadata = lockup.pointer("/metadata/lockupMetadataViewModel")?;
    let name = metadata
        .pointer("/title/content")
        .and_then(Value::as_str)?
        .to_owned();
    let thumbnail = widest(
        lockup
            .pointer(
                "/contentImage/collectionThumbnailViewModel/primaryThumbnail/thumbnailViewModel/image/sources",
            )
            .or_else(|| lockup.pointer("/contentImage/thumbnailViewModel/image/sources")),
    );
    let badge = lockup.get("contentImage").and_then(|image| {
        find(image, &|v| {
            v.pointer("/thumbnailBadgeViewModel/text")
                .and_then(Value::as_str)
        })
    });
    let videos = badge.and_then(parse_count);
    let first_part = metadata
        .pointer("/metadata/contentMetadataViewModel/metadataRows/0/metadataParts/0/text/content")
        .and_then(Value::as_str)
        .filter(|part| {
            let lower = part.to_ascii_lowercase();
            !lower.contains("view full playlist")
                && !lower.contains("playlist")
                && !lower.contains("podcast")
                && !lower.contains("video")
                && !lower.contains("episode")
        })
        .map(str::to_owned);
    Some(PlaylistSummary {
        id,
        name,
        thumbnail,
        channel: first_part,
        videos,
    })
}

pub fn playlist_from_renderer(renderer: &Value) -> Option<PlaylistSummary> {
    let id = renderer.get("playlistId")?.as_str()?.to_owned();
    let name = text(renderer.get("title")).unwrap_or_default();
    let thumbnail = widest(
        renderer
            .pointer("/thumbnails/0/thumbnails")
            .or_else(|| renderer.pointer("/thumbnail/thumbnails"))
            .or_else(|| renderer.pointer("/playlistVideoThumbnailRenderer/thumbnail/thumbnails")),
    );
    let channel =
        text(renderer.get("shortBylineText")).or_else(|| text(renderer.get("longBylineText")));
    let videos = text(renderer.get("videoCountText"))
        .or_else(|| text(renderer.get("videoCountShortText")))
        .or_else(|| {
            renderer
                .get("videoCount")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .and_then(|count| parse_count(&count));
    Some(PlaylistSummary {
        id,
        name,
        thumbnail,
        channel,
        videos,
    })
}

pub fn collect_playlists_and_ctoken(page: &Value) -> (Vec<PlaylistSummary>, Option<String>) {
    let mut playlists: Vec<PlaylistSummary> = Vec::new();
    let mut ctoken = None;
    let mut unknown = UnrecognisedItems::default();
    collect_listed(page, &mut |value, listed| {
        unknown.note(value, listed);
        if let Some(pl) = playlist_from_value(value) {
            if !playlists.iter().any(|known| known.id == pl.id) {
                playlists.push(pl);
            }
        } else if ctoken.is_none() {
            ctoken = extract_ctoken(value);
        }
    });
    unknown.report("playlists", playlists.len());
    (playlists, ctoken)
}

/// Parses one stat slot from a channel card (`subscriberCountText` or
/// `videoCountText`).
///
/// Modern `channelRenderer` responses put `@handle` in `subscriberCountText`
/// and the subscriber count in `videoCountText`, while `gridChannelRenderer`
/// keeps subscribers in `subscriberCountText` and video counts in
/// `videoCountText`. Classifying each segment by prefix/keyword handles both.
fn assign_channel_stat(text: &str, prefer_videos: bool, meta: &mut ChannelMeta) {
    for raw in text.split(['•', '·']) {
        let part = raw.trim();
        if part.is_empty() {
            continue;
        }
        if part.starts_with('@') {
            if meta.handle.is_none() {
                meta.handle = Some(part.to_owned());
            }
            continue;
        }
        let lower = part.to_ascii_lowercase();
        if lower.contains("subscriber") || (!lower.contains("video") && !prefer_videos) {
            if meta.subscribers.is_none() {
                meta.subscribers = parse_count(part);
            }
        } else if meta.videos.is_none() {
            meta.videos = parse_count(part);
        }
    }
}

fn channel_is_verified(renderer: &Value) -> bool {
    ["ownerBadges", "badges"].into_iter().any(|key| {
        renderer
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|badges| {
                badges.iter().any(|badge| {
                    let mbr = badge.get("metadataBadgeRenderer").unwrap_or(badge);
                    let style = mbr.get("style").and_then(Value::as_str).unwrap_or("");
                    let icon = mbr
                        .pointer("/icon/iconType")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    style.contains("VERIFIED") || icon.contains("CHECK")
                })
            })
    })
}

/// Reads a `Channel` and its metadata from a `channelRenderer` or
/// `gridChannelRenderer`.
pub fn channel_with_meta_from_value(value: &Value) -> Option<(Channel, ChannelMeta)> {
    let renderer = value
        .get("channelRenderer")
        .or_else(|| value.get("gridChannelRenderer"))?;
    let id = renderer.get("channelId")?.as_str()?.to_owned();
    let name = text(renderer.get("title")).unwrap_or_default();
    let avatar = widest(renderer.pointer("/thumbnail/thumbnails"));

    let mut meta = ChannelMeta::default();
    if let Some(canonical) = renderer
        .pointer("/navigationEndpoint/browseEndpoint/canonicalBaseUrl")
        .and_then(Value::as_str)
    {
        let trimmed = canonical.trim_start_matches('/');
        if trimmed.starts_with('@') {
            meta.handle = Some(trimmed.to_owned());
        }
    }
    if let Some(sub_text) = text(renderer.get("subscriberCountText")) {
        assign_channel_stat(&sub_text, false, &mut meta);
    }
    if let Some(vid_text) = text(renderer.get("videoCountText")) {
        assign_channel_stat(&vid_text, true, &mut meta);
    }
    meta.description = text(renderer.get("descriptionSnippet"))
        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|s| !s.is_empty());
    meta.verified = channel_is_verified(renderer);

    Some((Channel { id, name, avatar }, meta))
}

/// Reads a `Channel` from a `channelRenderer` or `gridChannelRenderer`.
pub fn channel_from_value(value: &Value) -> Option<Channel> {
    channel_with_meta_from_value(value).map(|(channel, _)| channel)
}

pub fn collect_channels_with_meta_and_ctoken(
    page: &Value,
) -> (Vec<Channel>, Vec<(String, ChannelMeta)>, Option<String>) {
    let mut channels: Vec<Channel> = Vec::new();
    let mut metas: Vec<(String, ChannelMeta)> = Vec::new();
    let mut ctoken = None;
    let mut unknown = UnrecognisedItems::default();
    collect_listed(page, &mut |value, listed| {
        unknown.note(value, listed);
        if let Some((ch, meta)) = channel_with_meta_from_value(value) {
            if !channels.iter().any(|known| known.id == ch.id) {
                if !meta.is_empty() {
                    metas.push((ch.id.clone(), meta));
                }
                channels.push(ch);
            } else if !meta.is_empty() {
                if let Some((_, known)) = metas.iter_mut().find(|(id, _)| id == &ch.id) {
                    known.merge_from(meta);
                } else {
                    metas.push((ch.id, meta));
                }
            }
        } else if ctoken.is_none() {
            ctoken = extract_ctoken(value);
        }
    });
    unknown.report("channels", channels.len());
    (channels, metas, ctoken)
}

pub fn collect_channels_and_ctoken(page: &Value) -> (Vec<Channel>, Option<String>) {
    let (channels, _, ctoken) = collect_channels_with_meta_and_ctoken(page);
    (channels, ctoken)
}

/// Collects history rows (`itemSectionRenderer` grouped by day header, or flat
/// continuation items) and the next continuation token.
pub fn collect_history_and_ctoken(page: &Value) -> (Vec<HistoryRow>, Option<String>) {
    let mut rows: Vec<HistoryRow> = Vec::new();
    let mut ctoken = None;
    let mut current_day: Option<String> = None;
    let mut unknown = UnrecognisedItems::default();
    collect_listed(page, &mut |value, listed| {
        unknown.note(value, listed);
        if let Some(section) = value.get("itemSectionRenderer") {
            let section_day = section
                .pointer("/header/itemSectionHeaderRenderer/title")
                .and_then(|t| {
                    text(Some(t)).or_else(|| {
                        t.pointer("/runs/0/text")
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                    })
                });
            if section_day.is_some() {
                current_day = section_day;
            }
            if let Some(contents) = section.get("contents").and_then(Value::as_array) {
                for item in contents {
                    if let Some(video) = video_from_value(item)
                        && !rows.iter().any(|r| r.video.id == video.id)
                    {
                        rows.push(HistoryRow {
                            video,
                            when: current_day.clone(),
                        });
                    } else if ctoken.is_none() {
                        ctoken = extract_ctoken(item);
                    }
                }
            }
        } else if ctoken.is_none() {
            ctoken = extract_ctoken(value);
        }
    });
    if rows.is_empty() {
        let (videos, next) = collect_videos_and_ctoken(page);
        rows = videos
            .into_iter()
            .map(|video| HistoryRow { video, when: None })
            .collect();
        if ctoken.is_none() {
            ctoken = next;
        }
    } else {
        unknown.report("history rows", rows.len());
    }
    (rows, ctoken)
}

// ---------------------------------------------------------------------------
// Rich text spans (`runs`, `attributedString` / `commandRuns`, or synthetic)
// ---------------------------------------------------------------------------

/// Converts a YouTube text object (`runs`, `simpleText`, or `content` + `commandRuns`)
/// into `Span`s, resolving links on `watching_id` with a start time as `Span::Seek`.
pub fn spans_from_value(value: Option<&Value>, watching_id: &str) -> Vec<Span> {
    let Some(value) = value else {
        return Vec::new();
    };
    if let Some(runs) = value.get("runs").and_then(Value::as_array) {
        return runs
            .iter()
            .filter_map(|run| {
                let text = run.get("text")?.as_str()?.to_owned();
                let endpoint = run.get("navigationEndpoint");
                Some(span_from_endpoint(text, endpoint, watching_id))
            })
            .collect();
    }
    if let Some(content) = value.get("content").and_then(Value::as_str) {
        return spans_from_attributed(content, value.get("commandRuns"), watching_id);
    }
    if let Some(simple) = value.get("simpleText").and_then(Value::as_str) {
        return vec![Span::Text(simple.to_owned())];
    }
    Vec::new()
}

fn spans_from_attributed(
    content: &str,
    command_runs: Option<&Value>,
    watching_id: &str,
) -> Vec<Span> {
    let Some(runs) = command_runs.and_then(Value::as_array) else {
        return if content.is_empty() {
            Vec::new()
        } else {
            vec![Span::Text(content.to_owned())]
        };
    };
    let utf16: Vec<u16> = content.encode_utf16().collect();
    let mut out = Vec::new();
    let mut cursor = 0usize;
    for run in runs {
        let Some(start) = run.get("startIndex").and_then(Value::as_u64) else {
            continue;
        };
        let Some(len) = run.get("length").and_then(Value::as_u64) else {
            continue;
        };
        let start = (start as usize).min(utf16.len());
        let end = start.saturating_add(len as usize).min(utf16.len());
        if start > cursor
            && let Ok(prefix) = String::from_utf16(&utf16[cursor..start])
            && !prefix.is_empty()
        {
            out.push(Span::Text(prefix));
        }
        if end > start
            && let Ok(slice) = String::from_utf16(&utf16[start..end])
        {
            let endpoint = run
                .pointer("/onTap/innertubeCommand")
                .or_else(|| run.get("onTap"));
            out.push(span_from_endpoint(slice, endpoint, watching_id));
        }
        cursor = cursor.max(end);
    }
    if cursor < utf16.len()
        && let Ok(rest) = String::from_utf16(&utf16[cursor..])
        && !rest.is_empty()
    {
        out.push(Span::Text(rest));
    }
    out
}

pub fn span_from_endpoint(text: String, endpoint: Option<&Value>, watching_id: &str) -> Span {
    let Some(endpoint) = endpoint else {
        return Span::Text(text);
    };
    if let Some(watch) = endpoint.get("watchEndpoint")
        && let Some(id) = watch.get("videoId").and_then(Value::as_str)
    {
        let start = watch
            .get("startTimeSeconds")
            .and_then(Value::as_u64)
            .map(|s| s as u32);
        let looks_like_timestamp = parse_length(&text).is_some();
        if id == watching_id && (start.is_some_and(|s| s > 0) || looks_like_timestamp) {
            return Span::Seek {
                text,
                seconds: start.unwrap_or(0),
            };
        }
        return Span::Video {
            text,
            id: id.to_owned(),
        };
    }
    if let Some(browse) = endpoint.get("browseEndpoint")
        && let Some(id) = browse.get("browseId").and_then(Value::as_str)
    {
        if id.starts_with("UC") {
            return Span::Channel {
                text,
                id: id.to_owned(),
            };
        }
        if let Some(pl) = id.strip_prefix("VL") {
            return Span::Link {
                text,
                url: format!("https://www.youtube.com/playlist?list={pl}"),
            };
        }
    }
    if let Some(url_ep) = endpoint.get("urlEndpoint")
        && let Some(raw_url) = url_ep.get("url").and_then(Value::as_str)
    {
        return Span::Link {
            text,
            url: unwrap_redirect(raw_url),
        };
    }
    Span::Text(text)
}

fn unwrap_redirect(raw: &str) -> String {
    if let Ok(parsed) = url::Url::parse(raw)
        && parsed.path() == "/redirect"
        && let Some((_, target)) = parsed.query_pairs().find(|(k, _)| k == "q")
    {
        return target.into_owned();
    }
    if raw.starts_with('/') {
        format!("https://www.youtube.com{raw}")
    } else {
        raw.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_unrecognised_renderers_in_item_lists_are_reported() {
        let page = serde_json::json!({
            "contents": [
                { "videoRenderer": {
                    "videoId": "aaaaaaaaaaa",
                    // Nested inside a known item: not a dropped list entry.
                    "someNewOverlayRenderer": {}
                } },
                { "brandNewVideoRenderer": { "videoId": "bbbbbbbbbbb" } },
                { "continuationItemRenderer": {} }
            ],
            // Outside any item list: not reported either.
            "header": { "someNewHeaderRenderer": {} }
        });
        let mut unknown = UnrecognisedItems::default();
        collect_listed(&page, &mut |value, listed| unknown.note(value, listed));
        assert_eq!(
            unknown.0.into_iter().collect::<Vec<_>>(),
            vec!["brandNewVideoRenderer".to_owned()]
        );
    }

    #[test]
    fn channel_renderers_extract_handle_counts_bio_and_verified_badge() {
        let page = serde_json::json!({
            "contents": [
                {
                    "channelRenderer": {
                        "channelId": "UC_modern",
                        "title": { "simpleText": "Computerphile" },
                        "navigationEndpoint": {
                            "browseEndpoint": { "canonicalBaseUrl": "/@Computerphile" }
                        },
                        "thumbnail": { "thumbnails": [{ "url": "https://img/a.jpg", "width": 176 }] },
                        "subscriberCountText": { "simpleText": "@Computerphile" },
                        "videoCountText": { "simpleText": "2.64M subscribers" },
                        "descriptionSnippet": { "runs": [{ "text": "Videos all about\n  computers." }] },
                        "ownerBadges": [{
                            "metadataBadgeRenderer": {
                                "icon": { "iconType": "CHECK_CIRCLE_THICK" },
                                "style": "BADGE_STYLE_TYPE_VERIFIED"
                            }
                        }]
                    }
                },
                {
                    "gridChannelRenderer": {
                        "channelId": "UC_grid",
                        "title": { "simpleText": "Grid Channel" },
                        "navigationEndpoint": {
                            "browseEndpoint": { "canonicalBaseUrl": "/@GridChan" }
                        },
                        "subscriberCountText": { "simpleText": "120K subscribers" },
                        "videoCountText": { "simpleText": "340 videos" }
                    }
                }
            ]
        });
        let (channels, metas, ctoken) = collect_channels_with_meta_and_ctoken(&page);
        assert!(ctoken.is_none());
        assert_eq!(channels.len(), 2);
        assert_eq!(
            metas,
            vec![
                (
                    "UC_modern".to_owned(),
                    ChannelMeta {
                        handle: Some("@Computerphile".to_owned()),
                        subscribers: Some(2_640_000),
                        videos: None,
                        description: Some("Videos all about computers.".to_owned()),
                        verified: true,
                    }
                ),
                (
                    "UC_grid".to_owned(),
                    ChannelMeta {
                        handle: Some("@GridChan".to_owned()),
                        subscribers: Some(120_000),
                        videos: Some(340),
                        description: None,
                        verified: false,
                    }
                ),
            ]
        );
    }
}
