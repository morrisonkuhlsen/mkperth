//! A work calendar: the days of the week that are worked, and the exceptions to them (holidays
//! off, and days worked although the week says not). Durations count its working days.

use std::collections::BTreeSet;

use mkperth_i18n::tr;
use thiserror::Error;

use crate::Date;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CalendarError {
    /// A calendar with no working day at all can place nothing.
    #[error("{}", tr!("err-calendar-no-working-days", name = .0.as_str()))]
    NoWorkingDays(String),
    /// A date past the end of the calendar's range.
    #[error("{}", tr!("err-calendar-out-of-range", name = .0.as_str()))]
    OutOfRange(String),
}

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
    /// Days worked although the week doesn't.
    pub extra_days: BTreeSet<Date>,
}

impl Default for Calendar {
    /// Monday to Friday, no holidays.
    fn default() -> Self {
        Calendar {
            name: "Standard".into(),
            week: [true, true, true, true, true, false, false],
            holidays: BTreeSet::new(),
            extra_days: BTreeSet::new(),
        }
    }
}

impl Calendar {
    pub fn is_working(&self, date: Date) -> bool {
        if self.extra_days.contains(&date) {
            return true;
        }
        if self.holidays.contains(&date) {
            return false;
        }
        self.week.get(date.weekday().index()).copied().unwrap_or(false)
    }

    fn check(&self) -> Result<(), CalendarError> {
        if self.week.iter().any(|w| *w) || !self.extra_days.is_empty() { Ok(()) } else { Err(CalendarError::NoWorkingDays(self.name.clone())) }
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
