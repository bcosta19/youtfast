//! Regression tests for YouTube response formats.
//!
//! Tests pin entries of `docs/youtube-api-format.md` (`YT-n`) and behaviour
//! preserved when migrating to the in-tree client. The JSON below is trimmed
//! from response formats, with synthetic ids and URLs.

use serde_json::Value;
use youtfast_youtube::innertube::{
    self, ChannelOrder, ChannelVideoTab, channel_header_counts, channel_order_token,
    channel_tab_params,
};

fn parse_videos(json: &str) -> Vec<youtfast_core::model::Video> {
    let val: Value = serde_json::from_str(json).unwrap();
    innertube::items::collect_videos_and_ctoken(&val).0
}

fn parse_playlists(json: &str) -> Vec<youtfast_core::model::PlaylistSummary> {
    let val: Value = serde_json::from_str(json).unwrap();
    innertube::items::collect_playlists_and_ctoken(&val).0
}

// ---------------------------------------------------------------------------
// YT-1: the sort token of a channel tab
// ---------------------------------------------------------------------------

/// Tokens of the sort chips YouTube sent on 2026-10-19: (channel, target id,
/// tab, order, token).
const SORT_CHIP_TOKENS: &[(&str, &str, ChannelVideoTab, ChannelOrder, &str)] = &[
    (
        "UC9-y-6csu5WGm29I7JiwpnA",
        "6b8fab71-0000-261f-96d9-14c14ef8d168",
        ChannelVideoTab::Videos,
        ChannelOrder::Latest,
        "4qmFsgJkEhhVQzkteS02Y3N1NVdHbTI5STdKaXdwbkEaSDhnWXVHaXg2S2tJb0NpUTJZamhtWVdJM01TMHdNREF3TFRJMk1XWXRPVFprT1MweE5HTXhOR1ZtT0dReE5qZ1lCQSUzRCUzRA%3D%3D",
    ),
    (
        "UC9-y-6csu5WGm29I7JiwpnA",
        "6b8fab71-0000-261f-96d9-14c14ef8d168",
        ChannelVideoTab::Videos,
        ChannelOrder::Popular,
        "4qmFsgJkEhhVQzkteS02Y3N1NVdHbTI5STdKaXdwbkEaSDhnWXVHaXg2S2tJb0NpUTJZamhtWVdJM01TMHdNREF3TFRJMk1XWXRPVFprT1MweE5HTXhOR1ZtT0dReE5qZ1lBZyUzRCUzRA%3D%3D",
    ),
    (
        "UC9-y-6csu5WGm29I7JiwpnA",
        "6b8fab71-0000-261f-96d9-14c14ef8d168",
        ChannelVideoTab::Videos,
        ChannelOrder::Oldest,
        "4qmFsgJkEhhVQzkteS02Y3N1NVdHbTI5STdKaXdwbkEaSDhnWXVHaXg2S2tJb0NpUTJZamhtWVdJM01TMHdNREF3TFRJMk1XWXRPVFprT1MweE5HTXhOR1ZtT0dReE5qZ1lCUSUzRCUzRA%3D%3D",
    ),
    (
        "UCX6OQ3DkcsbYNE6H8uQQuVA",
        "6b616d9b-0000-2748-9ca6-10d9a242eca6",
        ChannelVideoTab::Shorts,
        ChannelOrder::Popular,
        "4qmFsgJkEhhVQ1g2T1EzRGtjc2JZTkU2SDh1UVF1VkEaSDhnWXVHaXhTS2pvb0NpUTJZall4Tm1RNVlpMHdNREF3TFRJM05EZ3RPV05oTmkweE1HUTVZVEkwTW1WallUWVlBZyUzRCUzRA%3D%3D",
    ),
    (
        "UCSJ4gkVC6NrvII8umztf0Ow",
        "6bf3cdad-0000-26b3-90bc-582429af698c",
        ChannelVideoTab::Live,
        ChannelOrder::Popular,
        "4qmFsgJkEhhVQ1NKNGdrVkM2TnJ2SUk4dW16dGYwT3caSDhnWXVHaXh5S2tJb0NpUTJZbVl6WTJSaFpDMHdNREF3TFRJMllqTXRPVEJpWXkwMU9ESTBNamxoWmpZNU9HTVlEZyUzRCUzRA%3D%3D",
    ),
];

#[test]
fn yt1_sort_tokens_match_the_chips_youtube_sends() {
    for &(channel, target, tab, order, token) in SORT_CHIP_TOKENS {
        assert_eq!(
            channel_order_token(channel, tab, order, target),
            token,
            "{tab:?} {order:?}"
        );
    }
}

/// Decodes the base64 of a continuation token: standard or URL-safe
/// alphabet, `%3D` padding.
fn base64(text: &str) -> Vec<u8> {
    let text = text.replace("%3D", "=");
    let mut bits = 0u32;
    let mut count = 0;
    let mut out = Vec::new();
    for c in text.bytes().filter(|&c| c != b'=') {
        let value = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => panic!("not base64: {c}"),
        };
        bits = (bits << 6) | u32::from(value);
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
        }
    }
    out
}

/// A protobuf field: its number and either a varint or the bytes it holds.
#[derive(Debug)]
enum Field {
    Varint(u32, u64),
    Bytes(u32, Vec<u8>),
}

