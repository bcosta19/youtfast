//! Hooks for YouTFast's regression tests of YouTube's response formats
//! (`tests/youtube_api.rs`, listed in `docs/youtube-api-format.md`).
//!
//! Each function calls the code the queries themselves use, so a test fails
//! if that code goes back to reading or building an old format. Built only
//! with the `regression` feature, which YouTFast enables for its tests; this
//! is not a stable API.

use super::{channel, response};
use crate::{
    model::{PlaylistItem, VideoItem},
    param::{ChannelOrder, ChannelVideoTab, Language},
    serializer::MapResult,
};

/// The continuation token that sorts a channel's video tab.
pub fn channel_order_token(
    channel_id: &str,
    tab: ChannelVideoTab,
    order: ChannelOrder,
    target_id: &str,
) -> String {
    channel::order_ctoken(channel_id, tab, order, target_id)
}

/// The `params` each channel tab request sends, by tab name.
pub fn channel_tab_params() -> Vec<(&'static str, String)> {
    use channel::ChannelTab;
    [
        ("videos", ChannelTab::Videos),
        ("shorts", ChannelTab::Shorts),
        ("live", ChannelTab::Live),
        ("playlists", ChannelTab::Playlists),
        ("search", ChannelTab::Search),
    ]
    .into_iter()
    .map(|(name, tab)| {
        let value = serde_json::to_value(tab).expect("a tab serializes to its params");
        (name, value.as_str().unwrap_or_default().to_owned())
    })
    .collect()
}

/// Items of a video list (a channel's Videos, Shorts or Live tab) read from a
/// JSON array of list items, with the warnings mapping left. An item that does
/// not deserialize is skipped, as in a real response.
pub fn video_items(
    json: &str,
    lang: Language,
) -> Result<(Vec<VideoItem>, Vec<String>), serde_json::Error> {
    let items: MapResult<Vec<response::YouTubeListItem>> = serde_json::from_str(json)?;
    let mut mapper = response::YouTubeListMapper::<VideoItem>::new(lang);
    mapper.map_response(items);
    Ok((mapper.items, mapper.warnings))
}

/// Items of a playlist list (a channel's Playlists tab), like [`video_items`].
pub fn playlist_items(
    json: &str,
    lang: Language,
) -> Result<(Vec<PlaylistItem>, Vec<String>), serde_json::Error> {
    let items: MapResult<Vec<response::YouTubeListItem>> = serde_json::from_str(json)?;
    let mut mapper = response::YouTubeListMapper::<PlaylistItem>::new(lang);
    mapper.map_response(items);
    Ok((mapper.items, mapper.warnings))
}

/// The subscriber and video counts read from a channel page header's
/// `contentMetadataViewModel` (its `metadataRows`), with the warnings left.
pub fn channel_header_counts(
    json: &str,
    lang: Language,
) -> Result<(Option<u64>, Option<u64>, Vec<String>), serde_json::Error> {
    let view: response::PhMetadataView2 = serde_json::from_str(json)?;
    let mut warnings = Vec::new();
    let (subscribers, videos) = channel::header_counts(&view.metadata_rows, lang, &mut warnings);
    Ok((subscribers, videos, warnings))
}
