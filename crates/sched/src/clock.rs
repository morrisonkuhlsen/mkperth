//! Time on one calendar, in minutes from midnight on 1 January 1970. The calendar's work periods
//! are the working time: an activity starts at a working minute and finishes at the end of its
//! last one. Moments with no working time between them are the same moment for the schedule
//! (Friday 17:00 and Monday 08:00 on a five-day week); the functions here pick the one each use
//! needs.

use mkperth_core::{Calendar, CalendarError, Date, MINUTES_PER_DAY, Period};

use crate::ScheduleError;

/// How many idle days a search may cross before it gives up: years of holidays, never an
/// endless walk.
const SEARCH: i64 = 3660;

pub(crate) struct Clock<'a> {
    pub(crate) calendar: &'a Calendar,
}

fn day_of(t: i64) -> (i64, i64) {
    (t.div_euclid(MINUTES_PER_DAY), t.rem_euclid(MINUTES_PER_DAY))
}

impl Clock<'_> {
    fn none(&self) -> ScheduleError {
        ScheduleError::Calendar(CalendarError::NoWorkingDays(self.calendar.name.clone()))
    }

    fn date(day: i64) -> Result<Date, ScheduleError> {
        i32::try_from(day).ok().and_then(Date::from_days).ok_or(ScheduleError::TooLong)
    }

    /// The periods worked on day `day`, in minutes: none on a day off.
    pub(crate) fn periods(&self, day: i64) -> Result<&[Period], ScheduleError> {
        Ok(self.calendar.periods_on(Self::date(day)?))
    }

    /// The minutes worked on day `day`.
    pub(crate) fn day_work(&self, day: i64) -> Result<i64, ScheduleError> {
        Ok(self.periods(day)?.iter().map(|(a, b)| i64::from(*b) - i64::from(*a)).sum())
    }

    /// The minutes worked on `t`'s day before `t`.
    pub(crate) fn worked_before(&self, t: i64) -> Result<i64, ScheduleError> {
        let (day, m) = day_of(t);
        Ok(self.periods(day)?.iter().map(|(a, b)| (m.min(i64::from(*b)) - i64::from(*a)).max(0)).sum())
    }

    /// The first working minute at or after `t`.
    pub(crate) fn next_start(&self, t: i64) -> Result<i64, ScheduleError> {
        let (day, m) = day_of(t);
        for d in day..=day + SEARCH {
            let from = if d == day { m } else { 0 };
            if let Some((a, _)) = self.periods(d)?.iter().find(|(_, b)| i64::from(*b) > from) {
                return Ok(d * MINUTES_PER_DAY + from.max(i64::from(*a)));
            }
        }
        Err(self.none())
    }

    /// The end of the last working time at or before `t` (`t` itself inside a period).
    pub(crate) fn previous_finish(&self, t: i64) -> Result<i64, ScheduleError> {
        let (day, m) = day_of(t);
        for d in (day - SEARCH..=day).rev() {
            let to = if d == day { m } else { MINUTES_PER_DAY };
            if let Some((_, b)) = self.periods(d)?.iter().rev().find(|(a, _)| i64::from(*a) < to) {
                return Ok(d * MINUTES_PER_DAY + to.min(i64::from(*b)));
            }
        }
        Err(self.none())
    }

    /// A finish no earlier than `t`: `t` itself when working time ends there or runs through it,
    /// else the next working minute's start (the same moment as the last finish before).
    pub(crate) fn next_finish(&self, t: i64) -> Result<i64, ScheduleError> {
        let (day, m) = day_of(t);
        let inside = self.periods(day)?.iter().any(|(a, b)| i64::from(*a) < m && m <= i64::from(*b));
        // Midnight ends the day before's last period when it runs to 24:00.
        let at_midnight = m == 0 && self.periods(day - 1)?.iter().any(|(_, b)| i64::from(*b) == MINUTES_PER_DAY);
        if inside || at_midnight { Ok(t) } else { self.next_start(t) }
    }

    /// Whether `t` is a working minute.
    fn working_at(&self, t: i64) -> Result<bool, ScheduleError> {
        let (day, m) = day_of(t);
        Ok(self.periods(day)?.iter().any(|(a, b)| i64::from(*a) <= m && m < i64::from(*b)))
    }

    /// The latest working minute at or before `t`, strictly: for a date a start must not pass,
    /// even when no work lies between.
    pub(crate) fn start_by(&self, t: i64) -> Result<i64, ScheduleError> {
        if self.working_at(t)? { Ok(t) } else { Ok(self.previous_finish(t)? - 1) }
    }

    /// The end of `n` minutes of work counted from the first working minute at or after `t`; `t`
    /// itself for none.
    pub(crate) fn advance(&self, t: i64, n: i64) -> Result<i64, ScheduleError> {
        if n <= 0 {
            return Ok(t);
        }
        let mut at = self.next_start(t)?;
        let mut left = n;
        let mut idle = 0;
        loop {
            let (day, m) = day_of(at);
            let mut worked_today = false;
            for (a, b) in self.periods(day)? {
                let (a, b) = (i64::from(*a), i64::from(*b));
                if b <= m {
                    continue;
                }
                let from = m.max(a);
                let room = b - from;
                if left <= room {
                    return Ok(day * MINUTES_PER_DAY + from + left);
                }
                left -= room;
                worked_today = true;
            }
            idle = if worked_today { 0 } else { idle + 1 };
            if idle > SEARCH {
                return Err(self.none());
            }
            at = (day + 1) * MINUTES_PER_DAY;
        }
    }

    /// The start of `n` minutes of work counted back from `t`; `t` itself for none.
    pub(crate) fn retreat(&self, t: i64, n: i64) -> Result<i64, ScheduleError> {
        if n <= 0 {
            return Ok(t);
        }
        let mut at = self.previous_finish(t)?;
        let mut left = n;
        let mut idle = 0;
        loop {
            let (mut day, mut m) = day_of(at);
            // The end of a day's last period at 24:00 is its next midnight.
            if m == 0 {
                day -= 1;
                m = MINUTES_PER_DAY;
            }
            let mut worked_today = false;
            for (a, b) in self.periods(day)?.iter().rev() {
                let (a, b) = (i64::from(*a), i64::from(*b));
                if a >= m {
                    continue;
                }
                let to = m.min(b);
                let room = to - a;
                if left <= room {
                    return Ok(day * MINUTES_PER_DAY + to - left);
                }
                left -= room;
                worked_today = true;
            }
            idle = if worked_today { 0 } else { idle + 1 };
            if idle > SEARCH {
                return Err(self.none());
            }
            at = day * MINUTES_PER_DAY;
        }
    }

    /// `t` moved by `n` minutes of work: forward for a lag, back for a lead.
    pub(crate) fn shift(&self, t: i64, n: i64) -> Result<i64, ScheduleError> {
        if n >= 0 { self.advance(t, n) } else { self.retreat(t, -n) }
    }

    /// Minutes of work from `a` to `b`: negative when `b` comes first. Walks the days between;
    /// the scheduler counts with a running total instead.
    pub(crate) fn between(&self, a: i64, b: i64) -> Result<i64, ScheduleError> {
        let (from, to, sign) = if a <= b { (a, b, 1) } else { (b, a, -1) };
        let (d0, _) = day_of(from);
        let (d1, _) = day_of(to);
        let mut total = 0;
        for d in d0..d1 {
            total += self.day_work(d)?;
        }
        Ok(sign * (total + self.worked_before(to)? - self.worked_before(from)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(y: i32, m: u32, d: u32, h: i64, min: i64) -> i64 {
        i64::from(Date::from_ymd(y, m, d).unwrap().days()) * MINUTES_PER_DAY + h * 60 + min
    }

    #[test]
    fn minutes_on_a_five_day_week_of_eight_hours() {
        let calendar = Calendar::default();
        let c = Clock { calendar: &calendar };
        // Saturday 10 October 2026: the next working minute is Monday the 12th at 08:00, the last
        // finish before it Friday the 9th at 17:00.
        assert_eq!(c.next_start(t(2026, 10, 10, 0, 0)).unwrap(), t(2026, 10, 12, 8, 0));
        assert_eq!(c.previous_finish(t(2026, 10, 12, 8, 0)).unwrap(), t(2026, 10, 9, 17, 0));
        // Lunch is not work: 12:30 starts at 13:00.
        assert_eq!(c.next_start(t(2026, 10, 12, 12, 30)).unwrap(), t(2026, 10, 12, 13, 0));
        // Five days of eight hours from Monday the 5th: Friday the 9th at 17:00, and back.
        assert_eq!(c.advance(t(2026, 10, 5, 0, 0), 5 * 480).unwrap(), t(2026, 10, 9, 17, 0));
        assert_eq!(c.retreat(t(2026, 10, 9, 17, 0), 5 * 480).unwrap(), t(2026, 10, 5, 8, 0));
        // Half a day from Monday 08:00 ends at 12:00; another half starts after lunch.
        assert_eq!(c.advance(t(2026, 10, 5, 8, 0), 240).unwrap(), t(2026, 10, 5, 12, 0));
        assert_eq!(c.advance(t(2026, 10, 5, 12, 0), 240).unwrap(), t(2026, 10, 5, 17, 0));
        assert_eq!(c.between(t(2026, 10, 5, 0, 0), t(2026, 10, 12, 0, 0)).unwrap(), 5 * 480);
        assert_eq!(c.between(t(2026, 10, 5, 10, 0), t(2026, 10, 5, 14, 0)).unwrap(), 180);
        assert_eq!(c.between(t(2026, 10, 12, 0, 0), t(2026, 10, 5, 0, 0)).unwrap(), -5 * 480);
        assert_eq!(c.next_finish(t(2026, 10, 9, 17, 0)).unwrap(), t(2026, 10, 9, 17, 0));
        assert_eq!(c.next_finish(t(2026, 10, 10, 0, 0)).unwrap(), t(2026, 10, 12, 8, 0));
        assert_eq!(c.start_by(t(2026, 10, 10, 0, 0)).unwrap(), t(2026, 10, 9, 16, 59));
    }

    #[test]
    fn around_the_clock_and_short_days() {
        let calendar = Calendar::around_the_clock();
        let c = Clock { calendar: &calendar };
        // Two days around the clock from Friday 17:00: Sunday 17:00.
        assert_eq!(c.advance(t(2026, 10, 9, 17, 0), 2 * 1440).unwrap(), t(2026, 10, 11, 17, 0));
        assert_eq!(c.retreat(t(2026, 10, 11, 17, 0), 2 * 1440).unwrap(), t(2026, 10, 9, 17, 0));
        assert_eq!(c.retreat(t(2026, 10, 12, 0, 0), 1440).unwrap(), t(2026, 10, 11, 0, 0));
        // A short Friday: four hours.
        let mut short = Calendar::default();
        short.periods[4] = vec![(8 * 60, 12 * 60)];
        let c = Clock { calendar: &short };
        assert_eq!(c.advance(t(2026, 10, 8, 8, 0), 480 + 240).unwrap(), t(2026, 10, 9, 12, 0));
        assert_eq!(c.advance(t(2026, 10, 8, 8, 0), 480 + 300).unwrap(), t(2026, 10, 12, 9, 0));
    }

    #[test]
    fn no_working_day_and_the_ends_of_time_are_errors() {
        let calendar = Calendar { week: [false; 7], ..Calendar::default() };
        assert!(Clock { calendar: &calendar }.next_start(0).is_err());
        let calendar = Calendar::default();
        assert_eq!(Clock { calendar: &calendar }.next_start(i64::from(i32::MAX) * MINUTES_PER_DAY), Err(ScheduleError::TooLong));
    }
}
