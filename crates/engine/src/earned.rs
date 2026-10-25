//! Financial periods and the performance stored for them: periods added one by one or by the
//! week or the month, deleted with what was stored for them, and a period's close stored (each
//! activity's planned value by the period's end, its earned value and its actual cost to the
//! data date). Each is a command with undo.

use std::collections::HashSet;

use mkperth_core::{Date, FinancialPeriod, PeriodRecord, Project};
use mkperth_i18n::tr;
use serde_json::Value;

use crate::costs::{MAX_TEXT, bad, required, text};
use crate::{EngineError, Session};

/// The most periods a project may hold: a century of months, or twenty years of weeks.
const MAX_PERIODS: usize = 1200;

fn date(params: &Value, command: &str, name: &str) -> Result<Date, EngineError> {
    let d = required(params, command, name)?;
    Date::parse(&d).ok_or_else(|| bad(command, name))
}

/// `period` overlaps none of the project's.
fn fits(p: &Project, period: &FinancialPeriod) -> bool {
    !p.earned_value.periods.iter().any(|q| q.start <= period.finish && period.start <= q.finish)
}

fn insert(p: &mut Project, period: FinancialPeriod) {
    p.earned_value.periods.push(period);
    p.earned_value.periods.sort_by_key(|q| q.start);
}

/// The day before `d`, or `d` at the start of time.
fn before(d: Date) -> Date {
    Date::from_days(d.days() - 1).unwrap_or(d)
}

impl Session {
    /// The earned value commands; `None` when `id` isn't one of them.
    pub(crate) fn earned_command(&mut self, id: &str, params: &Value) -> Option<Result<(), EngineError>> {
        Some(match id {
            "period.add" => self.add_period(params),
            "period.generate" => self.generate_periods(params),
            "period.delete" => self.delete_period(params),
            "period.store" => self.store_period(params),
            _ => return None,
        })
    }

    /// A financial period: its `name` (unique), `start` and `finish`, overlapping none.
    fn add_period(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "period.add";
        let name = required(params, C, "name")?;
        let (start, finish) = (date(params, C, "start")?, date(params, C, "finish")?);
        if finish < start {
            return Err(bad(C, "finish"));
        }
        self.change(tr!("undo-add-period", name = name.as_str()), |p| {
            let period = FinancialPeriod { name: name.clone(), start, finish };
            if p.earned_value.periods.len() >= MAX_PERIODS || p.earned_value.periods.iter().any(|q| q.name == name) {
                return Err(bad(C, "name"));
            }
            if !fits(p, &period) {
                return Err(bad(C, "start"));
            }
            insert(p, period);
            Ok(())
        })
    }

    /// Periods from `from` to `to`, `every` week (Monday to Sunday, named by their Monday) or
    /// month (named `2026-11`); those that would overlap one already there are left out.
    fn generate_periods(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "period.generate";
        let (from, to) = (date(params, C, "from")?, date(params, C, "to")?);
        let every = required(params, C, "every")?;
        if to < from {
            return Err(bad(C, "to"));
        }
        let mut new = Vec::new();
        match every.as_str() {
            "month" => {
                let (mut y, mut m, _) = from.ymd();
                while let Some(first) = Date::from_ymd(y, m, 1).filter(|f| *f <= to) {
                    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
                    let last = Date::from_ymd(ny, nm, 1).map_or(first, before);
                    new.push(FinancialPeriod { name: format!("{y:04}-{m:02}"), start: first, finish: last });
                    (y, m) = (ny, nm);
                    if new.len() > MAX_PERIODS {
                        return Err(bad(C, "to"));
                    }
                }
            }
            "week" => {
                // From the Monday of `from`'s week.
                let mut monday = Date::from_days(from.days() - from.weekday().index() as i32).unwrap_or(from);
                while monday <= to {
                    let sunday = Date::from_days(monday.days() + 6).unwrap_or(monday);
                    new.push(FinancialPeriod { name: monday.to_string(), start: monday, finish: sunday });
                    let Some(next) = Date::from_days(monday.days() + 7) else { break };
                    monday = next;
                    if new.len() > MAX_PERIODS {
                        return Err(bad(C, "to"));
                    }
                }
            }
            _ => return Err(bad(C, "every")),
        }
        self.change(tr!("undo-generate-periods", n = new.len()), |p| {
            for period in new {
                if fits(p, &period) && !p.earned_value.periods.iter().any(|q| q.name == period.name) {
                    if p.earned_value.periods.len() >= MAX_PERIODS {
                        return Err(bad(C, "to"));
                    }
                    insert(p, period);
                }
            }
            Ok(())
        })
    }

    /// Deletes the period `name` and what was stored for it.
    fn delete_period(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "period.delete";
        let name = required(params, C, "name")?;
        self.change(tr!("undo-delete-period", name = name.as_str()), |p| {
            let before = p.earned_value.periods.len();
            p.earned_value.periods.retain(|q| q.name != name);
            if p.earned_value.periods.len() == before {
                return Err(bad(C, "name"));
            }
            p.earned_value.records.retain(|r| r.period != name);
            Ok(())
        })
    }

