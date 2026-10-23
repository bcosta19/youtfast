//! Google Takeout's YouTube files, for bringing an account's subscriptions
//! and history over without signing in:
//!
//! - `subscriptions/subscriptions.csv`: channel id, URL and title. The
//!   header row is in the account's language, so columns go by position.
//! - `history/watch-history.json`: what was watched and when. Takeout
//!   writes HTML unless JSON is chosen when the export is made.

use std::collections::HashMap;
use std::io::{BufRead, Read};

use serde::Deserialize;

use crate::model::{Channel, Video};
use crate::storage::library::{Import, Watched};
use crate::youtube::ytdlp;

/// Reads either Takeout file, told apart by its contents.
#[cfg(test)]
pub fn parse(bytes: &[u8]) -> Result<Import, String> {
    parse_reader(bytes)
}

/// Streaming: a big watch-history.json is deserialized as it is
/// read instead of sitting in memory a second time as raw bytes.
pub fn parse_reader<R: BufRead>(reader: R) -> Result<Import, String> {
    parse_limited(reader, 64 * 1024 * 1024)
}

struct Limited<R> {
    reader: R,
    remaining: u64,
}

impl<R: Read> Read for Limited<R> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            return match self.reader.read(&mut [0])? {
                0 => Ok(0),
                _ => Err(std::io::Error::other("Takeout exceeds the size limit")),
            };
        }
        let count = bytes.len().min(self.remaining as usize);
        let read = self.reader.read(&mut bytes[..count])?;
        self.remaining -= read as u64;
        Ok(read)
    }
}

fn parse_limited<R: Read>(reader: R, limit: u64) -> Result<Import, String> {
    let mut reader = Limited {
        reader,
        remaining: limit,
    };
    // A UTF-8 BOM first, if there is one.
    let mut prefix = [0; 3];
    let mut length = 0;
    while length < prefix.len() {
        let read = reader
            .read(&mut prefix[length..])
            .map_err(|_| "Could not read this file (64 MiB limit).")?;
        if read == 0 {
            break;
        }
        length += read;
    }
    let prefix = if prefix == [0xEF, 0xBB, 0xBF] {
        &[][..]
    } else {
        &prefix[..length]
    };
    let mut reader = std::io::BufReader::new(prefix.chain(reader));
    // The first byte that is not whitespace, without consuming it: the
    // parsers below want their text whole.
    let first = loop {
        let buffer = reader
            .fill_buf()
            .map_err(|_| "Could not read this file (64 MiB limit).")?;
        let count = buffer
            .iter()
            .take_while(|byte| byte.is_ascii_whitespace())
            .count();
        if count < buffer.len() {
            break Some(buffer[count]);
        }
        if buffer.is_empty() {
            break None;
        }
        reader.consume(count);
    };
    match first {
        Some(b'[') => Ok(Import {
            history: history(reader)?,
            ..Import::default()
        }),
        Some(b'<') => Err(
            "This is Takeout's HTML history. Export it again with JSON chosen as the \
             history format."
                .to_owned(),
        ),
        Some(_) => {
            let mut subscriptions = Vec::new();
            let mut known = std::collections::HashSet::new();
            loop {
                let mut line = String::new();
                let count = reader
                    .by_ref()
                    .take(1024 * 1024 + 1)
                    .read_line(&mut line)
                    .map_err(|_| "Could not read this subscriptions file (64 MiB limit).")?;
                if count == 0 {
                    break;
                }
                if count > 1024 * 1024 {
                    return Err("A subscriptions row exceeds 1 MiB.".into());
                }
                if let Some(channel) = subscription(&line)?
                    && known.insert(channel.id.clone())
                {
                    if subscriptions.len() == 20_000 {
                        return Err("This file exceeds 20,000 subscriptions.".into());
                    }
                    subscriptions.push(channel);
                }
            }
            if subscriptions.is_empty() {
                Err(
                    "No YouTube subscriptions or history found in this file. Choose \
                     subscriptions.csv or watch-history.json from the Takeout archive."
                        .to_owned(),
                )
            } else {
                Ok(Import {
                    subscriptions,
                    ..Import::default()
                })
            }
        }
        None => Err(
            "No YouTube subscriptions or history found in this file. Choose \
             subscriptions.csv or watch-history.json from the Takeout archive."
                .to_owned(),
        ),
    }
}

