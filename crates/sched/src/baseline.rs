//! The schedule against a baseline: each activity's variances, as P6 counts them (the baseline's
//! less the schedule's, so a negative variance is late): its start and its finish in working
//! days of its calendar, its duration in working minutes.

use mkperth_core::{Calendar, Date, Project};

use crate::{MAX_SPAN, Schedule, usage::calendar_named};

/// An activity's variances against the baseline in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Variance {
    /// Working days; negative when it starts (finishes) later than the baseline.
    pub start: i64,
    pub finish: i64,
    /// Working minutes; negative when it takes longer.
    pub duration: i64,
}

/// The working days of `calendar` from `from` to `to`: those in [from, to) when `to` is later,
/// else less those in [to, from).
pub fn working_days_between(calendar: &Calendar, from: Date, to: Date) -> i64 {
    let (lo, hi, sign) = if to >= from { (from, to, 1) } else { (to, from, -1) };
    let (mut d, mut n, mut steps) = (lo, 0i64, 0i64);
    while d < hi && steps < MAX_SPAN * 4 {
        n += i64::from(calendar.is_working(d));
        steps += 1;
        match d.succ() {
            Some(next) => d = next,
            None => break,
        }
    }
    sign * n
}

/// Each activity's variances against the project's baseline in use, in the project's order:
/// `None` for an activity the baseline doesn't have, or when no baseline is in use.
pub fn variances(project: &Project, schedule: &Schedule) -> Vec<Option<Variance>> {
    let Some(baseline) = project.baseline() else { return vec![None; project.activities.len()] };
    let by_id: std::collections::HashMap<&str, &mkperth_core::BaselineActivity> = baseline.activities.iter().map(|b| (b.id.as_str(), b)).collect();
    project
        .activities
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let (b, s) = (by_id.get(a.id.as_str())?, schedule.activities.get(i)?);
            let calendar = calendar_named(project, a.calendar.as_deref());
            Some(Variance {
                start: working_days_between(calendar, s.early_start, b.start),
                finish: working_days_between(calendar, s.early_finish, b.finish),
                duration: b.duration - a.working_minutes(calendar.day_minutes()),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use mkperth_core::{Activity, Baseline, BaselineActivity, Constraint};

    use super::*;

    fn day(y: i32, m: u32, d: u32) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    /// A takes 3 days from Monday 5 October 2026 (Monday to Friday): the 5th to the 7th, frozen
    /// as the baseline. Started on or after Thursday the 8th, it is the 8th to Monday the 12th:
    /// the 5th, 6th and 7th lie between both starts, and the 7th, 8th and 9th between both
    /// finishes, so both variances are −3, its duration's 0. Four days long, it finishes on the
    /// 13th: a finish variance of −4, a duration variance of −1 day (480 minutes).
    #[test]
    fn variances_by_hand() {
        let mut p = Project {
            name: "baseline".into(),
            start: day(2026, 10, 5),
            calendar: Calendar::default(),
            calendars: Vec::new(),
            lag_calendar: Default::default(),
            float_kind: Default::default(),
            critical_by: Default::default(),
            data_date: None,
            progress_mode: Default::default(),
            must_finish_by: None,
            activities: vec![
                Activity { id: "A".into(), duration: 3, ..Activity::default() },
                Activity { id: "N".into(), duration: 1, ..Activity::default() },
            ],
            relationships: Vec::new(),
            bands: Vec::new(),
            markers: Vec::new(),
            month_marks: Vec::new(),
            resources: Vec::new(),
            roles: Vec::new(),
            assignments: Vec::new(),
            comments: Vec::new(),
            levelling: Default::default(),
            baselines: Vec::new(),
            baseline_in_use: None,
            extra: Default::default(),
        };
        assert_eq!(variances(&p, &crate::schedule(&p).unwrap()), [None, None], "no baseline in use");
        p.baselines.push(Baseline {
            name: "BL1".into(),
            activities: vec![BaselineActivity { id: "A".into(), start: day(2026, 10, 5), finish: day(2026, 10, 7), duration: 3 * 480 }],
        });
        p.baseline_in_use = Some("BL1".into());
        assert_eq!(variances(&p, &crate::schedule(&p).unwrap())[0], Some(Variance { start: 0, finish: 0, duration: 0 }));
        p.activities[0].constraint = Some(Constraint::StartOnOrAfter(day(2026, 10, 8)));
        let v = variances(&p, &crate::schedule(&p).unwrap());
        assert_eq!(v, [Some(Variance { start: -3, finish: -3, duration: 0 }), None], "N isn't in the baseline");
        p.activities[0].duration = 4;
        assert_eq!(variances(&p, &crate::schedule(&p).unwrap())[0], Some(Variance { start: -3, finish: -4, duration: -480 }));
        // Earlier than the baseline is positive.
        assert_eq!(working_days_between(&p.calendar, day(2026, 10, 5), day(2026, 10, 12)), 5);
    }
}
