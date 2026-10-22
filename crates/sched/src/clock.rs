//! Time on one calendar. A time is a boundary between calendar days, counted from 1 January
//! 1970: `t` is the morning of day `t` and the evening of day `t − 1`. A working day of the
//! calendar runs from its morning to its evening.

use mkperth_core::{Calendar, CalendarError, Date};

use crate::ScheduleError;

/// How many idle days a search may cross before it gives up: years of holidays, never an
/// endless walk.
const SEARCH: i64 = 3660;

pub(crate) struct Clock<'a> {
    pub(crate) calendar: &'a Calendar,
}

impl Clock<'_> {
    fn none(&self) -> ScheduleError {
        ScheduleError::Calendar(CalendarError::NoWorkingDays(self.calendar.name.clone()))
    }

    /// Whether day `day` is worked.
    pub(crate) fn working(&self, day: i64) -> Result<bool, ScheduleError> {
        let date = i32::try_from(day).ok().and_then(Date::from_days).ok_or(ScheduleError::TooLong)?;
        Ok(self.calendar.is_working(date))
    }

    /// The first working day at or after `day`.
    fn day_at_or_after(&self, day: i64) -> Result<i64, ScheduleError> {
        for d in day..=day + SEARCH {
            if self.working(d)? {
                return Ok(d);
            }
        }
        Err(self.none())
    }

    /// The last working day at or before `day`.
    fn day_at_or_before(&self, day: i64) -> Result<i64, ScheduleError> {
        for d in (day - SEARCH..=day).rev() {
            if self.working(d)? {
                return Ok(d);
            }
        }
        Err(self.none())
    }

    /// The earliest morning of a working day at or after `t`.
    pub(crate) fn next_start(&self, t: i64) -> Result<i64, ScheduleError> {
        self.day_at_or_after(t)
    }

    /// The latest morning of a working day at or before `t`.
    pub(crate) fn previous_start(&self, t: i64) -> Result<i64, ScheduleError> {
        self.day_at_or_before(t)
    }

    /// The earliest evening of a working day at or after `t`.
    pub(crate) fn next_finish(&self, t: i64) -> Result<i64, ScheduleError> {
        Ok(self.day_at_or_after(t - 1)? + 1)
    }

    /// The latest evening of a working day at or before `t`.
    pub(crate) fn previous_finish(&self, t: i64) -> Result<i64, ScheduleError> {
        Ok(self.day_at_or_before(t - 1)? + 1)
    }

    /// The evening after `n` working days, counted from the first at or after `t`; `t` itself
    /// for none.
    pub(crate) fn advance(&self, t: i64, n: i64) -> Result<i64, ScheduleError> {
        if n <= 0 {
            return Ok(t);
        }
        let mut d = self.day_at_or_after(t)?;
        for _ in 1..n {
            d = self.day_at_or_after(d + 1)?;
        }
        Ok(d + 1)
    }

    /// The morning of the `n`th working day back from `t`; `t` itself for none.
    pub(crate) fn retreat(&self, t: i64, n: i64) -> Result<i64, ScheduleError> {
        if n <= 0 {
            return Ok(t);
        }
        let mut d = self.day_at_or_before(t - 1)?;
        for _ in 1..n {
            d = self.day_at_or_before(d - 1)?;
        }
        Ok(d)
    }

    /// `t` moved by `n` working days: forward for a lag, back for a lead.
    pub(crate) fn shift(&self, t: i64, n: i64) -> Result<i64, ScheduleError> {
        if n >= 0 { self.advance(t, n) } else { self.retreat(t, -n) }
    }

    /// Working days from `a` to `b`: negative when `b` comes first.
    pub(crate) fn between(&self, a: i64, b: i64) -> Result<i64, ScheduleError> {
        let (from, to, sign) = if a <= b { (a, b, 1) } else { (b, a, -1) };
        let mut count = 0;
        for day in from..to {
            if self.working(day)? {
                count += 1;
            }
        }
        Ok(sign * count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(y: i32, m: u32, d: u32) -> i64 {
        i64::from(Date::from_ymd(y, m, d).unwrap().days())
    }

    #[test]
    fn mornings_evenings_and_shifts_on_a_five_day_week() {
        let calendar = Calendar::default();
        let c = Clock { calendar: &calendar };
        // Saturday 10 October 2026: the next working morning is Monday the 12th, the last
        // evening before it Friday the 9th's (the morning of Saturday the 10th).
        assert_eq!(c.next_start(t(2026, 10, 10)).unwrap(), t(2026, 10, 12));
        assert_eq!(c.previous_finish(t(2026, 10, 12)).unwrap(), t(2026, 10, 10));
        assert_eq!(c.next_finish(t(2026, 10, 11)).unwrap(), t(2026, 10, 13));
        assert_eq!(c.previous_start(t(2026, 10, 11)).unwrap(), t(2026, 10, 9));
        // Five working days from Monday the 5th: the evening of Friday the 9th.
        assert_eq!(c.advance(t(2026, 10, 5), 5).unwrap(), t(2026, 10, 10));
        assert_eq!(c.retreat(t(2026, 10, 10), 5).unwrap(), t(2026, 10, 5));
        assert_eq!(c.shift(t(2026, 10, 10), -2).unwrap(), t(2026, 10, 8));
        assert_eq!(c.between(t(2026, 10, 5), t(2026, 10, 12)).unwrap(), 5);
        assert_eq!(c.between(t(2026, 10, 12), t(2026, 10, 5)).unwrap(), -5);
    }

    #[test]
    fn no_working_day_and_the_ends_of_time_are_errors() {
        let calendar = Calendar { week: [false; 7], ..Calendar::default() };
        assert!(Clock { calendar: &calendar }.next_start(0).is_err());
        let calendar = Calendar::default();
        assert_eq!(Clock { calendar: &calendar }.next_start(i64::from(i32::MAX)), Err(ScheduleError::TooLong));
    }
}
