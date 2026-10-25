//! PERT's analysis, beside the schedule and never in it: each activity with three estimates
//! (optimistic O, most likely M, pessimistic P) is taken at its expected duration
//! tₑ = (O + 4M + P) / 6, with a standard deviation σ = (P − O) / 6; the project is scheduled
//! again on a copy with those durations, and its expected duration T is the working days from
//! its start to that copy's finish. The variance of T is the sum of the variances of the
//! activities on the copy's longest path; the chance of finishing by a day is Φ((W − T) / σ),
//! W the working days to that day, Φ the normal distribution. Sources: Malcolm, Roseboom, Clark
//! and Fazar, "Application of a technique for research and development program evaluation"
//! (Operations Research, 1959); Moder, Phillips and Davis, *Project Management with CPM, PERT
//! and Precedence Diagramming*.

use mkperth_core::{Date, Project};

use crate::{MAX_SPAN, ScheduleError, Status, schedule, usage::calendar_named};

/// One activity's estimates worked out, in working days.
#[derive(Debug, Clone, PartialEq)]
pub struct PertActivity {
    /// Its place in the project's list.
    pub index: usize,
    pub expected: f64,
    pub sigma: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PertAnalysis {
    /// Every activity with estimates, in the project's order.
    pub activities: Vec<PertActivity>,
    /// The finish the schedule gives, with the planned durations.
    pub planned_finish: Date,
    /// The finish with each estimated activity at its expected duration.
    pub expected_finish: Date,
    /// The working days from the project's start to the expected finish: T.
    pub expected_days: i64,
    /// T's standard deviation, in working days.
    pub sigma: f64,
    /// The activities on the longest path of the expected schedule, by their place in the list.
    pub path: Vec<usize>,
    start: Date,
}

/// The normal distribution's Φ, by Abramowitz and Stegun's 7.1.26 (within 1.5 × 10⁻⁷).
pub fn normal(z: f64) -> f64 {
    let x = z.abs() / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let poly = t * (0.254_829_592 + t * (-0.284_496_736 + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    let erf = 1.0 - poly * (-x * x).exp();
    if z >= 0.0 { 0.5 * (1.0 + erf) } else { 0.5 * (1.0 - erf) }
}

/// The working days of the project's calendar from `from` to `to`, both counted.
fn working_days(project: &Project, from: Date, to: Date) -> i64 {
    let (mut d, mut n, mut steps) = (from, 0i64, 0i64);
    while d <= to && steps < MAX_SPAN * 4 {
        n += i64::from(project.calendar.is_working(d));
        steps += 1;
        match d.succ() {
            Some(next) => d = next,
            None => break,
        }
    }
    n
}

impl PertAnalysis {
    /// The chance of finishing by the evening of `day`.
    pub fn chance(&self, project: &Project, day: Date) -> f64 {
        let w = working_days(project, self.start, day) as f64;
        let t = self.expected_days as f64;
        if self.sigma <= f64::EPSILON {
            return if w >= t { 1.0 } else { 0.0 };
        }
        normal((w - t) / self.sigma)
    }

    /// The day the project finishes by with the chance of `z` standard deviations past T: the
    /// working day that many days (rounded up) from its start.
    pub fn finish_at(&self, project: &Project, z: f64) -> Date {
        let n = (self.expected_days as f64 + z * self.sigma).ceil().max(1.0) as i64;
        let (mut d, mut counted, mut steps) = (self.start, 0i64, 0i64);
        loop {
            counted += i64::from(project.calendar.is_working(d));
            if counted >= n || steps > MAX_SPAN * 4 {
                return d;
            }
            steps += 1;
            match d.succ() {
                Some(next) => d = next,
                None => return d,
            }
        }
    }
}

/// PERT's analysis of `project`: the schedule it would have at the expected durations, and the
/// spread of its finish. The project and its schedule are left as they are.
pub fn pert(project: &Project) -> Result<PertAnalysis, ScheduleError> {
    let planned = schedule(project)?;
    let mut copy = project.clone();
    let mut activities = Vec::new();
    for (i, a) in copy.activities.iter_mut().enumerate() {
        let Some(p) = a.pert else { continue };
        activities.push(PertActivity { index: i, expected: p.expected() / 100.0, sigma: p.sigma() / 100.0 });
        // Only what is still to start, and has work, takes its expected duration.
        let begun = a.actual_start.is_some() || a.actual_finish.is_some();
        if !begun && !a.kind.is_milestone() {
            let day = calendar_named(project, a.calendar.as_deref()).day_minutes();
            let minutes = (p.expected() / 100.0 * day as f64).round();
            a.duration_minutes = Some((minutes.max(0.0) as i64).min(MAX_SPAN * day));
            a.duration = a.duration_minutes.map_or(a.duration, |m| (m + day - 1) / day.max(1));
        }
    }
    let expected = schedule(&copy)?;
    // Summaries are on it through their children; the children count.
    let parents: std::collections::HashSet<&str> = project.activities.iter().filter_map(|a| a.parent.as_deref()).collect();
    let path: Vec<usize> = expected
        .activities
        .iter()
        .enumerate()
        .filter(|(i, s)| {
            s.longest_path && s.status != Status::Completed && project.activities.get(*i).is_some_and(|a| !parents.contains(a.id.as_str()))
        })
        .map(|(i, _)| i)
        .collect();
    let variance: f64 = activities.iter().filter(|a| path.contains(&a.index)).map(|a| a.sigma * a.sigma).sum();
    let start = project.start;
    Ok(PertAnalysis {
        expected_days: working_days(project, start, expected.finish),
        expected_finish: expected.finish,
        planned_finish: planned.finish,
        sigma: variance.sqrt(),
        path,
        activities,
        start,
    })
}

#[cfg(test)]
mod tests {
    use mkperth_core::{Activity, Calendar, Pert, Relationship, RelationshipType};

    use super::*;

    fn day(y: i32, m: u32, d: u32) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    /// From Monday 5 October 2026, Monday to Friday: A (planned 3 days; O 2, M 4, P 6) then B
    /// (planned 5; O 3, M 5, P 13). Planned, A is the 5th to the 7th and B the 8th to the 14th.
    /// Expected, A takes (2 + 16 + 6) / 6 = 4 days, the 5th to the 8th, and B (3 + 20 + 13) / 6
    /// = 6, the 9th to Friday the 16th: T = 10 working days. Their σ are 4/6 and 10/6, so T's is
    /// √(16/36 + 100/36) = √(116)/6 ≈ 1.795. By the 16th the chance is Φ(0) = 50%; by Monday
    /// the 19th (11 days) Φ(1/1.795) ≈ 71.1%; by the 14th (8 days) Φ(−1.114) ≈ 13.3%. With 90%
    /// (z = 1.2816) T + 2.30 = 12.3, so 13 working days: Wednesday the 21st.
    #[test]
    fn a_chain_by_hand() {
        let task = |id: &str, d: i64, o: i64, m: i64, p: i64| Activity {
            id: id.into(),
            duration: d,
            pert: Some(Pert { optimistic: o * 100, most_likely: m * 100, pessimistic: p * 100 }),
            ..Activity::default()
        };
        let project = Project {
            name: "pert".into(),
            start: day(2026, 10, 5),
            calendar: Calendar::default(),
            calendars: Vec::new(),
            lag_calendar: Default::default(),
            float_kind: Default::default(),
            critical_by: Default::default(),
            data_date: None,
            progress_mode: Default::default(),
            must_finish_by: None,
            activities: vec![task("A", 3, 2, 4, 6), task("B", 5, 3, 5, 13)],
            relationships: vec![Relationship {
                predecessor: "A".into(),
                successor: "B".into(),
                kind: RelationshipType::FinishToStart,
                lag: 0,
                lag_minutes: None,
                lag_kind: Default::default(),
            }],
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
            organisation: Default::default(),
            extra: Default::default(),
        };
        let a = pert(&project).unwrap();
        assert_eq!(a.planned_finish, day(2026, 10, 14), "the schedule keeps its durations");
        assert_eq!(schedule(&project).unwrap().finish, day(2026, 10, 14));
        assert_eq!((a.expected_finish, a.expected_days), (day(2026, 10, 16), 10));
        for (x, (te, sigma)) in a.activities.iter().zip([(4.0, 4.0 / 6.0), (6.0, 10.0 / 6.0)]) {
            assert!((x.expected - te).abs() < 1e-12 && (x.sigma - sigma).abs() < 1e-12);
        }
        assert_eq!(a.path, [0, 1]);
        assert!((a.sigma - 116f64.sqrt() / 6.0).abs() < 1e-12);
        assert!((a.chance(&project, day(2026, 10, 16)) - 0.5).abs() < 1e-6);
        assert!((a.chance(&project, day(2026, 10, 19)) - 0.7113).abs() < 1e-3);
        assert!((a.chance(&project, day(2026, 10, 14)) - 0.1326).abs() < 1e-3);
        assert_eq!(a.finish_at(&project, 1.2816), day(2026, 10, 21));
        assert_eq!(a.finish_at(&project, 0.0), day(2026, 10, 16));
    }

    #[test]
    fn the_normal_distribution() {
        assert!((normal(0.0) - 0.5).abs() < 1e-9);
        assert!((normal(1.0) - 0.841_344_7).abs() < 1e-6);
        assert!((normal(-1.959_964) - 0.025).abs() < 1e-6);
    }
}
