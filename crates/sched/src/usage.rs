//! A resource's work over time, for its histogram: each assignment's budget spread evenly over
//! its activity's working days, from the early start to the early finish on the activity's
//! calendar (the planned curve). In hundredths of the resource's unit, as the model keeps
//! units; each assignment's days add up to its budget exactly.

use std::collections::BTreeMap;

use mkperth_core::{Calendar, Date, Project};

use crate::{MAX_SPAN, Schedule};

/// The calendar named `name` in the project; the project's own when `None` or unknown.
pub(crate) fn calendar_named<'a>(project: &'a Project, name: Option<&str>) -> &'a Calendar {
    name.and_then(|n| project.calendars.iter().find(|c| c.name == n)).unwrap_or(&project.calendar)
}

/// The working days from `from` to `to` on `calendar`, at most `MAX_SPAN` of them.
pub(crate) fn working_days(calendar: &Calendar, from: Date, to: Date) -> Vec<Date> {
    let mut days = Vec::new();
    let mut d = from;
    while d <= to && days.len() < MAX_SPAN as usize {
        if calendar.is_working(d) {
            days.push(d);
        }
        match d.succ() {
            Some(next) => d = next,
            None => break,
        }
    }
    days
}

/// The work of the resource `resource` on each day it has some, in date order.
pub fn usage(project: &Project, schedule: &Schedule, resource: &str) -> Vec<(Date, i64)> {
    let mut by_day: BTreeMap<Date, i64> = BTreeMap::new();
    for a in project.assignments.iter().filter(|a| a.resource.as_deref() == Some(resource) && a.planned > 0) {
        let Some(k) = project.activities.iter().position(|x| x.id == a.activity) else { continue };
        let (Some(activity), Some(s)) = (project.activities.get(k), schedule.activities.get(k)) else { continue };
        let mut days = working_days(calendar_named(project, activity.calendar.as_deref()), s.early_start, s.early_finish);
        // A milestone, or an activity on no working day, takes its budget on its start.
        if days.is_empty() {
            days.push(s.early_start);
        }
        let n = days.len() as i64;
        let (each, left) = (a.planned / n, a.planned % n);
        for (k, d) in days.into_iter().enumerate() {
            // The hundredths that don't divide go to the first days, one each.
            let extra = i64::from((k as i64) < left);
            *by_day.entry(d).or_insert(0) += each + extra;
        }
    }
    by_day.into_iter().collect()
}

/// What the resource `resource` can do on each working day from `from` to `to`: its units an
/// hour over the hours of its calendar's working day.
pub fn capacity(project: &Project, resource: &str, from: Date, to: Date) -> Vec<(Date, i64)> {
    let Some(r) = project.resources.iter().find(|r| r.id == resource) else { return Vec::new() };
    let calendar = calendar_named(project, r.calendar.as_deref());
    let per_day = r.max_units.saturating_mul(calendar.day_minutes()) / 60;
    working_days(calendar, from, to).into_iter().map(|d| (d, per_day)).collect()
}

/// Days summed by week, each week by its Monday.
pub fn by_week(days: &[(Date, i64)]) -> Vec<(Date, i64)> {
    let mut weeks: BTreeMap<Date, i64> = BTreeMap::new();
    for (d, units) in days {
        let monday = Date::from_days(d.days() - d.weekday().index() as i32).unwrap_or(*d);
        *weeks.entry(monday).or_insert(0) += units;
    }
    weeks.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use mkperth_core::{Activity, Assignment, Region, Resource, UNIT};

    use super::*;

    fn day(y: i32, m: u32, d: u32) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    /// A10 takes 5 working days from Tuesday 3 November 2026 on Brazil's calendar (Monday the
    /// 2nd is All Souls' Day): the 3rd to the 6th and Monday the 9th. Ana's 40 hours are 8 a
    /// day; the week of the 2nd holds 32 of them, the week of the 9th 8. Concrete's 12.5 m³ over
    /// 3 days are 4.17, 4.17 and 4.16.
    #[test]
    fn budgets_spread_over_working_days() {
        let mut p = Project {
            name: String::new(),
            start: day(2026, 11, 2),
            calendar: Calendar { name: "Brazil".into(), region: Some(Region::Brazil), ..Calendar::default() },
            calendars: Vec::new(),
            lag_calendar: Default::default(),
            float_kind: Default::default(),
            critical_by: Default::default(),
            data_date: None,
            progress_mode: Default::default(),
            must_finish_by: None,
            activities: vec![
                Activity { id: "A10".into(), duration: 5, ..Activity::default() },
                Activity { id: "B10".into(), duration: 3, ..Activity::default() },
            ],
            relationships: Vec::new(),
            bands: Vec::new(),
            markers: Vec::new(),
            month_marks: Vec::new(),
            resources: vec![Resource { id: "R1".into(), ..Resource::default() }, Resource { id: "CONC".into(), ..Resource::default() }],
            roles: Vec::new(),
            assignments: vec![
                Assignment { activity: "A10".into(), resource: Some("R1".into()), planned: 40 * UNIT, ..Assignment::default() },
                Assignment { activity: "B10".into(), resource: Some("CONC".into()), planned: 1250, ..Assignment::default() },
            ],
            comments: Vec::new(),
            levelling: Default::default(),
            extra: Default::default(),
        };
        let s = crate::schedule(&p).unwrap();
        let ana = usage(&p, &s, "R1");
        assert_eq!(
            ana.iter().map(|(d, _)| *d).collect::<Vec<_>>(),
            [day(2026, 11, 3), day(2026, 11, 4), day(2026, 11, 5), day(2026, 11, 6), day(2026, 11, 9)]
        );
        assert!(ana.iter().all(|(_, u)| *u == 8 * UNIT));
        assert_eq!(by_week(&ana), [(day(2026, 11, 2), 32 * UNIT), (day(2026, 11, 9), 8 * UNIT)]);
        let concrete: Vec<i64> = usage(&p, &s, "CONC").into_iter().map(|(_, u)| u).collect();
        assert_eq!(concrete, [417, 417, 416]);
        assert_eq!(concrete.iter().sum::<i64>(), 1250, "the days add up to the budget");
        // Full time is 8 hours a working day: the week of the 2nd has 4 of them.
        let cap = capacity(&p, "R1", day(2026, 11, 2), day(2026, 11, 8));
        assert_eq!(by_week(&cap), [(day(2026, 11, 2), 32 * UNIT)]);
        // Two of them on one activity: 16 hours a day, twice the capacity.
        p.assignments.push(Assignment { activity: "A10".into(), resource: Some("R1".into()), planned: 40 * UNIT, ..Assignment::default() });
        assert!(usage(&p, &s, "R1").iter().all(|(_, u)| *u == 16 * UNIT));
    }
}
