//! Earned value: is the project on budget and on time? Each activity's budget at completion
//! (BAC) is the baseline's cost when a baseline is in use, else its budget now. Its planned value
//! (PV) by a day is that budget spread over its planned working days (the baseline's, else the
//! schedule's) up to that day; its earned value (EV) is the budget times how much of it is done,
//! measured by the project's technique; its actual cost (AC) is what was spent to the data date.
//! From them, the variances and indices (CV, SV, CPI, SPI), the estimate to complete (ETC) by
//! the project's technique, the estimate at completion (EAC), the variance at completion (VAC)
//! and the performance still needed (TCPI). A summary's are its children's. Money in hundredths.

use std::collections::{BTreeMap, HashMap, HashSet};

use mkperth_core::{Date, EtcTechnique, EvTechnique, Project};

use crate::costs::spread;
use crate::usage::{calendar_named, working_days};
use crate::{Schedule, Status};

/// An activity's (or the project's) earned value, in hundredths of the currency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Earned {
    /// The budget at completion.
    pub budget: i64,
    pub planned: i64,
    pub earned: i64,
    pub actual: i64,
    /// What the schedule prices the work left at.
    pub remaining: i64,
}

fn ratio(a: i64, b: i64) -> Option<f64> {
    (b != 0).then(|| a as f64 / b as f64)
}

impl Earned {
    /// Earned less actual: negative when the work done cost more than it was worth.
    pub fn cost_variance(&self) -> i64 {
        self.earned.saturating_sub(self.actual)
    }

    /// Earned less planned: negative when less is done than was planned by now.
    pub fn schedule_variance(&self) -> i64 {
        self.earned.saturating_sub(self.planned)
    }

    /// The cost performance index, EV / AC; none before anything is spent.
    pub fn cpi(&self) -> Option<f64> {
        ratio(self.earned, self.actual)
    }

    /// The schedule performance index, EV / PV; none before anything was planned.
    pub fn spi(&self) -> Option<f64> {
        ratio(self.earned, self.planned)
    }

    /// The estimate to complete: what is left as the schedule prices it, or the budget still to
    /// earn, as planned or at the performance so far (as planned while an index is unknown).
    pub fn etc(&self, technique: EtcTechnique) -> i64 {
        let left = self.budget.saturating_sub(self.earned);
        let factor = match technique {
            EtcTechnique::Remaining => return self.remaining,
            EtcTechnique::Planned => None,
            EtcTechnique::Cpi => self.cpi(),
            EtcTechnique::CpiSpi => self.cpi().zip(self.spi()).map(|(c, s)| c * s),
        };
        match factor.filter(|f| *f > 0.0) {
            Some(f) => (left as f64 / f).round().clamp(-9e15, 9e15) as i64,
            None => left,
        }
    }

    /// The estimate at completion: spent and still to spend.
    pub fn eac(&self, technique: EtcTechnique) -> i64 {
        self.actual.saturating_add(self.etc(technique))
    }

    /// The budget less the estimate at completion: negative when it will cost more.
    pub fn vac(&self, technique: EtcTechnique) -> i64 {
        self.budget.saturating_sub(self.eac(technique))
    }

    /// The to-complete performance index: the budget still to earn over the budget still to
    /// spend; none when it is all spent.
    pub fn tcpi(&self) -> Option<f64> {
        ratio(self.budget.saturating_sub(self.earned), self.budget.saturating_sub(self.actual))
    }

    fn add(&mut self, o: Earned) {
        self.budget = self.budget.saturating_add(o.budget);
        self.planned = self.planned.saturating_add(o.planned);
        self.earned = self.earned.saturating_add(o.earned);
        self.actual = self.actual.saturating_add(o.actual);
        self.remaining = self.remaining.saturating_add(o.remaining);
    }
}

