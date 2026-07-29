//! Human-readable formatting: sizes and the display shapes of dates.
//!
//! These are pure `->String` helpers that used to sit among the controller's
//! CXX-Qt glue. They compute nothing about time zones or the calendar — that
//! lives in the domain (`siderita_ops` UTC arithmetic, `properties` local time);
//! here we only turn already-known values into text.

/// A byte count as `B`/`KiB`/`MiB`/…, one decimal past the first step.
pub(crate) fn size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// The rounded size plus the exact byte count, for the properties panel.
pub(crate) fn size_full(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} bytes")
    } else {
        format!("{} · {bytes} bytes", size(bytes))
    }
}

/// A system timestamp as a local-time string (via the properties panel's
/// `localtime_r` conversion), or empty if it predates the epoch.
pub(crate) fn system_time(time: std::time::SystemTime) -> String {
    match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(elapsed) => crate::properties::format_time(elapsed.as_secs() as i64),
        Err(_) => String::new(),
    }
}

/// A compact Spanish local timestamp for small summary surfaces:
/// `28 jul 2026 · 19:51`.
pub(crate) fn system_time_short(time: std::time::SystemTime) -> String {
    match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(elapsed) => crate::properties::format_time_short(elapsed.as_secs() as i64),
        Err(_) => String::new(),
    }
}

/// Prettifies a spec `YYYY-MM-DDThh:mm:ss` stamp for display: `T` becomes a
/// space and the seconds are dropped, but an already-short `hh:mm` is left
/// intact and a non-conforming string is returned as-is.
pub(crate) fn trash_date(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let Some((date, time)) = raw.split_once('T') else {
        return raw.to_owned();
    };
    // Drop the seconds only when the time actually carries them (two colons).
    let hm = match (time.find(':'), time.rfind(':')) {
        (Some(first), Some(last)) if first != last => &time[..last],
        _ => time,
    };
    format!("{date} {hm}")
}

/// A freedesktop Trash timestamp as compact Spanish text. Invalid values retain
/// the established lenient formatting instead of disappearing from the UI.
pub(crate) fn trash_date_short(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let normalized = trash_date(raw);
    let Some((date, time)) = normalized.split_once(' ') else {
        return normalized;
    };
    let mut parts = date.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return normalized;
    };
    const MONTHS: [&str; 12] = [
        "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic",
    ];
    let Some(month_name) = month
        .parse::<usize>()
        .ok()
        .and_then(|value| value.checked_sub(1))
        .and_then(|index| MONTHS.get(index))
    else {
        return normalized;
    };
    let day = day
        .parse::<u8>()
        .map_or_else(|_| day.to_owned(), |value| value.to_string());
    format!("{day} {month_name} {year} · {time}")
}

/// Just the date half of a `YYYY-MM-DDThh:mm:ss` stamp — everything before the
/// `T`, or the whole string if there is none.
pub(crate) fn date_only(raw: &str) -> &str {
    raw.split('T').next().unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::{date_only, size, size_full, trash_date, trash_date_short};

    #[test]
    fn size_steps_up_at_1024_with_one_decimal() {
        assert_eq!(size(512), "512 B");
        assert_eq!(size(1024), "1.0 KiB");
        assert_eq!(size(1536), "1.5 KiB");
    }

    #[test]
    fn size_full_keeps_the_exact_count() {
        assert_eq!(size_full(512), "512 bytes");
        assert_eq!(size_full(1024), "1.0 KiB · 1024 bytes");
    }

    #[test]
    fn trash_date_is_compact_and_lenient() {
        assert_eq!(trash_date("2026-07-21T18:04:09"), "2026-07-21 18:04");
        assert_eq!(trash_date("2026-07-21T18:04"), "2026-07-21 18:04");
        assert_eq!(trash_date(""), "");
        assert_eq!(trash_date("desconocido"), "desconocido");
    }

    #[test]
    fn trash_date_short_uses_an_abbreviated_spanish_month() {
        assert_eq!(
            trash_date_short("2026-07-21T18:04:09"),
            "21 jul 2026 · 18:04"
        );
        assert_eq!(trash_date_short("desconocido"), "desconocido");
        assert_eq!(trash_date_short(""), "");
    }

    #[test]
    fn date_only_takes_the_date_half() {
        assert_eq!(date_only("2026-07-21T18:04:09"), "2026-07-21");
        assert_eq!(date_only("2026-07-21"), "2026-07-21");
    }
}
