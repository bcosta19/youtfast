//! Small formatting helpers.

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
pub fn views(count: u64) -> String {
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
    if count == 1 {
        "1 view".to_owned()
    } else {
        format!("{number} views")
    }
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
pub fn ago(then: i64, now: i64) -> String {
    let seconds = (now - then).max(0);
    let (count, unit) = match seconds {
        0..60 => return "just now".to_owned(),
        60..3_600 => (seconds / 60, "minute"),
        3_600..86_400 => (seconds / 3_600, "hour"),
        86_400..604_800 => (seconds / 86_400, "day"),
        604_800..2_629_800 => (seconds / 604_800, "week"),
        2_629_800..31_557_600 => (seconds / 2_629_800, "month"),
        _ => (seconds / 31_557_600, "year"),
    };
    if count == 1 {
        format!("1 {unit} ago")
    } else {
        format!("{count} {unit}s ago")
    }
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(ago(now, now), "just now");
        assert_eq!(ago(now - 59, now), "just now");
        assert_eq!(ago(now - 60, now), "1 minute ago");
        assert_eq!(ago(now - 7_200, now), "2 hours ago");
        assert_eq!(ago(now - 86_400 * 3, now), "3 days ago");
        assert_eq!(ago(now - 86_400 * 14, now), "2 weeks ago");
        assert_eq!(ago(now - 86_400 * 400, now), "1 year ago");
        // A clock behind the source's never says "in the future".
        assert_eq!(ago(now + 30, now), "just now");
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
        assert_eq!(views(1), "1 view");
        assert_eq!(views(950), "950 views");
        assert_eq!(views(1_000), "1K views");
        assert_eq!(views(1_250), "1.2K views");
        assert_eq!(views(12_400), "12K views");
        assert_eq!(views(1_200_000), "1.2M views");
        assert_eq!(views(3_000_000_000), "3B views");
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