/// The channel one subscriptions.csv line names, or `None` for the header
/// row, in whatever language it is.
fn subscription(line: &str) -> Result<Option<Channel>, String> {
    let fields = csv_fields(line)
        .ok_or_else(|| "This subscriptions CSV has an unclosed quote.".to_owned())?;
    let id = fields.first().map_or("", String::as_str).trim();
    if !(id.starts_with("UC") && id.len() == 24) {
        return Ok(None);
    }
    let name = fields.get(2).map_or(id, |name| name.trim());
    Ok(Some(Channel {
        id: id.to_owned(),
        name: name.to_owned(),
        avatar: None,
    }))
}

/// The fields of one CSV line: commas separate, double quotes enclose a
/// field that has commas, and `""` inside quotes is one quote.
fn csv_fields(line: &str) -> Option<Vec<String>> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            ('"', _) => quoted = !quoted,
            (',', false) => fields.push(std::mem::take(&mut field)),
            _ => field.push(c),
        }
    }
    fields.push(field);
    (!quoted).then_some(fields)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    title: Option<String>,
    title_url: Option<String>,
    #[serde(default)]
    subtitles: Vec<Subtitle>,
    time: Option<String>,
}

#[derive(Deserialize)]
struct Subtitle {
    name: Option<String>,
    url: Option<String>,
}

/// How Takeout begins a history title, in the languages it is most often
/// exported in. The rest are left as Takeout wrote them.
const WATCHED_PREFIXES: [&str; 6] = [
    "Watched ",
    "Assistiu a ",
    "Assistiu ",
    "Has visto ",
    "Viste ",
    "Vous avez regardé ",
];

fn history<R: Read>(reader: R) -> Result<Vec<Watched>, String> {
    struct History;
    impl<'de> serde::de::Visitor<'de> for History {
        type Value = Vec<Watched>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a Takeout history array")
        }

        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut rows: Vec<Watched> = Vec::new();
            let mut known: HashMap<String, usize> = HashMap::new();
            let limit = crate::storage::library::HISTORY_LIMIT;
            while let Some(entry) = seq.next_element::<Entry>()? {
                let Some(row) = watched(entry) else {
                    continue;
                };
                if let Some(&index) = known.get(&row.video.id) {
                    rows[index].at = rows[index].at.max(row.at);
                } else {
                    known.insert(row.video.id.clone(), rows.len());
                    rows.push(row);
                }
                if rows.len() == limit * 2 {
                    rows.sort_by_key(|row| std::cmp::Reverse(row.at));
                    rows.truncate(limit);
                    known = rows
                        .iter()
                        .enumerate()
                        .map(|(index, row)| (row.video.id.clone(), index))
                        .collect();
                }
            }
            rows.sort_by_key(|row| std::cmp::Reverse(row.at));
            rows.truncate(limit);
            Ok(rows)
        }
    }
    let mut decoder = serde_json::Deserializer::from_reader(reader);
    let rows = serde::Deserializer::deserialize_seq(&mut decoder, History).map_err(|_| {
        "Could not read this history file. Check its JSON format and the 64 MiB size limit."
            .to_owned()
    })?;
    decoder
        .end()
        .map_err(|_| "The history file has invalid or oversized trailing data.".to_owned())?;
    Ok(rows)
}

