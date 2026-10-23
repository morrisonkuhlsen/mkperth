//! A work calendar: the days of the week that are worked and the hours worked in each (its work
//! periods), and the exceptions (holidays off, and days worked although the week says not).
//! Durations count its working time.

use std::collections::{BTreeMap, BTreeSet};

use mkperth_i18n::tr;
use thiserror::Error;

use crate::{Date, Region};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CalendarError {
    /// A calendar with no working day at all can place nothing.
    #[error("{}", tr!("err-calendar-no-working-days", name = .0.as_str()))]
    NoWorkingDays(String),
    /// A date past the end of the calendar's range.
    #[error("{}", tr!("err-calendar-out-of-range", name = .0.as_str()))]
    OutOfRange(String),
}

/// A work period within a day: from and to, in minutes after midnight.
pub type Period = (u16, u16);

/// The periods of a working day when a calendar doesn't say: 08:00 to 12:00 and 13:00 to 17:00,
/// eight hours.
pub const DEFAULT_PERIODS: [Period; 2] = [(8 * 60, 12 * 60), (13 * 60, 17 * 60)];

/// The minutes in a day.
pub const MINUTES_PER_DAY: i64 = 1440;

/// How many days a search for a working day may look through before it gives up: years of
/// exceptions, never an endless walk.
const SEARCH: i32 = 3660;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Calendar {
    pub name: String,
    /// Worked days of the week, Monday first.
    pub week: [bool; 7],
    /// Days off although the week works them.
    pub holidays: BTreeSet<Date>,
    /// Days worked although the week doesn't, on their weekday's periods.
    pub extra_days: BTreeSet<Date>,
    /// Days worked on periods of their own (a short Christmas Eve, a Saturday morning): they
    /// override the week, the holidays and the region.
    pub exceptions: BTreeMap<Date, Vec<Period>>,
    /// A region whose holidays are off too, computed from their rules for any year.
    pub region: Option<Region>,
    /// The periods worked on each day of the week, Monday first, in order; a worked day with
    /// none works the default periods.
    pub periods: [Vec<Period>; 7],
}

impl Default for Calendar {
    /// Monday to Friday, no holidays.
    fn default() -> Self {
        Calendar {
            name: "Standard".into(),
            week: [true, true, true, true, true, false, false],
            holidays: BTreeSet::new(),
            extra_days: BTreeSet::new(),
            exceptions: BTreeMap::new(),
            region: None,
            periods: Default::default(),
        }
    }
}

impl Calendar {
    /// Every day worked: the 24-hour calendar lags can count on.
    pub fn every_day() -> Calendar {
        Calendar { name: "24 Hour".into(), week: [true; 7], ..Calendar::default() }
    }

    /// Around the clock, every day: the 24-hour calendar lags can count on.
    pub fn around_the_clock() -> Calendar {
        Calendar { name: "24 Hour".into(), week: [true; 7], periods: std::array::from_fn(|_| vec![(0, 1440)]), ..Calendar::default() }
    }

    /// The periods a weekday works when it works (the default ones if it names none).
    pub fn weekday_periods(&self, weekday: usize) -> &[Period] {
        match self.periods.get(weekday) {
            Some(p) if !p.is_empty() => p,
            _ => &DEFAULT_PERIODS,
        }
    }

    /// The periods `date` works when it works: its own, as an exception, or its weekday's.
    pub fn day_periods(&self, date: Date) -> &[Period] {
        match self.exceptions.get(&date) {
            Some(p) if !p.is_empty() => p,
            _ => self.weekday_periods(date.weekday().index()),
        }
    }

    /// The periods worked on `date`: none on a day off.
    pub fn periods_on(&self, date: Date) -> &[Period] {
        if self.is_working(date) { self.day_periods(date) } else { &[] }
    }

    /// The minutes of a working day, by which days of duration become working time: the first
    /// worked weekday's.
    pub fn day_minutes(&self) -> i64 {
        let weekday = self.week.iter().position(|w| *w).unwrap_or(0);
        let minutes: i64 = self.weekday_periods(weekday).iter().map(|(a, b)| i64::from(*b) - i64::from(*a)).sum();
        if minutes > 0 { minutes } else { 480 }
    }

    /// Whether it has any working day at all.
    pub fn works(&self) -> bool {
        self.week.iter().any(|w| *w) || !self.extra_days.is_empty() || self.exceptions.values().any(|p| !p.is_empty())
    }

