//! Minimal UTC timestamps without a date library.

use std::time::{SystemTime, UNIX_EPOCH};

/// Unix seconds and nanoseconds.
pub(crate) type Instant = (i64, u32);

pub(crate) fn now() -> Instant {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| {
            (
                i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
                d.subsec_nanos(),
            )
        })
        .unwrap_or((0, 0))
}

/// The current time as RFC 3339, for timestamps outside core (activity log, chats).
pub fn now_rfc3339() -> String {
    format(now())
}

/// Formats as `2026-10-11T13:41:02.123Z`.
pub(crate) fn format((secs, nanos): Instant) -> String {
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60,
        nanos / 1_000_000
    )
}

/// Unix seconds of a timestamp as [`format`] writes it (`2026-10-11T13:41:02.123Z`); the
/// fraction is dropped. `None` for anything else.
pub(crate) fn parse(text: &str) -> Option<i64> {
    let (date, clock) = text.strip_suffix('Z')?.split_once('T')?;
    let clock = clock.split('.').next()?;
    let number = |part: Option<&str>, digits: usize| {
        part.filter(|p| p.len() == digits)
            .and_then(|p| p.parse::<i64>().ok())
    };
    let mut ymd = date.split('-');
    let (year, month, day) = (
        number(ymd.next(), 4)?,
        number(ymd.next(), 2)?,
        number(ymd.next(), 2)?,
    );
    let mut hms = clock.split(':');
    let (hour, minute, second) = (
        number(hms.next(), 2)?,
        number(hms.next(), 2)?,
        number(hms.next(), 2)?,
    );
    let valid = (1..=12).contains(&month)
        && (1..=31).contains(&day)
        && hour < 24
        && minute < 60
        && second < 61;
    valid.then(|| from_civil(year, month, day, hour, minute, second))
}

/// Unix seconds for a UTC date and time (proleptic Gregorian, Howard Hinnant's algorithm).
pub(crate) fn from_civil(
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let year_of_era = y - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    days * 86_400 + hour * 3600 + minute * 60 + second
}

/// Year, month, day for days since 1970-01-01 (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_civil_inverts_format() {
        let secs = from_civil(2026, 10, 11, 13, 41, 2);
        assert_eq!(format((secs, 0)), "2026-10-11T13:41:02.000Z");
        assert_eq!(from_civil(1970, 1, 1, 0, 0, 0), 0);
    }

    #[test]
    fn parse_reads_what_format_writes() {
        let secs = 1_791_726_062;
        assert_eq!(parse(&format((secs, 123_456_789))), Some(secs));
        assert_eq!(parse("2026-13-01T00:00:00.000Z"), None);
        assert_eq!(parse("1791726062"), None);
    }

    #[test]
    fn formats_rfc3339_with_millis() {
        assert_eq!(format((0, 0)), "1970-01-01T00:00:00.000Z");
        assert_eq!(
            format((1_791_726_062, 123_456_789)),
            "2026-10-11T13:41:02.123Z"
        );
    }
}