fn varint(bytes: &[u8], at: &mut usize) -> u64 {
    let mut value = 0u64;
    let mut shift = 0;
    loop {
        let byte = bytes[*at];
        *at += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte < 0x80 {
            return value;
        }
        shift += 7;
    }
}

fn fields(bytes: &[u8]) -> Vec<Field> {
    let mut at = 0;
    let mut out = Vec::new();
    while at < bytes.len() {
        let key = varint(bytes, &mut at);
        let number = (key >> 3) as u32;
        match key & 7 {
            0 => out.push(Field::Varint(number, varint(bytes, &mut at))),
            2 => {
                let len = varint(bytes, &mut at) as usize;
                out.push(Field::Bytes(number, bytes[at..at + len].to_vec()));
                at += len;
            }
            wire => panic!("unexpected wire type {wire}"),
        }
    }
    out
}

fn bytes_of(fields: &[Field], number: u32) -> Vec<u8> {
    fields
        .iter()
        .find_map(|field| match field {
            Field::Bytes(n, bytes) if *n == number => Some(bytes.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no field {number} in {fields:?}"))
}

/// The tab message of a sort token: 80226972 → 3 (base64) → 110 → 3 → tab.
fn sort_tab_message(token: &str, tab_field: u32) -> Vec<Field> {
    let outer = fields(&bytes_of(&fields(&base64(token)), 80_226_972));
    let inner = String::from_utf8(bytes_of(&outer, 3)).unwrap();
    let inner = fields(&base64(&inner));
    let section = fields(&bytes_of(&fields(&bytes_of(&inner, 110)), 3));
    fields(&bytes_of(&section, tab_field))
}

#[test]
fn yt1_sort_tokens_use_the_envelope_not_the_refused_fields() {
    // Refused since 2026-10: the target as field 2 and the order as fields
    // 3/4/5 of the tab message. Now: one envelope {1: target, 3: order}.
    let target = "6461d7c8-0000-2040-87aa-089e0827e420";
    for (tab, tab_field, envelope, orders) in [
        (ChannelVideoTab::Videos, 15, 8, [4, 2, 5]),
        (ChannelVideoTab::Shorts, 10, 7, [4, 2, 5]),
        (ChannelVideoTab::Live, 14, 8, [12, 14, 13]),
    ] {
        for (order, expected) in [
            ChannelOrder::Latest,
            ChannelOrder::Popular,
            ChannelOrder::Oldest,
        ]
        .into_iter()
        .zip(orders)
        {
            let token = channel_order_token("UCXuqSBlHAE6Xw-yeJA0Tunw", tab, order, target);
            let message = sort_tab_message(&token, tab_field);
            let numbers: Vec<u32> = message
                .iter()
                .map(|field| match field {
                    Field::Varint(n, _) | Field::Bytes(n, _) => *n,
                })
                .collect();
            assert_eq!(
                numbers,
                [envelope],
                "{tab:?} {order:?}: refused fields came back"
            );
            let sort = fields(&bytes_of(&message, envelope));
            assert!(
                matches!(&sort[..], [Field::Bytes(1, t), Field::Varint(3, o)] if t == target.as_bytes() && *o == expected),
                "{tab:?} {order:?}: {sort:?}"
            );
        }
    }
}

#[test]
fn yt1_each_order_of_each_tab_has_its_own_token() {
    let target = "00000000-0000-0000-0000-000000000000";
    for tab in [
        ChannelVideoTab::Videos,
        ChannelVideoTab::Shorts,
        ChannelVideoTab::Live,
    ] {
        let tokens: std::collections::HashSet<String> = [
            ChannelOrder::Latest,
            ChannelOrder::Popular,
            ChannelOrder::Oldest,
        ]
        .into_iter()
        .map(|order| channel_order_token("UCx", tab, order, target))
        .collect();
        assert_eq!(tokens.len(), 3, "{tab:?}");
    }
}

// ---------------------------------------------------------------------------
// YT-2: Shorts items without `thumbnail`
// ---------------------------------------------------------------------------

#[test]
fn yt2_a_short_with_only_a_thumbnail_view_model_is_kept() {
    // The Shorts tab since 2026-10: no `thumbnail` field at all.
    let json = r#"[
      { "richItemRenderer": { "content": { "shortsLockupViewModel": {
          "entityId": "shorts-shelf-item-aaaaaaaaaaa",
          "accessibilityText": "A short, 63 million views - play Short",
          "thumbnailViewModel": { "thumbnailViewModel": { "image": { "sources": [
              { "url": "https://i.ytimg.com/vi/aaaaaaaaaaa/sardefault.jpg", "width": 405, "height": 720 }
          ] } } },
          "overlayMetadata": {
              "primaryText": { "content": "A short" },
              "secondaryText": { "content": "63M views" }
          }
      } } } }
    ]"#;
    let items = parse_videos(json);
    assert_eq!(items.len(), 1, "the short was dropped");
    let short = &items[0];
    assert_eq!(short.id, "aaaaaaaaaaa");
    assert_eq!(short.title, "A short");
    assert_eq!(short.views, Some(63_000_000));
    assert!(short.short);
}

#[test]
fn yt2_a_short_with_the_older_thumbnail_is_still_kept() {
    let json = r#"[
      { "shortsLockupViewModel": {
          "entityId": "shorts-shelf-item-bbbbbbbbbbb",
          "thumbnail": { "sources": [
              { "url": "https://i.ytimg.com/vi/bbbbbbbbbbb/frame0.jpg", "width": 1080, "height": 1920 }
          ] },
          "overlayMetadata": { "primaryText": { "content": "Another short" } }
      } }
    ]"#;
    let items = parse_videos(json);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "bbbbbbbbbbb");
    assert_eq!(items[0].title, "Another short");
    assert!(items[0].short);
}

