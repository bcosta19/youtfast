//! Talking to YouTube through yt-dlp, the most closely maintained
//! extractor there is: search runs it as a child process, and playback
//! hands the watch link to mpv, whose ytdl hook runs it again for the
//! streams (its answer comes back here through [`ytdl_details`] and
//! [`available_resolutions`]). When YouTube changes something, updating
//! yt-dlp fixes YouTFast without a release. The account and the feeds go
//! through rustypipe instead (`src/innertube.rs`).

use std::cmp::Reverse;
use std::io::ErrorKind;

use serde::Deserialize;
use tokio::process::Command;

use crate::model::{Resolution, Video};

const SEARCH_RESULTS: usize = 30;

/// The yt-dlp to run: `YOUTFAST_YTDLP` when set, else the one on PATH.
fn ytdlp() -> String {
    std::env::var("YOUTFAST_YTDLP").unwrap_or_else(|_| "yt-dlp".to_owned())
}

pub async fn search(query: &str) -> Result<Vec<Video>, String> {
    let output = Command::new(ytdlp())
        .args([
            "--flat-playlist",
            "--dump-single-json",
            "--no-warnings",
            "--ignore-config",
            // Everything after this is a URL, so a query starting with a
            // dash is never read as an option.
            "--",
        ])
        .arg(format!("ytsearch{SEARCH_RESULTS}:{query}"))
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|error| match error.kind() {
            ErrorKind::NotFound => "yt-dlp is not installed or not on PATH".to_owned(),
            _ => format!("could not run yt-dlp: {error}"),
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr.lines().rev().find(|line| !line.trim().is_empty());
        return Err(reason.unwrap_or("yt-dlp failed").trim().to_owned());
    }
    parse_search(&output.stdout)
}

#[derive(Deserialize)]
struct Playlist {
    #[serde(default)]
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    id: Option<String>,
    title: Option<String>,
    channel: Option<String>,
    channel_id: Option<String>,
    uploader: Option<String>,
    duration: Option<f64>,
    view_count: Option<u64>,
    live_status: Option<String>,
    /// Unix seconds; only in a full (not flat) answer.
    timestamp: Option<i64>,
}

impl Entry {
    /// The video this entry describes, when it is one.
    fn into_video(self) -> Option<Video> {
        let id = self.id.filter(|id| is_video_id(id))?;
        Some(Video {
            id,
            title: self.title.unwrap_or_default(),
            channel: self.channel.or(self.uploader),
            channel_id: self.channel_id.filter(|id| id.starts_with("UC")),
            duration: self.duration,
            views: self.view_count,
            live: self.live_status.as_deref() == Some("is_live"),
            published_at: self.timestamp,
            published: None,
            short: false,
        })
    }
}

fn parse_search(json: &[u8]) -> Result<Vec<Video>, String> {
    let playlist: Playlist = serde_json::from_slice(json)
        .map_err(|error| format!("could not read yt-dlp's answer: {error}"))?;
    Ok(playlist
        .entries
        .into_iter()
        .filter_map(Entry::into_video)
        .collect())
}

/// The video yt-dlp's answer describes: what a video opened from a link
/// lacks until it loads (title, channel, length).
pub fn ytdl_details(json: &str) -> Option<Video> {
    serde_json::from_str::<Entry>(json).ok()?.into_video()
}

#[derive(Deserialize)]
struct VideoInfo {
    #[serde(default)]
    formats: Vec<Format>,
}

#[derive(Deserialize)]
struct Format {
    width: Option<u32>,
    height: Option<u32>,
    fps: Option<f64>,
    vcodec: Option<String>,
}

/// The resolutions a video offers, highest first, each with its best frame
/// rate, from yt-dlp's JSON for it. Audio-only formats and storyboards
/// (`vcodec` "none") are left out.
pub fn available_resolutions(json: &str) -> Vec<Resolution> {
    let info: VideoInfo = match serde_json::from_str(json) {
        Ok(info) => info,
        Err(error) => {
            log::warn!("could not read the video's formats: {error}");
            return Vec::new();
        }
    };
    let mut resolutions: Vec<Resolution> = Vec::new();
    for format in info.formats {
        if format.vcodec.as_deref().is_none_or(|codec| codec == "none") {
            continue;
        }
        let Some(height) = format.height else {
            continue;
        };
        let fps = format.fps.unwrap_or(0.0).round() as u32;
        let resolution = Resolution::from_size(format.width.unwrap_or(height), height, fps);
        match resolutions
            .iter_mut()
            .find(|known| known.lines == resolution.lines)
        {
            Some(known) => known.fps = known.fps.max(resolution.fps),
            None => resolutions.push(resolution),
        }
    }
    resolutions.sort_by_key(|resolution| Reverse(resolution.lines));
    resolutions
}

