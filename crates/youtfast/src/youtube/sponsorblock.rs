//! SponsorBlock (sponsor.ajay.app): the parts of a video its users marked
//! as sponsored, self-promotion, intro and so on.
//!
//! The request names only the first four hex digits of the id's SHA-256,
//! so the service learns a bucket of thousands of videos, not which one
//! plays; the answer is filtered here. Off unless the settings turn it on.

use sha2::{Digest, Sha256};

use crate::model::Segment;

/// What the request asks for: every category YouTFast names, segments to
/// skip only (not the "full video is a sponsor" labels).
const CATEGORIES: &str =
    r#"["sponsor","selfpromo","interaction","intro","outro","preview","music_offtopic","filler"]"#;

/// The segments of `id`, in time order. None
/// known is an empty list, not an error.
pub fn segments(id: &str) -> Result<Vec<Segment>, String> {
    let url = format!(
        "https://sponsor.ajay.app/api/skipSegments/{}?categories={}&actionTypes={}",
        prefix(id),
        encode(CATEGORIES),
        encode(r#"["skip"]"#),
    );
    let response = ehttp::fetch_blocking(&ehttp::Request::get(url))
        .map_err(|_| "SponsorBlock could not be reached".to_owned())?;
    match response.status {
        200 => Ok(parse(response.text().unwrap_or_default(), id)),
        // No video in the bucket has segments.
        404 => Ok(Vec::new()),
        status => Err(format!("SponsorBlock answered {status}")),
    }
}

/// The hash prefix the request names.
fn prefix(id: &str) -> String {
    Sha256::digest(id.as_bytes())
        .iter()
        .take(2)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// `[`, `"` and `,` for a query string.
fn encode(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            '[' => "%5B".to_owned(),
            ']' => "%5D".to_owned(),
            '"' => "%22".to_owned(),
            ',' => "%2C".to_owned(),
            c => c.to_string(),
        })
        .collect()
}

/// The bucket's answer, down to `id`'s segments in time order.
fn parse(json: &str, id: &str) -> Vec<Segment> {
    let Ok(serde_json::Value::Array(videos)) = serde_json::from_str(json) else {
        return Vec::new();
    };
    let mut segments: Vec<Segment> = videos
        .iter()
        .filter(|video| video["videoID"] == id)
        .flat_map(|video| video["segments"].as_array().cloned().unwrap_or_default())
        .filter_map(|segment| {
            let times = segment["segment"].as_array()?;
            let start = times.first()?.as_f64()?;
            let end = times.get(1)?.as_f64()?;
            let category = segment["category"].as_str()?.to_owned();
            (end > start).then_some(Segment {
                category,
                start,
                end,
            })
        })
        .collect();
    segments.sort_by(|a, b| a.start.total_cmp(&b.start));
    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_names_only_a_hash_prefix() {
        // SHA-256 of "dQw4w9WgXcQ" starts 5f6b.
        assert_eq!(prefix("dQw4w9WgXcQ"), "5f6b");
        assert_eq!(encode(r#"["skip"]"#), "%5B%22skip%22%5D");
    }

    #[test]
    fn only_the_videos_segments_are_kept_in_time_order() {
        let json = r#"[
            {"videoID": "other", "segments": [
                {"category": "sponsor", "segment": [1.0, 2.0]}
            ]},
            {"videoID": "abc", "segments": [
                {"category": "outro", "segment": [90.0, 100.0]},
                {"category": "sponsor", "segment": [10.5, 30.0]},
                {"category": "intro", "segment": [5.0, 5.0]}
            ]}
        ]"#;
        let segments = parse(json, "abc");
        assert_eq!(
            segments,
            vec![
                Segment {
                    category: "sponsor".to_owned(),
                    start: 10.5,
                    end: 30.0,
                },
                Segment {
                    category: "outro".to_owned(),
                    start: 90.0,
                    end: 100.0,
                },
            ]
        );
        assert!(parse("oops", "abc").is_empty());
    }
}
