//! Civil calendar math and UTC formatting.
//!
//! The calendar core is extracted from TimeKit, which carries the most
//! complete variant: `civil_from_days` there is identical to CrashKit's
//! copy (both are Howard Hinnant's civil-from-days algorithm), and
//! TimeKit adds the inverse conversion, leap-year/month-length rules,
//! weekday calculation, and the [`Civil`] broken-down-time struct.
//!
//! Formatting:
//! - [`format_iso8601_utc`] renders `YYYY-MM-DDTHH:MM:SSZ` — the shape
//!   TimeKit's ISO parser accepts and its formatter produces.
//! - [`format_filename_utc_ms`] renders `YYYYMMDD-HHMMSS` — CrashKit's
//!   `Timestamp::filename_part`, used for dump filenames.
//!
//! The FILETIME conversions are the pure halves of the clock code in
//! TimeKit's and CrashKit's platform layers: a Windows FILETIME is a
//! count of 100 ns intervals since 1601-01-01; both DLLs clamp pre-epoch
//! values to 0 rather than returning negative times.

use crate::error::{err, Result, INVALID_ARG};

/// 100 ns intervals between the FILETIME epoch (1601-01-01) and the
/// Unix epoch (1970-01-01).
pub const FILETIME_EPOCH_DIFF_100NS: u64 = 116_444_736_000_000_000;

/// Is `y` a leap year in the proleptic Gregorian calendar?
pub fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Days in month `m` (1–12) of year `y`; 0 for an out-of-range month.
pub fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// (year, month, day) → days since 1970-01-01 (Hinnant's algorithm).
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y_adj = if m <= 2 { y - 1 } else { y };
    let era = if y_adj >= 0 { y_adj } else { y_adj - 399 } / 400;
    let yoe = y_adj - era * 400; // [0, 399]
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64; // Mar=0..Feb=11
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Days since 1970-01-01 → (year, month, day).
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Weekday for a day count since 1970-01-01: 0 = Sunday … 6 = Saturday.
pub fn weekday_from_days(z: i64) -> u32 {
    (z + 4).rem_euclid(7) as u32 // 1970-01-01 was a Thursday (4)
}

/// A broken-down civil date-time (no timezone attached), as in TimeKit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Civil {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl Civil {
    /// Check the fields form a real date/time. A second of 60 is
    /// accepted (leap-second tolerance), matching TimeKit's validation.
    pub fn validate(&self) -> Result<()> {
        if !(1..=12).contains(&self.month)
            || self.day == 0
            || self.day > days_in_month(self.year, self.month)
            || self.hour > 23
            || self.minute > 59
            || self.second > 60
        {
            return err(INVALID_ARG, format!("invalid civil time: {self:?}"));
        }
        Ok(())
    }

    /// Seconds since the Unix epoch *as if* this were UTC (the
    /// wall-clock base TimeKit converts to/from real instants).
    pub fn wall_secs(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day) * 86_400
            + self.hour as i64 * 3600
            + self.minute as i64 * 60
            + self.second as i64
    }
}

/// Break Unix seconds (UTC) down into a [`Civil`].
pub fn civil_from_unix(secs: i64) -> Civil {
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    Civil {
        year,
        month,
        day,
        hour: (tod / 3600) as u32,
        minute: ((tod % 3600) / 60) as u32,
        second: (tod % 60) as u32,
    }
}

/// Break Unix milliseconds (UTC) down into a [`Civil`] plus the
/// millisecond remainder (0–999).
pub fn civil_from_unix_ms(ms: i64) -> (Civil, u32) {
    let secs = ms.div_euclid(1000);
    let rem = ms.rem_euclid(1000) as u32;
    (civil_from_unix(secs), rem)
}

/// Format Unix seconds as ISO-8601 UTC: `YYYY-MM-DDTHH:MM:SSZ`.
pub fn format_iso8601_utc(secs: i64) -> String {
    let c = civil_from_unix(secs);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        c.year, c.month, c.day, c.hour, c.minute, c.second
    )
}

/// Format Unix milliseconds as CrashKit's filename timestamp:
/// `YYYYMMDD-HHMMSS` (UTC, sub-second part dropped).
pub fn format_filename_utc_ms(ms: i64) -> String {
    let (c, _) = civil_from_unix_ms(ms);
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        c.year, c.month, c.day, c.hour, c.minute, c.second
    )
}

/// Unix seconds for a Windows FILETIME value; 0 for pre-epoch values.
/// (The pure half of TimeKit's `now_unix` platform code.)
pub fn unix_secs_from_filetime(ft: u64) -> i64 {
    if ft < FILETIME_EPOCH_DIFF_100NS {
        return 0;
    }
    ((ft - FILETIME_EPOCH_DIFF_100NS) / 10_000_000) as i64
}

