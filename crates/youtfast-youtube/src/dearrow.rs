//! DeArrow (sponsor.ajay.app / dearrow-thumb.ajay.app): community titles
//! and thumbnails for YouTube videos.
//!
//! Branding lookups send only the first four hex digits of the video ID's
//! SHA-256, not the full ID. The bucket response is filtered locally and kept
//! only in memory. Community thumbnails, when separately enabled, fetch from
//! `dearrow-thumb.ajay.app` with the video ID and are likewise never written
//! to disk.

use sha2::{Digest, Sha256};

/// Cap the decoded bucket before parsing; the service can return many video IDs.
const MAX_BRANDING_BYTES: u64 = 1024 * 1024;

/// Default base URL for the DeArrow branding API.
pub const DEFAULT_API_BASE: &str = "https://sponsor.ajay.app";

/// Base URL for the DeArrow thumbnail server (memory-only; never cached on disk).
pub const THUMBNAIL_BASE: &str = "https://dearrow-thumb.ajay.app";

/// Selected community thumbnail metadata from a DeArrow branding response.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Thumbnail {
    /// A community-voted frame at `timestamp` seconds.
    Timestamp(f64),
}

/// Community branding picked for a single video ID.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Branding {
    pub title: Option<String>,
    pub thumbnail: Option<Thumbnail>,
}

impl Branding {
    /// Builds the DeArrow thumbnail URL when a community thumbnail exists.
    pub fn thumbnail_url(&self, video_id: &str) -> Option<String> {
        self.thumbnail.and_then(|Thumbnail::Timestamp(seconds)| {
            (seconds.is_finite() && seconds >= 0.0).then(|| thumbnail_url(video_id, seconds))
        })
    }
}

/// The 4-hex-character SHA-256 prefix sent to DeArrow's branding endpoint.
pub fn hash_prefix(video_id: &str) -> String {
    Sha256::digest(video_id.as_bytes())
        .iter()
        .take(2)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn thumbnail_url(video_id: &str, timestamp: f64) -> String {
    let mut url = url::Url::parse(&format!("{THUMBNAIL_BASE}/api/v1/getThumbnail"))
        .expect("THUMBNAIL_BASE is a valid URL");
    url.query_pairs_mut()
        .append_pair("videoID", video_id)
        .append_pair("time", &timestamp.to_string());
    url.into()
}

static AGENT: std::sync::LazyLock<ureq::Agent> = std::sync::LazyLock::new(|| {
    ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(10)))
        .http_status_as_error(false)
        .max_redirects(0)
        .build()
        .into()
});

/// Queries DeArrow's default branding endpoint for `video_id` using only its
/// 4-hex-character SHA-256 prefix.
pub fn branding(video_id: &str) -> Result<Branding, String> {
    fetch_from(DEFAULT_API_BASE, video_id)
}

/// Queries `{base}/api/branding/{4-hex-prefix}` and filters the returned
/// bucket locally for `video_id`. Returns `Ok(Branding::default())` on 404 or
/// when `video_id` has no matching entries in the bucket.
pub fn fetch_from(base: &str, video_id: &str) -> Result<Branding, String> {
    let url = format!(
        "{}/api/branding/{}",
        base.trim_end_matches('/'),
        hash_prefix(video_id),
    );
    let mut response = AGENT
        .get(url)
        .call()
        .map_err(|_| "DeArrow could not be reached".to_owned())?;
    match response.status().as_u16() {
        200 => {
            let body = response
                .body_mut()
                .with_config()
                .limit(MAX_BRANDING_BYTES)
                .read_to_vec()
                .map_err(|_| {
                    "DeArrow branding response exceeds limits or could not be read".to_owned()
                })?;
            let bucket = serde_json::from_slice(&body)
                .map_err(|_| "DeArrow returned an invalid branding bucket".to_owned())?;
            Ok(select(&bucket, video_id))
        }
        404 => Ok(Branding::default()),
        status => Err(format!("DeArrow answered {status}")),
    }
}

fn select(bucket: &serde_json::Map<String, serde_json::Value>, video_id: &str) -> Branding {
    let Some(entry) = bucket.get(video_id) else {
        return Branding::default();
    };
    let title = entry["titles"]
        .as_array()
        .and_then(|titles| titles.first())
        .and_then(select_title);
    let thumbnail = entry["thumbnails"]
        .as_array()
        .and_then(|thumbs| thumbs.first())
        .and_then(select_thumbnail);
    Branding { title, thumbnail }
}

fn entry_accepted(item: &serde_json::Value) -> bool {
    if item["original"].as_bool() != Some(false) {
        return false;
    }
    let votes = item["votes"].as_i64().unwrap_or(0);
    let locked = item["locked"].as_bool().unwrap_or(false);
    votes >= 0 || locked
}

fn select_title(item: &serde_json::Value) -> Option<String> {
    if !entry_accepted(item) {
        return None;
    }
    let raw = item["title"].as_str()?;
    let cleaned = clean_title(raw);
    (!cleaned.is_empty()).then_some(cleaned)
}