#[test]
fn account_feed_shorts_preserve_known_owners_for_channel_blocks() {
    use youtfast_core::model::Channel;
    use youtfast_storage::library::Library;
    use youtfast_youtube::innertube::items::collect_videos_and_ctoken;

    let page = serde_json::json!({
        "contents": [{"richShelfRenderer": {"contents": [
            {"shortsLockupViewModel": {
                "entityId": "shorts-shelf-item-aaaaaaaaaaa",
                "channelId": "UC_blocked",
                "thumbnail": {"sources": [{}]}
            }},
            {"shortsLockupViewModel": {
                "entityId": "shorts-shelf-item-bbbbbbbbbbb",
                "thumbnail": {"sources": [{}]}
            }},
            {"shortsLockupViewModel": {
                "entityId": "shorts-shelf-item-ccccccccccc",
                "thumbnail": {"sources": [{}]}
            }},
            {"shortsLockupViewModel": {
                "onTap": {"innertubeCommand": {"reelWatchEndpoint": {
                    "videoId": "bbbbbbbbbbb",
                    "channelId": "UC_blocked"
                }}},
                "thumbnail": {"sources": [{}]}
            }}
        ]}}]
    });
    let (videos, _) = collect_videos_and_ctoken(&page);
    assert_eq!(videos.len(), 3);
    let mut library = Library::default();
    assert!(library.block_channel(Channel {
        id: "UC_blocked".into(),
        name: "Blocked".into(),
        avatar: None,
    }));
    let visible_shorts: Vec<_> = videos
        .iter()
        .filter(|video| video.short && !library.is_hidden(video))
        .map(|video| video.id.as_str())
        .collect();
    assert_eq!(visible_shorts, ["ccccccccccc"]);
}

// ---------------------------------------------------------------------------
// YT-3: the Playlists tab's params
// ---------------------------------------------------------------------------

fn tab_params(name: &str) -> String {
    channel_tab_params()
        .into_iter()
        .find(|(tab, _)| *tab == name)
        .map(|(_, params)| params)
        .unwrap_or_else(|| panic!("no {name} tab"))
}

#[test]
fn yt3_the_playlists_tab_sends_the_params_youtube_uses() {
    // What youtube.com's own Playlists tab sends since 2026-10.
    assert_eq!(tab_params("playlists"), "EglwbGF5bGlzdHPyBgoKCEIGCgIQaCIA");
}

#[test]
fn yt3_the_playlists_tab_never_sends_the_params_that_return_home() {
    // YouTube answers this one with the channel's Home tab, without an error.
    assert_ne!(tab_params("playlists"), "EglwbGF5bGlzdHMgAQ%3D%3D");
}

#[test]
fn yt3_the_other_tabs_keep_the_params_youtube_uses() {
    // Unchanged on 2026-10-20; a change here needs the same check as YT-3.
    assert_eq!(tab_params("videos"), "EgZ2aWRlb3PyBgQKAjoA");
    assert_eq!(tab_params("live"), "EgdzdHJlYW1z8gYECgJ6AA%3D%3D");
    assert_eq!(tab_params("search"), "EgZzZWFyY2jyBgQKAloA");
}

// ---------------------------------------------------------------------------
// YT-4: podcasts in a channel's Playlists tab
// ---------------------------------------------------------------------------

/// A Playlists tab grid item, trimmed from a real response.
fn lockup(content_type: &str, id: &str, title: &str, badge: &str) -> String {
    format!(
        r#"{{ "lockupViewModel": {{
          "contentId": "{id}",
          "contentType": "{content_type}",
          "contentImage": {{ "collectionThumbnailViewModel": {{ "primaryThumbnail": {{ "thumbnailViewModel": {{
              "image": {{ "sources": [ {{ "url": "https://i.ytimg.com/vi/cccccccccc/hq720.jpg", "width": 720, "height": 404 }} ] }},
              "overlays": [ {{ "thumbnailOverlayBadgeViewModel": {{ "thumbnailBadges": [
                  {{ "thumbnailBadgeViewModel": {{ "text": "{badge}" }} }}
              ] }} }} ]
          }} }} }} }},
          "metadata": {{ "lockupMetadataViewModel": {{
              "title": {{ "content": "{title}" }},
              "metadata": {{ "contentMetadataViewModel": {{ "metadataRows": [
                  {{ "metadataParts": [ {{ "text": {{ "content": "View full playlist" }} }} ] }}
              ] }} }}
          }} }}
      }} }}"#
    )
}