/// How much of activity `i` is done, from 0 to 1, by `technique`: finished is all of it, not
/// started none; under way, its percent complete judged, its duration or its units spent, or a
/// fixed share (nothing for 0/100, half for 50/50, the custom share).
pub fn performance(project: &Project, schedule: &Schedule, i: usize, technique: EvTechnique) -> f64 {
    let (Some(a), Some(s)) = (project.activities.get(i), schedule.activities.get(i)) else { return 0.0 };
    let duration = || {
        let day = calendar_named(project, a.calendar.as_deref()).day_minutes();
        ratio(a.working_minutes(day).saturating_sub(s.remaining_minutes), a.working_minutes(day)).unwrap_or(0.0)
    };
    let share = match s.status {
        Status::Completed => return 1.0,
        Status::NotStarted => match technique {
            // Judged done in part before it is marked started, it counts.
            EvTechnique::Physical => a.percent_complete.unwrap_or(0) as f64 / 10_000.0,
            _ => 0.0,
        },
        Status::InProgress => match technique {
            EvTechnique::Physical => a.percent_complete.unwrap_or(0) as f64 / 10_000.0,
            EvTechnique::Duration => duration(),
            EvTechnique::Units => {
                let mine = project.assignments.iter().filter(|x| x.activity == a.id);
                let (done, total) =
                    mine.fold((0i64, 0i64), |(d, t), x| (d.saturating_add(x.actual), t.saturating_add(x.actual).saturating_add(x.remaining_units())));
                // Without units to count, by its duration.
                ratio(done, total).unwrap_or_else(duration)
            }
            EvTechnique::ZeroHundred => 0.0,
            EvTechnique::FiftyFifty => 0.5,
            EvTechnique::Custom(share) => share as f64 / 10_000.0,
        },
    };
    share.clamp(0.0, 1.0)
}

/// Each activity's budget at completion spread over its planned working days: the baseline's
/// when one is in use and holds it, else the schedule's.
fn planned_days(project: &Project, schedule: &Schedule) -> Vec<(i64, Vec<(Date, i64)>)> {
    let costs = crate::costs(project, schedule);
    let baseline = project.baseline();
    project
        .activities
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let calendar = calendar_named(project, a.calendar.as_deref());
            let (budget, from, to) = match (baseline.and_then(|b| b.activities.iter().find(|x| x.id == a.id)), schedule.activities.get(i)) {
                (Some(b), _) => (b.cost, b.start, b.finish),
                (None, Some(s)) => (costs.get(i).map_or(0, |c| c.budget), s.early_start, s.early_finish),
                (None, None) => return (0, Vec::new()),
            };
            let days = working_days(calendar, from, to);
            let days = if days.is_empty() { vec![from] } else { days };
            (budget, spread(budget, &days))
        })
        .collect()
}

/// The activities with none under them: the ones whose figures are their own.
fn leaves(project: &Project) -> Vec<bool> {
    let parents: HashSet<&str> = project.activities.iter().filter_map(|a| a.parent.as_deref()).collect();
    project.activities.iter().map(|a| !parents.contains(a.id.as_str())).collect()
}

/// Each activity's earned value, its planned value counted by the end of `at`; a summary's are
/// its children's.
pub fn earned_value(project: &Project, schedule: &Schedule, at: Date) -> Vec<Earned> {
    let costs = crate::costs(project, schedule);
    let leaf = leaves(project);
    let technique = project.earned_value.technique;
    let own: Vec<Earned> = planned_days(project, schedule)
        .into_iter()
        .enumerate()
        .map(|(i, (budget, days))| {
            if !leaf.get(i).copied().unwrap_or(false) {
                return Earned::default();
            }
            let c = costs.get(i).copied().unwrap_or_default();
            let planned = days.iter().filter(|(d, _)| *d <= at).fold(0i64, |s, (_, v)| s.saturating_add(*v));
            let done = performance(project, schedule, i, technique);
            let earned = (budget as f64 * done).round().clamp(-9e15, 9e15) as i64;
            Earned { budget, planned, earned, actual: c.actual, remaining: c.remaining }
        })
        .collect();
    let index: HashMap<&str, usize> = project.activities.iter().enumerate().map(|(i, a)| (a.id.as_str(), i)).collect();
    let mut out = own.clone();
    for (i, e) in own.iter().enumerate() {
        // Up the WBS, each summary above takes its figures (never further than there are activities).
        let mut up = project.activities.get(i).and_then(|a| a.parent.as_deref()).and_then(|p| index.get(p).copied());
        let mut steps = 0;
        while let Some(p) = up.filter(|_| steps <= project.activities.len()) {
            if let Some(slot) = out.get_mut(p) {
                slot.add(*e);
            }
            up = project.activities.get(p).and_then(|a| a.parent.as_deref()).and_then(|q| index.get(q).copied());
            steps += 1;
        }
    }
    out
}

