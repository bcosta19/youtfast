//! History's local merge. Account day headings are display text, not timestamps.
use std::collections::{HashMap, HashSet};

use crate::model::{HistoryRow, Video};
use crate::storage::library::Watched;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HistoryFilter {
    #[default]
    All,
    Account,
    Device,
}

/// A displayed row; only device-backed rows can remove a local entry.
#[derive(Debug)]
pub struct Row<'a> {
    pub video: &'a Video,
    pub when: Option<String>,
    pub device: Option<&'a Watched>,
}

impl Row<'_> {
    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        self.video.title.to_lowercase().contains(&query)
            || self
                .video
                .channel
                .as_deref()
                .unwrap_or_default()
                .to_lowercase()
                .contains(&query)
    }
}

fn date(at: i64) -> Option<jiff::civil::Date> {
    jiff::Timestamp::from_second(at)
        .ok()
        .map(|at| at.to_zoned(jiff::tz::TimeZone::system()).date())
}

// The request uses RustyPipe's UTC default, independently of the UI's clock.
fn account_date(at: i64) -> Option<jiff::civil::Date> {
    jiff::Timestamp::from_second(at)
        .ok()
        .map(|at| at.to_zoned(jiff::tz::TimeZone::UTC).date())
}

/// RustyPipe's default request language is English. Only these two exact
/// headings have an unambiguous relative meaning; never parse weekdays or
/// localized month/date strings as timestamps.
fn section_date(heading: Option<&str>, now: i64) -> Option<jiff::civil::Date> {
    let today = account_date(now)?;
    match heading? {
        "Today" => Some(today),
        "Yesterday" => today.checked_sub(jiff::Span::new().days(1)).ok(),
        _ => None,
    }
}

pub fn account_rows<'a>(account: &'a [HistoryRow], device: &'a [Watched]) -> Vec<Row<'a>> {
    let device: HashMap<_, _> = device
        .iter()
        .map(|watched| (watched.video.id.as_str(), watched))
        .collect();
    let mut seen = HashSet::new();
    account
        .iter()
        .filter_map(|row| {
            if !seen.insert(row.video.id.as_str()) {
                return None;
            }
            let watched = device.get(row.video.id.as_str()).copied();
            Some(Row {
                video: watched.map_or(&row.video, |watched| &watched.video),
                when: row.when.clone(),
                device: watched,
            })
        })
        .collect()
}

/// Preserve account order, overlay device metadata for shared ids, and insert
/// device-only rows newest first before the first older or unknown section.
/// Same-day device-only rows precede the account rows (their times are unknown).
pub fn merge<'a>(account: &'a [HistoryRow], device: &'a [Watched], now: i64) -> Vec<Row<'a>> {
    let account_ids: HashSet<_> = account.iter().map(|row| row.video.id.as_str()).collect();
    let mut seen = HashSet::new();
    let mut local: Vec<_> = device
        .iter()
        .filter(|watched| !account_ids.contains(watched.video.id.as_str()))
        .collect();
    local.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| a.video.id.cmp(&b.video.id)));
    local.retain(|watched| seen.insert(watched.video.id.as_str()));
    let mut local = local.into_iter().peekable();
    let mut result = Vec::with_capacity(account.len() + device.len());
    let mut heading = None;
    let mut first = true;
    for row in account_rows(account, device) {
        if first || row.when != heading {
            first = false;
            let day = section_date(row.when.as_deref(), now);
            while local.peek().is_some_and(|watched| {
                day.is_none() || account_date(watched.at).is_none_or(|date| Some(date) >= day)
            }) {
                let watched = local.next().unwrap();
                result.push(Row {
                    video: &watched.video,
                    when: date(watched.at).map(|date| date.to_string()),
                    device: Some(watched),
                });
            }
            heading.clone_from(&row.when);
        }
        result.push(row);
    }
    result.extend(local.map(|watched| Row {
        video: &watched.video,
        when: date(watched.at).map(|date| date.to_string()),
        device: Some(watched),
    }));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn watched(id: &str, at: i64) -> Watched {
        Watched {
            video: Video::from_id(id.into()),
            at,
            position: 42.0,
        }
    }
    fn account(id: &str, day: &str) -> HistoryRow {
        HistoryRow {
            video: Video::from_id(id.into()),
            when: Some(day.into()),
        }
    }
    fn timestamp(day: &str) -> i64 {
        day.parse::<jiff::civil::Date>()
            .unwrap()
            .at(12, 0, 0, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)
            .unwrap()
            .timestamp()
            .as_second()
    }

    #[test]
    fn shared_ids_keep_account_order_and_device_resume_and_search_metadata() {
        let account = [
            account("shared", "Today"),
            account("only-account", "Yesterday"),
            account("shared", "Yesterday"),
        ];
        let mut local = watched("shared", timestamp("2026-10-22"));
        local.video.title = "Rust history".into();
        local.video.channel = Some("Ciência".into());
        local.video.duration = Some(100.0);
        let device = [local];
        let rows = merge(&account, &device, timestamp("2026-10-23"));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].when.as_deref(), Some("Today"));
        assert_eq!(rows[0].device.unwrap().position, 42.0);
        assert_eq!(rows[0].video.duration, Some(100.0));
        assert!(rows[0].matches(" RUST "));
        assert!(rows[0].matches("ciência"));
        assert!(!rows[0].matches("absent"));
        assert!(rows[1].device.is_none());
    }

    #[test]
    fn local_rows_use_mapped_days_then_unknown_sections_without_reordering_account() {
        let account = [
            account("a", "Today"),
            account("b", "Yesterday"),
            account("c", "Monday"),
            account("d", "20 October"),
        ];
        let device = [
            watched("old", timestamp("2026-10-01")),
            watched("yesterday", timestamp("2026-10-22")),
            watched("today", timestamp("2026-10-23")),
        ];
        let rows = merge(&account, &device, timestamp("2026-10-23"));
        assert_eq!(
            rows.iter()
                .map(|row| row.video.id.as_str())
                .collect::<Vec<_>>(),
            ["today", "a", "yesterday", "b", "old", "c", "d"]
        );
        assert_eq!(rows[1].when.as_deref(), Some("Today"));
        assert_eq!(rows[5].when.as_deref(), Some("Monday"));
        assert_eq!(
            merge(&account, &device, timestamp("2026-10-23"))
                .iter()
                .map(|row| row.video.id.as_str())
                .collect::<Vec<_>>(),
            rows.iter()
                .map(|row| row.video.id.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn unknown_or_localized_headings_are_not_guessed_and_ties_are_stable() {
        let account = [account("account", "Hoje")];
        let device = [watched("z", 1), watched("a", 1), watched("a", 0)];
        let rows = merge(&account, &device, 0);
        assert_eq!(
            rows.iter()
                .map(|row| row.video.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "z", "account"]
        );
        assert_eq!(section_date(Some("Monday"), 0), None);
        assert_eq!(section_date(Some("Hoje"), 0), None);
        assert_eq!(section_date(None, 0), None);
        assert_eq!(merge(&[], &device, 0).len(), 2);
    }
}