#[test]
fn yt4_a_podcast_is_listed_as_a_playlist() {
    let json = format!(
        "[{}, {}]",
        lockup(
            "LOCKUP_CONTENT_TYPE_PLAYLIST",
            "PLaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "A playlist",
            "10 videos"
        ),
        lockup(
            "LOCKUP_CONTENT_TYPE_PODCAST",
            "PLbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "A podcast",
            "12 episodes"
        ),
    );
    let items = parse_playlists(&json);
    let names: Vec<&str> = items.iter().map(|item| item.name.as_str()).collect();
    assert_eq!(
        names,
        ["A playlist", "A podcast"],
        "the podcast was dropped"
    );
    assert_eq!(items[1].id, "PLbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    assert_eq!(items[1].videos, Some(12));
}

// ---------------------------------------------------------------------------
// YT-5: the counts of a channel's page header
// ---------------------------------------------------------------------------

#[test]
fn yt5_a_subscriber_part_with_an_accessibility_label_is_read() {
    // The header since 2026-10: only the subscriber part has two keys.
    let json: Value = serde_json::from_str(
        r#"{ "metadataRows": [
        { "metadataParts": [ { "text": { "content": "@handle" } } ] },
        { "metadataParts": [
            { "text": { "content": "572K subscribers" }, "accessibilityLabel": "572 thousand subscribers" },
            { "text": { "content": "151 videos", "styleRuns": [ { "startIndex": 0, "length": 10 } ] } }
        ] }
    ] }"#,
    )
    .unwrap();
    let (subscribers, videos) = channel_header_counts(&json);
    // The regression showed the video count as the subscribers (151).
    assert_eq!(subscribers, Some(572_000));
    assert_eq!(videos, Some(151));
}

#[test]
fn yt5_an_abbreviated_video_count_is_not_read_digit_by_digit() {
    let json: Value = serde_json::from_str(
        r#"{ "metadataRows": [
        { "metadataParts": [ { "text": { "content": "@handle" } } ] },
        { "metadataParts": [
            { "text": { "content": "161K subscribers" }, "accessibilityLabel": "161 thousand subscribers" },
            { "text": { "content": "2.2K videos" } }
        ] }
    ] }"#,
    )
    .unwrap();
    let (subscribers, videos) = channel_header_counts(&json);
    assert_eq!(subscribers, Some(161_000));
    // Read digit by digit, "2.2K" was 22.
    assert_eq!(videos, Some(2_200));
}

// ---------------------------------------------------------------------------
// YT-6: the channel's avatar on a video lockup
// ---------------------------------------------------------------------------

fn video_lockup(image: &str) -> String {
    format!(
        r#"[{{ "lockupViewModel": {{
          "contentId": "aaaaaaaaaaa",
          "contentType": "LOCKUP_CONTENT_TYPE_VIDEO",
          "contentImage": {{ "thumbnailViewModel": {{
              "image": {{ "sources": [ {{ "url": "https://i.ytimg.com/vi/aaaaaaaaaaa/hq720.jpg", "width": 720, "height": 404 }} ] }},
              "overlays": [ {{ "thumbnailOverlayBadgeViewModel": {{ "thumbnailBadges": [
                  {{ "thumbnailBadgeViewModel": {{ "text": "12:34" }} }}
              ] }} }} ]
          }} }},
          "metadata": {{ "lockupMetadataViewModel": {{
              "title": {{ "content": "A video" }},
              {image}
              "metadata": {{ "contentMetadataViewModel": {{ "metadataRows": [
                  {{ "metadataParts": [ {{ "text": {{ "content": "A channel",
                      "commandRuns": [ {{ "startIndex": 0, "length": 9, "onTap": {{ "innertubeCommand": {{
                          "commandMetadata": {{ "webCommandMetadata": {{ "webPageType": "WEB_PAGE_TYPE_CHANNEL" }} }},
                          "browseEndpoint": {{ "browseId": "UCaaaaaaaaaaaaaaaaaaaaaa" }} }} }} }} ] }} }} ] }},
                  {{ "metadataParts": [
                      {{ "text": {{ "content": "1.2K views" }} }},
                      {{ "text": {{ "content": "2 days ago" }} }}
                  ] }}
              ] }} }}
          }} }}
      }} }}]"#
    )
}

#[test]
fn yt6_a_video_lockup_keeps_its_channels_avatar() {
    // Home, subscriptions and related lists since 2026-10: the avatar sits
    // beside the title, not on the channel's row.
    let json = video_lockup(
        r#""image": { "decoratedAvatarViewModel": { "avatar": { "avatarViewModel": {
            "image": { "sources": [
                { "url": "https://yt3.ggpht.com/avatar=s68", "width": 68, "height": 68 }
            ] } } } } },"#,
    );
    let items = parse_videos(&json);
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].channel_id.as_deref(),
        Some("UCaaaaaaaaaaaaaaaaaaaaaa"),
        "the channel was dropped"
    );
    assert_eq!(
        items[0].channel_avatar.as_deref(),
        Some("https://yt3.ggpht.com/avatar=s68"),
        "the avatar was dropped"
    );
}

