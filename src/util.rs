//! Small formatting helpers.

use crate::appearance::i18n::{Locale, gettext, interpolate, ngettext};

/// `4:05`, or `1:02:03` past an hour.
pub fn duration(seconds: f64) -> String {
    let total = if seconds.is_finite() && seconds > 0.0 {
        seconds as u64
    } else {
        0
    };
    let (hours, minutes, seconds) = (total / 3600, total / 60 % 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// `950 views`, `12K views`, `1.2M views`, the way YouTube shortens them.
pub fn views(count: u64, locale: Locale) -> String {
    let short = |value: f64, unit: &str| {
        if value < 10.0 {
            let text = format!("{value:.1}");
            format!("{}{unit}", text.trim_end_matches(".0"))
        } else {
            format!("{}{unit}", value as u64)
        }
    };
    let number = match count {
        0..1_000 => count.to_string(),
        1_000..1_000_000 => short(count as f64 / 1e3, "K"),
        1_000_000..1_000_000_000 => short(count as f64 / 1e6, "M"),
        _ => short(count as f64 / 1e9, "B"),
    };
    let label = ngettext(
        locale,
        "{count} view",
        "{count} views",
        count.min(u32::MAX as u64) as u32,
    );
    interpolate(&label, &[("count", number)])
}

/// `34K`, `1.2M`: a count without its unit, shortened the way YouTube does.
pub fn compact(count: u64) -> String {
    let short = |value: f64, unit: &str| {
        if value < 10.0 {
            let text = format!("{value:.1}");
            format!("{}{unit}", text.trim_end_matches(".0"))
        } else {
            format!("{}{unit}", value as u64)
        }
    };
    match count {
        0..1_000 => count.to_string(),
        1_000..1_000_000 => short(count as f64 / 1e3, "K"),
        1_000_000..1_000_000_000 => short(count as f64 / 1e6, "M"),
        _ => short(count as f64 / 1e9, "B"),
    }
}

/// `1,234,567`: a full number with thousands separators.
pub fn thousands(count: u64) -> String {
    let digits = count.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// Which day-group a publish time belongs to, against `now`'s local
/// clock: "today", "yesterday", "this week" (the last seven days),
/// "this month", "earlier", and "undated" when the time is missing.
pub fn age_bucket(published_at: Option<i64>, now: i64) -> &'static str {
    let Some(at) = published_at else {
        return "undated";
    };
    let Ok(then) =
        jiff::Timestamp::from_second(at).map(|t| t.to_zoned(jiff::tz::TimeZone::system()).date())
    else {
        return "undated";
    };
    let Ok(today) =
        jiff::Timestamp::from_second(now).map(|t| t.to_zoned(jiff::tz::TimeZone::system()).date())
    else {
        return "undated";
    };
    let days = (today - then).get_days();
    match days {
        ..=0 => "today",
        1 => "yesterday",
        2..=6 => "this week",
        7..=30 => "this month",
        _ => "earlier",
    }
}

/// The current time in Unix seconds.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

/// `just now`, `5 minutes ago`, `2 days ago`, `3 years ago`: how long before
/// `now` the Unix time `then` was, the way YouTube says it.
pub fn ago(then: i64, now: i64, locale: Locale) -> String {
    let seconds = now.saturating_sub(then).max(0);
    let (count, singular, plural) = match seconds {
        0..60 => return gettext(locale, "just now").into_owned(),
        60..3_600 => (seconds / 60, "{count} minute ago", "{count} minutes ago"),
        3_600..86_400 => (seconds / 3_600, "{count} hour ago", "{count} hours ago"),
        86_400..604_800 => (seconds / 86_400, "{count} day ago", "{count} days ago"),
        604_800..2_629_800 => (seconds / 604_800, "{count} week ago", "{count} weeks ago"),
        2_629_800..31_557_600 => (
            seconds / 2_629_800,
            "{count} month ago",
            "{count} months ago",
        ),
        _ => (
            seconds / 31_557_600,
            "{count} year ago",
            "{count} years ago",
        ),
    };
    let text = ngettext(locale, singular, plural, count.min(u32::MAX as i64) as u32);
    interpolate(&text, &[("count", count.to_string())])
}

/// The last few values put, by key, each with when it was put: a bounded
/// cache that forgets the oldest first.
pub struct Recent<T> {
    capacity: usize,
    /// Oldest first: (key, Unix seconds put, value).
    entries: std::collections::VecDeque<(String, i64, T)>,
}

impl<T> Recent<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: std::collections::VecDeque::new(),
        }
    }

    /// The value put under `key` at most `max_age` seconds before `now`.
    pub fn get(&self, key: &str, now: i64, max_age: i64) -> Option<&T> {
        self.entries
            .iter()
            .find(|(k, at, _)| k == key && now - at <= max_age)
            .map(|(_, _, value)| value)
    }

    /// Puts a value, replacing the key's older one; past the capacity the
    /// oldest goes.
    pub fn put(&mut self, key: String, now: i64, value: T) {
        self.entries.retain(|(k, _, _)| *k != key);
        self.entries.push_back((key, now, value));
        while self.entries.len() > self.capacity {
            self.entries.pop_front();
        }
    }

    pub fn remove(&mut self, key: &str) {
        self.entries.retain(|(entry, _, _)| entry != key);
    }
}

