//! Versioned library backup (JSON) and subscription feed export (OPML 2.0).
//!
//! Backups only ever carry the device library (`subscriptions`, `history` with
//! resume positions, and `watch_later`). Accounts, sessions, cookies, and
//! notification timestamps stay out of exported files.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};

use crate::library::{HISTORY_LIMIT, Import, Library, Watched};
use crate::takeout::{Limited, read_bom_prefix};
use youtfast_core::model::{Channel, Video};

pub const BACKUP_VERSION: u32 = 1;
pub const WATCH_LATER_LIMIT: usize = 5_000;
pub const SUBSCRIPTIONS_LIMIT: usize = 20_000;
const MAX_BACKUP_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Backup {
    pub version: u32,
    #[serde(default)]
    pub subscriptions: Vec<Channel>,
    #[serde(default)]
    pub history: Vec<Watched>,
    #[serde(default)]
    pub watch_later: Vec<Video>,
}

/// Serializes the device library (subscriptions, history with resume positions,
/// and Watch later) as pretty JSON. Never includes accounts or sessions.
pub fn to_json(library: &Library) -> Result<String, String> {
    export_json(library, MAX_BACKUP_BYTES as usize)
}

fn export_json(library: &Library, max_bytes: usize) -> Result<String, String> {
    if library.subscriptions.len() > SUBSCRIPTIONS_LIMIT {
        return Err("The library exceeds the backup limit of 20,000 subscriptions.".to_owned());
    }
    if library.history.len() > HISTORY_LIMIT {
        return Err("The library exceeds the backup limit of 1,000 history entries.".to_owned());
    }
    if library.watch_later.len() > WATCH_LATER_LIMIT {
        return Err("The library exceeds the backup limit of 5,000 Watch later videos.".to_owned());
    }

    #[derive(Serialize)]
    struct Export<'a> {
        version: u32,
        subscriptions: &'a [Channel],
        history: &'a [Watched],
        watch_later: &'a [Video],
    }

    struct Output {
        bytes: Vec<u8>,
        limit: usize,
        exceeded: bool,
    }

    impl Write for Output {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > self.limit - self.bytes.len() {
                self.exceeded = true;
                return Err(io::Error::other("backup size limit"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let mut output = Output {
        bytes: Vec::new(),
        limit: max_bytes,
        exceeded: false,
    };
    let backup = Export {
        version: BACKUP_VERSION,
        subscriptions: &library.subscriptions,
        history: &library.history,
        watch_later: &library.watch_later,
    };
    serde_json::to_writer_pretty(&mut output, &backup).map_err(|_| {
        if output.exceeded {
            "The library backup exceeds the 64 MiB size limit.".to_owned()
        } else {
            "Could not serialize the library backup.".to_owned()
        }
    })?;
    String::from_utf8(output.bytes).map_err(|_| "Could not serialize the library backup.".to_owned())
}

/// Renders an OPML 2.0 XML document containing each subscribed channel's
/// YouTube Atom feed and channel page URLs.
pub fn to_opml(subscriptions: &[Channel]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <opml version=\"2.0\">\n  \
         <head>\n    \
         <title>YouTube Subscriptions</title>\n  \
         </head>\n  \
         <body>\n    \
         <outline text=\"YouTube Subscriptions\" title=\"YouTube Subscriptions\">\n",
    );
    for channel in subscriptions {
        let id = channel.id.trim();
        if id.is_empty() {
            continue;
        }
        let name = if channel.name.is_empty() {
            id
        } else {
            &channel.name
        };
        let label = escape_xml(name);
        let escaped_id = escape_xml(id);
        let _ = writeln!(
            xml,
            "      <outline type=\"rss\" text=\"{label}\" title=\"{label}\" \
             xmlUrl=\"https://www.youtube.com/feeds/videos.xml?channel_id={escaped_id}\" \
             htmlUrl=\"https://www.youtube.com/channel/{escaped_id}\"/>"
        );
    }
    xml.push_str(
        "    </outline>\n  \
         </body>\n\
         </opml>\n",
    );
    xml
}

fn escape_xml(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Parses a versioned library backup JSON stream, enforcing size and item limits.
pub fn parse_reader<R: Read>(reader: R) -> Result<Import, String> {
    parse_limited(reader, MAX_BACKUP_BYTES)
}

fn parse_limited<R: Read>(reader: R, limit: u64) -> Result<Import, String> {
    let mut limited = Limited::new(reader, limit);
    let (prefix, length) = match read_bom_prefix(&mut limited) {
        Ok(head) => head,
        Err(_) if limited.exceeded => {
            return Err("This backup file exceeds the 64 MiB size limit.".to_owned());
        }
        Err(_) => {
            return Err(
                "Could not read this backup file. Check its JSON format and the 64 MiB size limit."
                    .to_owned(),
            );
        }
    };
    let backup = {
        let mut buf = std::io::BufReader::new(prefix[..length].chain(&mut limited));
        let mut decoder = serde_json::Deserializer::from_reader(&mut buf);
        let backup = Backup::deserialize(&mut decoder);
        match backup {
            Ok(backup) => {
                if decoder.end().is_err() {
                    return Err(if limited.exceeded {
                        "This backup file exceeds the 64 MiB size limit.".to_owned()
                    } else {
                        "The backup file has invalid or oversized trailing data.".to_owned()
                    });
                }
                backup
            }
            Err(_) => {
                return Err(if limited.exceeded {
                    "This backup file exceeds the 64 MiB size limit.".to_owned()
                } else {
                    "Could not read this backup file. Check its JSON format and the 64 MiB size limit."
                        .to_owned()
                });
            }
        }
    };

    if backup.version != BACKUP_VERSION {
        return Err(
            "This backup version is not supported. Update YouTFast to import it.".to_owned(),
        );
    }

    let mut subscriptions = Vec::new();
    let mut seen_channels = HashSet::new();
    for channel in backup.subscriptions {
        if channel.id.trim().is_empty() {
            continue;
        }
        if seen_channels.insert(channel.id.clone()) {
            if subscriptions.len() == SUBSCRIPTIONS_LIMIT {
                return Err("This backup file exceeds 20,000 subscriptions.".to_owned());
            }
            subscriptions.push(channel);
        }
    }

    let mut history: Vec<Watched> = Vec::new();
    let mut seen_history: HashMap<String, usize> = HashMap::new();
    for mut row in backup.history {
        if row.video.id.trim().is_empty() {
            continue;
        }
        if !row.position.is_finite() || row.position < 0.0 {
            row.position = 0.0;
        }
        if let Some(&index) = seen_history.get(&row.video.id) {
            if row.at > history[index].at {
                history[index] = row;
            }
        } else {
            seen_history.insert(row.video.id.clone(), history.len());
            history.push(row);
        }
    }
    history.sort_by_key(|row| Reverse(row.at));
    history.truncate(HISTORY_LIMIT);

    let mut watch_later = Vec::new();
    let mut seen_watch_later = HashSet::new();
    for video in backup.watch_later {
        if video.id.trim().is_empty() {
            continue;
        }
        if watch_later.len() == WATCH_LATER_LIMIT {
            break;
        }
        if seen_watch_later.insert(video.id.clone()) {
            watch_later.push(video);
        }
    }

    Ok(Import {
        subscriptions,
        history,
        watch_later,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Account;
    use youtfast_core::model::AccountSource;

    fn sample_channel(id: &str, name: &str) -> Channel {
        Channel {
            id: id.to_owned(),
            name: name.to_owned(),
            avatar: None,
        }
    }

    fn sample_video(id: &str, title: &str) -> Video {
        Video {
            title: title.to_owned(),
            ..Video::from_id(id.to_owned())
        }
    }

    #[test]
    fn json_round_trip_preserves_resume_positions_and_watch_later_without_accounts() {
        let mut library = Library::default();
        library.subscriptions = vec![sample_channel("UC_x5XG1OV2P6uZZ5FSM9Ttw", "Synthetic")];
        library.watch_later = vec![sample_video("WL000000001", "Watch Later Video")];
        library.accounts = vec![Account {
            id: 1,
            name: "Private Account".to_owned(),
            source: AccountSource::Browser("firefox:secret-profile".into()),
        }];
        library.active_account = Some(1);
        library.alerts_seen_at = Some(1_700_000_000);
        library.alerts_notified_at = Some(1_700_000_100);
        library.record_watch(sample_video("VID00000001", "First Video"), 100);
        library.set_position("VID00000001", 123.5, 600.0);

        let json = to_json(&library).unwrap();
        assert!(!json.contains("accounts"));
        assert!(!json.contains("active_account"));
        assert!(!json.contains("Private Account"));
        assert!(!json.contains("secret-profile"));
        assert!(!json.contains("alerts_seen_at"));
        assert!(!json.contains("alerts_notified_at"));
        assert!(!json.contains("cookies"));

        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["version"], BACKUP_VERSION);

        let import = parse_reader(json.as_bytes()).unwrap();
        assert_eq!(import.subscriptions, library.subscriptions);
        assert_eq!(import.history, library.history);
        assert_eq!(import.history[0].position, 123.5);
        assert_eq!(import.watch_later, library.watch_later);

        // Takeout reader also accepts a YouTFast JSON backup.
        let via_takeout =
            crate::takeout::parse_reader(std::io::BufReader::new(json.as_bytes())).unwrap();
        assert_eq!(via_takeout, import);
    }

    #[test]
    fn export_at_the_item_and_byte_limits_can_be_restored() {
        let mut library = Library::default();
        library.subscriptions = (0..SUBSCRIPTIONS_LIMIT)
            .map(|i| sample_channel(&format!("UC{i:022}"), "Channel"))
            .collect();
        library.watch_later = (0..WATCH_LATER_LIMIT)
            .map(|i| sample_video(&format!("W{i:010}"), "Video"))
            .collect();
        let json = to_json(&library).unwrap();
        let restored = parse_limited(json.as_bytes(), json.len() as u64).unwrap();
        assert_eq!(restored.subscriptions, library.subscriptions);
        assert_eq!(restored.watch_later, library.watch_later);
        assert_eq!(export_json(&library, json.len()).unwrap(), json);
        assert!(
            export_json(&library, json.len() - 1)
                .unwrap_err()
                .contains("64 MiB")
        );

        library.watch_later.push(sample_video("W9999999999", "Overflow"));
        assert!(to_json(&library).unwrap_err().contains("Watch later"));
        library
            .subscriptions
            .push(sample_channel("UCoverflow", "Overflow"));
        assert!(to_json(&library).unwrap_err().contains("subscriptions"));
    }

    #[test]
    fn opml_export_escapes_xml_characters_and_renders_youtube_feeds() {
        let channels = vec![
            sample_channel("UC1234567890123456789012", "Tom & Jerry <\"Shorts\"> '1'"),
            sample_channel("", "Skipped Empty"),
        ];
        let opml = to_opml(&channels);
        assert!(
            opml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<opml version=\"2.0\">")
        );
        assert!(
            opml.contains(
                "<outline text=\"YouTube Subscriptions\" title=\"YouTube Subscriptions\">"
            )
        );
        assert!(opml.contains(
            "<outline type=\"rss\" text=\"Tom &amp; Jerry &lt;&quot;Shorts&quot;&gt; &apos;1&apos;\" \
             title=\"Tom &amp; Jerry &lt;&quot;Shorts&quot;&gt; &apos;1&apos;\" \
             xmlUrl=\"https://www.youtube.com/feeds/videos.xml?channel_id=UC1234567890123456789012\" \
             htmlUrl=\"https://www.youtube.com/channel/UC1234567890123456789012\"/>"
        ));
        assert!(!opml.contains("Skipped Empty"));
    }

    #[test]
    fn unsupported_version_and_trailing_data_are_rejected() {
        let wrong_version =
            r#"{"version": 99, "subscriptions": [], "history": [], "watch_later": []}"#;
        assert_eq!(
            parse_reader(wrong_version.as_bytes()).unwrap_err(),
            "This backup version is not supported. Update YouTFast to import it."
        );

        let trailing = r#"{"version": 1} trailing"#;
        assert_eq!(
            parse_reader(trailing.as_bytes()).unwrap_err(),
            "The backup file has invalid or oversized trailing data."
        );
    }

    #[test]
    fn bom_and_size_limit_are_enforced() {
        let with_bom = "\u{feff}{\"version\": 1}";
        let import = parse_reader(with_bom.as_bytes()).unwrap();
        assert_eq!(import, Import::default());

        let padded = format!("{{\"version\": 1}}{}", " ".repeat(16));
        assert!(parse_limited(padded.as_bytes(), 14).is_err());
        assert_eq!(
            parse_limited(padded.as_bytes(), 14).unwrap_err(),
            "This backup file exceeds the 64 MiB size limit."
        );
    }

    #[test]
    fn subscriptions_limit_history_limit_and_watch_later_dedup_are_enforced() {
        // More than 20,000 unique subscriptions is rejected.
        let too_many_subs: Vec<Channel> = (0..=SUBSCRIPTIONS_LIMIT)
            .map(|i| sample_channel(&format!("UC{i:022}"), &format!("Channel {i}")))
            .collect();
        let json = serde_json::to_string(&Backup {
            version: BACKUP_VERSION,
            subscriptions: too_many_subs,
            history: Vec::new(),
            watch_later: Vec::new(),
        })
        .unwrap();
        assert_eq!(
            parse_reader(json.as_bytes()).unwrap_err(),
            "This backup file exceeds 20,000 subscriptions."
        );

        // History deduplicates, sanitizes positions, and keeps latest 1,000 ordered by Reverse(at).
        let mut history: Vec<Watched> = (0..1_200)
            .map(|i| Watched {
                video: sample_video(&format!("V{i:010}"), &format!("Video {i}")),
                at: i,
                position: if i == 1_199 { -5.0 } else { 42.0 },
            })
            .collect();
        history.push(Watched {
            video: sample_video("V0000001198", "Updated"),
            at: 5_000,
            position: 77.0,
        });
        let watch_later: Vec<Video> = (0..(WATCH_LATER_LIMIT + 10))
            .map(|i| sample_video(&format!("W{i:010}"), &format!("WL {i}")))
            .chain(std::iter::once(sample_video("W0000000000", "Dup")))
            .collect();
        let json = serde_json::to_string(&Backup {
            version: BACKUP_VERSION,
            subscriptions: vec![
                sample_channel("", "Empty"),
                sample_channel("UC1", "One"),
                sample_channel("UC1", "Dup"),
            ],
            history,
            watch_later,
        })
        .unwrap();
        let import = parse_reader(json.as_bytes()).unwrap();
        assert_eq!(import.subscriptions.len(), 1);
        assert_eq!(import.history.len(), HISTORY_LIMIT);
        assert_eq!(import.history[0].video.id, "V0000001198");
        assert_eq!(import.history[0].at, 5_000);
        assert_eq!(import.history[0].position, 77.0);
        assert_eq!(import.history[1].video.id, "V0000001199");
        assert_eq!(import.history[1].position, 0.0);
        assert_eq!(import.watch_later.len(), WATCH_LATER_LIMIT);
        assert_eq!(import.watch_later[0].id, "W0000000000");
    }
}
