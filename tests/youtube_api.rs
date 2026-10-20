//! Regression tests for YouTube response formats.
//!
//! Each test pins one entry of `docs/youtube-api-format.md` (`YT-n`): the
//! format YouTube sends now is read or built, and the format it stopped
//! accepting or sending is not what the code goes back to. They run the
//! vendored rustypipe's own parsing and token code through its
//! `client::regression` hooks; the JSON below is trimmed from real responses,
//! with synthetic ids and URLs.

use rustypipe::client::regression;
use rustypipe::param::{ChannelOrder, ChannelVideoTab, Language};

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
            regression::channel_order_token(channel, tab, order, target),
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
            let token =
                regression::channel_order_token("UCXuqSBlHAE6Xw-yeJA0Tunw", tab, order, target);
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
        .map(|order| regression::channel_order_token("UCx", tab, order, target))
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
    let (items, warnings) = regression::video_items(json, Language::En).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(items.len(), 1, "the short was dropped");
    let short = &items[0];
    assert_eq!(short.id, "aaaaaaaaaaa");
    assert_eq!(short.name, "A short");
    assert_eq!(short.view_count, Some(63_000_000));
    assert_eq!(short.thumbnail.len(), 1);
    assert_eq!(short.thumbnail[0].height, 720);
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
    let (items, warnings) = regression::video_items(json, Language::En).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].thumbnail[0].width, 1080);
}

// ---------------------------------------------------------------------------
// YT-3: the Playlists tab's params
// ---------------------------------------------------------------------------

fn tab_params(name: &str) -> String {
    regression::channel_tab_params()
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
    let (items, _) = regression::playlist_items(&json, Language::En).unwrap();
    let names: Vec<&str> = items.iter().map(|item| item.name.as_str()).collect();
    assert_eq!(
        names,
        ["A playlist", "A podcast"],
        "the podcast was dropped"
    );
    assert_eq!(items[1].id, "PLbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    assert_eq!(items[1].video_count, Some(12));
}

// ---------------------------------------------------------------------------
// YT-5: the counts of a channel's page header
// ---------------------------------------------------------------------------

#[test]
fn yt5_a_subscriber_part_with_an_accessibility_label_is_read() {
    // The header since 2026-10: only the subscriber part has two keys.
    let json = r#"{ "metadataRows": [
        { "metadataParts": [ { "text": { "content": "@handle" } } ] },
        { "metadataParts": [
            { "text": { "content": "572K subscribers" }, "accessibilityLabel": "572 thousand subscribers" },
            { "text": { "content": "151 videos", "styleRuns": [ { "startIndex": 0, "length": 10 } ] } }
        ] }
    ] }"#;
    let (subscribers, videos, warnings) =
        regression::channel_header_counts(json, Language::En).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    // The regression showed the video count as the subscribers (151).
    assert_eq!(subscribers, Some(572_000));
    assert_eq!(videos, Some(151));
}

#[test]
fn yt5_an_abbreviated_video_count_is_not_read_digit_by_digit() {
    let json = r#"{ "metadataRows": [
        { "metadataParts": [ { "text": { "content": "@handle" } } ] },
        { "metadataParts": [
            { "text": { "content": "161K subscribers" }, "accessibilityLabel": "161 thousand subscribers" },
            { "text": { "content": "2.2K videos" } }
        ] }
    ] }"#;
    let (subscribers, videos, _) = regression::channel_header_counts(json, Language::En).unwrap();
    assert_eq!(subscribers, Some(161_000));
    // Read digit by digit, "2.2K" was 22.
    assert_eq!(videos, Some(2_200));
}