/// Unix milliseconds for a Windows FILETIME value; 0 for pre-epoch
/// values. (The pure half of CrashKit's `now_unix_ms` platform code.)
pub fn unix_ms_from_filetime(ft: u64) -> i64 {
    if ft < FILETIME_EPOCH_DIFF_100NS {
        return 0;
    }
    ((ft - FILETIME_EPOCH_DIFF_100NS) / 10_000) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates() {
        // Ported from CrashKit's lib.rs tests.
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_000), (2024, 10, 4));
        assert_eq!(civil_from_days(20_746), (2026, 10, 20));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
    }

    #[test]
    fn civil_roundtrip() {
        // Ported in spirit from TimeKit: days_from_civil inverts
        // civil_from_days across era boundaries and leap days.
        for &(y, m, d) in &[
            (1970, 1, 1),
            (1969, 12, 31),
            (2000, 2, 29),
            (1900, 3, 1),
            (2024, 2, 29),
            (2026, 10, 20),
            (2400, 12, 31),
        ] {
            assert_eq!(civil_from_days(days_from_civil(y, m, d)), (y, m, d));
        }
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2026, 10, 20), 20_746);
    }

    #[test]
    fn leap_and_month_lengths() {
        assert!(is_leap(2000));
        assert!(!is_leap(1900));
        assert!(is_leap(2024));
        assert!(!is_leap(2026));
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2026, 4), 30);
        assert_eq!(days_in_month(2026, 13), 0);
    }

    #[test]
    fn weekdays() {
        assert_eq!(weekday_from_days(0), 4); // 1970-01-01: Thursday
        assert_eq!(weekday_from_days(days_from_civil(2026, 10, 20)), 2); // Tuesday
        assert_eq!(weekday_from_days(days_from_civil(2000, 1, 1)), 6); // Saturday
    }

    #[test]
    fn civil_from_unix_and_validation() {
        let c = civil_from_unix(0);
        assert_eq!(
            (c.year, c.month, c.day, c.hour, c.minute, c.second),
            (1970, 1, 1, 0, 0, 0)
        );
        assert_eq!(civil_from_unix(-1).year, 1969);
        assert!(c.validate().is_ok());
        assert!(Civil {
            year: 2026,
            month: 2,
            day: 29,
            hour: 0,
            minute: 0,
            second: 0
        }
        .validate()
        .is_err());
        assert_eq!(civil_from_unix(c.wall_secs()), c);
    }

    #[test]
    fn unix_ms_breakdown() {
        let base = days_from_civil(2026, 10, 20) * 86_400_000 + 15 * 3_600_000 + 12 * 60_000;
        let (c, rem) = civil_from_unix_ms(base + 345);
        assert_eq!(
            (c.year, c.month, c.day, c.hour, c.minute),
            (2026, 10, 20, 15, 12)
        );
        assert_eq!(rem, 345);
        // Negative instants still break down with a positive remainder.
        let (c, rem) = civil_from_unix_ms(-1);
        assert_eq!((c.year, c.month, c.day, c.second), (1969, 12, 31, 59));
        assert_eq!(rem, 999);
    }

    #[test]
    fn formatting() {
        assert_eq!(format_iso8601_utc(0), "1970-01-01T00:00:00Z");
        let secs = days_from_civil(2026, 10, 20) * 86_400 + 15 * 3600 + 12 * 60;
        assert_eq!(format_iso8601_utc(secs), "2026-10-20T15:12:00Z");
        // CrashKit's filename shape: "20261020-151530".
        let ms = (days_from_civil(2026, 10, 20) * 86_400 + 15 * 3600 + 15 * 60 + 30) * 1000;
        assert_eq!(format_filename_utc_ms(ms), "20261020-151530");
    }

    #[test]
    fn filetime_conversion() {
        assert_eq!(unix_secs_from_filetime(FILETIME_EPOCH_DIFF_100NS), 0);
        assert_eq!(unix_ms_from_filetime(FILETIME_EPOCH_DIFF_100NS), 0);
        assert_eq!(unix_ms_from_filetime(FILETIME_EPOCH_DIFF_100NS + 10_000), 1);
        assert_eq!(
            unix_secs_from_filetime(FILETIME_EPOCH_DIFF_100NS + 10_000_000),
            1
        );
        assert_eq!(unix_secs_from_filetime(0), 0); // clamped, not negative
        assert_eq!(unix_ms_from_filetime(0), 0);
    }
}