fn watched(entry: Entry) -> Option<Watched> {
    // Ads and removed videos have no link to a video.
    let id = ytdlp::video_id_from_link(entry.title_url.as_deref()?)?;
    let at = entry
        .time
        .as_deref()
        .and_then(|time| time.parse::<jiff::Timestamp>().ok())?
        .as_second();
    let mut title = entry.title.unwrap_or_default();
    if let Some(stripped) = WATCHED_PREFIXES
        .iter()
        .find_map(|prefix| title.strip_prefix(prefix))
    {
        title = stripped.to_owned();
    }
    let channel = entry.subtitles.into_iter().next();
    let channel_id = channel
        .as_ref()
        .and_then(|channel| channel.url.as_deref())
        .and_then(|url| url.rsplit("/channel/").next())
        .filter(|id| id.starts_with("UC"))
        .map(str::to_owned);
    Some(Watched {
        video: Video {
            title,
            channel: channel.and_then(|channel| channel.name),
            channel_id,
            ..Video::from_id(id)
        },
        at,
        position: 0.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_and_a_split_bom_are_detected_across_buffers() {
        struct OneByte<'a>(&'a [u8]);
        impl Read for OneByte<'_> {
            fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
                let len = bytes.len().min(self.0.len()).min(1);
                bytes[..len].copy_from_slice(&self.0[..len]);
                self.0 = &self.0[len..];
                Ok(len)
            }
        }
        let bytes = format!("\u{feff}{}[]", " ".repeat(20_000));
        let import = parse_reader(std::io::BufReader::with_capacity(
            1,
            OneByte(bytes.as_bytes()),
        ))
        .unwrap();
        assert!(import.history.is_empty());
    }

    #[test]
    fn malformed_and_oversized_inputs_never_return_a_partial_import() {
        assert!(parse(b"[{\"title\":\"private title\"}, broken]").is_err());
        assert!(parse(b"[] trailing").is_err());
        assert!(parse(b"").is_err());
        assert!(parse(b"UC_x5XG1OV2P6uZZ5FSM9Ttw,url,\"unclosed").is_err());
        assert!(parse_limited(&b"[]          "[..], 4).is_err());
        assert!(parse_limited(&b"[]"[..], 2).is_ok());
    }

    #[test]
    fn streaming_history_keeps_only_the_newest_unique_videos() {
        let entries: Vec<_> = (0..5_000).map(|index| serde_json::json!({
            "titleUrl": format!("https://www.youtube.com/watch?v={index:011}"),
            "time": jiff::Timestamp::from_second(1_700_000_000 + index).unwrap().to_string(),
        })).collect();
        let json = serde_json::to_vec(&entries).unwrap();
        let import = parse(&json).unwrap();
        assert_eq!(import.history.len(), crate::storage::library::HISTORY_LIMIT);
        assert_eq!(import.history[0].at, 1_700_004_999);
        assert_eq!(import.history.last().unwrap().at, 1_700_004_000);
    }

    #[test]
    fn subscriptions_in_any_language() {
        let csv = "\u{feff}ID do canal,URL do canal,Título do canal\n\
                   UCBR8-60-B28hp2BmDPdntcQ,http://www.youtube.com/channel/UCBR8-60-B28hp2BmDPdntcQ,YouTube\n\
                   UC_x5XG1OV2P6uZZ5FSM9Ttw,http://www.youtube.com/channel/UC_x5XG1OV2P6uZZ5FSM9Ttw,\"Google for Developers, \"\"Official\"\"\"\n\
                   \n";
        let import = parse(csv.as_bytes()).unwrap();
        assert_eq!(import.subscriptions.len(), 2);
        assert_eq!(import.subscriptions[0].name, "YouTube");
        assert_eq!(import.subscriptions[1].id, "UC_x5XG1OV2P6uZZ5FSM9Ttw");
        assert_eq!(
            import.subscriptions[1].name,
            "Google for Developers, \"Official\""
        );
    }

    #[test]
    fn history_from_json() {
        let json = r#"[
            {"header": "YouTube", "title": "Watched Never Gonna Give You Up",
             "titleUrl": "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
             "subtitles": [{"name": "Rick Astley",
                            "url": "https://www.youtube.com/channel/UCuAXFkgsw1L7xaCfnd5JJOw"}],
             "time": "2024-05-01T12:34:56.789Z", "products": ["YouTube"]},
            {"header": "YouTube", "title": "Assistiu a Um vídeo",
             "titleUrl": "https://www.youtube.com/watch?v=abcdefghijk",
             "time": "2024-04-30T08:00:00Z"},
            {"header": "YouTube", "title": "Watched a video that has been removed",
             "time": "2024-04-29T08:00:00Z"}
        ]"#;
        let import = parse(json.as_bytes()).unwrap();
        assert_eq!(import.history.len(), 2);
        let first = &import.history[0];
        assert_eq!(first.video.id, "dQw4w9WgXcQ");
        assert_eq!(first.video.title, "Never Gonna Give You Up");
        assert_eq!(first.video.channel.as_deref(), Some("Rick Astley"));
        assert_eq!(
            first.video.channel_id.as_deref(),
            Some("UCuAXFkgsw1L7xaCfnd5JJOw")
        );
        assert_eq!(first.at, 1_714_566_896);
        assert_eq!(import.history[1].video.title, "Um vídeo");
    }

    #[test]
    fn html_history_is_explained() {
        let error = parse(b"<html><body>...").unwrap_err();
        assert!(error.contains("JSON"));
        assert!(parse(b"just some text").is_err());
    }
}