/// The project's earned value: its activities' with none under them, summed.
pub fn project_earned(project: &Project, schedule: &Schedule, at: Date) -> Earned {
    let leaf = leaves(project);
    earned_value(project, schedule, at).into_iter().zip(leaf).filter(|(_, l)| *l).fold(Earned::default(), |mut total, (e, _)| {
        total.add(e);
        total
    })
}

/// The planned value day by day, summed to date: the S-curve's planned line.
pub fn planned_curve(project: &Project, schedule: &Schedule) -> Vec<(Date, i64)> {
    let leaf = leaves(project);
    let mut days: BTreeMap<Date, i64> = BTreeMap::new();
    for (i, (_, spread)) in planned_days(project, schedule).into_iter().enumerate() {
        if !leaf.get(i).copied().unwrap_or(false) {
            continue;
        }
        for (d, v) in spread {
            let slot = days.entry(d).or_insert(0);
            *slot = slot.saturating_add(v);
        }
    }
    let mut total = 0i64;
    days.into_iter()
        .map(|(d, v)| {
            total = total.saturating_add(v);
            (d, total)
        })
        .collect()
}

/// A point of the S-curve: at a period's close, the planned value by then and, once stored,
/// the earned value and the actual cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeriodPoint {
    pub period: String,
    pub finish: Date,
    pub planned: i64,
    pub earned: Option<i64>,
    pub actual: Option<i64>,
}

