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

#[cfg(test)]
mod tests {
    use super::*;

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
}