#[test]
fn yt6_a_video_lockup_with_another_image_is_still_read() {
    // An image of another shape leaves the avatar out, not the video.
    let json = video_lockup(r#""image": { "somethingElse": {} },"#);
    let items = parse_videos(&json);
    assert_eq!(items.len(), 1, "the video was dropped");
    assert_eq!(
        items[0].channel_id.as_deref(),
        Some("UCaaaaaaaaaaaaaaaaaaaaaa")
    );
    assert!(items[0].channel_avatar.is_none());
}

// ---------------------------------------------------------------------------
// YT-7: video lockups on a channel's own tabs
// ---------------------------------------------------------------------------

/// A video of a channel's Videos tab as YouTube sent it on 2026-11-10: one
/// metadata row, no channel row; the length in a bottom overlay, beside other
/// overlays.
const CHANNEL_TAB_LOCKUP: &str = r#"[{ "richItemRenderer": { "content": { "lockupViewModel": {
    "contentId": "bbbbbbbbbbb",
    "contentType": "LOCKUP_CONTENT_TYPE_VIDEO",
    "contentImage": { "thumbnailViewModel": {
        "image": { "sources": [ { "url": "https://i.ytimg.com/vi/bbbbbbbbbbb/hq720.jpg", "width": 720, "height": 404 } ] },
        "overlays": [
            { "thumbnailBottomOverlayViewModel": { "badges": [
                { "thumbnailBadgeViewModel": { "text": "19:25", "badgeStyle": "THUMBNAIL_OVERLAY_BADGE_STYLE_DEFAULT" } }
            ] } },
            { "animatedThumbnailOverlayViewModel": { "thumbnail": {} } }
        ]
    } },
    "metadata": { "lockupMetadataViewModel": {
        "title": { "content": "A video" },
        "metadata": { "contentMetadataViewModel": { "delimiter": " • ", "metadataRows": [
            { "metadataParts": [
                { "text": { "content": "105K views" } },
                { "accessibilityLabel": "4 hours ago", "text": { "content": "4 hours ago" } }
            ] }
        ] } }
    } }
} } } }]"#;

#[test]
fn yt7_a_lone_metadata_row_is_read_as_views_and_age() {
    // Read as the channel's row, it lost views and age, and nothing was left
    // for the second row the code expected.
    let items = parse_videos(CHANNEL_TAB_LOCKUP);
    assert_eq!(items.len(), 1);
    let video = &items[0];
    assert_eq!(video.views, Some(105_000));
    assert_eq!(video.published.as_deref(), Some("4 hours ago"));
    assert!(video.published_at.is_some());
}

#[test]
fn yt7_the_length_in_a_bottom_overlay_is_read() {
    let items = parse_videos(CHANNEL_TAB_LOCKUP);
    assert_eq!(items[0].duration, Some(f64::from(19 * 60 + 25)));
}

#[test]
fn yt7_a_lockup_with_a_channel_row_keeps_every_field() {
    // Home, search and related lists still send the channel's row first.
    let items = parse_videos(&video_lockup(""));
    let video = &items[0];
    assert_eq!(
        video.channel_id.as_deref(),
        Some("UCaaaaaaaaaaaaaaaaaaaaaa")
    );
    assert_eq!(video.views, Some(1_200));
    assert_eq!(video.published.as_deref(), Some("2 days ago"));
    assert_eq!(video.duration, Some(f64::from(12 * 60 + 34)));
}

// ---------------------------------------------------------------------------
// Members-only badges
// ---------------------------------------------------------------------------

/// A channel-tab video with YouTube's "Members only" badge and age sharing a
/// metadata row, as @SamayRainaOfficial's Videos tab sent on 2026-11-16: the
/// channel row, then `[age] + badges`, with no view count.
const MEMBERS_ONLY_LOCKUP: &str = r#"[{ "richItemRenderer": { "content": { "lockupViewModel": {
    "contentId": "ccccccccccc",
    "contentType": "LOCKUP_CONTENT_TYPE_VIDEO",
    "contentImage": { "thumbnailViewModel": {
        "image": { "sources": [ { "url": "https://i.ytimg.com/vi/ccccccccccc/hq720.jpg", "width": 720, "height": 404 } ] }
    } },
    "metadata": { "lockupMetadataViewModel": {
        "title": { "content": "A members-only video" },
        "metadata": { "contentMetadataViewModel": { "metadataRows": [
            { "metadataParts": [ { "text": { "content": "Samay Raina",
                "commandRuns": [ { "onTap": { "innertubeCommand":
                    { "browseEndpoint": { "browseId": "UCAov2BBv1ZJav0c_yHEciAw" } } } } ] } } ] },
            { "metadataParts": [ { "text": { "content": "2 months ago" } } ],
                "badges": [ { "badgeViewModel": {
                    "badgeText": "Members only",
                    "badgeStyle": "BADGE_MEMBERS_ONLY",
                    "iconName": "SPONSORSHIP_STAR"
                } } ] }
        ] } }
    } }
} } } }]"#;

#[test]
fn a_channel_lockup_keeps_the_members_only_badge() {
    let items = parse_videos(MEMBERS_ONLY_LOCKUP);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].members_only, Some(true));
    assert!(items[0].is_members_only());
}

// YT-10: the badge shares the age's metadata row and the card has no view
// count, so the age must not be read as views.
#[test]
fn yt10_a_members_only_channel_lockup_keeps_its_age() {
    let items = parse_videos(MEMBERS_ONLY_LOCKUP);
    let video = &items[0];
    assert_eq!(video.channel.as_deref(), Some("Samay Raina"));
    assert_eq!(
        video.channel_id.as_deref(),
        Some("UCAov2BBv1ZJav0c_yHEciAw")
    );
    assert_eq!(video.views, None);
    assert_eq!(video.published.as_deref(), Some("2 months ago"));
    assert!(video.published_at.is_some());
}

#[test]
fn yt10_a_scheduled_card_keeps_its_date_and_views() {
    let json = r#"[{ "richItemRenderer": { "content": { "lockupViewModel": {
        "contentId": "eeeeeeeeeee",
        "contentType": "LOCKUP_CONTENT_TYPE_VIDEO",
        "metadata": { "lockupMetadataViewModel": {
            "title": { "content": "A scheduled premiere" },
            "metadata": { "contentMetadataViewModel": { "metadataRows": [
                { "metadataParts": [
                    { "text": { "content": "105K views" } },
                    { "text": { "content": "Scheduled for Nov 23, 2026" } }
                ] }
            ] } }
        } }
    } } } }]"#;
    let items = parse_videos(json);
    let video = &items[0];
    assert_eq!(video.views, Some(105_000));
    assert_eq!(
        video.published.as_deref(),
        Some("Scheduled for Nov 23, 2026")
    );
}