    pub fn is_working(&self, date: Date) -> bool {
        if let Some(p) = self.exceptions.get(&date) {
            return !p.is_empty();
        }
        if self.extra_days.contains(&date) {
            return true;
        }
        if self.holidays.contains(&date) || self.region.is_some_and(|r| r.is_holiday(date)) {
            return false;
        }
        self.week.get(date.weekday().index()).copied().unwrap_or(false)
    }

    fn check(&self) -> Result<(), CalendarError> {
        if self.works() { Ok(()) } else { Err(CalendarError::NoWorkingDays(self.name.clone())) }
    }

    /// `date` if it is a working day, else the next one.
    pub fn next_working(&self, date: Date) -> Result<Date, CalendarError> {
        self.check()?;
        let mut d = date;
        for _ in 0..SEARCH {
            if self.is_working(d) {
                return Ok(d);
            }
            d = d.succ().ok_or_else(|| CalendarError::OutOfRange(self.name.clone()))?;
        }
        Err(CalendarError::NoWorkingDays(self.name.clone()))
    }

    /// `date` if it is a working day, else the one before.
    pub fn previous_working(&self, date: Date) -> Result<Date, CalendarError> {
        self.check()?;
        let mut d = date;
        for _ in 0..SEARCH {
            if self.is_working(d) {
                return Ok(d);
            }
            d = d.pred().ok_or_else(|| CalendarError::OutOfRange(self.name.clone()))?;
        }
        Err(CalendarError::NoWorkingDays(self.name.clone()))
    }

    /// The working day `n` working days after working day `from` (`n` may be negative: before
    /// it). `from` itself is moved to the next working day first.
    pub fn add_working_days(&self, from: Date, n: i64) -> Result<Date, CalendarError> {
        let mut d = self.next_working(from)?;
        let step = |d: Date| if n >= 0 { d.succ() } else { d.pred() };
        let mut left = n.unsigned_abs();
        let mut idle = 0;
        while left > 0 {
            d = step(d).ok_or_else(|| CalendarError::OutOfRange(self.name.clone()))?;
            if self.is_working(d) {
                left -= 1;
                idle = 0;
            } else {
                idle += 1;
                if idle > SEARCH {
                    return Err(CalendarError::NoWorkingDays(self.name.clone()));
                }
            }
        }
        Ok(d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: u32, d: u32) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    #[test]
    fn weekdays_holidays_and_extra_days() {
        let mut cal = Calendar::default();
        // Friday 2 October 2026 → the next working day after the weekend is Monday the 5th.
        assert_eq!(cal.next_working(day(2026, 10, 3)), Ok(day(2026, 10, 5)));
        assert_eq!(cal.add_working_days(day(2026, 10, 2), 1), Ok(day(2026, 10, 5)));
        assert_eq!(cal.add_working_days(day(2026, 10, 5), -1), Ok(day(2026, 10, 2)));
        assert_eq!(cal.add_working_days(day(2026, 10, 5), 10), Ok(day(2026, 10, 19)));
        // 12 October (a Monday) off: ten days after the 5th move to the 20th.
        cal.holidays.insert(day(2026, 10, 12));
        assert_eq!(cal.add_working_days(day(2026, 10, 5), 10), Ok(day(2026, 10, 20)));
        // Saturday the 10th worked.
        cal.extra_days.insert(day(2026, 10, 10));
        assert_eq!(cal.add_working_days(day(2026, 10, 9), 1), Ok(day(2026, 10, 10)));
    }

    #[test]
    fn a_calendar_that_works_no_day_is_an_error_not_a_hang() {
        let cal = Calendar { week: [false; 7], ..Calendar::default() };
        assert!(matches!(cal.next_working(day(2026, 1, 1)), Err(CalendarError::NoWorkingDays(_))));
        // Every weekday a holiday for years: the search stops.
        let mut cal = Calendar { week: [true, false, false, false, false, false, false], ..Calendar::default() };
        let mut d = day(2026, 1, 1);
        for _ in 0..4000 {
            cal.holidays.insert(d);
            d = d.succ().unwrap();
        }
        assert!(cal.next_working(day(2026, 1, 1)).is_err());
        assert!(Calendar::default().add_working_days(day(9999, 12, 30), 5).is_err());
    }
}
