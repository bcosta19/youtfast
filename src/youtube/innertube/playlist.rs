//! A playlist in the layout YouTube moved to and rustypipe does not read
//! yet: its videos as `lockupViewModel`s straight in the page's section,
//! and the next page behind a `continuationItemViewModel`. Read through
//! rustypipe's `raw` request (`browse` `VL<id>`), with the session when
//! there is one (Watch later and liked videos always need it), without
//! it otherwise.

use rustypipe::client::RustyPipe;
use rustypipe::model::paginator::ContinuationEndpoint;
use serde_json::{Value, json};

use super::edit::{collect, post_as, text};
use super::home::{from_lockup, from_renderer};
use super::{More, PlaylistDetails};
use crate::model::Video;

/// The first page of a playlist's videos and the next page's directions.
pub async fn videos(rp: &RustyPipe, id: &str) -> Result<(Vec<Video>, Option<More>), String> {
    let (_, videos, more) = page_details(rp, id).await?;
    Ok((videos, more))
}

/// Metadata and videos from the same response, including YouTube's newer header.
pub async fn page_details(
    rp: &RustyPipe,
    id: &str,
) -> Result<(PlaylistDetails, Vec<Video>, Option<More>), String> {
    let body = json!({ "browseId": format!("VL{id}") });
    // Signed in first, for a private list; signed out when there is no
    // session to send.
    let (page, authenticated) = match post_as(rp, "browse", &body, true).await {
        Ok(page) => (page, true),
        Err(_) => (post_as(rp, "browse", &body, false).await?, false),
    };
    let (videos, more) = page_of(&page, authenticated);
    let mut details = metadata(&page);
    if details.thumbnail.is_none() {
        details.thumbnail = videos.first().map(Video::thumbnail);
    }
    Ok((details, videos, more))
}

