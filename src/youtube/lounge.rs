//! YouTube on TV's Lounge transport. This is independent of RustyPipe's
//! account session: pairing identifiers and tokens live only in this worker.
//! Protocol references: ytcast/youtube/remote.go and casttube/YouTubeSession.py.
//! No browser data, cookies, response bodies or transport errors are exported.

use std::time::Duration;

use serde_json::Value;

use crate::desktop::cast::{Control, Error};

const BASE: &str = "https://www.youtube.com/api/lounge";
const LIMIT: usize = 256 * 1024;

#[derive(Default)]
struct Status {
    video: Option<String>,
    state: Option<i64>,
    seconds: Option<f64>,
    volume: Option<f64>,
    ad: bool,
}

impl Status {
    fn sponsor_target(&self, video: &str, segments: &[crate::model::Segment]) -> Option<f64> {
        if self.video.as_deref() != Some(video) || self.state != Some(1) || self.ad {
            return None;
        }
        crate::desktop::cast::sponsor_target(self.seconds?, segments)
    }
}

pub struct Session {
    client: reqwest::Client,
    token: String,
    id: String,
    sid: String,
    gsid: String,
    rid: u64,
    offset: u64,
    aid: u64,
    status: Status,
}

impl Session {
    pub async fn connect(screen: &str) -> Result<Self, Error> {
        let client = super::innertube::http()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(12))
            .build()
            .map_err(|_| Error::Unavailable)?;
        let data = fetch(
            client
                .post(format!("{BASE}/pairing/get_lounge_token_batch"))
                .form(&[("screen_ids", screen)]),
        )
        .await?;
        let value: Value = serde_json::from_slice(&data).map_err(|_| Error::InvalidResponse)?;
        let token = value["screens"][0]["loungeToken"]
            .as_str()
            .filter(|token| !token.is_empty() && token.len() < 8192)
            .ok_or(Error::ConnectionExpired)?
            .to_owned();
        let mut session = Self {
            client,
            token,
            id: format!("youtfast-{:016x}", rand::random::<u64>()),
            sid: String::new(),
            gsid: String::new(),
            rid: 1,
            offset: 0,
            aid: 0,
            status: Status::default(),
        };
        let query = vec![
            ("CVER", "1".into()),
            ("RID", "1".into()),
            ("VER", "8".into()),
            ("app", "youtube-desktop".into()),
            ("device", "REMOTE_CONTROL".into()),
            ("id", session.id.clone()),
            ("name", "YouTFast".into()),
            ("loungeIdToken", session.token.clone()),
        ];
        let data = fetch(
            session
                .client
                .post(format!("{BASE}/bc/bind"))
                .query(&query)
                .header("Content-Length", "0"),
        )
        .await?;
        session.read(&data)?;
        if session.sid.is_empty() || session.gsid.is_empty() {
            return Err(Error::InvalidResponse);
        }
        Ok(session)
    }

    fn query(&self) -> Vec<(&'static str, String)> {
        vec![
            ("app", "youtube-desktop".into()),
            ("device", "REMOTE_CONTROL".into()),
            ("id", self.id.clone()),
            ("name", "YouTFast".into()),
            ("CVER", "1".into()),
            ("RID", self.rid.to_string()),
            ("SID", self.sid.clone()),
            ("VER", "8".into()),
            ("gsessionid", self.gsid.clone()),
            ("loungeIdToken", self.token.clone()),
        ]
    }

    async fn command(&mut self, command: &str, values: &[(&str, String)]) -> Result<(), Error> {
        self.rid += 1;
        // Reserve the offset before awaiting: cancellation must not reuse it.
        self.offset += 1;
        let mut form = vec![
            ("count".into(), "1".into()),
            ("ofs".into(), self.offset.to_string()),
            ("req0__sc".into(), command.into()),
        ];
        form.extend(
            values
                .iter()
                .map(|(key, value)| (format!("req0_{key}"), value.clone())),
        );
        let data = fetch(
            self.client
                .post(format!("{BASE}/bc/bind"))
                .query(&self.query())
                .form(&form),
        )
        .await?;
        self.read(&data)
    }

    async fn poll(&mut self) -> Result<(), Error> {
        let mut query = self.query();
        query.retain(|(key, _)| *key != "RID");
        query.extend([
            ("RID", "rpc".into()),
            ("AID", self.aid.to_string()),
            ("CI", "1".into()),
            ("TYPE", "xmlhttp".into()),
            ("t", "1".into()),
            ("v", "2".into()),
        ]);
        let data = fetch(
            self.client
                .post(format!("{BASE}/bc/bind"))
                .query(&query)
                .header("Content-Length", "0")
                .timeout(Duration::from_secs(6)),
        )
        .await?;
        self.read(&data)
    }

    pub async fn play(&mut self, video: &str, seconds: u32) -> Result<(), Error> {
        self.status = Status::default();
        self.command(
            "setPlaylist",
            &[
                ("videoId", video.into()),
                ("videoIds", video.into()),
                ("currentTime", seconds.to_string()),
                ("currentIndex", "0".into()),
            ],
        )
        .await?;
        tokio::time::sleep(Duration::from_secs(1)).await;
        self.command("getNowPlaying", &[]).await?;
        for _ in 0..12 {
            if self.status.video.as_deref() == Some(video) && self.status.state == Some(1) {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(750)).await;
            match self.poll().await {
                Ok(()) | Err(Error::TimedOut) => {}
                Err(error) => return Err(error),
            }
        }
        Err(Error::Unconfirmed)
    }

    /// Always request fresh telemetry; a cached position must never trigger a seek.
    pub async fn skip_sponsor(
        &mut self,
        video: &str,
        segments: &[crate::model::Segment],
        enabled: &std::sync::atomic::AtomicBool,
    ) -> Result<Option<f64>, Error> {
        self.status.video = None;
        self.status.state = None;
        self.status.seconds = None;
        self.command("getNowPlaying", &[]).await?;
        for _ in 0..3 {
            if self.status.video.is_some()
                && self.status.state.is_some()
                && self.status.seconds.is_some()
            {
                break;
            }
            self.poll().await?;
        }
        let Some(end) = self.status.sponsor_target(video, segments) else {
            return Ok(None);
        };
        if !enabled.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(None);
        }
        self.command("seekTo", &[("newTime", end.to_string())])
            .await?;
        // Do not reuse the pre-seek position while waiting for a TV event.
        self.status.seconds = None;
        Ok(Some(end))
    }

    pub async fn control(&mut self, control: Control) -> Result<(), Error> {
        match control {
            Control::Play => self.command("play", &[]).await,
            Control::Pause => self.command("pause", &[]).await,
            Control::Forward | Control::Rewind => {
                self.status.seconds = None;
                self.command("getNowPlaying", &[]).await?;
                for _ in 0..3 {
                    if self.status.seconds.is_some() {
                        break;
                    }
                    self.poll().await?;
                }
                let at = self.status.seconds.ok_or(Error::Unconfirmed)?;
                let target = (at
                    + if control == Control::Forward {
                        10.0
                    } else {
                        -10.0
                    })
                .max(0.0);
                self.command("seekTo", &[("newTime", target.to_string())])
                    .await
            }
            Control::VolumeUp | Control::VolumeDown => {
                self.status.volume = None;
                self.command("getVolume", &[]).await?;
                for _ in 0..3 {
                    if self.status.volume.is_some() {
                        break;
                    }
                    self.poll().await?;
                }
                let volume = self.status.volume.ok_or(Error::Unconfirmed)?;
                let target = (volume
                    + if control == Control::VolumeUp {
                        10.0
                    } else {
                        -10.0
                    })
                .clamp(0.0, 100.0);
                self.command(
                    "setVolume",
                    &[("volume", target.to_string()), ("muted", "false".into())],
                )
                .await
            }
            Control::Stop => Err(Error::InvalidRequest), // DIAL owns the app lifecycle.
        }
    }

    fn read(&mut self, bytes: &[u8]) -> Result<(), Error> {
        for packet in packets(bytes)? {
            let rows = packet.as_array().ok_or(Error::InvalidResponse)?;
            for row in rows {
                if let Some(sequence) = row[0].as_u64() {
                    self.aid = self.aid.max(sequence);
                }
                let event = &row[1];
                let name = event[0].as_str().unwrap_or_default();
                let data = &event[1];
                #[cfg(test)]
                eprintln!(
                    "Lounge event: {}",
                    match name {
                        "c" | "S" => "session",
                        "nowPlaying" => "now-playing",
                        "onStateChange" => "state",
                        "onVolumeChanged" => "volume",
                        "noop" => "keepalive",
                        "loungeStatus" => "screen-list",
                        "playlistModified" => "playlist-updated",
                        "onAutoplayModeChanged" => "autoplay-setting",
                        "gracefulReconnect" => "reconnect",
                        "onPlaybackQualityChanged" => "quality",
                        "onAdStateChange" => "advertisement",
                        "onError" => "player-error",
                        _ => "other",
                    }
                );
                match name {
                    "c" => self.sid = data.as_str().unwrap_or_default().to_owned(),
                    "S" => self.gsid = data.as_str().unwrap_or_default().to_owned(),
                    "nowPlaying" | "onStateChange" => {
                        if let Some(video) = data["videoId"].as_str() {
                            if self.status.video.as_deref() != Some(video) {
                                self.status.seconds = None;
                                self.status.state = None;
                            }
                            self.status.video = Some(video.into());
                        }
                        if let Some(state) = number(&data["state"]) {
                            self.status.state = Some(state as i64);
                        }
                        if let Some(time) = number(&data["currentTime"]).filter(|v| *v >= 0.0) {
                            self.status.seconds = Some(time);
                        }
                    }
                    "adPlaying" | "onAdStateChange" => {
                        // Only an explicit end/skip event clears an observed ad.
                        self.status.ad =
                            !matches!(number(&data["adState"]), Some(-1.0 | 0.0 | 1082.0));
                    }
                    "onVolumeChanged" => {
                        if let Some(volume) =
                            number(&data["volume"]).filter(|v| (0.0..=100.0).contains(v))
                        {
                            self.status.volume = Some(volume);
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|v| v.is_finite())
}

fn packets(mut bytes: &[u8]) -> Result<Vec<Value>, Error> {
    let mut packets = Vec::new();
    while !bytes.is_empty() {
        // BrowserChannel may return more than one length-prefixed JSON packet.
        let start = bytes.iter().position(|c| *c == b'[');
        let Some(start) = start else {
            if bytes.iter().all(u8::is_ascii_whitespace) {
                break;
            }
            return Err(Error::InvalidResponse);
        };
        if !bytes[..start]
            .iter()
            .all(|c| c.is_ascii_digit() || c.is_ascii_whitespace())
        {
            return Err(Error::InvalidResponse);
        }
        let mut values = serde_json::Deserializer::from_slice(&bytes[start..]).into_iter::<Value>();
        let value = values
            .next()
            .ok_or(Error::InvalidResponse)?
            .map_err(|_| Error::InvalidResponse)?;
        bytes = &bytes[start + values.byte_offset()..];
        packets.push(value);
        if packets.len() > 128 {
            return Err(Error::InvalidResponse);
        }
    }
    Ok(packets)
}

async fn fetch(request: reqwest::RequestBuilder) -> Result<Vec<u8>, Error> {
    let mut response = request
        .header("Origin", "https://www.youtube.com")
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Safari/537.36")
        .send()
        .await
        .map_err(|error| {
            #[cfg(test)]
            eprintln!(
                "Lounge transport failure: connect={}, timeout={}, body={}",
                error.is_connect(),
                error.is_timeout(),
                error.is_body()
            );
            if error.is_timeout() { Error::TimedOut } else { Error::Failed }
        })?;
    #[cfg(test)]
    eprintln!("Lounge response status: {}", response.status().as_u16());
    if matches!(response.status().as_u16(), 400 | 401 | 403 | 404) {
        return Err(Error::ConnectionExpired);
    }
    if !response.status().is_success() {
        return Err(Error::Failed);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| {
        if error.is_timeout() {
            Error::TimedOut
        } else {
            Error::Failed
        }
    })? {
        if bytes.len() + chunk.len() > LIMIT {
            return Err(Error::InvalidResponse);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_channel_reads_multiple_packets_and_rejects_non_protocol_data() {
        let values =
            packets(b"42\n[[0,[\"c\",\"synthetic-sid\"]]]\n15\n[[1,[\"S\",\"synthetic-gsid\"]]]\n")
                .unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values[1][0][1][0], "S");
        assert!(packets(b"arbitrary error [private diagnostic]").is_err());
        assert!(packets(b"42\n[[broken").is_err());
    }

    #[test]
    fn sponsor_target_requires_content_playback_and_suppresses_advertisements() {
        let segment = crate::model::Segment {
            category: "sponsor".into(),
            start: 10.0,
            end: 30.0,
        };
        let mut status = Status {
            video: Some("abcdefghijk".into()),
            state: Some(1),
            seconds: Some(12.0),
            ..Default::default()
        };
        assert_eq!(
            status.sponsor_target("abcdefghijk", std::slice::from_ref(&segment)),
            Some(30.0)
        );
        for state in [-1, 0, 2, 3, 1081, 1082] {
            status.state = Some(state);
            assert_eq!(
                status.sponsor_target("abcdefghijk", std::slice::from_ref(&segment)),
                None
            );
        }
        status.state = Some(1);
        status.ad = true;
        assert_eq!(
            status.sponsor_target("abcdefghijk", std::slice::from_ref(&segment)),
            None
        );
        status.ad = false;
        assert_eq!(
            status.sponsor_target("another_one", std::slice::from_ref(&segment)),
            None
        );
        status.seconds = None;
        assert_eq!(status.sponsor_target("abcdefghijk", &[segment]), None);
    }

    #[test]
    fn playback_confirmation_requires_an_actual_tv_event_not_just_a_successful_post() {
        let client = super::super::innertube::http().build().unwrap();
        let mut session = Session {
            client,
            token: String::new(),
            id: "synthetic".into(),
            sid: String::new(),
            gsid: String::new(),
            rid: 1,
            offset: 0,
            aid: 0,
            status: Status::default(),
        };
        session
            .read(b"[[0,[\"c\",\"synthetic\"]],[1,[\"S\",\"synthetic\"]],[2,[\"noop\"]]]")
            .unwrap();
        assert!(session.status.video.is_none());
        session.read(br#"[[3,["nowPlaying",{"videoId":"abcdefghijk","state":"1","currentTime":"12.5"}]],[4,["onVolumeChanged",{"volume":"65"}]]]"#).unwrap();
        assert_eq!(session.status.video.as_deref(), Some("abcdefghijk"));
        assert_eq!(session.status.state, Some(1));
        assert_eq!(session.status.seconds, Some(12.5));
        assert_eq!(session.status.volume, Some(65.0));
        session
            .read(br#"[[5,["adPlaying",{"adState":"1081","currentTime":"1"}]]]"#)
            .unwrap();
        assert!(session.status.ad);
        assert_eq!(session.status.seconds, Some(12.5));
        session
            .read(br#"[[6,["onAdStateChange",{"adState":"1082"}]]]"#)
            .unwrap();
        assert!(!session.status.ad);
        session
            .read(br#"[[7,["nowPlaying",{"videoId":"another_one"}]]]"#)
            .unwrap();
        assert!(session.status.seconds.is_none());
        assert!(session.status.state.is_none());
        assert_eq!(session.aid, 7);
        assert_eq!(number(&serde_json::json!("NaN")), None);
    }
}
