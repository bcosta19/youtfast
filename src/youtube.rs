//! Talking to YouTube through yt-dlp, the most closely maintained
//! extractor there is: playback hands the watch link to mpv, whose ytdl
//! hook runs yt-dlp for the streams (its answer comes back here through
//! [`ytdl_loaded`]). When YouTube changes something, updating yt-dlp
//! fixes YouTFast without a release. Search and everything else go
//! through rustypipe instead (`src/innertube.rs`).

use std::cmp::Reverse;

use serde::Deserialize;

use crate::model::{Resolution, Video};

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
    /// The offered streams; only in a full (not flat) answer.
    #[serde(default)]
    formats: Vec<Format>,
}

impl Entry {
    /// The video this entry describes, when it is one.
    fn into_video(self) -> Option<Video> {
        let id = self.id.filter(|id| is_video_id(id))?;
        Some(Video {
            id,
            title: self.title.unwrap_or_default(),
            channel: self.channel.or(self.uploader),
            channel_avatar: None,
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

/// Everything the watch page takes from yt-dlp's answer, in one parse of
/// it: the video's details (what a video opened from a link lacks until it
/// loads) and the resolutions it offers. The JSON can be megabytes; reading
/// it twice on the interface's thread would hitch a frame.
pub fn ytdl_loaded(json: &str) -> (Option<Video>, Vec<Resolution>) {
    let mut entry: Entry = match serde_json::from_str(json) {
        Ok(entry) => entry,
        Err(error) => {
            log::warn!("could not read yt-dlp's answer: {error}");
            return (None, Vec::new());
        }
    };
    let resolutions = resolutions_from(std::mem::take(&mut entry.formats));
    (entry.into_video(), resolutions)
}

#[derive(Deserialize)]
struct Format {
    width: Option<u32>,
    height: Option<u32>,
    fps: Option<f64>,
    vcodec: Option<String>,
}

/// The resolutions a video offers, highest first, each with its best frame
/// rate. Audio-only formats and storyboards (`vcodec` "none") are left out.
fn resolutions_from(formats: Vec<Format>) -> Vec<Resolution> {
    let mut resolutions: Vec<Resolution> = Vec::new();
    for format in formats {
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
        let labels: Vec<String> = ytdl_loaded(json)
            .1
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
        let video = ytdl_loaded(json).0.unwrap();
        assert_eq!(video.title, "Never Gonna Give You Up");
        assert_eq!(
            video.channel_id.as_deref(),
            Some("UCuAXFkgsw1L7xaCfnd5JJOw")
        );
        assert_eq!(video.duration, Some(213.0));
        assert_eq!(video.published_at, Some(1_256_453_463));
        assert_eq!(ytdl_loaded("{}").0, None);
    }

    #[test]
    fn vertical_videos_by_their_smaller_side() {
        let json = r#"{"formats": [
            {"vcodec": "vp9", "width": 1080, "height": 1920, "fps": 30},
            {"vcodec": "vp9", "width": 720, "height": 1280, "fps": 30}
        ]}"#;
        let lines: Vec<u32> = ytdl_loaded(json).1.iter().map(|r| r.lines).collect();
        assert_eq!(lines, [1080, 720]);
        assert!(ytdl_loaded("not json").1.is_empty());
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
