//! What the work costs, in hundredths of the currency. A resource's work is spread evenly over
//! its activity's working days and priced each day at the resource's price then, so a price
//! that changes mid-activity counts from its day on; an expense falls on the activity's first
//! day, its last, or evenly over its days (its accrual). Each is counted three ways: the budget
//! (the planned units or cost over the planned dates), the actual (over the days from the actual
//! start to the data date, or the actual finish), and the remaining (over the days from the data
//! date to the early finish); at completion is actual plus remaining. A summary's costs are its
//! children's.

use std::collections::{BTreeMap, HashMap};

use mkperth_core::{Accrual, Date, Project, UNIT};

use crate::usage::{calendar_named, working_days};
use crate::{Schedule, Status};

/// An activity's (or the project's) costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cost {
    pub budget: i64,
    pub actual: i64,
    pub remaining: i64,
}

impl Cost {
    pub fn at_completion(&self) -> i64 {
        self.actual.saturating_add(self.remaining)
    }

    /// The budget less the cost at completion: negative when it costs more.
    pub fn variance(&self) -> i64 {
        self.budget.saturating_sub(self.at_completion())
    }

    fn add(&mut self, other: Cost) {
        self.budget = self.budget.saturating_add(other.budget);
        self.actual = self.actual.saturating_add(other.actual);
        self.remaining = self.remaining.saturating_add(other.remaining);
    }
}

/// `amount` shared evenly over `days`, the hundredths that don't divide to the first days.
pub(crate) fn spread(amount: i64, days: &[Date]) -> Vec<(Date, i64)> {
    let n = days.len() as i64;
    if n == 0 {
        return Vec::new();
    }
    let (each, left) = (amount / n, amount % n);
    days.iter().enumerate().map(|(k, d)| (*d, each + i64::from((k as i64) < left.abs()) * left.signum())).collect()
}

/// An activity's days three ways: planned, actual and remaining.
struct Days {
    planned: Vec<Date>,
    actual: Vec<Date>,
    remaining: Vec<Date>,
}

fn days_of(project: &Project, i: usize, schedule: &Schedule) -> Option<Days> {
    let (a, s) = (project.activities.get(i)?, schedule.activities.get(i)?);
    let calendar = calendar_named(project, a.calendar.as_deref());
    let span = |from: Date, to: Date| {
        let days = working_days(calendar, from, to);
        if days.is_empty() { vec![from] } else { days }
    };
    let planned = span(s.early_start, s.early_finish);
    let actual = match (s.status, a.actual_start.or(a.actual_finish)) {
        (Status::NotStarted, _) | (_, None) => Vec::new(),
        (_, Some(start)) => {
            let to = a.actual_finish.or_else(|| project.data_date.and_then(|d| Date::from_days(d.days() - 1))).unwrap_or(start).max(start);
            span(start, to)
        }
    };
    let remaining = match s.status {
        Status::Completed => Vec::new(),
        Status::InProgress => span(project.data_date.map_or(s.early_start, |d| d.max(s.early_start)).min(s.early_finish), s.early_finish),
        Status::NotStarted => planned.clone(),
    };
    Some(Days { planned, actual, remaining })
}

/// Each day's cost of `units` of resource work spread over `days` at the resource's price then.
fn priced(units: i64, days: &[Date], price: &dyn Fn(Date) -> i64) -> Vec<(Date, i64)> {
    spread(units, days)
        .into_iter()
        .map(|(d, u)| (d, i64::try_from(i128::from(u) * i128::from(price(d)) / i128::from(UNIT)).unwrap_or(i64::MAX)))
        .collect()
}

/// An expense's amount over `days` by its accrual.
fn accrued(amount: i64, days: &[Date], accrual: Accrual) -> Vec<(Date, i64)> {
    match (accrual, days.first(), days.last()) {
        (Accrual::Start, Some(d), _) | (Accrual::End, _, Some(d)) => vec![(*d, amount)],
        _ => spread(amount, days),
    }
}

