//! Optional, signed-out metadata. Captions stay in memory; no credentials or
//! response bodies are logged. Network and parsing run on the backend.
use std::io::Read;

#[derive(Clone, Debug, PartialEq)]
pub struct Extras {
    pub preview: Option<Preview>,
    pub language: String,
    pub transcript: Result<Vec<Cue>, String>,
}

impl Extras {
    pub fn retry_needed(&self) -> bool {
        self.transcript.is_err()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cue {
    pub seconds: f64,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Preview {
    pub template: String,
    pub width: u32,
    pub height: u32,
    pub columns: u32,
    pub rows: u32,
    pub count: u32,
    pub interval_ms: u32,
}

impl Preview {
    pub fn frame(&self, seconds: f64) -> Option<(String, egui::Rect)> {
        let per_page = self.columns.checked_mul(self.rows)?;
        if per_page == 0 || self.interval_ms == 0 || self.count == 0 || !seconds.is_finite() {
            return None;
        }
        let frame =
            ((seconds.max(0.0) * 1000.0 / f64::from(self.interval_ms)) as u32).min(self.count - 1);
        let page = frame / per_page;
        let cell = frame % per_page;
        let x = (cell % self.columns) as f32 / self.columns as f32;
        let y = (cell / self.columns) as f32 / self.rows as f32;
        Some((
            self.template.replace("$M", &page.to_string()),
            egui::Rect::from_min_size(
                egui::pos2(x, y),
                egui::vec2(1.0 / self.columns as f32, 1.0 / self.rows as f32),
            ),
        ))
    }
}

pub async fn load(
    rp: &rustypipe::client::RustyPipe,
    id: &str,
    language: &str,
) -> Result<Extras, String> {
    let player = tokio::time::timeout(
        std::time::Duration::from_secs(25),
        rp.query().unauthenticated().player(id),
    )
    .await
    .map_err(|_| "The request timed out. Retry.".to_owned())?
    .map_err(|_| "Could not complete the YouTube request. Retry.".to_owned())?;
    let preview = player
        .preview_frames
        .iter()
        .filter(|p| {
            p.frame_width > 0
                && p.frame_height > 0
                && p.frame_width <= 640
                && p.frame_height <= 360
                && p.frames_per_page_x > 0
                && p.frames_per_page_y > 0
                && p.frame_width
                    .checked_mul(p.frames_per_page_x)
                    .is_some_and(|v| v <= 4096)
                && p.frame_height
                    .checked_mul(p.frames_per_page_y)
                    .is_some_and(|v| v <= 4096)
        })
        .min_by_key(|p| p.frame_width.abs_diff(160))
        .map(|p| Preview {
            template: p.url_template.clone(),
            width: p.frame_width,
            height: p.frame_height,
            columns: p.frames_per_page_x,
            rows: p.frames_per_page_y,
            count: p.total_count,
            interval_ms: p.duration_per_frame,
        });
    let subtitles = player.subtitles;
    let subtitle = subtitles.iter().min_by_key(|s| {
        (
            u8::from(
                s.lang.split('-').next() != Some(language.split('-').next().unwrap_or(language)),
            ),
            u8::from(s.auto_generated),
        )
    });
    let Some(subtitle) = subtitle else {
        return Ok(Extras {
            preview,
            language: String::new(),
            transcript: Ok(Vec::new()),
        });
    };
    let url = subtitle.url.clone();
    let language = subtitle.lang_name.clone();
    let transcript = tokio::task::spawn_blocking(move || download(&url))
        .await
        .unwrap_or_else(
            |_| Err("Não foi possível carregar a transcrição. Tente novamente.".into()),
        );
    Ok(Extras {
        preview,
        language,
        transcript,
    })
}

fn download(url: &str) -> Result<Vec<Cue>, String> {
    const MAX: u64 = 2 * 1024 * 1024;
    if !url.starts_with("https://www.youtube.com/api/timedtext?") {
        return Err("Fonte da transcrição indisponível.".into());
    }
    let url = url
        .split('&')
        .filter(|p| !p.starts_with("fmt="))
        .collect::<Vec<_>>()
        .join("&")
        + "&fmt=json3";
    let mut response = ehttp::Request::get(url)
        .with_timeout(Some(std::time::Duration::from_secs(20)))
        .fetch_raw_native(true)
        .map_err(|_| "Não foi possível carregar a transcrição. Tente novamente.")?;
    if !response.status().is_success() {
        return Err("A transcrição está indisponível. Tente novamente.".into());
    }
    let mut data = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX + 1)
        .read_to_end(&mut data)
        .map_err(|_| "Não foi possível ler a transcrição.")?;
    if data.len() as u64 > MAX {
        return Err("A transcrição excede o limite permitido.".into());
    }
    parse(&data)
}

fn parse(data: &[u8]) -> Result<Vec<Cue>, String> {
    let doc: serde_json::Value =
        serde_json::from_slice(data).map_err(|_| "A transcrição recebida é inválida.")?;
    let events = doc["events"]
        .as_array()
        .ok_or("A transcrição recebida é inválida.")?;
    let mut cues: Vec<Cue> = Vec::new();
    for event in events.iter().take(20_000) {
        let Some(ms) = event["tStartMs"]
            .as_f64()
            .filter(|v| v.is_finite() && *v >= 0.0)
        else {
            continue;
        };
        let Some(segments) = event["segs"].as_array() else {
            continue;
        };
        let text = segments
            .iter()
            .filter_map(|s| s["utf8"].as_str())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !text.is_empty() && cues.last().is_none_or(|c| c.text != text) {
            cues.push(Cue {
                seconds: ms / 1000.0,
                text,
            });
        }
    }
    cues.sort_by(|a, b| a.seconds.total_cmp(&b.seconds));
    Ok(cues)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "manual public network check; no account or storage"]
    async fn public_video_provides_optional_metadata_without_a_session() {
        let rp = rustypipe::client::RustyPipe::builder()
            .no_storage()
            .no_reporter()
            .no_botguard()
            .build()
            .unwrap();
        let extras = load(&rp, "aircAruvnKk", "en")
            .await
            .expect("Public metadata request failed");
        assert!(extras.preview.is_some(), "Public video had no storyboard");
        assert!(
            !extras
                .transcript
                .expect("Public timed captions request failed")
                .is_empty(),
            "Public video had no timed captions"
        );
    }
    #[test]
    fn captions_skip_window_events_and_preserve_timestamped_text() {
        let cues=parse(br#"{"events":[{"tStartMs":0,"wWinId":1},{"tStartMs":1250,"segs":[{"utf8":"hello "},{"utf8":"world\n"}]},{"tStartMs":2000,"segs":[{"utf8":"hello world"}]},{"tStartMs":-1,"segs":[{"utf8":"bad"}]}]}"#).unwrap();
        assert_eq!(
            cues,
            vec![Cue {
                seconds: 1.25,
                text: "hello world".into()
            }]
        );
        assert!(parse(b"<html>blocked</html>").is_err());
    }
    #[test]
    fn preview_crosses_sprite_pages_without_leaving_the_grid() {
        let p = Preview {
            template: "https://i.ytimg.com/sb/test/M$M.jpg".into(),
            width: 160,
            height: 90,
            columns: 5,
            rows: 5,
            count: 51,
            interval_ms: 1000,
        };
        let (url, uv) = p.frame(25.0).unwrap();
        assert!(url.ends_with("M1.jpg"));
        assert_eq!(uv.min, egui::Pos2::ZERO);
        let (_, uv) = p.frame(999.0).unwrap();
        assert!(uv.max.x <= 1.0 && uv.max.y <= 1.0);
        assert!(p.frame(f64::NAN).is_none());
    }
}