/// A video's short link, starting `at` seconds in when given.
pub fn video_link(id: &str, at: Option<u64>) -> String {
    match at {
        Some(at) if at > 0 => format!("https://youtu.be/{id}?t={at}"),
        _ => format!("https://youtu.be/{id}"),
    }
}

/// Where the share menu sends a link: each site's own share page, opened
/// in the browser, and an e-mail. `title` goes with the link where the
/// site takes text.
pub fn share_targets(link: &str, title: &str) -> [(&'static str, String); 7] {
    let url = percent_encode(link);
    let text = percent_encode(&format!("{title} {link}"));
    let subject = percent_encode(title);
    [
        ("WhatsApp", format!("https://wa.me/?text={text}")),
        (
            "Telegram",
            format!("https://t.me/share/url?url={url}&text={subject}"),
        ),
        ("X", format!("https://x.com/intent/post?text={text}")),
        (
            "Reddit",
            format!("https://www.reddit.com/submit?url={url}&title={subject}"),
        ),
        (
            "Facebook",
            format!("https://www.facebook.com/sharer/sharer.php?u={url}"),
        ),
        (
            "LinkedIn",
            format!("https://www.linkedin.com/sharing/share-offsite/?url={url}"),
        ),
        ("E-mail", format!("mailto:?subject={subject}&body={url}")),
    ]
}

/// Text as a URL query value: everything but unreserved characters is
/// `%XX`-escaped, byte by byte.
fn percent_encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_links_start_where_asked_and_escape_their_text() {
        assert_eq!(video_link("abc", None), "https://youtu.be/abc");
        assert_eq!(video_link("abc", Some(0)), "https://youtu.be/abc");
        assert_eq!(video_link("abc", Some(83)), "https://youtu.be/abc?t=83");
        let targets = share_targets("https://youtu.be/abc?t=83", "Rock & roll é");
        assert_eq!(
            targets[0].1,
            "https://wa.me/?text=Rock%20%26%20roll%20%C3%A9%20https%3A%2F%2Fyoutu.be%2Fabc%3Ft%3D83"
        );
        assert_eq!(
            targets[6].1,
            "mailto:?subject=Rock%20%26%20roll%20%C3%A9&body=https%3A%2F%2Fyoutu.be%2Fabc%3Ft%3D83"
        );
    }

    #[test]
    fn recent_keeps_the_newest_and_forgets_the_old() {
        let mut recent = Recent::new(2);
        recent.put("a".to_owned(), 0, 1);
        recent.put("b".to_owned(), 10, 2);
        assert_eq!(recent.get("a", 20, 60), Some(&1));
        // Too old to trust.
        assert_eq!(recent.get("a", 100, 60), None);
        // Putting a key again replaces it and makes it the newest.
        recent.put("a".to_owned(), 30, 3);
        recent.put("c".to_owned(), 40, 4);
        assert_eq!(recent.get("b", 40, 60), None);
        assert_eq!(recent.get("a", 40, 60), Some(&3));
        assert_eq!(recent.get("c", 40, 60), Some(&4));
    }

    #[test]
    fn time_ago() {
        let now = 1_000_000_000;
        assert_eq!(ago(now, now, Locale::PortugueseBrazil), "agora mesmo");
        assert_eq!(ago(now - 59, now, Locale::PortugueseBrazil), "agora mesmo");
        assert_eq!(ago(now - 60, now, Locale::PortugueseBrazil), "há 1 minuto");
        assert_eq!(
            ago(now - 7_200, now, Locale::PortugueseBrazil),
            "há 2 horas"
        );
        assert_eq!(
            ago(now - 86_400 * 3, now, Locale::PortugueseBrazil),
            "há 3 dias"
        );
        assert_eq!(
            ago(now - 86_400 * 14, now, Locale::PortugueseBrazil),
            "há 2 semanas"
        );
        assert_eq!(
            ago(now - 86_400 * 400, now, Locale::PortugueseBrazil),
            "há 1 ano"
        );
        // A clock behind the source's never says "in the future".
        assert_eq!(ago(now + 30, now, Locale::PortugueseBrazil), "agora mesmo");
        assert_eq!(ago(now, now, Locale::Spanish), "ahora mismo");
        assert_eq!(ago(now - 60, now, Locale::Spanish), "hace 1 minuto");
        assert_eq!(ago(now - 7_200, now, Locale::Spanish), "hace 2 horas");
        assert_eq!(ago(now, now, Locale::French), "à l'instant");
        assert_eq!(ago(now - 60, now, Locale::French), "il y a 1 minute");
        assert_eq!(ago(now - 7_200, now, Locale::French), "il y a 2 heures");
        assert_eq!(ago(now, now, Locale::German), "gerade eben");
        assert_eq!(ago(now - 60, now, Locale::German), "vor 1 Minute");
        assert_eq!(ago(now - 7_200, now, Locale::German), "vor 2 Stunden");
        assert_eq!(ago(now, now, Locale::Italian), "adesso");
        assert_eq!(ago(now - 60, now, Locale::Italian), "1 minuto fa");
        assert_eq!(ago(now - 7_200, now, Locale::Italian), "2 ore fa");
    }

    #[test]
    fn durations() {
        assert_eq!(duration(0.0), "0:00");
        assert_eq!(duration(245.9), "4:05");
        assert_eq!(duration(3723.0), "1:02:03");
        assert_eq!(duration(f64::NAN), "0:00");
        assert_eq!(duration(-3.0), "0:00");
    }

    #[test]
    fn view_counts() {
        assert_eq!(views(0, Locale::English), "0 views");
        assert_eq!(views(1, Locale::English), "1 view");
        assert_eq!(views(1_250, Locale::English), "1.2K views");
        assert_eq!(ago(0, 60, Locale::English), "1 minute ago");
        assert_eq!(ago(0, 7200, Locale::English), "2 hours ago");
        assert_eq!(views(1, Locale::PortugueseBrazil), "1 visualização");
        assert_eq!(views(950, Locale::PortugueseBrazil), "950 visualizações");
        assert_eq!(views(1_000, Locale::PortugueseBrazil), "1K visualizações");
        assert_eq!(views(1_250, Locale::PortugueseBrazil), "1.2K visualizações");
        assert_eq!(views(12_400, Locale::PortugueseBrazil), "12K visualizações");
        assert_eq!(
            views(1_200_000, Locale::PortugueseBrazil),
            "1.2M visualizações"
        );
        assert_eq!(
            views(3_000_000_000, Locale::PortugueseBrazil),
            "3B visualizações"
        );
        assert_eq!(views(1, Locale::Spanish), "1 visualización");
        assert_eq!(views(2, Locale::Spanish), "2 visualizaciones");
        assert_eq!(views(1, Locale::French), "1 vue");
        assert_eq!(views(2, Locale::French), "2 vues");
        assert_eq!(views(1, Locale::German), "1 Aufruf");
        assert_eq!(views(2, Locale::German), "2 Aufrufe");
        assert_eq!(views(1, Locale::Italian), "1 visualizzazione");
        assert_eq!(views(2, Locale::Italian), "2 visualizzazioni");
    }

    #[test]
    fn compact_counts() {
        assert_eq!(compact(0), "0");
        assert_eq!(compact(950), "950");
        assert_eq!(compact(34_000), "34K");
        assert_eq!(compact(1_250), "1.2K");
        assert_eq!(compact(1_200_000), "1.2M");
        assert_eq!(compact(3_000_000_000), "3B");
    }

    #[test]
    fn age_buckets() {
        // Use mid-day UTC times so the local zone cannot straddle a day.
        let now = (jiff::Timestamp::now().as_second() / 86_400) * 86_400 + 43_200;
        assert_eq!(age_bucket(Some(now), now), "today");
        assert_eq!(age_bucket(Some(now - 3_600), now), "today");
        assert_eq!(age_bucket(Some(now - 86_400), now), "yesterday");
        assert_eq!(age_bucket(Some(now - 86_400 * 4), now), "this week");
        assert_eq!(age_bucket(Some(now - 86_400 * 15), now), "this month");
        assert_eq!(age_bucket(Some(now - 86_400 * 200), now), "earlier");
        assert_eq!(age_bucket(None, now), "undated");
    }

    #[test]
    fn thousands_separated() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(950), "950");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(1_234_567), "1,234,567");
        assert_eq!(thousands(12_345_678_901), "12,345,678,901");
    }
}