#[test]
fn yt10_an_unlinked_channel_name_is_not_read_as_a_view_count() {
    let json = r#"[{ "richItemRenderer": { "content": { "lockupViewModel": {
        "contentId": "fffffffffff",
        "contentType": "LOCKUP_CONTENT_TYPE_VIDEO",
        "metadata": { "lockupMetadataViewModel": {
            "title": { "content": "A members-only video" },
            "metadata": { "contentMetadataViewModel": { "metadataRows": [
                { "metadataParts": [ { "text": { "content": "50 Cent" } } ] },
                { "metadataParts": [ { "text": { "content": "2 months ago" } } ],
                    "badges": [ { "badgeViewModel": {
                        "badgeStyle": "BADGE_MEMBERS_ONLY"
                    } } ] }
            ] } }
        } }
    } } } }]"#;
    let items = parse_videos(json);
    let video = &items[0];
    assert_eq!(video.channel.as_deref(), Some("50 Cent"));
    assert_eq!(video.channel_id, None);
    assert_eq!(video.views, None);
    assert_eq!(video.published.as_deref(), Some("2 months ago"));
    assert!(video.is_members_only());
}

#[test]
fn yt10_unlinked_channel_names_like_premier_league_and_tech_reviews_are_preserved() {
    for (channel_name, stats_parts) in [
        ("Premier League", r#"["105K views", "4 hours ago"]"#),
        ("Tech Reviews", r#"["105K views", "4 hours ago"]"#),
        ("Bird Watching", r#"["2 months ago"]"#),
        ("Will Smith", r#"["2 months ago"]"#),
        ("5 Minute Crafts", r#"["10K views", "1 day ago"]"#),
    ] {
        let parts: Vec<String> = serde_json::from_str(stats_parts).unwrap();
        let stats_json: Vec<serde_json::Value> = parts
            .iter()
            .map(|text| serde_json::json!({ "text": { "content": text } }))
            .collect();
        let json = serde_json::json!([{ "richItemRenderer": { "content": { "lockupViewModel": {
            "contentId": "ggggggggggg",
            "contentType": "LOCKUP_CONTENT_TYPE_VIDEO",
            "metadata": { "lockupMetadataViewModel": {
                "title": { "content": "A video" },
                "metadata": { "contentMetadataViewModel": { "metadataRows": [
                    { "metadataParts": [ { "text": { "content": channel_name } } ] },
                    { "metadataParts": stats_json }
                ] } }
            } }
        } } } }])
        .to_string();
        let items = parse_videos(&json);
        let video = &items[0];
        assert_eq!(
            video.channel.as_deref(),
            Some(channel_name),
            "lost unlinked channel name {channel_name}"
        );
        assert!(video.published.is_some());
    }
}

#[test]
fn yt10_oversized_relative_age_does_not_overflow() {
    assert_eq!(
        innertube::items::parse_relative_age("999999999999999999 years ago"),
        None
    );
    assert_eq!(innertube::items::parse_relative_age("-5 days ago"), None);
    assert_eq!(
        innertube::items::parse_relative_age("5 Minute Crafts"),
        None
    );
    assert!(innertube::items::parse_relative_age("Streamed 2 days ago").is_some());

    let json = r#"[{ "richItemRenderer": { "content": { "lockupViewModel": {
        "contentId": "hhhhhhhhhhh",
        "contentType": "LOCKUP_CONTENT_TYPE_VIDEO",
        "metadata": { "lockupMetadataViewModel": {
            "title": { "content": "An oversized age" },
            "metadata": { "contentMetadataViewModel": { "metadataRows": [
                { "metadataParts": [ { "text": { "content": "999999999999999999 years ago" } } ] }
            ] } }
        } }
    } } } }]"#;
    let items = parse_videos(json);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].published_at, None);
    assert_eq!(
        items[0].published.as_deref(),
        Some("999999999999999999 years ago")
    );
}

#[test]
fn a_stored_video_without_members_only_still_loads() {
    let video: youtfast_core::model::Video =
        serde_json::from_str(r#"{"id":"aaaaaaaaaaa","title":"Old card"}"#).unwrap();
    assert_eq!(video.members_only, None);
    assert!(!video.is_members_only());
}

#[test]
fn an_ordinary_channel_lockup_is_not_members_only() {
    let items = parse_videos(CHANNEL_TAB_LOCKUP);
    assert_eq!(items[0].members_only, Some(false));
    assert!(!items[0].is_members_only());
}

#[test]
fn an_older_video_renderer_keeps_the_members_only_badge() {
    let json = r#"[{ "videoRenderer": {
        "videoId": "ddddddddddd",
        "title": { "simpleText": "A members-only video" },
        "badges": [ { "metadataBadgeRenderer": {
            "style": "BADGE_STYLE_TYPE_MEMBERS_ONLY",
            "label": "Members only"
        } } ]
    } }]"#;
    let items = parse_videos(json);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].members_only, Some(true));
    assert!(items[0].is_members_only());
}

