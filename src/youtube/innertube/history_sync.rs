//! Opt-in playback reports, following yt-dlp's YouTube `_mark_watched`.
//! The player response and tracking URL remain only on the backend thread.

use rand::Rng;
use rustypipe::client::{ClientType, RustyPipe};
use serde_json::{Value, json};
use url::Url;

const CPN_ALPHABET: &[u8; 64] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";

fn cpn() -> String {
    let mut rng = rand::rng();
    (0..16)
        .map(|_| CPN_ALPHABET[rng.random_range(0..64)] as char)
        .collect()
}

fn tracking_url(player: &Value, cpn: &str) -> Option<Url> {
    let base = player
        .pointer("/playbackTracking/videostatsPlaybackUrl/baseUrl")?
        .as_str()?;
    if !RustyPipe::history_tracking_url_allowed(base)
        || cpn.len() != 16
        || !cpn.bytes().all(|byte| CPN_ALPHABET.contains(&byte))
    {
        return None;
    }
    let mut url = Url::parse(base).ok()?;
    // Replace rather than append: a player response may already carry them.
    let query: Vec<_> = url
        .query_pairs()
        .filter(|(key, _)| !matches!(key.as_ref(), "ver" | "cpn" | "el"))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    url.query_pairs_mut()
        .clear()
        .extend_pairs(query)
        .append_pair("ver", "2")
        .append_pair("cpn", cpn)
        .append_pair("el", "detailpage");
    Some(url)
}

/// Reports one playback using the same session that requested the player.
/// Errors intentionally carry neither dependency messages nor private data.
pub async fn mark_watched(rp: &RustyPipe, id: &str) -> Result<(), ()> {
    let raw = rp
        .query()
        .authenticated()
        .raw(ClientType::Desktop, "player", &json!({ "videoId": id }))
        .await
        .map_err(|_| ())?;
    let player: Value = serde_json::from_str(&raw).map_err(|_| ())?;
    let url = tracking_url(&player, &cpn()).ok_or(())?;
    rp.history_tracking_get(url.as_str()).await.map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(base: &str) -> Value {
        json!({ "playbackTracking": { "videostatsPlaybackUrl": { "baseUrl": base } } })
    }

    #[test]
    fn playback_url_is_extracted_and_query_values_are_replaced() {
        let url = tracking_url(
            &player("https://www.youtube.com/api/stats/playback?token=synthetic&ver=1&cpn=old&el=shorts"),
            "abcdefghijkl-_01",
        )
        .unwrap();
        let query: Vec<_> = url.query_pairs().collect();
        assert_eq!(query.len(), 4);
        for (key, value) in [
            ("token", "synthetic"),
            ("ver", "2"),
            ("cpn", "abcdefghijkl-_01"),
            ("el", "detailpage"),
        ] {
            assert!(query.iter().any(|(k, v)| k == key && v == value));
        }
        assert!(tracking_url(&json!({}), "abcdefghijkl-_01").is_none());
    }

    #[test]
    fn tracking_never_accepts_non_https_or_non_youtube_destinations() {
        for base in [
            "http://www.youtube.com/api/stats/playback",
            "https://youtube.com.evil.invalid/api/stats/playback",
            "https://notyoutube.com/api/stats/playback",
            "https://youtube.com@evil.invalid/api/stats/playback",
            "https://user:password@youtube.com/api/stats/playback",
            "https://youtube.com:444/api/stats/playback",
            "https://youtube.com/api/stats/playback#fragment",
            "file:///api/stats/playback",
            "not a url",
        ] {
            assert!(tracking_url(&player(base), "abcdefghijkl-_01").is_none());
        }
        for base in [
            "https://youtube.com/api/stats/playback",
            "https://www.youtube.com/api/stats/playback",
            "https://music.youtube.com/api/stats/playback",
        ] {
            assert!(tracking_url(&player(base), "abcdefghijkl-_01").is_some());
        }
    }

    #[test]
    fn playback_nonces_have_ytdlps_length_and_alphabet() {
        for _ in 0..64 {
            let value = cpn();
            assert_eq!(value.len(), 16);
            assert!(value.bytes().all(|byte| CPN_ALPHABET.contains(&byte)));
        }
        for invalid in ["short", "abcdefghijklmnopq", "abcdefghijklmno!"] {
            assert!(tracking_url(&player("https://youtube.com/"), invalid).is_none());
        }
    }
}