/// The costs of each activity day by day, three ways (budget, actual, remaining), before
/// summaries take their children's.
fn by_day(project: &Project, schedule: &Schedule) -> Vec<[Vec<(Date, i64)>; 3]> {
    let index: HashMap<&str, usize> = project.activities.iter().enumerate().map(|(i, a)| (a.id.as_str(), i)).collect();
    let days: Vec<Option<Days>> = (0..project.activities.len()).map(|i| days_of(project, i, schedule)).collect();
    let mut out: Vec<[Vec<(Date, i64)>; 3]> = vec![Default::default(); project.activities.len()];
    for x in &project.assignments {
        let (Some(&i), Some(r)) = (index.get(x.activity.as_str()), x.resource.as_deref().and_then(|r| project.resources.iter().find(|y| y.id == r)))
        else {
            continue;
        };
        let (Some(Some(d)), Some(slot)) = (days.get(i), out.get_mut(i)) else { continue };
        let price = |day: Date| r.price_on(day);
        let [budget, actual, remaining] = slot;
        budget.extend(priced(x.planned, &d.planned, &price));
        actual.extend(priced(x.actual, &d.actual, &price));
        remaining.extend(priced(x.remaining_units(), &d.remaining, &price));
    }
    for e in &project.expenses {
        let Some(&i) = index.get(e.activity.as_str()) else { continue };
        let (Some(Some(d)), Some(slot)) = (days.get(i), out.get_mut(i)) else { continue };
        let [budget, actual, remaining] = slot;
        budget.extend(accrued(e.planned, &d.planned, e.accrual));
        actual.extend(accrued(e.actual, &d.actual, e.accrual));
        remaining.extend(accrued(e.remaining_cost(), &d.remaining, e.accrual));
    }
    out
}

/// Each activity's costs, in the project's order; a summary's are its own and its children's.
pub fn costs(project: &Project, schedule: &Schedule) -> Vec<Cost> {
    let own: Vec<Cost> = by_day(project, schedule)
        .iter()
        .map(|[b, a, r]| {
            let sum = |v: &Vec<(Date, i64)>| v.iter().fold(0i64, |s, (_, c)| s.saturating_add(*c));
            Cost { budget: sum(b), actual: sum(a), remaining: sum(r) }
        })
        .collect();
    let index: HashMap<&str, usize> = project.activities.iter().enumerate().map(|(i, a)| (a.id.as_str(), i)).collect();
    let mut out = own.clone();
    for (i, cost) in own.iter().enumerate() {
        // Up the WBS, each summary above it takes its costs (never further than there are activities).
        let mut at = project.activities.get(i).and_then(|a| a.parent.as_deref()).and_then(|p| index.get(p).copied());
        let mut steps = 0;
        while let Some(p) = at.filter(|_| steps <= project.activities.len()) {
            if let Some(slot) = out.get_mut(p) {
                slot.add(*cost);
            }
            at = project.activities.get(p).and_then(|a| a.parent.as_deref()).and_then(|q| index.get(q).copied());
            steps += 1;
        }
    }
    out
}

/// The project's costs: every activity's own, summed.
pub fn project_cost(project: &Project, schedule: &Schedule) -> Cost {
    let parents: std::collections::HashSet<&str> = project.activities.iter().filter_map(|a| a.parent.as_deref()).collect();
    costs(project, schedule).into_iter().zip(&project.activities).filter(|(_, a)| !parents.contains(a.id.as_str())).fold(
        Cost::default(),
        |mut total, (c, _)| {
            total.add(c);
            total
        },
    )
}

