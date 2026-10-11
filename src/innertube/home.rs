//! The account's home page: YouTube's own recommendations. rustypipe has
//! no call for it any more, so the first page goes through its `raw`
//! request (signed in, as the web client) to `browse` `FEwhat_to_watch`
//! and is read here; the pages after it are an ordinary `browse`
//! continuation, which rustypipe reads itself ([`super::more_videos`]).

use rustypipe::client::RustyPipe;
use rustypipe::model::paginator::ContinuationEndpoint;
use serde_json::{Value, json};

use super::More;
use super::edit::{collect, find, post, text};
use crate::model::Video;

/// What YouTube recommends to the account, and the next page. An account
/// with its history paused gets an empty page, not an error.
pub async fn recommended(rp: &RustyPipe) -> Result<(Vec<Video>, Option<More>), String> {
    let page = post(rp, "browse", &json!({ "browseId": "FEwhat_to_watch" })).await?;
    let (videos, ctoken) = read(&page);
    let more = ctoken.map(|ctoken| More {
        ctoken,
        endpoint: ContinuationEndpoint::Browse,
        visitor_data: page
            .pointer("/responseContext/visitorData")
            .and_then(Value::as_str)
            .map(str::to_owned),
        authenticated: true,
        ours: false,
    });
    Ok((videos, more))
}

/// The videos on a home page, in order and once each, and its
/// continuation token. Ads, Shorts shelves and playlists are left out.
fn read(page: &Value) -> (Vec<Video>, Option<String>) {
    let mut videos: Vec<Video> = Vec::new();
    let mut ctoken = None;
    collect(page, &mut |value| {
        let video = if let Some(renderer) = value.get("videoRenderer") {
            from_renderer(renderer)
        } else if let Some(lockup) = value.get("lockupViewModel") {
            from_lockup(lockup)
        } else {
            if ctoken.is_none()
                && let Some(token) = value
                    .pointer(
                        "/continuationItemRenderer/continuationEndpoint/continuationCommand/token",
                    )
                    .and_then(Value::as_str)
            {
                ctoken = Some(token.to_owned());
            }
            None
        };
        if let Some(video) = video
            && !videos.iter().any(|known| known.id == video.id)
        {
            videos.push(video);
        }
    });
    (videos, ctoken)
}

/// The older card: `videoRenderer`.
pub(super) fn from_renderer(renderer: &Value) -> Option<Video> {
    let id = renderer.get("videoId")?.as_str()?.to_owned();
    let owner = renderer
        .get("ownerText")
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
        });
    Some(Video {
        id,
        title: text(renderer.get("title")).unwrap_or_default(),
        channel: text(owner),
        channel_id: owner.and_then(|owner| {
            owner
                .pointer("/runs/0/navigationEndpoint/browseEndpoint/browseId")
                .and_then(Value::as_str)
                .map(str::to_owned)
        }),
        duration: text(renderer.get("lengthText")).and_then(|length| parse_length(&length)),
        views: text(renderer.get("viewCountText")).and_then(|views| parse_count(&views)),
        live,
        published_at: None,
        published: text(renderer.get("publishedTimeText")),
        short: false,
    })
}

/// The newer card: a `lockupViewModel` of a video. Its rows are the
/// channel, then `views · age`.
pub(super) fn from_lockup(lockup: &Value) -> Option<Video> {
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
    let channel_id = rows
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
    // The badge over the thumbnail: the length, or LIVE.
    let badge = lockup.get("contentImage").and_then(|image| {
        find(image, &|value| {
            value
                .pointer("/thumbnailBadgeViewModel/text")
                .and_then(Value::as_str)
        })
    });
    let live = badge.is_some_and(|badge| badge.eq_ignore_ascii_case("live"));
    let (views, published) = match (part(1, 0), part(1, 1)) {
        // A live stream's row is `N watching`, with no age.
        (Some(first), second) => (parse_count(&first), second),
        (None, _) => (None, None),
    };
    Some(Video {
        id,
        title,
        channel: part(0, 0),
        channel_id,
        duration: badge.and_then(parse_length),
        views,
        live,
        published_at: None,
        published,
        short: false,
    })
}

/// `"12:34"` or `"1:02:03"` in seconds.
fn parse_length(text: &str) -> Option<f64> {
    let mut seconds = 0.0;
    for part in text.trim().split(':') {
        seconds = seconds * 60.0 + part.parse::<u32>().ok()? as f64;
    }
    Some(seconds)
}

