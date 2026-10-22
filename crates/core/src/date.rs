//! Civil dates of the proleptic Gregorian calendar, as a count of days from 1 January 1970.
//! The conversions are the usual civil-from-days arithmetic (Hinnant's), in whole numbers.

use std::fmt;

/// A day, counted from 1 January 1970 (negative before it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date(i32);

/// The day of the week, Monday first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    pub const ALL: [Weekday; 7] =
        [Weekday::Monday, Weekday::Tuesday, Weekday::Wednesday, Weekday::Thursday, Weekday::Friday, Weekday::Saturday, Weekday::Sunday];

    /// Monday is 0, Sunday 6.
    pub fn index(self) -> usize {
        self as usize
    }
}

/// The years a date may be in: far beyond any schedule, and well inside the arithmetic.
const YEARS: std::ops::RangeInclusive<i32> = -9999..=9999;

impl Date {
    /// The date of `year`, `month` (1–12) and `day`; `None` if there is no such day.
    pub fn from_ymd(year: i32, month: u32, day: u32) -> Option<Date> {
        if !YEARS.contains(&year) || !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
            return None;
        }
        let y = i64::from(if month <= 2 { year - 1 } else { year });
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = i64::from(month);
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        i32::try_from(era * 146_097 + doe - 719_468).ok().map(Date)
    }

    /// The day `days` from 1 January 1970, if its year is in range.
    pub fn from_days(days: i32) -> Option<Date> {
        let date = Date(days);
        YEARS.contains(&date.ymd().0).then_some(date)
    }

    /// Days from 1 January 1970.
    pub fn days(self) -> i32 {
        self.0
    }

    /// Year, month (1–12) and day.
    pub fn ymd(self) -> (i32, u32, u32) {
        let z = i64::from(self.0) + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = yoe + era * 400 + i64::from(month <= 2);
        (year as i32, month, day)
    }

    pub fn weekday(self) -> Weekday {
        // 1 January 1970 was a Thursday.
        Weekday::ALL.get((i64::from(self.0) + 3).rem_euclid(7) as usize).copied().unwrap_or(Weekday::Monday)
    }

    /// The next day, if it is in range.
    pub fn succ(self) -> Option<Date> {
        self.0.checked_add(1).and_then(Date::from_days)
    }

    /// The day before, if it is in range.
    pub fn pred(self) -> Option<Date> {
        self.0.checked_sub(1).and_then(Date::from_days)
    }

    /// `yyyy-mm-dd`, as ISO 8601 writes it.
    pub fn parse(text: &str) -> Option<Date> {
        let mut parts = text.trim().splitn(3, '-');
        let year = parts.next()?.parse().ok()?;
        let month = parts.next()?.parse().ok()?;
        let day = parts.next()?.parse().ok()?;
        Date::from_ymd(year, month, day)
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (y, m, d) = self.ymd();
        write!(f, "{y:04}-{m:02}-{d:02}")
    }
}

fn is_leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dates() {
        assert_eq!(Date::from_ymd(1970, 1, 1).map(Date::days), Some(0));
        assert_eq!(Date::from_ymd(2000, 3, 1).map(Date::days), Some(11_017));
        assert_eq!(Date::from_ymd(1969, 12, 31).map(Date::days), Some(-1));
        // 22 October 2026 is a Thursday.
        assert_eq!(Date::from_ymd(2026, 10, 22).map(Date::weekday), Some(Weekday::Thursday));
        assert_eq!(Date::from_ymd(2024, 2, 29).map(|d| d.to_string()).as_deref(), Some("2024-02-29"));
        for bad in [(2023, 2, 29), (2026, 13, 1), (2026, 4, 31), (2026, 1, 0), (10_000, 1, 1)] {
            assert_eq!(Date::from_ymd(bad.0, bad.1, bad.2), None, "{bad:?}");
        }
    }

    #[test]
    fn every_day_of_four_centuries_round_trips() {
        let start = Date::from_ymd(1900, 1, 1).unwrap().days();
        for days in start..start + 146_097 {
            let date = Date::from_days(days).unwrap();
            let (y, m, d) = date.ymd();
            assert_eq!(Date::from_ymd(y, m, d), Some(date));
            assert_eq!(Date::parse(&date.to_string()), Some(date));
        }
    }

    #[test]
    fn the_ends_of_the_range_are_none_not_a_crash() {
        assert_eq!(Date::from_days(i32::MAX), None);
        assert_eq!(Date::from_days(i32::MIN), None);
        let last = Date::from_ymd(9999, 12, 31).unwrap();
        assert_eq!(last.succ(), None);
        assert_eq!(Date::from_ymd(-9999, 1, 1).unwrap().pred(), None);
        assert_eq!(Date::parse("2026-1"), None);
        assert_eq!(Date::parse("x-1-1"), None);
    }
}
