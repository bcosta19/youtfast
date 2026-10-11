//! A playlist in the layout YouTube moved to and rustypipe does not read
//! yet: its videos as `lockupViewModel`s straight in the page's section,
//! and the next page behind a `continuationItemViewModel`. Read through
//! rustypipe's `raw` request (`browse` `VL<id>`), with the session when
//! there is one (Watch later and liked videos always need it), without
//! it otherwise.

use rustypipe::client::RustyPipe;
use rustypipe::model::paginator::ContinuationEndpoint;
use serde_json::{Value, json};

use super::More;
use super::edit::{collect, post_as, text};
use super::home::{from_lockup, from_renderer};
use crate::model::Video;

/// The first page of a playlist's videos and the next page's directions.
pub async fn videos(rp: &RustyPipe, id: &str) -> Result<(Vec<Video>, Option<More>), String> {
    let body = json!({ "browseId": format!("VL{id}") });
    // Signed in first, for a private list; signed out when there is no
    // session to send.
    let (page, authenticated) = match post_as(rp, "browse", &body, true).await {
        Ok(page) => (page, true),
        Err(_) => (post_as(rp, "browse", &body, false).await?, false),
    };
    Ok(page_of(&page, authenticated))
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