#[test]
fn a_watch_page_keeps_the_members_only_badge() {
    let page = serde_json::json!({
        "contents": { "twoColumnWatchNextResults": { "results": { "results": { "contents": [
            { "videoPrimaryInfoRenderer": {
                "title": { "runs": [ { "text": "A members-only video" } ] },
                "badges": [ { "metadataBadgeRenderer": {
                    "style": "BADGE_STYLE_TYPE_MEMBERS_ONLY",
                    "label": "Members only"
                } } ]
            } },
            { "videoSecondaryInfoRenderer": {
                "owner": { "videoOwnerRenderer": {
                    "title": { "runs": [ { "text": "A channel" } ] },
                    "navigationEndpoint": { "browseEndpoint": { "browseId": "UCaaaaaaaaaaaaaaaaaaaaaa" } }
                } },
                "attributedDescription": { "content": "" }
            } }
        ] } } } }
    });
    let details = innertube::watch::parse_details(&page, "ddddddddddd");
    assert_eq!(details.members_only, Some(true));
    assert_eq!(details.title, "A members-only video");
}

#[test]
fn a_watch_page_distinguishes_confirmed_public_from_missing_primary() {
    let public_page = serde_json::json!({
        "contents": { "twoColumnWatchNextResults": { "results": { "results": { "contents": [
            { "videoPrimaryInfoRenderer": {
                "title": { "runs": [ { "text": "Public video" } ] }
            } }
        ] } } } }
    });
    assert_eq!(
        innertube::watch::parse_details(&public_page, "ddddddddddd").members_only,
        Some(false)
    );
    let empty_page = serde_json::json!({});
    assert_eq!(
        innertube::watch::parse_details(&empty_page, "ddddddddddd").members_only,
        None
    );
}

#[test]
fn comment_button_continuations_keep_replies_and_next_page() {
    let page = serde_json::json!({
        "onResponseReceivedEndpoints": [{
            "appendContinuationItemsAction": { "continuationItems": [
                { "commentThreadRenderer": {
                    "comment": { "commentRenderer": {
                        "commentId": "comment-id",
                        "contentText": { "simpleText": "A comment" }
                    } },
                    "replies": { "commentRepliesRenderer": { "contents": [{
                        "continuationItemRenderer": { "button": { "buttonRenderer": {
                            "command": { "continuationCommand": { "token": "replies-next" } }
                        } } }
                    }] } }
                } },
                { "continuationItemRenderer": { "button": { "buttonRenderer": {
                    "command": { "continuationCommand": { "token": "comments-next" } }
                } } } }
            ] }
        }]
    });
    let (rows, next) = innertube::watch::parse_comments_page(&page, "aaaaaaaaaaa");
    assert_eq!(rows[0].id, "comment-id");
    assert_eq!(rows[0].replies_token.as_deref(), Some("replies-next"));
    assert_eq!(next.as_deref(), Some("comments-next"));
}

fn modern_comment_surface_page(content: &str) -> Value {
    serde_json::json!({
        "onResponseReceivedEndpoints": [{
            "reloadContinuationItemsCommand": { "continuationItems": [{
                "commentThreadRenderer": {
                    "commentViewModel": { "commentViewModel": {
                        "commentId": "comment-id",
                        "commentKey": "comment-entity",
                        "commentSurfaceKey": "surface-entity"
                    } }
                }
            }] }
        }],
        "frameworkUpdates": { "entityBatchUpdate": { "mutations": [
            { "entityKey": "unrelated-surface", "payload": {
                "commentSurfaceEntityPayload": {
                    "voiceReplyContainerViewModel": { "voiceReplyContainerViewModel": {
                        "transcriptText": { "content": "Another comment's transcript" }
                    } }
                }
            } },
            { "entityKey": "comment-entity", "payload": { "commentEntityPayload": {
                "properties": { "commentId": "comment-id", "content": { "content": content } }
            } } },
            { "entityKey": "surface-entity", "payload": {
                "commentSurfaceEntityPayload": {
                    "imageAttachmentViewModel": { "commentImageAttachmentViewModel": {
                        "image": { "sources": [
                            { "url": "https://example.com/small.webp", "width": 160 },
                            { "url": "https://example.com/attachment.webp", "width": 640 }
                        ] }
                    } },
                    "voiceReplyContainerViewModel": { "voiceReplyContainerViewModel": {
                        "transcriptText": { "content": "A voice transcript" }
                    } }
                }
            } }
        ] } }
    })
}

#[test]
fn modern_comment_surfaces_keep_images_and_voice_transcripts() {
    let page = modern_comment_surface_page("");
    let (rows, _) = innertube::watch::parse_comments_page(&page, "aaaaaaaaaaa");
    assert_eq!(
        rows[0].image.as_deref(),
        Some("https://example.com/attachment.webp")
    );
    assert_eq!(
        rows[0].text,
        vec![youtfast_core::model::Span::Text(
            "A voice transcript".into()
        )]
    );
}

#[test]
fn modern_comment_surface_transcripts_do_not_replace_written_content() {
    let page = modern_comment_surface_page("Written content");
    let (rows, _) = innertube::watch::parse_comments_page(&page, "aaaaaaaaaaa");
    assert_eq!(
        rows[0].text,
        vec![youtfast_core::model::Span::Text("Written content".into())]
    );
}