fn metadata(page: &Value) -> PlaylistDetails {
    let old = page.pointer("/header/playlistHeaderRenderer");
    let modern = page.pointer("/header/pageHeaderRenderer/content/pageHeaderViewModel");
    let metadata = page.pointer("/metadata/playlistMetadataRenderer");
    let mut details = PlaylistDetails {
        name: old.and_then(|header| text(header.get("title")))
            .or_else(|| modern.and_then(|header| header.pointer("/title/dynamicTextViewModel/text/content").and_then(Value::as_str).map(str::to_owned)))
            .or_else(|| metadata.and_then(|data| data.get("title")).and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_default(),
        description: old.and_then(|header| text(header.get("descriptionText")))
            .or_else(|| modern.and_then(|header| header.pointer("/description/descriptionPreviewViewModel/description/content").and_then(Value::as_str).map(str::to_owned)))
            .or_else(|| metadata.and_then(|data| data.get("description")).and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_default(),
        channel: old.and_then(|header| text(header.get("ownerText")))
            .or_else(|| modern.and_then(|header| header.pointer("/metadata/contentMetadataViewModel/metadataRows/0/metadataParts/0/avatarStackViewModel/text/content").and_then(Value::as_str).map(str::to_owned))),
        videos: old.and_then(|header| text(header.get("numVideosText")))
            .or_else(|| modern.and_then(|header| header.pointer("/metadata/contentMetadataViewModel/metadataRows/1/metadataParts/1/text/content").and_then(Value::as_str).map(str::to_owned)))
            .and_then(|count| video_count(&count)),
        thumbnail: modern.and_then(|header| header.pointer("/heroImage/contentPreviewImageViewModel/image"))
            .or_else(|| old.and_then(|header| header.pointer("/playlistHeaderBanner/heroPlaylistThumbnailRenderer/thumbnail")))
            .and_then(thumbnail),
    };
    // Sidebar information belongs to the playlist itself; do not traverse
    // arbitrary recommendations, whose images and text are unrelated.
    if let Some(sidebar) = page
        .pointer("/sidebar/playlistSidebarRenderer/items")
        .and_then(Value::as_array)
    {
        for item in sidebar {
            if let Some(info) = item.get("playlistSidebarPrimaryInfoRenderer") {
                if details.thumbnail.is_none() {
                    details.thumbnail = info
                        .pointer("/thumbnailRenderer/playlistVideoThumbnailRenderer/thumbnail")
                        .and_then(thumbnail);
                }
                if details.description.is_empty() {
                    details.description = text(info.get("description")).unwrap_or_default();
                }
            }
        }
    }
    details
}

fn thumbnail(value: &Value) -> Option<String> {
    value
        .get("sources")
        .or_else(|| value.get("thumbnails"))?
        .as_array()?
        .iter()
        .max_by_key(|source| {
            source
                .get("width")
                .and_then(Value::as_u64)
                .unwrap_or_default()
        })?
        .get("url")?
        .as_str()
        .map(str::to_owned)
}

/// Accept exact localized counts; abbreviated or unfamiliar labels stay unknown.
fn video_count(text: &str) -> Option<u64> {
    let text = text.trim();
    let suffix_at = text.find(char::is_alphabetic).unwrap_or(text.len());
    let (number, suffix) = text.split_at(suffix_at);
    if !matches!(
        suffix.trim().to_lowercase().as_str(),
        "" | "video" | "videos" | "vídeo" | "vídeos"
    ) || !number
        .chars()
        .all(|ch| ch.is_ascii_digit() || ch.is_whitespace() || matches!(ch, ',' | '.'))
    {
        return None;
    }
    number
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()
}

/// The page after one [`videos`] or this read.
pub async fn more(rp: &RustyPipe, more: &More) -> Result<(Vec<Video>, Option<More>), String> {
    let body = json!({ "continuation": more.ctoken });
    let page = post_as(rp, "browse", &body, more.authenticated).await?;
    Ok(page_of(&page, more.authenticated))
}

fn page_of(page: &Value, authenticated: bool) -> (Vec<Video>, Option<More>) {
    let (videos, ctoken) = read(page);
    let more = ctoken.map(|ctoken| More {
        ctoken,
        endpoint: ContinuationEndpoint::Browse,
        visitor_data: None,
        authenticated,
        ours: true,
    });
    (videos, more)
}

/// The playlist's videos on a page, in order and once each, and the next
/// page's token. Only the list's own items count: a continuation can end
/// with a shelf of suggested playlists, whose videos are not the list's.
fn read(page: &Value) -> (Vec<Video>, Option<String>) {
    let mut videos: Vec<Video> = Vec::new();
    let mut ctoken = None;
    collect(page, &mut |value| {
        let items = value
            .pointer("/itemSectionRenderer/contents")
            .or_else(|| value.pointer("/appendContinuationItemsAction/continuationItems"))
            .or_else(|| value.pointer("/playlistVideoListRenderer/contents"))
            .and_then(Value::as_array);
        for item in items.into_iter().flatten() {
            let video = if let Some(lockup) = item.get("lockupViewModel") {
                from_lockup(lockup)
            } else if let Some(renderer) = item.get("playlistVideoRenderer") {
                from_playlist_renderer(renderer)
            } else {
                if ctoken.is_none() {
                    ctoken = item
                        .pointer(
                            "/continuationItemViewModel/continuationCommand/innertubeCommand/continuationCommand/token",
                        )
                        .or_else(|| {
                            item.pointer(
                                "/continuationItemRenderer/continuationEndpoint/continuationCommand/token",
                            )
                        })
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                }
                None
            };
            if let Some(video) = video
                && !videos.iter().any(|known| known.id == video.id)
            {
                videos.push(video);
            }
        }
        // The section and its continuation sit side by side in the list.
        if ctoken.is_none()
            && let Some(token) = value
                .pointer(
                    "/continuationItemViewModel/continuationCommand/innertubeCommand/continuationCommand/token",
                )
                .and_then(Value::as_str)
        {
            ctoken = Some(token.to_owned());
        }
    });
    (videos, ctoken)
}

/// The older playlist row, when a continuation still sends it.
fn from_playlist_renderer(renderer: &Value) -> Option<Video> {
    let mut video = from_renderer(renderer)?;
    if video.duration.is_none() {
        video.duration = renderer
            .get("lengthSeconds")
            .and_then(Value::as_str)
            .and_then(|seconds| seconds.parse().ok());
    }
    if video.channel.is_none() {
        video.channel = text(renderer.get("shortBylineText"));
    }
    Some(video)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playlist_counts_accept_localized_exact_values_and_reject_abbreviations() {
        assert_eq!(video_count("1,234 videos"), Some(1234));
        assert_eq!(video_count("1.234 vídeos"), Some(1234));
        assert_eq!(video_count("1\u{202f}234 vídeos"), Some(1234));
        assert_eq!(video_count("1 vídeo"), Some(1));
        assert_eq!(video_count("1.2K videos"), None);
        assert_eq!(video_count("1,2 mil vídeos"), None);
        assert_eq!(video_count("Nenhum vídeo"), None);
    }

    #[test]
    fn modern_playlist_metadata_stays_with_its_header() {
        let page = json!({
            "header": { "pageHeaderRenderer": { "content": { "pageHeaderViewModel": {
                "title": { "dynamicTextViewModel": { "text": { "content": "Aulas de Rust" } } },
                "description": { "descriptionPreviewViewModel": { "description": { "content": "Uma aula por semana" } } },
                "metadata": { "contentMetadataViewModel": { "metadataRows": [
                    { "metadataParts": [{ "avatarStackViewModel": { "text": { "content": "Escola" } } }] },
                    { "metadataParts": [{ "text": { "content": "Pública" } }, { "text": { "content": "1,234 vídeos" } }] }
                ] } },
                "heroImage": { "contentPreviewImageViewModel": { "image": { "sources": [
                    { "url": "https://i.ytimg.com/synthetic-small.jpg", "width": 160 },
                    { "url": "https://i.ytimg.com/synthetic-large.jpg", "width": 640 }
                ] } } }
            } } } },
            "recommendations": { "playlistHeaderRenderer": { "title": { "simpleText": "Outro nome" } } }
        });
        let details = metadata(&page);
        assert_eq!(details.name, "Aulas de Rust");
        assert_eq!(details.description, "Uma aula por semana");
        assert_eq!(details.channel.as_deref(), Some("Escola"));
        assert_eq!(details.videos, Some(1234));
        assert_eq!(
            details.thumbnail.as_deref(),
            Some("https://i.ytimg.com/synthetic-large.jpg")
        );
    }

    #[test]
    fn older_playlist_metadata_uses_its_own_sidebar_thumbnail() {
        let page = json!({
            "header": { "playlistHeaderRenderer": {
                "title": { "simpleText": "Favoritos" },
                "ownerText": { "runs": [{ "text": "Canal" }] },
                "numVideosText": { "runs": [{ "text": "42" }, { "text": " vídeos" }] }
            } },
            "sidebar": { "playlistSidebarRenderer": { "items": [{ "playlistSidebarPrimaryInfoRenderer": {
                "description": { "simpleText": "Seleção" },
                "thumbnailRenderer": { "playlistVideoThumbnailRenderer": { "thumbnail": { "thumbnails": [
                    { "url": "https://i.ytimg.com/synthetic-cover.jpg", "width": 480 }
                ] } } }
            } }] } }
        });
        let details = metadata(&page);
        assert_eq!(details.name, "Favoritos");
        assert_eq!(details.channel.as_deref(), Some("Canal"));
        assert_eq!(details.videos, Some(42));
        assert_eq!(details.description, "Seleção");
        assert_eq!(
            details.thumbnail.as_deref(),
            Some("https://i.ytimg.com/synthetic-cover.jpg")
        );
    }

    fn lockup(id: &str, title: &str) -> String {
        format!(
            r#"{{ "lockupViewModel": {{
                "contentId": "{id}",
                "contentType": "LOCKUP_CONTENT_TYPE_VIDEO",
                "metadata": {{ "lockupMetadataViewModel": {{
                    "title": {{ "content": "{title}" }}
                }} }}
            }} }}"#
        )
    }

    #[test]
    fn the_newer_layout_reads_the_lists_videos_and_next_page() {
        let first = format!(
            r#"{{ "contents": {{ "twoColumnBrowseResultsRenderer": {{ "tabs": [{{
                "tabRenderer": {{ "content": {{ "sectionListRenderer": {{ "contents": [
                    {{ "itemSectionRenderer": {{ "contents": [{}, {}, {}] }} }},
                    {{ "continuationItemViewModel": {{ "continuationCommand": {{
                        "innertubeCommand": {{ "continuationCommand": {{ "token": "page-2" }} }}
                    }} }} }}
                ] }} }} }}
            }}] }} }} }}"#,
            lockup("aaaaaaaaaaa", "One"),
            lockup("bbbbbbbbbbb", "Two"),
            lockup("aaaaaaaaaaa", "One again"),
        );
        let (videos, ctoken) = read(&serde_json::from_str(&first).unwrap());
        let titles: Vec<&str> = videos.iter().map(|video| video.title.as_str()).collect();
        assert_eq!(titles, ["One", "Two"]);
        assert_eq!(ctoken.as_deref(), Some("page-2"));
    }

    /// Against YouTube itself, signed out: `cargo test -- --ignored`.
    #[tokio::test]
    #[ignore = "needs the network"]
    async fn a_public_playlist_reads_from_youtube() {
        let rp = RustyPipe::builder().no_storage().build().unwrap();
        let (videos, _) = videos(&rp, "PLFgquLnL59alCl_2TQvOiD5Vgm1hCaGSI")
            .await
            .unwrap();
        assert!(videos.len() > 50, "{} videos", videos.len());
        assert!(videos.iter().all(|video| !video.title.is_empty()));
        assert!(
            videos
                .iter()
                .filter(|video| video.channel.is_some())
                .count()
                > 50
        );
    }

    #[test]
    fn a_continuation_leaves_out_the_suggestions_shelf() {
        let next = format!(
            r#"{{ "onResponseReceivedActions": [{{ "appendContinuationItemsAction": {{
                "continuationItems": [
                    {},
                    {{ "itemSectionRenderer": {{ "contents": [
                        {{ "horizontalShelfViewModel": {{ "items": [{}] }} }}
                    ] }} }}
                ]
            }} }}] }}"#,
            lockup("ccccccccccc", "Three"),
            lockup("ddddddddddd", "Suggested"),
        );
        let (videos, ctoken) = read(&serde_json::from_str(&next).unwrap());
        let titles: Vec<&str> = videos.iter().map(|video| video.title.as_str()).collect();
        assert_eq!(titles, ["Three"]);
        assert_eq!(ctoken, None);
    }
}
