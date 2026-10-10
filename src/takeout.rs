//! Google Takeout's YouTube files, for bringing an account's subscriptions
//! and history over without signing in:
//!
//! - `subscriptions/subscriptions.csv`: channel id, URL and title. The
//!   header row is in the account's language, so columns go by position.
//! - `history/watch-history.json`: what was watched and when. Takeout
//!   writes HTML unless JSON is chosen when the export is made.

use std::io::BufRead;

use serde::Deserialize;

use crate::library::{Import, Watched};
use crate::model::{Channel, Video};
use crate::youtube;

/// Reads either Takeout file, told apart by its contents.
pub fn parse(bytes: &[u8]) -> Result<Import, String> {
    parse_reader(bytes)
}

/// [`parse`], streaming: a big watch-history.json is deserialized as it is
/// read instead of sitting in memory a second time as raw bytes.
pub fn parse_reader<R: BufRead>(mut reader: R) -> Result<Import, String> {
    // A UTF-8 BOM first, if there is one.
    if let Ok(buf) = reader.fill_buf()
        && buf.starts_with(&[0xEF, 0xBB, 0xBF])
    {
        reader.consume(3);
    }
    // The first byte that is not whitespace, without consuming it: the
    // parsers below want their text whole.
    let first = reader
        .fill_buf()
        .map_err(|error| format!("Could not read this file: {error}"))?
        .iter()
        .find(|byte| !byte.is_ascii_whitespace())
        .copied();
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
            for line in reader.lines() {
                let line = line.map_err(|error| format!("Could not read this file: {error}"))?;
                if let Some(channel) = subscription(&line) {
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
fn subscription(line: &str) -> Option<Channel> {
    let fields = csv_fields(line);
    let id = fields.first()?.trim();
    if !(id.starts_with("UC") && id.len() == 24) {
        return None;
    }
    let name = fields.get(2).map_or(id, |name| name.trim());
    Some(Channel {
        id: id.to_owned(),
        name: name.to_owned(),
        avatar: None,
    })
}

/// The fields of one CSV line: commas separate, double quotes enclose a
/// field that has commas, and `""` inside quotes is one quote.
fn csv_fields(line: &str) -> Vec<String> {
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
    fields
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

fn history<R: std::io::Read>(reader: R) -> Result<Vec<Watched>, String> {
    let entries: Vec<Entry> = serde_json::from_reader(reader)
        .map_err(|error| format!("Could not read this history file: {error}"))?;
    Ok(entries
        .into_iter()
        .filter_map(|entry| {
            // Ads and removed videos have no link to a video.
            let id = youtube::video_id_from_link(entry.title_url.as_deref()?)?;
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
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

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
