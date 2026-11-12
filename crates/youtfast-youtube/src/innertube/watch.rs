//! Watch page metadata (`details`), comments (`comments`), and player
//! storyboards/captions (`player_extras`).

use std::collections::HashMap;

use serde_json::{Value, json};

use super::client::{Client, Error};
use super::edit::{collect, find, text};
use super::items::{
    at_least_value, collect_videos_and_ctoken, extract_ctoken, parse_count, parse_length,
    spans_from_value, widest,
};
use youtfast_core::model::{Channel, Chapter, CommentRow, Details, Span};

/// The width a comment's avatar is fetched at: it is drawn about 36 px
/// square, and a page shows dozens.
pub const COMMENT_AVATAR: u32 = 64;

/// Fetches watch-page metadata, description, chapters, related videos and
/// the initial comments continuation token.
pub async fn details(rp: &Client, id: &str) -> Result<Details, String> {
    let page = rp
        .post_raw("next", &json!({ "videoId": id }), false, None)
        .await
        .map_err(super::text)?;
    Ok(parse_details(&page, id))
}

pub fn parse_details(page: &Value, id: &str) -> Details {
    let primary = find(page, &|v| v.get("videoPrimaryInfoRenderer"));
    let secondary = find(page, &|v| v.get("videoSecondaryInfoRenderer"));
    let owner = secondary.and_then(|s| s.pointer("/owner/videoOwnerRenderer"));

    let title = primary
        .and_then(|p| text(p.get("title")))
        .unwrap_or_default();

    let channel_id = owner
        .and_then(|o| {
            o.pointer("/navigationEndpoint/browseEndpoint/browseId")
                .or_else(|| o.pointer("/title/runs/0/navigationEndpoint/browseEndpoint/browseId"))
                .and_then(Value::as_str)
        })
        .unwrap_or_default()
        .to_owned();
    let channel_name = owner.and_then(|o| text(o.get("title"))).unwrap_or_default();
    let channel_avatar = owner.and_then(|o| widest(o.pointer("/thumbnail/thumbnails")));
    let subscribers = owner
        .and_then(|o| text(o.get("subscriberCountText")))
        .and_then(|s| parse_count(&s));

    let view_renderer = primary.and_then(|p| p.pointer("/viewCount/videoViewCountRenderer"));
    let views = view_renderer
        .and_then(|vr| text(vr.get("viewCount")).or_else(|| text(vr.get("shortViewCount"))))
        .and_then(|v| parse_count(&v))
        .unwrap_or(0);
    let live = view_renderer
        .and_then(|vr| vr.get("isLive"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let likes = extract_like_count(page, primary).map(|n| n as u32);

    let published =
        primary.and_then(|p| text(p.get("dateText")).or_else(|| text(p.get("relativeDateText"))));
    let published_date = published.as_deref().and_then(parse_date_text);

    let description = secondary
        .and_then(|s| {
            if let Some(attr) = s.get("attributedDescription") {
                Some(spans_from_value(Some(attr), id))
            } else {
                s.get("description")
                    .map(|desc| spans_from_value(Some(desc), id))
            }
        })
        .unwrap_or_default();

    let chapters = extract_chapters(page);

    let related_root = page
        .pointer("/contents/twoColumnWatchNextResults/secondaryResults/secondaryResults")
        .unwrap_or(&Value::Null);
    let (mut related, _) = collect_videos_and_ctoken(related_root);
    related.retain(|v| v.id != id);

    let (comments_token, comment_count) = extract_comments_entry(page);
    let visitor_data = page
        .pointer("/responseContext/visitorData")
        .and_then(Value::as_str)
        .map(str::to_owned);

    Details {
        id: id.to_owned(),
        title,
        channel: Channel {
            id: channel_id,
            name: channel_name,
            avatar: channel_avatar,
        },
        subscribers,
        views,
        likes,
        published,
        published_date,
        live,
        description,
        chapters,
        related,
        comments_token,
        comment_count,
        visitor_data,
    }
}

fn extract_like_count(page: &Value, primary: Option<&Value>) -> Option<u64> {
    if let Some(primary) = primary
        && let Some(count) = find(primary, &|v| {
            let btn = v.get("likeButtonViewModel")?.get("likeButtonViewModel")?;
            let title = btn
                .pointer(
                    "/toggleButtonViewModel/toggleButtonViewModel/defaultButtonViewModel/buttonViewModel/title",
                )
                .and_then(Value::as_str)?;
            parse_count(title)
        })
    {
        return Some(count);
    }
    find(page, &|v| {
        v.pointer("/likeCountEntity/likeCountIfIndifferent/content")
            .or_else(|| v.pointer("/likeCountEntity/expandedLikeCountIfIndifferent/content"))
            .and_then(Value::as_str)
            .and_then(parse_count)
    })
}

fn parse_date_text(raw: &str) -> Option<String> {
    let cleaned = raw
        .trim()
        .strip_prefix("Premiered ")
        .or_else(|| raw.trim().strip_prefix("Streamed live on "))
        .or_else(|| raw.trim().strip_prefix("Started streaming on "))
        .unwrap_or(raw.trim());
    let mut parts = cleaned.split_whitespace();
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

fn extract_chapters(page: &Value) -> Vec<Chapter> {
    let mut chapters: Vec<Chapter> = Vec::new();
    collect(page, &mut |value| {
        if let Some(ch) = value.get("chapterRenderer") {
            let Some(name) = text(ch.get("title")) else {
                return;
            };
            let Some(ms) = ch.get("timeRangeStartMillis").and_then(Value::as_u64) else {
                return;
            };
            let seconds = (ms / 1000) as u32;
            if !chapters.iter().any(|c| c.seconds == seconds) {
                chapters.push(Chapter { name, seconds });
            }
        } else if let Some(macro_item) = value.get("macroMarkersListItemRenderer") {
            let Some(name) = text(macro_item.get("title")) else {
                return;
            };
            let seconds = macro_item
                .pointer("/onTap/watchEndpoint/startTimeSeconds")
                .and_then(Value::as_u64)
                .map(|s| s as u32)
                .or_else(|| {
                    text(macro_item.get("timeDescription"))
                        .and_then(|t| parse_length(&t))
                        .map(|s| s as u32)
                });
            if let Some(seconds) = seconds
                && !chapters.iter().any(|c| c.seconds == seconds)
            {
                chapters.push(Chapter { name, seconds });
            }
        }
    });
    chapters
}

fn extract_comments_entry(page: &Value) -> (Option<String>, Option<u64>) {
    let mut token = None;
    let mut count = None;

    if let Some(panels) = page.get("engagementPanels").and_then(Value::as_array) {
        for panel in panels {
            let renderer = panel
                .get("engagementPanelSectionListRenderer")
                .unwrap_or(panel);
            let id = renderer
                .get("panelIdentifier")
                .or_else(|| renderer.get("targetId"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            if id.contains("comment") {
                if count.is_none() {
                    count = text(
                        renderer
                            .pointer("/header/engagementPanelTitleHeaderRenderer/contextualInfo"),
                    )
                    .and_then(|s| parse_count(&s));
                }
                if token.is_none() {
                    token = find(renderer, &extract_ctoken);
                }
            }
        }
    }

    collect(page, &mut |value| {
        if count.is_none()
            && let Some(header) = value.get("commentsHeaderRenderer")
        {
            count = text(header.get("countText"))
                .or_else(|| text(header.get("commentsCount")))
                .and_then(|s| parse_count(&s));
        }
        if count.is_none()
            && let Some(entry) = value.get("commentsEntryPointHeaderRenderer")
        {
            count = text(entry.get("commentCount")).and_then(|s| parse_count(&s));
        }
        if token.is_none()
            && let Some(section) = value.get("itemSectionRenderer")
        {
            let is_comments = section
                .get("sectionIdentifier")
                .or_else(|| section.get("targetId"))
                .and_then(Value::as_str)
                .is_some_and(|s| s.contains("comment"));
            if is_comments {
                token = find(section, &extract_ctoken);
            }
        }
    });

    (token, count)
}

/// Fetches a page of comments (or of a comment's replies), and the next page's
/// continuation token when there is one.
pub async fn comments(
    rp: &Client,
    ctoken: &str,
    visitor_data: Option<&str>,
    watching_id: &str,
) -> Result<(Vec<CommentRow>, Option<String>), String> {
    let page = rp
        .post_raw(
            "next",
            &json!({ "continuation": ctoken }),
            false,
            visitor_data,
        )
        .await
        .map_err(super::text)?;
    let (rows, next) = parse_comments_page(&page, watching_id);
    if rows.is_empty()
        && let Some(reload_token) = next.as_deref()
        && reload_token != ctoken
    {
        let second = rp
            .post_raw(
                "next",
                &json!({ "continuation": reload_token }),
                false,
                visitor_data,
            )
            .await
            .map_err(super::text)?;
        return Ok(parse_comments_page(&second, watching_id));
    }
    Ok((rows, next))
}

pub fn parse_comments_page(page: &Value, watching_id: &str) -> (Vec<CommentRow>, Option<String>) {
    let entities = collect_comment_entities(page);
    let mut rows: Vec<CommentRow> = Vec::new();
    let mut next_token: Option<String> = None;

    let mut action_lists = Vec::new();
    collect(page, &mut |value| {
        if let Some(items) = value
            .pointer("/reloadContinuationItemsCommand/continuationItems")
            .or_else(|| value.pointer("/appendContinuationItemsAction/continuationItems"))
            .and_then(Value::as_array)
        {
            action_lists.push(items);
        }
    });

    for items in action_lists {
        for item in items {
            if let Some(thread) = item.get("commentThreadRenderer") {
                if let Some(row) = parse_comment_thread(thread, &entities, watching_id) {
                    rows.push(row);
                }
            } else if let Some(comment) = item.get("commentRenderer") {
                if let Some(row) = parse_legacy_comment(comment, None, watching_id) {
                    rows.push(row);
                }
            } else if let Some(vm) = item.get("commentViewModel") {
                if let Some(row) = parse_modern_comment(vm, None, &entities, watching_id) {
                    rows.push(row);
                }
            } else if next_token.is_none() {
                next_token = extract_ctoken(item);
            }
        }
    }

    (rows, next_token)
}

#[derive(Default)]
struct CommentEntities<'a> {
    comments: HashMap<&'a str, &'a Value>,
    toolbar_states: HashMap<&'a str, &'a Value>,
    surfaces: HashMap<&'a str, &'a Value>,
}

fn collect_comment_entities(page: &Value) -> CommentEntities<'_> {
    let mut out = CommentEntities::default();
    let Some(mutations) = page
        .pointer("/frameworkUpdates/entityBatchUpdate/mutations")
        .and_then(Value::as_array)
    else {
        return out;
    };
    for m in mutations {
        if let Some(payload) = m.pointer("/payload/commentEntityPayload") {
            if let Some(id) = payload
                .pointer("/properties/commentId")
                .and_then(Value::as_str)
            {
                out.comments.insert(id, payload);
            }
            if let Some(key) = m.get("entityKey").and_then(Value::as_str) {
                out.comments.insert(key, payload);
            }
        }
        if let Some(state) = m.pointer("/payload/engagementToolbarStateEntityPayload")
            && let Some(key) = m.get("entityKey").and_then(Value::as_str)
        {
            out.toolbar_states.insert(key, state);
        }
        if let Some(surface) = m.pointer("/payload/commentSurfaceEntityPayload")
            && let Some(key) = m.get("entityKey").and_then(Value::as_str)
        {
            out.surfaces.insert(key, surface);
        }
    }
    out
}

fn parse_comment_thread(
    thread: &Value,
    entities: &CommentEntities<'_>,
    watching_id: &str,
) -> Option<CommentRow> {
    let replies_token = thread.get("replies").and_then(|r| find(r, &extract_ctoken));
    let mut row = if let Some(vm) = thread
        .pointer("/commentViewModel/commentViewModel")
        .or_else(|| thread.get("commentViewModel"))
    {
        parse_modern_comment(vm, replies_token, entities, watching_id)?
    } else {
        let legacy = thread.pointer("/comment/commentRenderer")?;
        parse_legacy_comment(legacy, replies_token, watching_id)?
    };
    row.pinned |= thread.get("renderingPriority").and_then(Value::as_str)
        == Some("RENDERING_PRIORITY_PINNED_COMMENT");
    Some(row)
}

fn parse_modern_comment(
    vm: &Value,
    replies_token: Option<String>,
    entities: &CommentEntities<'_>,
    watching_id: &str,
) -> Option<CommentRow> {
    let id = vm
        .get("commentId")
        .or_else(|| vm.get("commentKey"))
        .and_then(Value::as_str)?;
    let payload = entities
        .comments
        .get(id)
        .or_else(|| {
            vm.get("commentKey")
                .and_then(Value::as_str)
                .and_then(|k| entities.comments.get(k))
        })
        .copied()?;

    let author_obj = payload.get("author");
    let author = author_obj
        .and_then(|a| a.get("displayName").or_else(|| a.get("channelCommand")))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let author_id = author_obj
        .and_then(|a| a.get("channelId"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let author_avatar = author_obj.and_then(|a| {
        at_least_value(a.pointer("/avatar/image/sources"), COMMENT_AVATAR).or_else(|| {
            a.get("avatarThumbnailUrl")
                .and_then(Value::as_str)
                .map(super::items::absolute)
        })
    });
    let by_owner = author_obj
        .and_then(|a| a.get("isCreator"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let surface = vm
        .get("commentSurfaceKey")
        .and_then(Value::as_str)
        .and_then(|key| entities.surfaces.get(key));
    let mut text_spans = spans_from_value(payload.pointer("/properties/content"), watching_id);
    if text_spans.is_empty() {
        text_spans = spans_from_value(
            surface.and_then(|surface| {
                surface.pointer(
                    "/voiceReplyContainerViewModel/voiceReplyContainerViewModel/transcriptText",
                )
            }),
            watching_id,
        );
    }
    let published = payload
        .pointer("/properties/publishedTime")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();

    let image = at_least_value(
        surface.and_then(|surface| {
            surface
                .pointer("/imageAttachmentViewModel/commentImageAttachmentViewModel/image/sources")
        }),
        320,
    )
    .or_else(|| {
        at_least_value(
            payload
                .pointer("/properties/imageAttachment/image/sources")
                .or_else(|| vm.pointer("/imageAttachment/image/sources")),
            320,
        )
    });

    let toolbar = payload.get("toolbar");
    let likes = toolbar
        .and_then(|t| {
            t.get("likeCountNotliked")
                .or_else(|| t.get("likeCountLiked"))
                .or_else(|| t.get("likeCountA11y"))
                .and_then(Value::as_str)
        })
        .and_then(parse_count)
        .map(|n| n as u32);
    let replies = toolbar
        .and_then(|t| {
            t.get("replyCount")
                .or_else(|| t.get("replyCountA11y"))
                .and_then(Value::as_str)
        })
        .and_then(parse_count)
        .unwrap_or(0) as u32;

    let toolbar_state_key = payload
        .pointer("/Properties/toolbarStateKey")
        .or_else(|| payload.pointer("/properties/toolbarStateKey"))
        .or_else(|| vm.get("toolbarStateKey"))
        .and_then(Value::as_str);
    let hearted = toolbar_state_key
        .and_then(|k| entities.toolbar_states.get(k))
        .and_then(|s| s.get("heartState"))
        .and_then(Value::as_str)
        == Some("TOOLBAR_HEART_STATE_HEARTED")
        || toolbar
            .and_then(|t| t.get("creatorThumbnailUrl"))
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty());

    let pinned = vm.get("pinnedText").is_some();

    Some(CommentRow {
        id: id.to_owned(),
        author,
        author_id,
        author_avatar,
        text: text_spans,
        image,
        published,
        likes,
        replies,
        replies_token,
        pinned,
        by_owner,
        hearted,
    })
}

pub fn parse_legacy_comment(
    renderer: &Value,
    replies_token: Option<String>,
    watching_id: &str,
) -> Option<CommentRow> {
    let id = renderer
        .get("commentId")
        .or_else(|| renderer.get("id"))?
        .as_str()?
        .to_owned();
    let author = text(renderer.get("authorText")).or_else(|| {
        renderer
            .pointer("/author/name")
            .and_then(Value::as_str)
            .map(str::to_owned)
    });
    let author_id = renderer
        .pointer("/authorEndpoint/browseEndpoint/browseId")
        .or_else(|| renderer.pointer("/author/id"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let author_avatar = at_least_value(
        renderer
            .pointer("/authorThumbnail/thumbnails")
            .or_else(|| renderer.pointer("/author/avatar")),
        COMMENT_AVATAR,
    );
    let text_spans = if let Some(content) = renderer.get("contentText") {
        spans_from_value(Some(content), watching_id)
    } else if let Some(arr) = renderer.get("text").and_then(Value::as_array) {
        arr.iter()
            .filter_map(|item| span_from_synthetic_component(item, watching_id))
            .collect()
    } else {
        Vec::new()
    };
    let image = at_least_value(
        renderer
            .pointer("/backstageAttachment/backstageImageRenderer/image/thumbnails")
            .or_else(|| renderer.get("image")),
        320,
    );
    let published = text(renderer.get("publishedTimeText"))
        .or_else(|| {
            renderer
                .get("publish_date_txt")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_default();
    let likes = text(renderer.get("voteCount"))
        .and_then(|c| parse_count(&c))
        .or_else(|| renderer.get("like_count").and_then(Value::as_u64))
        .map(|n| n as u32);
    let replies = renderer
        .get("replyCount")
        .or_else(|| renderer.get("reply_count"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    let replies_token = replies_token.or_else(|| {
        renderer
            .pointer("/replies/ctoken")
            .and_then(Value::as_str)
            .map(str::to_owned)
    });
    let pinned = renderer.get("pinnedCommentBadge").is_some()
        || renderer
            .get("pinned")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let by_owner = renderer
        .get("authorIsChannelOwner")
        .or_else(|| renderer.get("by_owner"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let hearted = renderer
        .pointer("/actionButtons/commentActionButtonsRenderer/creatorHeart")
        .is_some()
        || renderer
            .get("hearted")
            .and_then(Value::as_bool)
            .unwrap_or(false);

    Some(CommentRow {
        id,
        author,
        author_id,
        author_avatar,
        text: text_spans,
        image,
        published,
        likes,
        replies,
        replies_token,
        pinned,
        by_owner,
        hearted,
    })
}

/// Converts a synthetic or legacy text component JSON object into a `Span`.
pub fn span_from_synthetic_component(component: &Value, watching_id: &str) -> Option<Span> {
    if let Some(t) = component.get("Text") {
        let text = t.get("text")?.as_str()?.to_owned();
        return Some(Span::Text(text));
    }
    if let Some(w) = component.get("Web") {
        let text = w.get("text")?.as_str()?.to_owned();
        let url = w.get("url")?.as_str()?.to_owned();
        return Some(Span::Link { text, url });
    }
    if let Some(yt) = component.get("YouTube") {
        let text = yt.get("text")?.as_str()?.to_owned();
        let target = yt.get("target")?;
        if let Some(v) = target.get("Video") {
            let id = v.get("id")?.as_str()?.to_owned();
            let start_time = v.get("start_time").and_then(Value::as_u64).unwrap_or(0) as u32;
            if id == watching_id {
                return Some(Span::Seek {
                    text,
                    seconds: start_time,
                });
            }
            return Some(Span::Video { text, id });
        }
        if let Some(ch) = target.get("Channel") {
            let id = ch.get("id")?.as_str()?.to_owned();
            return Some(Span::Channel { text, id });
        }
        if let Some(pl) = target.get("Playlist") {
            let id = pl.get("id")?.as_str()?;
            return Some(Span::Link {
                text,
                url: format!("https://www.youtube.com/playlist?list={id}"),
            });
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Player extras: storyboards and caption tracks (`extras.rs`)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoryboardSpec {
    pub url_template: String,
    pub frame_width: u32,
    pub frame_height: u32,
    pub frames_per_page_x: u32,
    pub frames_per_page_y: u32,
    pub total_count: u32,
    pub duration_per_frame: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptionTrack {
    pub url: String,
    pub lang: String,
    pub lang_name: String,
    pub auto_generated: bool,
}

/// Fetches unauthenticated storyboard and caption metadata from `/youtubei/v1/player`.
pub async fn player_extras(
    rp: &Client,
    id: &str,
) -> Result<(Vec<StoryboardSpec>, Vec<CaptionTrack>), Error> {
    let player = rp.post_player_unauthenticated(id).await?;
    let storyboards = player
        .pointer("/storyboards/playerStoryboardSpecRenderer/spec")
        .or_else(|| player.pointer("/storyboards/playerLiveStoryboardSpecRenderer/spec"))
        .and_then(Value::as_str)
        .map(parse_storyboard_spec)
        .unwrap_or_default();
    let captions = parse_caption_tracks(&player);
    Ok((storyboards, captions))
}

pub fn parse_storyboard_spec(spec: &str) -> Vec<StoryboardSpec> {
    let mut parts = spec.split('|');
    let Some(base_url) = parts.next() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (level, entry) in parts.enumerate() {
        let fields: Vec<&str> = entry.split('#').collect();
        if fields.len() < 8 {
            continue;
        }
        let Ok(frame_width) = fields[0].parse::<u32>() else {
            continue;
        };
        let Ok(frame_height) = fields[1].parse::<u32>() else {
            continue;
        };
        let Ok(total_count) = fields[2].parse::<u32>() else {
            continue;
        };
        let Ok(frames_per_page_x) = fields[3].parse::<u32>() else {
            continue;
        };
        let Ok(frames_per_page_y) = fields[4].parse::<u32>() else {
            continue;
        };
        let Ok(duration_per_frame) = fields[5].parse::<u32>() else {
            continue;
        };
        let name = fields[6];
        let sigh = fields[7];
        let sep = if base_url.contains('?') { '&' } else { '?' };
        let url_template = format!(
            "{}{sep}sigh={sigh}",
            base_url
                .replace("$L", &level.to_string())
                .replace("$N", name)
        );
        out.push(StoryboardSpec {
            url_template,
            frame_width,
            frame_height,
            frames_per_page_x,
            frames_per_page_y,
            total_count,
            duration_per_frame,
        });
    }
    out
}

fn parse_caption_tracks(player: &Value) -> Vec<CaptionTrack> {
    let Some(tracks) = player
        .pointer("/captions/playerCaptionsTracklistRenderer/captionTracks")
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    tracks
        .iter()
        .filter_map(|track| {
            let url = track.get("baseUrl")?.as_str()?.to_owned();
            let lang = track.get("languageCode")?.as_str()?.to_owned();
            let lang_name = text(track.get("name")).unwrap_or_else(|| lang.clone());
            let auto_generated = track.get("kind").and_then(Value::as_str) == Some("asr");
            Some(CaptionTrack {
                url,
                lang,
                lang_name,
                auto_generated,
            })
        })
        .collect()
}
