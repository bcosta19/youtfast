//! Channel pages: Videos, Shorts, Live, Playlists, Search and About tabs,
//! sort tokens (`YT-1`), tab parameters (`YT-3`), and header counts (`YT-5`).

use std::borrow::Cow;

use serde_json::{Value, json};

use super::More;
use super::client::{Client, ContinuationEndpoint, base64_urlencoded, proto_bytes, proto_varint};
use super::discovery::ChannelOrder;
use super::edit::{collect, find, text};
use super::items::{collect_playlists_and_ctoken, collect_videos_and_ctoken, parse_count, widest};
use youtfast_core::model::{ChannelAbout, ChannelHeader, ChannelTab, PlaylistSummary, Video};

pub const TAB_VIDEOS_PARAMS: &str = "EgZ2aWRlb3PyBgQKAjoA";
pub const TAB_SHORTS_PARAMS: &str = "EgZzaG9ydHPyBgUKA5oBAA%3D%3D";
pub const TAB_LIVE_PARAMS: &str = "EgdzdHJlYW1z8gYECgJ6AA%3D%3D";
pub const TAB_PLAYLISTS_PARAMS: &str = "EglwbGF5bGlzdHPyBgoKCEIGCgIQaCIA";
pub const TAB_SEARCH_PARAMS: &str = "EgZzZWFyY2jyBgQKAloA";
pub const TAB_ABOUT_PARAMS: &str = "EgVhYm91dPIGBAoCEgA%3D";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelVideoTab {
    Videos,
    Shorts,
    Live,
}

impl ChannelVideoTab {
    pub fn params(self) -> &'static str {
        match self {
            Self::Videos => TAB_VIDEOS_PARAMS,
            Self::Shorts => TAB_SHORTS_PARAMS,
            Self::Live => TAB_LIVE_PARAMS,
        }
    }
}

/// The `params` values YouTFast sends for each channel tab (`YT-3`).
pub fn channel_tab_params() -> Vec<(&'static str, String)> {
    vec![
        ("videos", TAB_VIDEOS_PARAMS.to_owned()),
        ("shorts", TAB_SHORTS_PARAMS.to_owned()),
        ("live", TAB_LIVE_PARAMS.to_owned()),
        ("playlists", TAB_PLAYLISTS_PARAMS.to_owned()),
        ("search", TAB_SEARCH_PARAMS.to_owned()),
        ("about", TAB_ABOUT_PARAMS.to_owned()),
    ]
}

/// Builds a channel tab sort continuation token (`YT-1`).
pub fn channel_order_token(
    channel_id: &str,
    tab: ChannelVideoTab,
    order: ChannelOrder,
    target_id: &str,
) -> String {
    let (tab_field, envelope, order_val) = match tab {
        ChannelVideoTab::Videos => (
            15,
            8,
            match order {
                ChannelOrder::Latest => 4,
                ChannelOrder::Popular => 2,
                ChannelOrder::Oldest => 5,
            },
        ),
        ChannelVideoTab::Shorts => (
            10,
            7,
            match order {
                ChannelOrder::Latest => 4,
                ChannelOrder::Popular => 2,
                ChannelOrder::Oldest => 5,
            },
        ),
        ChannelVideoTab::Live => (
            14,
            8,
            match order {
                ChannelOrder::Latest => 12,
                ChannelOrder::Popular => 14,
                ChannelOrder::Oldest => 13,
            },
        ),
    };

    let mut sort_msg = Vec::new();
    proto_bytes(1, target_id.as_bytes(), &mut sort_msg);
    proto_varint(3, order_val, &mut sort_msg);

    let mut tab_msg = Vec::new();
    proto_bytes(envelope, &sort_msg, &mut tab_msg);

    let mut section_msg = Vec::new();
    proto_bytes(tab_field, &tab_msg, &mut section_msg);

    let mut inner_110 = Vec::new();
    proto_bytes(3, &section_msg, &mut inner_110);

    let mut inner_root = Vec::new();
    proto_bytes(110, &inner_110, &mut inner_root);

    let inner_b64 = base64_urlencoded(&inner_root);

    let mut outer_80226972 = Vec::new();
    proto_bytes(2, channel_id.as_bytes(), &mut outer_80226972);
    proto_bytes(3, inner_b64.as_bytes(), &mut outer_80226972);

    let mut outer_root = Vec::new();
    proto_bytes(80_226_972, &outer_80226972, &mut outer_root);

    base64_urlencoded(&outer_root)
}