#[test]
fn comment_threads_keep_pinned_priority_in_both_models() {
    for modern in [false, true] {
        for pinned in [false, true] {
            let mut page = modern_comment_surface_page("A comment");
            let thread = &mut page["onResponseReceivedEndpoints"][0]["reloadContinuationItemsCommand"]
                ["continuationItems"][0]["commentThreadRenderer"];
            if !modern {
                thread.as_object_mut().unwrap().remove("commentViewModel");
                thread["comment"] = serde_json::json!({ "commentRenderer": {
                    "commentId": "comment-id",
                    "contentText": { "simpleText": "A comment" }
                } });
            }
            thread["renderingPriority"] = serde_json::json!(if pinned {
                "RENDERING_PRIORITY_PINNED_COMMENT"
            } else {
                "RENDERING_PRIORITY_UNKNOWN"
            });
            let (rows, _) = innertube::watch::parse_comments_page(&page, "aaaaaaaaaaa");
            assert_eq!(rows[0].pinned, pinned, "modern={modern}, pinned={pinned}");
        }
    }
}

#[test]
fn shorts_accessibility_counts_keep_spelled_out_multipliers() {
    for (count, expected) in [
        ("63 million", 63_000_000),
        ("1.5 thousand", 1_500),
        ("2.3 billion", 2_300_000_000),
        ("1,234", 1_234),
        ("63M", 63_000_000),
        ("0", 0),
    ] {
        let short = serde_json::json!({ "shortsLockupViewModel": {
            "entityId": "shorts-shelf-item-aaaaaaaaaaa",
            "thumbnail": { "sources": [{ "url": "https://example.com/short.webp" }] },
            "accessibilityText": format!("A short, {count} views - play Short")
        } });
        let videos = innertube::items::collect_videos_and_ctoken(&short).0;
        assert_eq!(videos[0].views, Some(expected), "count={count}");
    }
}

#[test]
fn yt11_a_bot_check_in_http_200_playability_reason_is_a_request_limit() {
    let value = serde_json::json!({
        "playabilityStatus": {
            "status": "LOGIN_REQUIRED",
            "reason": "Sign in to confirm you're not a bot"
        }
    });
    assert!(innertube::client::response_reports_limit(200, &value));
}

#[test]
fn yt11_a_bot_check_in_http_403_error_message_is_not_an_account_rejection() {
    let value = serde_json::json!({"error": {"message": "Sign in to confirm you're not a bot"}});
    assert!(innertube::client::response_reports_limit(403, &value));
    let value = serde_json::json!({"error": {"message": "Invalid credentials"}});
    assert!(!innertube::client::response_reports_limit(403, &value));
}

#[test]
fn yt11_a_sign_in_reason_without_bot_check_wording_is_not_a_request_limit() {
    let value = serde_json::json!({
        "playabilityStatus": {
            "status": "LOGIN_REQUIRED",
            "reason": "Sign in to confirm your age"
        }
    });
    assert!(!innertube::client::response_reports_limit(200, &value));
}

#[tokio::test]
#[ignore = "manual public network check against YouTube"]
async fn live_public_endpoints_round_trip() {
    use youtfast_core::model::{ChannelTab, PageItems};

    let rp = innertube::Client::default_client().unwrap();

    // 1. Search (Videos)
    let (items, more) =
        innertube::search_filtered(&rp, "rust programming", innertube::SearchOptions::default())
            .await
            .expect("live search failed");
    let PageItems::Videos(videos) = items else {
        panic!("expected videos");
    };
    assert!(!videos.is_empty(), "search returned no videos");
    assert!(more.is_some(), "search returned no continuation");

    // 2. Channel RSS
    let rss_videos = innertube::rss::fetch(&rp, "UCYO_jab_esuFRV4b17AJtAw")
        .await
        .expect("live RSS fetch failed");
    assert!(!rss_videos.is_empty(), "RSS returned no videos");

    // 3. Channel Videos tab + Popular sort
    let (header, tab_videos, tab_more) = innertube::channel_tab(
        &rp,
        "UCYO_jab_esuFRV4b17AJtAw",
        &ChannelTab::Videos,
        ChannelOrder::Popular,
    )
    .await
    .expect("live channel_tab failed");
    assert_eq!(header.id, "UCYO_jab_esuFRV4b17AJtAw");
    assert!(!header.name.is_empty());
    assert!(header.subscribers.is_some());
    assert!(!tab_videos.is_empty());
    assert!(tab_more.is_some());

    // 4. Channel Playlists tab
    let (_, playlists, _) = innertube::channel_playlists_page(&rp, "UCYO_jab_esuFRV4b17AJtAw")
        .await
        .expect("live channel_playlists_page failed");
    assert!(!playlists.is_empty(), "channel playlists returned empty");

    // 5. Channel About tab
    let (_, about) = innertube::channel_about(&rp, "UCYO_jab_esuFRV4b17AJtAw", false)
        .await
        .expect("live channel_about failed");
    assert!(!about.description.is_empty());

    // 6. Watch details + Comments
    let details = innertube::details(&rp, "aircAruvnKk")
        .await
        .expect("live details failed");
    assert!(!details.title.is_empty());
    assert_eq!(details.channel.id, "UCYO_jab_esuFRV4b17AJtAw");
    assert!(!details.related.is_empty());
    let ctoken = details
        .comments_token
        .as_deref()
        .expect("missing comments_token");
    let (comments, _) =
        innertube::comments(&rp, ctoken, details.visitor_data.as_deref(), "aircAruvnKk")
            .await
            .expect("live comments failed");
    assert!(!comments.is_empty(), "comments returned empty");
}