    /// Stores a period's close: the period `name`, else the last one begun before the data
    /// date. For each activity with none under it, its planned value by the period's end, and
    /// its earned value and actual cost to the data date; what was stored for it before is
    /// replaced.
    fn store_period(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "period.store";
        let name = text(params, C, "name")?.filter(|n| !n.is_empty() && n.chars().count() <= MAX_TEXT);
        let schedule = self.schedule.as_ref().map_err(|e| EngineError::Schedule(e.clone()))?.clone();
        let p = &self.project;
        let data_date = p.data_date.ok_or_else(|| EngineError::Param(C.into(), "data_date".into()))?;
        let period = match &name {
            Some(n) => p.earned_value.periods.iter().find(|q| q.name == *n),
            None => p.earned_value.periods.iter().rev().find(|q| q.start < data_date),
        }
        .cloned()
        .ok_or_else(|| bad(C, "name"))?;
        let by_end = mkperth_sched::earned_value(p, &schedule, period.finish);
        let parents: HashSet<&str> = p.activities.iter().filter_map(|a| a.parent.as_deref()).collect();
        let records: Vec<PeriodRecord> = p
            .activities
            .iter()
            .zip(by_end)
            .filter(|(a, _)| !parents.contains(a.id.as_str()))
            .map(|(a, e)| PeriodRecord {
                period: period.name.clone(),
                activity: a.id.clone(),
                planned: e.planned,
                earned: e.earned,
                actual: e.actual,
            })
            .collect();
        self.change(tr!("undo-store-period", name = period.name.as_str()), |p| {
            p.earned_value.records.retain(|r| r.period != period.name);
            p.earned_value.records.extend(records);
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn run(s: &mut Session, id: &str, params: Value) {
        if let Err(e) = s.run(id, &params) {
            panic!("{id} {params}: {e}");
        }
    }

    /// Months generated over the sample's span, a week added by hand refused where it overlaps;
    /// a close stored for the period begun before the data date, replaced when stored again,
    /// gone with its period.
    #[test]
    fn periods_and_their_closes() {
        let mut s = Session::default();
        run(&mut s, "period.generate", json!({"from": "2026-11-03", "to": "2027-01-10", "every": "month"}));
        let names: Vec<_> = s.project.earned_value.periods.iter().map(|q| q.name.clone()).collect();
        assert_eq!(names, ["2026-11", "2026-12", "2027-01"]);
        assert_eq!(s.project.earned_value.periods[1].finish, Date::from_ymd(2026, 12, 31).unwrap());
        assert!(s.run("period.add", &json!({"name": "W", "start": "2026-11-30", "finish": "2026-12-06"})).is_err(), "overlaps");
        run(&mut s, "period.add", json!({"name": "Before", "start": "2026-10-26", "finish": "2026-10-31"}));
        assert_eq!(s.project.earned_value.periods[0].name, "Before", "in date order");
        // Weeks from a Wednesday start on its Monday.
        let mut w = Session::default();
        run(&mut w, "period.generate", json!({"from": "2026-11-04", "to": "2026-11-15", "every": "week"}));
        let weeks: Vec<_> = w.project.earned_value.periods.iter().map(|q| (q.start.to_string(), q.finish.to_string())).collect();
        assert_eq!(weeks, [("2026-11-02".into(), "2026-11-08".into()), ("2026-11-09".into(), "2026-11-15".into())]);

        // A close needs a data date; then it goes to November, begun before 1 December.
        assert!(s.run("period.store", &json!({})).is_err());
        run(&mut s, "project.options", json!({"data_date": "2026-12-01"}));
        run(&mut s, "activity.edit", json!({"id": "A10", "actual_start": "2026-11-03", "percent_complete": 60}));
        run(&mut s, "expense.add", json!({"activity": "A10", "name": "Survey", "planned": 500, "actual": 350}));
        run(&mut s, "period.store", json!({}));
        let r = s.project.earned_value.records.iter().find(|r| r.activity == "A10").unwrap().clone();
        assert_eq!((r.period.as_str(), r.earned, r.actual), ("2026-11", 30_000, 35_000));
        assert!(s.project.earned_value.records.iter().all(|r| r.period == "2026-11"));
        assert!(!s.project.earned_value.records.iter().any(|r| r.activity == "W1"), "summaries take their children's");
        let count = s.project.earned_value.records.len();
        run(&mut s, "period.store", json!({"name": "2026-11"}));
        assert_eq!(s.project.earned_value.records.len(), count, "replaced, not added");
        let points = mkperth_sched::period_points(&s.project, s.schedule.as_ref().unwrap());
        assert_eq!(points.iter().find(|q| q.period == "2026-11").and_then(|q| q.earned), Some(30_000));
        // Renamed, an activity carries what was stored for it.
        run(&mut s, "activity.edit", json!({"id": "A10", "new_id": "A11"}));
        assert!(s.project.earned_value.records.iter().any(|r| r.activity == "A11"));
        run(&mut s, "period.delete", json!({"name": "2026-11"}));
        assert!(s.project.earned_value.records.is_empty());
        // The techniques, and a percent out of bounds refused.
        run(&mut s, "project.options", json!({"ev_technique": "custom", "ev_share": 25, "etc_technique": "cpi"}));
        assert_eq!(s.project.earned_value.technique, mkperth_core::EvTechnique::Custom(2_500));
        let before = s.project.clone();
        assert!(s.run("activity.edit", &json!({"id": "A10", "percent_complete": 101})).is_err());
        assert!(s.run("project.options", &json!({"ev_technique": "magic"})).is_err());
        assert!(s.run("period.generate", &json!({"from": "2026-11-01", "to": "2026-12-01", "every": "day"})).is_err());
        assert_eq!(s.project, before);
    }
}
