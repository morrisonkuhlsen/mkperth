//! Several projects scheduled together, with relationships between their activities. They are
//! put into one network: each activity's ID marked with its project's place, each project's
//! calendars renamed as its own, and a start milestone for each project tied start-to-start to
//! every one of its activities, so none starts before its project does. The network is scheduled
//! once and the dates handed back to each project. The late dates count back from the latest
//! finish among them; progress is measured to the latest of their data dates.

use mkperth_core::{Activity, ActivityKind, Constraint, Project, Relationship, RelationshipType};

use crate::{Schedule, ScheduleError};

/// A relationship between two of the projects' activities: each end a project's place in the
/// list and an activity's ID in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub predecessor: (usize, String),
    pub successor: (usize, String),
    pub kind: RelationshipType,
    /// Working days; `lag_minutes` takes its place when set.
    pub lag: i64,
    pub lag_minutes: Option<i64>,
}

/// Marks what belongs to the project at place `k`: a separator no ID or name holds.
fn mark(k: usize, id: &str) -> String {
    format!("{k}\u{1f}{id}")
}

/// Schedules `projects` together, tied by `links` (a link to an activity that isn't there is
/// left out); a schedule for each, in their order.
pub fn schedule_together(projects: &[&Project], links: &[Link]) -> Result<Vec<Schedule>, ScheduleError> {
    let Some(first) = projects.first() else { return Ok(Vec::new()) };
    let mut merged = Project {
        name: first.name.clone(),
        start: projects.iter().map(|p| p.start).min().unwrap_or(first.start),
        calendar: first.calendar.clone(),
        calendars: Vec::new(),
        lag_calendar: first.lag_calendar,
        float_kind: first.float_kind,
        critical_by: first.critical_by,
        data_date: projects.iter().filter_map(|p| p.data_date).max(),
        progress_mode: first.progress_mode,
        must_finish_by: None,
        activities: Vec::new(),
        relationships: Vec::new(),
        bands: Vec::new(),
        markers: Vec::new(),
        month_marks: Vec::new(),
        resources: Vec::new(),
        roles: Vec::new(),
        assignments: Vec::new(),
        comments: Vec::new(),
        levelling: first.levelling.clone(),
        baselines: Vec::new(),
        baseline_in_use: None,
        expenses: Vec::new(),
        cost_accounts: Vec::new(),
        currency: String::new(),
        budget: Default::default(),
        earned_value: Default::default(),
        classification: Default::default(),
        external: Vec::new(),
        extra: Default::default(),
    };
    // Where each project's activities and relationships start in the network.
    let mut ranges: Vec<(usize, usize, usize, usize)> = Vec::new();
    for (k, p) in projects.iter().enumerate() {
        let own = mark(k, &p.calendar.name);
        for c in std::iter::once(&p.calendar).chain(p.calendars.iter()) {
            let mut c = c.clone();
            c.name = mark(k, &c.name);
            merged.calendars.push(c);
        }
        let (a0, r0) = (merged.activities.len(), merged.relationships.len());
        for a in &p.activities {
            let mut a = a.clone();
            a.id = mark(k, &a.id);
            a.parent = a.parent.map(|x| mark(k, &x));
            a.calendar = Some(a.calendar.map_or_else(|| own.clone(), |c| mark(k, &c)));
            if let Some(l) = &mut a.levelled {
                l.resource = mark(k, &l.resource);
            }
            merged.activities.push(a);
        }
        for r in &p.relationships {
            merged.relationships.push(Relationship { predecessor: mark(k, &r.predecessor), successor: mark(k, &r.successor), ..r.clone() });
        }
        for r in &p.resources {
            let mut r = r.clone();
            r.id = mark(k, &r.id);
            r.calendar = r.calendar.map(|c| mark(k, &c));
            r.roles = r.roles.iter().map(|x| mark(k, x)).collect();
            merged.resources.push(r);
        }
        for r in &p.roles {
            let mut r = r.clone();
            r.id = mark(k, &r.id);
            merged.roles.push(r);
        }
        for x in &p.assignments {
            let mut x = x.clone();
            x.activity = mark(k, &x.activity);
            x.resource = x.resource.map(|r| mark(k, &r));
            x.role = x.role.map(|r| mark(k, &r));
            merged.assignments.push(x);
        }
        ranges.push((a0, merged.activities.len(), r0, merged.relationships.len()));
    }
    // Each project's start: a milestone on it, tied start-to-start to every activity with none
    // under it.
    for (k, p) in projects.iter().enumerate() {
        let start = mark(k, "\u{1f}start");
        let parents: std::collections::HashSet<&str> = p.activities.iter().filter_map(|a| a.parent.as_deref()).collect();
        for a in p.activities.iter().filter(|a| !parents.contains(a.id.as_str())) {
            merged.relationships.push(Relationship {
                predecessor: start.clone(),
                successor: mark(k, &a.id),
                kind: RelationshipType::StartToStart,
                lag: 0,
                lag_minutes: None,
            });
        }
        merged.activities.push(Activity {
            id: start,
            kind: ActivityKind::StartMilestone,
            constraint: Some(Constraint::StartOnOrAfter(p.start)),
            calendar: Some(mark(k, &p.calendar.name)),
            ..Activity::default()
        });
    }
    for l in links {
        let there = |(k, id): &(usize, String)| projects.get(*k).is_some_and(|p| p.activities.iter().any(|a| a.id == *id));
        if there(&l.predecessor) && there(&l.successor) {
            merged.relationships.push(Relationship {
                predecessor: mark(l.predecessor.0, &l.predecessor.1),
                successor: mark(l.successor.0, &l.successor.1),
                kind: l.kind,
                lag: l.lag,
                lag_minutes: l.lag_minutes,
            });
        }
    }
    let all = crate::schedule(&merged)?;
    Ok(ranges
        .into_iter()
        .zip(projects)
        .map(|((a0, a1, r0, r1), p)| {
            let mut activities: Vec<_> = all.activities.get(a0..a1).map(<[_]>::to_vec).unwrap_or_default();
            for (s, a) in activities.iter_mut().zip(&p.activities) {
                s.id = a.id.clone();
            }
            let finish = activities.iter().map(|s| s.early_finish).max().unwrap_or(p.start).max(p.start);
            let ignored = all.ignored.iter().filter(|r| (r0..r1).contains(*r)).map(|r| r - r0).collect();
            Schedule { activities, finish, ignored }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use mkperth_core::{Calendar, Date};

    use super::*;

    fn day(y: i32, m: u32, d: u32) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    fn project(name: &str, tasks: &[(&str, i64)], ties: &[(&str, &str)]) -> Project {
        let mut p = mkperth_core::Project {
            name: name.into(),
            start: day(2026, 11, 2),
            calendar: Calendar::default(),
            calendars: Vec::new(),
            lag_calendar: Default::default(),
            float_kind: Default::default(),
            critical_by: Default::default(),
            data_date: None,
            progress_mode: Default::default(),
            must_finish_by: None,
            activities: Vec::new(),
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
            expenses: Vec::new(),
            cost_accounts: Vec::new(),
            currency: String::new(),
            budget: Default::default(),
            earned_value: Default::default(),
            classification: Default::default(),
            external: Vec::new(),
            extra: Default::default(),
        };
        p.activities = tasks.iter().map(|(id, d)| Activity { id: (*id).into(), duration: *d, ..Activity::default() }).collect();
        p.relationships = ties
            .iter()
            .map(|(a, b)| Relationship {
                predecessor: (*a).into(),
                successor: (*b).into(),
                kind: RelationshipType::FinishToStart,
                lag: 0,
                lag_minutes: None,
            })
            .collect();
        p
    }

    /// P: A (5 days) then B (2); Q: X (3) then Y (4); both from Monday 2 November 2026 on a
    /// Monday-to-Friday week. Alone, Q's Y is the 5th to the 10th. Tied, A → Y finish-to-start
    /// and X → B finish-to-start with a lag of 5 days:
    ///
    /// - A the 2nd to the 6th; X the 2nd to the 4th.
    /// - Y after X (the 4th) and A (the 6th): Monday the 9th to Thursday the 12th.
    /// - B after A (the 6th) and X's finish, the 4th, and 5 working days more (the 5th, 6th, 9th,
    ///   10th, 11th): Thursday the 12th to Friday the 13th. Both projects finish by the 13th.
    ///
    /// Backwards from the 13th: B late = early, float 0. Y's late finish the 13th, start the
    /// 10th: float 1. A must finish before Y's late start (the 10th) and B's (the 12th): by the
    /// 9th, float 1. X before Y's (the 10th: the 9th) and 5 days before B's (the 12th: back
    /// over the 11th, 10th, 9th, 6th and 5th, so by the 4th): the 4th, float 0.
    #[test]
    fn two_projects_tied_by_hand() {
        let p = project("P", &[("A", 5), ("B", 2)], &[("A", "B")]);
        let q = project("Q", &[("X", 3), ("Y", 4)], &[("X", "Y")]);
        let alone = crate::schedule(&q).unwrap();
        assert_eq!((alone.activities[1].early_start, alone.activities[1].early_finish), (day(2026, 11, 5), day(2026, 11, 10)));
        let links = [
            Link { predecessor: (0, "A".into()), successor: (1, "Y".into()), kind: RelationshipType::FinishToStart, lag: 0, lag_minutes: None },
            Link { predecessor: (1, "X".into()), successor: (0, "B".into()), kind: RelationshipType::FinishToStart, lag: 5, lag_minutes: None },
        ];
        let both = schedule_together(&[&p, &q], &links).unwrap();
        let dates = |s: &Schedule, k: usize| {
            let a = &s.activities[k];
            (a.id.clone(), a.early_start, a.early_finish, a.total_float)
        };
        assert_eq!(dates(&both[0], 0), ("A".into(), day(2026, 11, 2), day(2026, 11, 6), 1));
        assert_eq!(dates(&both[0], 1), ("B".into(), day(2026, 11, 12), day(2026, 11, 13), 0));
        assert_eq!(dates(&both[1], 0), ("X".into(), day(2026, 11, 2), day(2026, 11, 4), 0));
        assert_eq!(dates(&both[1], 1), ("Y".into(), day(2026, 11, 9), day(2026, 11, 12), 1));
        assert_eq!((both[0].finish, both[1].finish), (day(2026, 11, 13), day(2026, 11, 12)));
        // A link to an activity that isn't there is left out.
        let stray =
            [Link { predecessor: (0, "nope".into()), successor: (1, "Y".into()), kind: RelationshipType::FinishToStart, lag: 0, lag_minutes: None }];
        let apart = schedule_together(&[&p, &q], &stray).unwrap();
        assert_eq!(apart[1].activities[1].early_start, day(2026, 11, 5));
    }

    /// A project that starts later keeps its activities after its own start, tied or not.
    #[test]
    fn each_project_keeps_its_start() {
        let p = project("P", &[("A", 2)], &[]);
        let mut q = project("Q", &[("X", 1)], &[]);
        q.start = day(2026, 11, 9);
        let both = schedule_together(&[&p, &q], &[]).unwrap();
        assert_eq!(both[1].activities[0].early_start, day(2026, 11, 9));
        assert_eq!(both[0].activities[0].early_start, day(2026, 11, 2));
    }
}