/// The budget spent day by day, in date order: the spending plan.
pub fn spending(project: &Project, schedule: &Schedule) -> Vec<(Date, i64)> {
    let mut days: BTreeMap<Date, i64> = BTreeMap::new();
    for [budget, _, _] in by_day(project, schedule) {
        for (d, c) in budget {
            let slot = days.entry(d).or_insert(0);
            *slot = slot.saturating_add(c);
        }
    }
    days.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use mkperth_core::{Activity, Assignment, Calendar, Expense, Price, Resource};

    use super::*;

    fn day(y: i32, m: u32, d: u32) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    /// A takes 5 days from Monday 5 October 2026 (Monday to Friday), under the summary W. Ana's
    /// 40 hours on it are 8 a day; she costs 10.00 an hour, 12.00 from Thursday the 8th: 3 days
    /// × 8 × 10 + 2 × 8 × 12 = 240 + 192 = 432.00. A permit of 300.00 falls on its start, a
    /// crane of 500.00 on its end, fuel of 600.00 evenly (120.00 a day): the budget is
    /// 1 832.00, spent 500, 200, 200, 216 and 716 over the five days.
    fn project() -> Project {
        let expense = |name: &str, planned: i64, accrual: Accrual| Expense {
            activity: "A".into(),
            name: name.into(),
            planned: planned * 100,
            accrual,
            ..Expense::default()
        };
        Project {
            name: "costs".into(),
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
                Activity { id: "W".into(), ..Activity::default() },
                Activity { id: "A".into(), duration: 5, parent: Some("W".into()), ..Activity::default() },
            ],
            relationships: Vec::new(),
            bands: Vec::new(),
            markers: Vec::new(),
            month_marks: Vec::new(),
            resources: vec![Resource {
                id: "ANA".into(),
                prices: vec![Price { from: None, per_unit: 1000 }, Price { from: Some(day(2026, 10, 8)), per_unit: 1200 }],
                ..Resource::default()
            }],
            roles: Vec::new(),
            assignments: vec![Assignment { activity: "A".into(), resource: Some("ANA".into()), planned: 40 * UNIT, ..Assignment::default() }],
            comments: Vec::new(),
            levelling: Default::default(),
            baselines: Vec::new(),
            baseline_in_use: None,
            expenses: vec![expense("Permit", 300, Accrual::Start), expense("Crane", 500, Accrual::End), expense("Fuel", 600, Accrual::Uniform)],
            cost_accounts: Vec::new(),
            currency: String::new(),
            budget: Default::default(),
            earned_value: Default::default(),
            extra: Default::default(),
        }
    }

    #[test]
    fn a_budget_by_hand() {
        let p = project();
        let s = crate::schedule(&p).unwrap();
        let c = costs(&p, &s);
        assert_eq!(c[1], Cost { budget: 183_200, actual: 0, remaining: 183_200 });
        assert_eq!(c[0], c[1], "the summary takes its child's");
        assert_eq!(project_cost(&p, &s), c[1], "counted once");
        assert_eq!(spending(&p, &s).into_iter().map(|(_, c)| c / 100).collect::<Vec<_>>(), [500, 200, 200, 216, 716]);
    }

    /// Progress told on Thursday the 8th: begun on the 5th, Ana's 24 hours done (8 a day on the
    /// 5th to the 7th at 10.00: 240.00) and 16 left (the 8th and 9th at 12.00: 192.00); the
    /// permit paid (300.00), fuel 360.00 of it (240.00 left), the crane still to pay. Actual
    /// 240 + 660 = 900.00, remaining 192 + 740 = 932.00: at completion 1 832.00, on budget.
    #[test]
    fn actual_and_remaining_by_hand() {
        let mut p = project();
        p.data_date = Some(day(2026, 10, 8));
        p.activities[1].actual_start = Some(day(2026, 10, 5));
        p.assignments[0].actual = 24 * UNIT;
        p.expenses[0].actual = 30_000;
        p.expenses[2].actual = 36_000;
        let s = crate::schedule(&p).unwrap();
        let c = costs(&p, &s)[1];
        assert_eq!((c.actual, c.remaining, c.at_completion(), c.variance()), (90_000, 93_200, 183_200, 0));
        // A remaining told as more than planned is over budget.
        p.expenses[1].remaining = Some(70_000);
        assert_eq!(costs(&p, &s)[1].variance(), -20_000);
    }

    /// A price taking effect before the first: the first then holds only before it.
    #[test]
    fn prices_over_time() {
        let r = Resource {
            prices: vec![Price { from: Some(day(2026, 10, 10)), per_unit: 500 }, Price { from: Some(day(2026, 10, 1)), per_unit: 300 }],
            ..Resource::default()
        };
        assert_eq!(r.price_on(day(2026, 9, 1)), 500, "before any, the first");
        assert_eq!(r.price_on(day(2026, 10, 5)), 300);
        assert_eq!(r.price_on(day(2026, 10, 12)), 500);
        assert_eq!(Resource::default().price_on(day(2026, 10, 12)), 0);
    }
}