/// `"1,234 views"`, `"1.2M views"`, `"35K watching"` as a number.
fn parse_count(text: &str) -> Option<u64> {
    let word = text.split_whitespace().next()?.replace(',', "");
    let (number, scale) = match word.chars().last()? {
        'K' | 'k' => (&word[..word.len() - 1], 1e3),
        'M' => (&word[..word.len() - 1], 1e6),
        'B' => (&word[..word.len() - 1], 1e9),
        _ => (word.as_str(), 1.0),
    };
    let value: f64 = number.parse().ok()?;
    Some((value * scale).round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_card_kinds_are_read_once_each_with_the_next_page() {
        let page: Value = serde_json::from_str(
            r#"{ "contents": { "twoColumnBrowseResultsRenderer": { "tabs": [{
            "tabRenderer": { "content": { "richGridRenderer": { "contents": [
                { "richItemRenderer": { "content": { "videoRenderer": {
                    "videoId": "aaaaaaaaaaa",
                    "title": { "runs": [{ "text": "Old card" }] },
                    "ownerText": { "runs": [{ "text": "Chan", "navigationEndpoint":
                        { "browseEndpoint": { "browseId": "UCold" } } }] },
                    "lengthText": { "simpleText": "1:02:03" },
                    "viewCountText": { "simpleText": "1,234 views" },
                    "publishedTimeText": { "simpleText": "2 days ago" }
                } } } },
                { "richItemRenderer": { "content": { "lockupViewModel": {
                    "contentId": "bbbbbbbbbbb",
                    "contentType": "LOCKUP_CONTENT_TYPE_VIDEO",
                    "contentImage": { "thumbnailViewModel": { "overlays": [
                        { "thumbnailOverlayBadgeViewModel": { "thumbnailBadges": [
                            { "thumbnailBadgeViewModel": { "text": "12:34" } }
                        ] } }
                    ] } },
                    "metadata": { "lockupMetadataViewModel": {
                        "title": { "content": "New card" },
                        "metadata": { "contentMetadataViewModel": { "metadataRows": [
                            { "metadataParts": [{ "text": { "content": "Other",
                                "commandRuns": [{ "onTap": { "innertubeCommand":
                                    { "browseEndpoint": { "browseId": "UCnew" } } } }] } }] },
                            { "metadataParts": [
                                { "text": { "content": "1.2M views" } },
                                { "text": { "content": "3 weeks ago" } }
                            ] }
                        ] } }
                    } }
                } } } },
                { "richItemRenderer": { "content": { "lockupViewModel": {
                    "contentId": "PLx", "contentType": "LOCKUP_CONTENT_TYPE_PLAYLIST"
                } } } },
                { "richItemRenderer": { "content": { "videoRenderer": {
                    "videoId": "aaaaaaaaaaa"
                } } } },
                { "continuationItemRenderer": { "continuationEndpoint": {
                    "continuationCommand": { "token": "next-page" }
                } } }
            ] } } }
        }] } } }"#,
        )
        .unwrap();
        let (videos, ctoken) = read(&page);
        assert_eq!(ctoken.as_deref(), Some("next-page"));
        assert_eq!(videos.len(), 2);
        let old = &videos[0];
        assert_eq!(
            (old.id.as_str(), old.title.as_str(), old.channel.as_deref()),
            ("aaaaaaaaaaa", "Old card", Some("Chan"))
        );
        assert_eq!(old.channel_id.as_deref(), Some("UCold"));
        assert_eq!(old.duration, Some(3723.0));
        assert_eq!(old.views, Some(1234));
        assert_eq!(old.published.as_deref(), Some("2 days ago"));
        let new = &videos[1];
        assert_eq!(
            (new.id.as_str(), new.title.as_str(), new.channel.as_deref()),
            ("bbbbbbbbbbb", "New card", Some("Other"))
        );
        assert_eq!(new.channel_id.as_deref(), Some("UCnew"));
        assert_eq!(new.duration, Some(754.0));
        assert_eq!(new.views, Some(1_200_000));
        assert_eq!(new.published.as_deref(), Some("3 weeks ago"));
    }

    #[test]
    fn counts_and_lengths_read_youtubes_wording() {
        assert_eq!(parse_count("35K watching"), Some(35_000));
        assert_eq!(parse_count("No views"), None);
        assert_eq!(parse_length("LIVE"), None);
        assert_eq!(parse_length("0:59"), Some(59.0));
    }
}