fn select_thumbnail(item: &serde_json::Value) -> Option<Thumbnail> {
    if !entry_accepted(item) {
        return None;
    }
    item["timestamp"]
        .as_f64()
        .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
        .map(Thumbnail::Timestamp)
}

/// Strips DeArrow's `>` word-case escape marker and trims outer whitespace.
fn clean_title(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    let mut at_word_start = true;
    while let Some(ch) = chars.next() {
        if ch == '>'
            && at_word_start
            && chars
                .peek()
                .is_some_and(|next| !next.is_whitespace() && *next != '>')
        {
            at_word_start = false;
            continue;
        }
        at_word_start = ch.is_whitespace();
        out.push(ch);
    }
    out.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str, video_id: &str) -> Branding {
        select(&serde_json::from_str(json).unwrap(), video_id)
    }

    fn server(status: u16, body: &str) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};
        let body = body.to_owned();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let mut buf = [0_u8; 4096];
            let read = stream.read(&mut buf).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..read]);
            let path = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("")
                .to_owned();
            let response = format!(
                "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            path
        });
        (base, handle)
    }

    #[test]
    fn hash_prefix_uses_four_hex_digits_of_sha256() {
        assert_eq!(hash_prefix("dQw4w9WgXcQ"), "5f6b");
        assert_eq!(hash_prefix("dQw4w9WgXcQ").len(), 4);
    }

    #[test]
    fn parse_filters_bucket_and_uses_only_first_ranked_entries() {
        let json = r#"{
            "otherVideo": {
                "titles": [{"title": "Wrong Video Title", "original": false, "votes": 100}],
                "thumbnails": [{"timestamp": 9.0, "original": false, "votes": 50}]
            },
            "targetVideo": {
                "titles": [
                    {"title": "  >How >iOS 19 >Works (5 > 3)  ", "original": false, "votes": 4},
                    {"title": "Runner Up", "original": false, "votes": 100}
                ],
                "thumbnails": [
                    {"timestamp": 42.5, "original": false, "votes": 2},
                    {"timestamp": 1.0, "original": true, "votes": 100}
                ]
            }
        }"#;
        let branding = parse(json, "targetVideo");
        assert_eq!(branding.title.as_deref(), Some("How iOS 19 Works (5 > 3)"));
        assert_eq!(branding.thumbnail, Some(Thumbnail::Timestamp(42.5)));
        assert_eq!(
            branding.thumbnail_url("targetVideo").as_deref(),
            Some(
                "https://dearrow-thumb.ajay.app/api/v1/getThumbnail?videoID=targetVideo&time=42.5"
            )
        );

        let missing = parse(json, "missingVideo");
        assert_eq!(missing, Branding::default());
        assert!(missing.thumbnail_url("missingVideo").is_none());
    }

    #[test]
    fn original_or_invalid_winner_keeps_youtube_branding() {
        for winner in [
            r#"{"original": true, "locked": true, "votes": 0, "title": "Original", "timestamp": 5}"#,
            r#"{"original": false, "votes": -1, "title": "Downvoted", "timestamp": 5}"#,
            r#"{"original": false, "votes": 5, "title": "   ", "timestamp": -1}"#,
            r#"{"original": false, "votes": 5}"#,
        ] {
            let json = format!(
                r#"{{"v":{{"titles":[{winner},{{"original":false,"title":"Runner up"}}],"thumbnails":[{winner},{{"original":false,"timestamp":8}}]}}}}"#
            );
            let branding = parse(&json, "v");
            assert_eq!(branding, Branding::default(), "{winner}");
            assert!(branding.thumbnail_url("v").is_none());
        }
    }

    #[test]
    fn fetch_from_queries_hash_prefix_and_handles_404() {
        let body = r#"{"dQw4w9WgXcQ":{"titles":[{"title":">Clear Title","original":false,"votes":1}],"thumbnails":[{"original":false,"votes":0}]}}"#;
        let (base, handle) = server(200, body);
        let got = fetch_from(&base, "dQw4w9WgXcQ").unwrap();
        assert_eq!(handle.join().unwrap(), "/api/branding/5f6b");
        assert_eq!(got.title.as_deref(), Some("Clear Title"));
        assert!(got.thumbnail_url("dQw4w9WgXcQ").is_none());

        let (base_404, handle_404) = server(404, "");
        let got_404 = fetch_from(&base_404, "dQw4w9WgXcQ").unwrap();
        assert_eq!(handle_404.join().unwrap(), "/api/branding/5f6b");
        assert_eq!(got_404, Branding::default());
    }

    #[test]
    fn oversized_branding_bucket_is_rejected_before_parsing() {
        let body = format!("{{}}{}", " ".repeat(MAX_BRANDING_BYTES as usize));
        let (base, worker) = server(200, &body);
        assert!(fetch_from(&base, "video").is_err());
        worker.join().unwrap();
    }

    #[test]
    fn invalid_branding_response_is_not_a_miss() {
        let (base, worker) = server(200, "[]");
        assert!(fetch_from(&base, "video").is_err());
        worker.join().unwrap();
    }
}