fn is_video_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// The video id in a YouTube link, so pasting one opens it instead of
/// searching for it. Bare ids are not accepted: an 11-letter word such as
/// "programming" would be one.
pub fn video_id_from_link(input: &str) -> Option<String> {
    let input = input.trim();
    let rest = input
        .strip_prefix("https://")
        .or_else(|| input.strip_prefix("http://"))
        .unwrap_or(input);
    let rest = rest
        .strip_prefix("www.")
        .or_else(|| rest.strip_prefix("m."))
        .unwrap_or(rest);
    let rest = rest.strip_prefix("music.").unwrap_or(rest);

    let candidate = if let Some(path) = rest.strip_prefix("youtu.be/") {
        path
    } else {
        let path = rest.strip_prefix("youtube.com/")?;
        if let Some(query) = path.strip_prefix("watch?") {
            query.split('&').find_map(|pair| pair.strip_prefix("v="))?
        } else {
            ["shorts/", "live/", "embed/", "v/"]
                .iter()
                .find_map(|prefix| path.strip_prefix(prefix))?
        }
    };
    let id: String = candidate
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    is_video_id(&id).then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_flat_search_results() {
        let json = br#"{
            "_type": "playlist",
            "id": "rust egui",
            "entries": [
                {"_type": "url", "ie_key": "Youtube", "id": "dQw4w9WgXcQ",
                 "title": "A video", "channel": "A channel", "duration": 212.0,
                 "view_count": 1500000000, "live_status": null},
                {"_type": "url", "ie_key": "Youtube", "id": "abcdefghijk",
                 "title": "Live now", "uploader": "Someone", "duration": null,
                 "view_count": null, "live_status": "is_live"},
                {"_type": "url", "id": "UCnot-a-video-id", "title": "A channel"}
            ]
        }"#;
        let videos = parse_search(json).unwrap();
        assert_eq!(videos.len(), 2);
        assert_eq!(videos[0].id, "dQw4w9WgXcQ");
        assert_eq!(videos[0].channel.as_deref(), Some("A channel"));
        assert_eq!(videos[0].duration, Some(212.0));
        assert_eq!(videos[0].views, Some(1_500_000_000));
        assert!(!videos[0].live);
        assert_eq!(videos[1].channel.as_deref(), Some("Someone"));
        assert!(videos[1].live);
    }

    #[test]
    fn empty_search() {
        assert!(
            parse_search(br#"{"_type": "playlist"}"#)
                .unwrap()
                .is_empty()
        );
        assert!(parse_search(b"not json").is_err());
    }

    #[test]
    fn resolutions_from_formats() {
        let json = r#"{"id": "dQw4w9WgXcQ", "formats": [
            {"format_id": "sb0", "vcodec": "none", "acodec": "none", "width": 160, "height": 90},
            {"format_id": "251", "vcodec": "none", "acodec": "opus"},
            {"format_id": "134", "vcodec": "avc1", "width": 640, "height": 360, "fps": 30},
            {"format_id": "247", "vcodec": "vp9", "width": 1280, "height": 720, "fps": 30},
            {"format_id": "302", "vcodec": "vp9", "width": 1280, "height": 720, "fps": 60},
            {"format_id": "303", "vcodec": "vp9", "width": 1920, "height": 1080, "fps": 59.94},
            {"format_id": "18", "vcodec": "avc1", "acodec": "mp4a", "width": 640, "height": 360}
        ]}"#;
        let labels: Vec<String> = available_resolutions(json)
            .into_iter()
            .map(Resolution::label)
            .collect();
        assert_eq!(labels, ["1080p60", "720p60", "360p"]);
    }

    #[test]
    fn details_of_a_linked_video() {
        let json = r#"{"id": "dQw4w9WgXcQ", "title": "Never Gonna Give You Up",
                       "channel": "Rick Astley", "channel_id": "UCuAXFkgsw1L7xaCfnd5JJOw",
                       "duration": 213, "view_count": 1700000000, "timestamp": 1256453463,
                       "live_status": "not_live", "formats": []}"#;
        let video = ytdl_details(json).unwrap();
        assert_eq!(video.title, "Never Gonna Give You Up");
        assert_eq!(
            video.channel_id.as_deref(),
            Some("UCuAXFkgsw1L7xaCfnd5JJOw")
        );
        assert_eq!(video.duration, Some(213.0));
        assert_eq!(video.published_at, Some(1_256_453_463));
        assert_eq!(ytdl_details("{}"), None);
    }

    #[test]
    fn vertical_videos_by_their_smaller_side() {
        let json = r#"{"formats": [
            {"vcodec": "vp9", "width": 1080, "height": 1920, "fps": 30},
            {"vcodec": "vp9", "width": 720, "height": 1280, "fps": 30}
        ]}"#;
        let lines: Vec<u32> = available_resolutions(json)
            .iter()
            .map(|r| r.lines)
            .collect();
        assert_eq!(lines, [1080, 720]);
        assert!(available_resolutions("not json").is_empty());
    }

    #[test]
    fn video_links() {
        let id = Some("dQw4w9WgXcQ".to_owned());
        for link in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtube.com/watch?feature=share&v=dQw4w9WgXcQ&t=42",
            "http://m.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://music.youtube.com/watch?v=dQw4w9WgXcQ&list=RD",
            "youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ?si=abc",
            "https://www.youtube.com/shorts/dQw4w9WgXcQ",
            "https://www.youtube.com/live/dQw4w9WgXcQ?feature=share",
            "https://www.youtube.com/embed/dQw4w9WgXcQ",
            "  https://youtu.be/dQw4w9WgXcQ  ",
        ] {
            assert_eq!(video_id_from_link(link), id, "{link}");
        }
    }

    #[test]
    fn searches_are_not_links() {
        for text in [
            "programming",
            "dQw4w9WgXcQ",
            "rust egui tutorial",
            "https://example.com/watch?v=dQw4w9WgXcQ",
            "https://www.youtube.com/@channel",
            "https://youtu.be/short",
        ] {
            assert_eq!(video_id_from_link(text), None, "{text}");
        }
    }
}