/// Each financial period's close: the planned value by then, and what was stored for it.
pub fn period_points(project: &Project, schedule: &Schedule) -> Vec<PeriodPoint> {
    let ev = &project.earned_value;
    let leaf: HashSet<&str> = project.activities.iter().zip(leaves(project)).filter(|(_, l)| *l).map(|(a, _)| a.id.as_str()).collect();
    ev.periods
        .iter()
        .map(|p| {
            let records: Vec<_> = ev.records.iter().filter(|r| r.period == p.name && leaf.contains(r.activity.as_str())).collect();
            let sum = |f: fn(&mkperth_core::PeriodRecord) -> i64| records.iter().fold(0i64, |s, r| s.saturating_add(f(r)));
            let stored = !records.is_empty();
            PeriodPoint {
                period: p.name.clone(),
                finish: p.finish,
                planned: project_earned(project, schedule, p.finish).planned,
                earned: stored.then(|| sum(|r| r.earned)),
                actual: stored.then(|| sum(|r| r.actual)),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use mkperth_core::{Activity, Assignment, Baseline, BaselineActivity, Calendar, Expense, Relationship, Resource, UNIT};

    use super::*;

    fn day(y: i32, m: u32, d: u32) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    /// A (5 days) then B (10 days), from Monday 2 November 2026 on a Monday-to-Friday week: A
    /// the 2nd to the 6th, B the 9th to the 20th, frozen as the baseline. A's work costs
    /// 1 000.00 and B's 2 000.00, 200.00 a working day; Ana's 40 hours on A and 80 on B cost
    /// nothing, there to count units. BAC 3 000.00. The periods are the three weeks; the
    /// planned value by each Friday: 1 000, 1 000 + 5 × 200 = 2 000, 3 000.
    fn project() -> Project {
        let a = |id: &str, duration| Activity { id: id.into(), duration, ..Activity::default() };
        let work = |activity: &str, planned| Expense { activity: activity.into(), name: "Work".into(), planned, ..Expense::default() };
        let hours = |activity: &str, planned| Assignment {
            activity: activity.into(),
            resource: Some("ANA".into()),
            planned: planned * UNIT,
            ..Assignment::default()
        };
        let frozen = |id: &str, start, finish, days: i64, cost| BaselineActivity { id: id.into(), start, finish, duration: days * 480, cost };
        Project {
            name: "earned".into(),
            start: day(2026, 11, 2),
            calendar: Calendar::default(),
            calendars: Vec::new(),
            lag_calendar: Default::default(),
            float_kind: Default::default(),
            critical_by: Default::default(),
            data_date: None,
            progress_mode: Default::default(),
            must_finish_by: None,
            activities: vec![a("A", 5), a("B", 10)],
            relationships: vec![Relationship {
                predecessor: "A".into(),
                successor: "B".into(),
                kind: Default::default(),
                lag: 0,
                lag_minutes: None,
                lag_kind: Default::default(),
            }],
            bands: Vec::new(),
            markers: Vec::new(),
            month_marks: Vec::new(),
            resources: vec![Resource { id: "ANA".into(), ..Resource::default() }],
            roles: Vec::new(),
            assignments: vec![hours("A", 40), hours("B", 80)],
            comments: Vec::new(),
            levelling: Default::default(),
            baselines: vec![Baseline {
                name: "Plan".into(),
                activities: vec![
                    frozen("A", day(2026, 11, 2), day(2026, 11, 6), 5, 100_000),
                    frozen("B", day(2026, 11, 9), day(2026, 11, 20), 10, 200_000),
                ],
            }],
            baseline_in_use: Some("Plan".into()),
            expenses: vec![work("A", 100_000), work("B", 200_000)],
            cost_accounts: Vec::new(),
            currency: String::new(),
            budget: Default::default(),
            earned_value: Default::default(),
            classification: Default::default(),
            external: Vec::new(),
            organisation: Default::default(),
            extra: Default::default(),
        }
    }

    /// The progress told at each period's close (the data date the Monday after):
    ///
    /// 1. Monday 9 November: A begun on the 2nd, a day left (of 5: 80 % by duration), judged 70 %
    ///    done, 30 of its 40 hours spent (75 %), 900.00 spent. B not begun.
    /// 2. Monday 16 November: A finished on the 9th, 1 100.00 spent. B begun on the 10th, 6 days
    ///    left (of 10: 40 %), judged 40 %, 36 of 80 hours (45 %), 800.00 spent.
    /// 3. Monday 23 November: B a day left (90 %), judged 85 %, 70 of 80 hours (87.5 %),
    ///    1 900.00 spent.
    fn at(period: usize) -> Project {
        let mut p = project();
        let a = 0;
        let b = 1;
        match period {
            1 => {
                p.data_date = Some(day(2026, 11, 9));
                p.activities[a].actual_start = Some(day(2026, 11, 2));
                p.activities[a].remaining_minutes = Some(480);
                p.activities[a].percent_complete = Some(7_000);
                p.assignments[a].actual = 30 * UNIT;
                p.assignments[a].remaining = Some(10 * UNIT);
                p.expenses[a].actual = 90_000;
            }
            _ => {
                p.data_date = Some(day(2026, 11, if period == 2 { 16 } else { 23 }));
                p.activities[a].actual_start = Some(day(2026, 11, 2));
                p.activities[a].actual_finish = Some(day(2026, 11, 9));
                p.activities[a].percent_complete = Some(10_000);
                p.assignments[a].actual = 40 * UNIT;
                p.expenses[a].actual = 110_000;
                p.activities[b].actual_start = Some(day(2026, 11, 10));
                let (left, judged, spent, cost) = if period == 2 { (6, 4_000, 36, 80_000) } else { (1, 8_500, 70, 190_000) };
                p.activities[b].remaining_minutes = Some(left * 480);
                p.activities[b].percent_complete = Some(judged);
                p.assignments[b].actual = spent * UNIT;
                p.assignments[b].remaining = Some((80 - spent) * UNIT);
                p.expenses[b].actual = cost;
            }
        }
        p
    }

    /// The earned value at each close, by each technique (in currency):
    ///
    /// | period | physical | duration | units | 0/100 | 50/50 | 25 % |
    /// |---|---|---|---|---|---|---|
    /// | 1 | 700 (A .70) | 800 (A .80) | 750 (A .75) | 0 | 500 | 250 |
    /// | 2 | 1 000 + 800 = 1 800 | 1 000 + 800 = 1 800 | 1 000 + 900 = 1 900 | 1 000 | 2 000 | 1 500 |
    /// | 3 | 1 000 + 1 700 = 2 700 | 1 000 + 1 800 = 2 800 | 1 000 + 1 750 = 2 750 | 1 000 | 2 000 | 1 500 |
    ///
    /// The planned value 1 000, 2 000, 3 000 and the actual cost 900, 1 900, 3 000 whatever the
    /// technique.
    #[test]
    fn earned_value_by_hand_for_each_technique() {
        let table: [(EvTechnique, [i64; 3]); 6] = [
            (EvTechnique::Physical, [700, 1_800, 2_700]),
            (EvTechnique::Duration, [800, 1_800, 2_800]),
            (EvTechnique::Units, [750, 1_900, 2_750]),
            (EvTechnique::ZeroHundred, [0, 1_000, 1_000]),
            (EvTechnique::FiftyFifty, [500, 2_000, 2_000]),
            (EvTechnique::Custom(2_500), [250, 1_500, 1_500]),
        ];
        let closes = [day(2026, 11, 6), day(2026, 11, 13), day(2026, 11, 20)];
        for (technique, earned) in table {
            for period in 1..=3 {
                let mut p = at(period);
                p.earned_value.technique = technique;
                let s = crate::schedule(&p).unwrap();
                let e = project_earned(&p, &s, closes[period - 1]);
                assert_eq!(e.earned, earned[period - 1] * 100, "{technique:?} in period {period}");
                assert_eq!((e.budget, e.planned), (300_000, period as i64 * 100_000), "{technique:?} in period {period}");
                assert_eq!(e.actual, [90_000, 190_000, 300_000][period - 1], "{technique:?} in period {period}");
            }
        }
    }

    /// At the third close, judged physically: BAC 3 000, PV 3 000, EV 2 700, AC 3 000. CV and SV
    /// −300; CPI and SPI 0.9. Still to earn 300: as planned, ETC 300 and EAC 3 300; at the CPI,
    /// 300 / 0.9 = 333.33 and 3 333.33; at CPI × SPI, 300 / 0.81 = 370.37 and 3 370.37; by what
    /// is left, B's 2 000 less its 1 900 spent: 100 and 3 100. TCPI 300 / 0: none.
    #[test]
    fn indices_and_estimates_by_hand() {
        let p = at(3);
        let s = crate::schedule(&p).unwrap();
        let e = project_earned(&p, &s, day(2026, 11, 20));
        assert_eq!((e.cost_variance(), e.schedule_variance()), (-30_000, -30_000));
        assert!((e.cpi().unwrap() - 0.9).abs() < 1e-9 && (e.spi().unwrap() - 0.9).abs() < 1e-9);
        let estimates: Vec<(i64, i64)> = EtcTechnique::ALL.iter().map(|t| (e.etc(*t), e.eac(*t))).collect();
        assert_eq!(estimates, [(10_000, 310_000), (30_000, 330_000), (33_333, 333_333), (37_037, 337_037)]);
        assert_eq!(e.vac(EtcTechnique::Cpi), -33_333);
        assert_eq!(e.tcpi(), None);
        // The planned line: 200 a working day, all of it by the 20th.
        let curve = planned_curve(&p, &s);
        assert_eq!((curve.first(), curve.last()), (Some(&(day(2026, 11, 2), 20_000)), Some(&(day(2026, 11, 20), 300_000))));
    }

    /// A summary over both takes their figures; the project counts them once.
    #[test]
    fn a_summary_takes_its_childrens() {
        let mut p = at(2);
        p.activities.insert(0, Activity { id: "W".into(), ..Activity::default() });
        for a in p.activities.iter_mut().skip(1) {
            a.parent = Some("W".into());
        }
        let s = crate::schedule(&p).unwrap();
        let all = earned_value(&p, &s, day(2026, 11, 13));
        assert_eq!(all[0].earned, 180_000);
        assert_eq!(project_earned(&p, &s, day(2026, 11, 13)), all[0]);
    }
}