/// Parses `(subscribers, videos)` from a channel header's `metadataRows` (`YT-5`).
pub fn channel_header_counts(metadata: &Value) -> (Option<u64>, Option<u64>) {
    let mut subscribers = None;
    let mut videos = None;
    if let Some(rows) = metadata.get("metadataRows").and_then(Value::as_array) {
        for row in rows {
            if let Some(parts) = row.get("metadataParts").and_then(Value::as_array) {
                for part in parts {
                    let Some(content) = part
                        .pointer("/text/content")
                        .or_else(|| part.get("simpleText"))
                        .and_then(Value::as_str)
                    else {
                        continue;
                    };
                    let lower = content.to_ascii_lowercase();
                    if subscribers.is_none()
                        && (lower.contains("subscriber") || lower.contains("inscrito"))
                    {
                        subscribers = parse_count(content);
                    } else if videos.is_none()
                        && (lower.contains("video") || lower.contains("vídeo"))
                    {
                        videos = parse_count(content);
                    }
                }
            }
        }
    }
    (subscribers, videos)
}

/// Extracts `ChannelHeader` from a channel `browse` response.
pub fn parse_channel_header(page: &Value, fallback_id: &str) -> ChannelHeader {
    let modern = page.pointer("/header/pageHeaderRenderer/content/pageHeaderViewModel");
    let c4 = page.pointer("/header/c4TabbedHeaderRenderer");
    let meta = page.pointer("/metadata/channelMetadataRenderer");

    let id = meta
        .and_then(|m| m.get("externalId"))
        .or_else(|| c4.and_then(|c| c.get("channelId")))
        .and_then(Value::as_str)
        .unwrap_or(fallback_id)
        .to_owned();

    let name = modern
        .and_then(|m| {
            m.pointer("/title/dynamicTextViewModel/text/content")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .or_else(|| {
            page.pointer("/header/pageHeaderRenderer/pageTitle")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .or_else(|| {
            c4.and_then(|c| c.get("title"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .or_else(|| {
            meta.and_then(|m| m.get("title"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_default();

    let metadata_vm = modern.and_then(|m| m.pointer("/metadata/contentMetadataViewModel"));
    let (mut subscribers, mut videos) = metadata_vm
        .map(channel_header_counts)
        .unwrap_or((None, None));
    if subscribers.is_none() {
        subscribers = c4
            .and_then(|c| text(c.get("subscriberCountText")))
            .and_then(|s| parse_count(&s));
    }
    if videos.is_none() {
        videos = c4
            .and_then(|c| text(c.get("videosCountText")))
            .and_then(|v| parse_count(&v));
    }

    let handle = metadata_vm
        .and_then(|m| {
            m.pointer("/metadataRows/0/metadataParts/0/text/content")
                .and_then(Value::as_str)
                .filter(|s| s.starts_with('@'))
                .map(str::to_owned)
        })
        .or_else(|| {
            c4.and_then(|c| text(c.get("channelHandleText")))
                .filter(|s| s.starts_with('@'))
        })
        .or_else(|| {
            meta.and_then(|m| m.get("vanityChannelUrl"))
                .and_then(Value::as_str)
                .and_then(|url| url.rsplit('/').next())
                .filter(|s| s.starts_with('@'))
                .map(str::to_owned)
        });

    let avatar = widest(
        modern
            .and_then(|m| {
                m.pointer("/image/decoratedAvatarViewModel/avatar/avatarViewModel/image/sources")
                    .or_else(|| m.pointer("/image/contentPreviewImageViewModel/image/sources"))
            })
            .or_else(|| c4.and_then(|c| c.pointer("/avatar/thumbnails")))
            .or_else(|| meta.and_then(|m| m.pointer("/avatar/thumbnails"))),
    );

    let banner = widest(
        modern
            .and_then(|m| m.pointer("/banner/imageBannerViewModel/image/sources"))
            .or_else(|| c4.and_then(|c| c.pointer("/banner/thumbnails"))),
    );

    let (has_shorts, has_live) = detect_channel_tabs(page);

    ChannelHeader {
        id,
        name,
        handle,
        avatar,
        banner,
        subscribers,
        videos,
        has_shorts,
        has_live,
    }
}

fn detect_channel_tabs(page: &Value) -> (bool, bool) {
    let mut has_shorts = false;
    let mut has_live = false;
    if let Some(tabs) = page
        .pointer("/contents/twoColumnBrowseResultsRenderer/tabs")
        .and_then(Value::as_array)
    {
        for tab in tabs {
            let renderer = tab
                .get("tabRenderer")
                .or_else(|| tab.get("expandableTabRenderer"));
            let Some(renderer) = renderer else {
                continue;
            };
            let title = renderer
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_ascii_lowercase();
            let params = renderer
                .pointer("/endpoint/browseEndpoint/params")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if title == "shorts" || params == TAB_SHORTS_PARAMS {
                has_shorts = true;
            } else if title == "live" || params == TAB_LIVE_PARAMS {
                has_live = true;
            }
        }
    }
    (has_shorts, has_live)
}

/// Resolves a channel's `UC…` id from a handle or vanity path (`@handle`, `c/name`).
pub async fn resolve_channel_id<'a>(rp: &Client, id: &'a str) -> Result<Cow<'a, str>, String> {
    if id.starts_with("UC") {
        return Ok(Cow::Borrowed(id));
    }
    let url = format!("https://www.youtube.com/{}", id.trim_start_matches('/'));
    let resp = rp
        .post_raw(
            "navigation/resolve_url",
            &json!({ "url": url }),
            false,
            None,
        )
        .await
        .map_err(super::text)?;
    let resolved = resp
        .pointer("/endpoint/browseEndpoint/browseId")
        .and_then(Value::as_str)
        .filter(|b| b.starts_with("UC"))
        .ok_or_else(|| "Could not complete the YouTube request. Retry.".to_owned())?;
    Ok(Cow::Owned(resolved.to_owned()))
}

/// Fetches a channel's Videos, Shorts, Live or Search tab.
pub async fn channel_tab(
    rp: &Client,
    id: &str,
    tab: &ChannelTab,
    order: ChannelOrder,
) -> Result<(ChannelHeader, Vec<Video>, Option<More>), String> {
    let id = resolve_channel_id(rp, id).await?;
    let id = id.as_ref();

    let (body, video_tab) = match tab {
        ChannelTab::Search(query) => (
            json!({
                "browseId": id,
                "params": TAB_SEARCH_PARAMS,
                "query": query,
            }),
            None,
        ),
        ChannelTab::Videos => (
            json!({ "browseId": id, "params": ChannelVideoTab::Videos.params() }),
            Some(ChannelVideoTab::Videos),
        ),
        ChannelTab::Shorts => (
            json!({ "browseId": id, "params": ChannelVideoTab::Shorts.params() }),
            Some(ChannelVideoTab::Shorts),
        ),
        ChannelTab::Live => (
            json!({ "browseId": id, "params": ChannelVideoTab::Live.params() }),
            Some(ChannelVideoTab::Live),
        ),
        _ => unreachable!("channel_tab is for the video tabs"),
    };

    let page = rp
        .post_raw("browse", &body, false, None)
        .await
        .map_err(super::text)?;
    let header = parse_channel_header(&page, id);
    let visitor_data = page
        .pointer("/responseContext/visitorData")
        .and_then(Value::as_str)
        .map(str::to_owned);

    let active_page = if order != ChannelOrder::Latest
        && let Some(vtab) = video_tab
    {
        let token = find_sort_chip_token(&page, order)
            .or_else(|| {
                find_target_uuid(&page).map(|target| channel_order_token(id, vtab, order, &target))
            })
            .ok_or_else(|| "Could not complete the YouTube request. Retry.".to_owned())?;
        rp.post_raw(
            "browse",
            &json!({ "continuation": token }),
            false,
            visitor_data.as_deref(),
        )
        .await
        .map_err(super::text)?
    } else {
        page
    };

    let (mut videos, ctoken) = collect_videos_and_ctoken(&active_page);
    let owner = super::Owner::of(&header);
    let shorts = *tab == ChannelTab::Shorts;
    for video in &mut videos {
        video.short |= shorts;
        owner.fill(video);
    }
    let more = ctoken.map(|ctoken| More {
        ctoken,
        endpoint: ContinuationEndpoint::Browse,
        visitor_data,
        authenticated: false,
        ours: false,
        owner: Some(owner),
    });
    Ok((header, videos, more))
}

fn find_sort_chip_token(page: &Value, order: ChannelOrder) -> Option<String> {
    let index = match order {
        ChannelOrder::Latest => 0,
        ChannelOrder::Popular => 1,
        ChannelOrder::Oldest => 2,
    };
    let mut chips = Vec::new();
    collect(page, &mut |value| {
        if let Some(token) = value
            .pointer("/chipViewModel/tapCommand/innertubeCommand/continuationCommand/token")
            .or_else(|| {
                value.pointer("/chipCloudChipRenderer/navigationEndpoint/continuationCommand/token")
            })
            .and_then(Value::as_str)
        {
            chips.push(token.to_owned());
        }
    });
    chips.get(index).cloned()
}

fn find_target_uuid(page: &Value) -> Option<String> {
    find(page, &|value| {
        value
            .get("targetId")
            .and_then(Value::as_str)
            .filter(|s| s.len() == 36 && s.chars().filter(|&c| c == '-').count() == 4)
            .map(str::to_owned)
    })
}

/// Fetches a channel's Playlists tab.
pub async fn channel_playlists_page(
    rp: &Client,
    id: &str,
) -> Result<(ChannelHeader, Vec<PlaylistSummary>, Option<More>), String> {
    let id = resolve_channel_id(rp, id).await?;
    let id = id.as_ref();
    let page = rp
        .post_raw(
            "browse",
            &json!({ "browseId": id, "params": TAB_PLAYLISTS_PARAMS }),
            false,
            None,
        )
        .await
        .map_err(super::text)?;
    let header = parse_channel_header(&page, id);
    let visitor_data = page
        .pointer("/responseContext/visitorData")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let (playlists, ctoken) = collect_playlists_and_ctoken(&page);
    let more = ctoken.map(|ctoken| More {
        ctoken,
        endpoint: ContinuationEndpoint::Browse,
        visitor_data,
        authenticated: false,
        ours: false,
        owner: None,
    });
    Ok((header, playlists, more))
}

/// Fetches a channel's About tab.
pub async fn channel_about(
    rp: &Client,
    id: &str,
    _needs_header: bool,
) -> Result<(Option<ChannelHeader>, ChannelAbout), String> {
    let id = resolve_channel_id(rp, id).await?;
    let id = id.as_ref();
    let page = rp
        .post_raw(
            "browse",
            &json!({ "browseId": id, "params": TAB_ABOUT_PARAMS }),
            false,
            None,
        )
        .await
        .map_err(super::text)?;
    let header = parse_channel_header(&page, id);
    let about = parse_channel_about(&page, &header);
    Ok((Some(header), about))
}

fn parse_channel_about(page: &Value, header: &ChannelHeader) -> ChannelAbout {
    let vm = find(page, &|v| v.get("aboutChannelViewModel"));
    let meta = page.pointer("/metadata/channelMetadataRenderer");

    let description = vm
        .and_then(|v| v.pointer("/description/content").and_then(Value::as_str))
        .or_else(|| meta.and_then(|m| m.get("description").and_then(Value::as_str)))
        .unwrap_or_default()
        .to_owned();

    let url = vm
        .and_then(|v| {
            v.pointer("/displayCanonicalChannelUrl/content")
                .or_else(|| v.pointer("/canonicalChannelUrl/content"))
                .and_then(Value::as_str)
        })
        .or_else(|| meta.and_then(|m| m.get("channelUrl").and_then(Value::as_str)))
        .map(|u| {
            if u.starts_with("http://") || u.starts_with("https://") {
                u.to_owned()
            } else {
                format!("https://{u}")
            }
        })
        .unwrap_or_default();

    let views = vm
        .and_then(|v| v.pointer("/viewCountText/content").and_then(Value::as_str))
        .and_then(parse_count);

    let subscribers = vm
        .and_then(|v| {
            v.pointer("/subscriberCountText/content")
                .and_then(Value::as_str)
        })
        .and_then(parse_count)
        .or(header.subscribers);

    let videos = vm
        .and_then(|v| v.pointer("/videoCountText/content").and_then(Value::as_str))
        .and_then(parse_count)
        .or(header.videos);

    let country = vm
        .and_then(|v| v.pointer("/country/content").and_then(Value::as_str))
        .map(str::to_owned);

    let created = vm
        .and_then(|v| {
            text(v.get("joinedDateText")).or_else(|| {
                v.pointer("/joinedDateText/content")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
        })
        .and_then(|joined| parse_joined_date(&joined));

    let mut links = Vec::new();
    if let Some(link_items) = vm.and_then(|v| v.get("links")).and_then(Value::as_array) {
        for item in link_items {
            let link_vm = item.get("channelExternalLinkViewModel").unwrap_or(item);
            let title = link_vm
                .pointer("/title/content")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let raw_url = link_vm
                .pointer("/link/commandRuns/0/onTap/innertubeCommand/urlEndpoint/url")
                .or_else(|| link_vm.pointer("/link/content"))
                .and_then(Value::as_str);
            if let Some(raw_url) = raw_url {
                let clean = unwrap_about_url(raw_url);
                if !clean.is_empty() {
                    links.push((title, clean));
                }
            }
        }
    }

    ChannelAbout {
        description,
        views,
        subscribers,
        videos,
        country,
        created,
        url,
        links,
    }
}

fn unwrap_about_url(raw: &str) -> String {
    if let Ok(parsed) = url::Url::parse(raw)
        && parsed.path() == "/redirect"
        && let Some((_, q)) = parsed.query_pairs().find(|(k, _)| k == "q")
    {
        return q.into_owned();
    }
    if raw.starts_with("http://") || raw.starts_with("https://") {
        raw.to_owned()
    } else if !raw.is_empty() {
        format!("https://{raw}")
    } else {
        String::new()
    }
}

/// Parses `"Joined Oct 24, 2009"` into `"2009-10-24"`.
fn parse_joined_date(joined: &str) -> Option<String> {
    let text = joined.strip_prefix("Joined ").unwrap_or(joined).trim();
    let mut parts = text.split_whitespace();
    let month_str = parts.next()?;
    let day_str = parts.next()?.trim_end_matches(',');
    let year_str = parts.next()?;
    let month = match &month_str.to_ascii_lowercase()[..3.min(month_str.len())] {
        "jan" => 1,
        "feb" => 2,
        "mar" => 3,
        "apr" => 4,
        "may" => 5,
        "jun" => 6,
        "jul" => 7,
        "aug" => 8,
        "sep" => 9,
        "oct" => 10,
        "nov" => 11,
        "dec" => 12,
        _ => return None,
    };
    let day: u8 = day_str.parse().ok()?;
    let year: i32 = year_str.parse().ok()?;
    Some(format!("{year:04}-{month:02}-{day:02}"))
}
