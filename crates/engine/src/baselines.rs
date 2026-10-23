//! Baselines: the plan frozen to measure the schedule against. Assigning one keeps each
//! activity's scheduled dates and duration under a name; one of them is in use, for the bars
//! under the schedule's and the variances. Each change is a command with undo.

use mkperth_core::{Baseline, BaselineActivity};
use mkperth_i18n::tr;
use serde_json::Value;

use crate::{EngineError, Session};

/// The most baselines a project may keep.
pub const MAX_BASELINES: usize = 50;
/// The longest a baseline's name may be, in characters.
const MAX_NAME: usize = 200;

/// The `name` given, trimmed and not too long; `None` when not given.
fn name(params: &Value, command: &str) -> Result<Option<String>, EngineError> {
    match params.get("name") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if !s.trim().is_empty() && s.chars().count() <= MAX_NAME => Ok(Some(s.trim().to_string())),
        Some(_) => Err(EngineError::Value(command.into(), "name".into())),
    }
}

impl Session {
    /// The baseline commands; `None` when `id` isn't one of them.
    pub(crate) fn baseline_command(&mut self, id: &str, params: &Value) -> Option<Result<(), EngineError>> {
        Some(match id {
            "baseline.assign" => self.assign_baseline(params),
            "baseline.use" => self.use_baseline(params),
            "baseline.delete" => self.delete_baseline(params),
            _ => return None,
        })
    }

    /// Freezes the schedule as it is under `name` (else "Baseline" and the next number): each
    /// activity's early start and finish and its duration (a summary's, its working days
    /// between them). A baseline of that name is taken again; the first is put in use.
    fn assign_baseline(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "baseline.assign";
        let schedule = self.schedule.clone()?;
        let name = name(params, C)?.unwrap_or_else(|| {
            let n = (1..=MAX_BASELINES + 1).find(|n| !self.project.baselines.iter().any(|b| b.name == tr!("baseline-numbered", n = *n))).unwrap_or(1);
            tr!("baseline-numbered", n = n)
        });
        self.change(tr!("undo-assign-baseline", name = name.as_str()), |p| {
            let parents: std::collections::HashSet<String> = p.activities.iter().filter_map(|a| a.parent.clone()).collect();
            let activities = p
                .activities
                .iter()
                .zip(&schedule.activities)
                .map(|(a, s)| {
                    let calendar = a.calendar.as_deref().and_then(|n| p.calendars.iter().find(|c| c.name == n)).unwrap_or(&p.calendar);
                    let day = calendar.day_minutes();
                    let duration = if parents.contains(&a.id) {
                        (mkperth_sched::working_days_between(calendar, s.early_start, s.early_finish) + 1).max(0) * day
                    } else {
                        a.working_minutes(day)
                    };
                    BaselineActivity { id: a.id.clone(), start: s.early_start, finish: s.early_finish, duration }
                })
                .collect();
            let baseline = Baseline { name: name.clone(), activities };
            let full = p.baselines.len() >= MAX_BASELINES;
            match p.baselines.iter_mut().find(|b| b.name == name) {
                Some(b) => *b = baseline,
                None if full => return Err(EngineError::Value(C.into(), "name".into())),
                None => p.baselines.push(baseline),
            }
            if p.baseline().is_none() {
                p.baseline_in_use = Some(name.clone());
            }
            Ok(())
        })
    }

    /// Compares the schedule with the baseline `name`, or with none (`null`).
    fn use_baseline(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "baseline.use";
        let name = name(params, C)?;
        self.change(tr!("undo-use-baseline"), |p| {
            if let Some(n) = &name
                && !p.baselines.iter().any(|b| b.name == *n)
            {
                return Err(EngineError::Value(C.into(), "name".into()));
            }
            p.baseline_in_use = name;
            Ok(())
        })
    }

    /// Deletes the baseline `name`; in use, none is.
    fn delete_baseline(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "baseline.delete";
        let name = name(params, C)?.ok_or_else(|| EngineError::Param(C.into(), "name".into()))?;
        self.change(tr!("undo-delete-baseline", name = name.as_str()), |p| {
            let before = p.baselines.len();
            p.baselines.retain(|b| b.name != name);
            if p.baselines.len() == before {
                return Err(EngineError::Value(C.into(), "name".into()));
            }
            if p.baseline_in_use.as_deref() == Some(name.as_str()) {
                p.baseline_in_use = None;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// The sample frozen, then B10 held a week later: its variances against the baseline; a
    /// second baseline taken, used, deleted; each one undo.
    #[test]
    fn baselines_are_assigned_used_and_deleted() {
        let mut s = Session::default();
        s.run("baseline.assign", &Value::Null).unwrap();
        assert_eq!(s.project.baseline_in_use.as_deref(), Some("Baseline 1"));
        let k = s.project.activities.iter().position(|a| a.id == "B10").unwrap();
        let schedule = s.schedule.clone().unwrap();
        let start = schedule.activities[k].early_start;
        assert_eq!(s.project.baseline().unwrap().activities[k].start, start);
        // Five working days later (B10 is on the project's Monday-to-Friday calendar).
        let later =
            (0..14).filter_map(|n| mkperth_core::Date::from_days(start.days() + n)).filter(|d| s.project.calendar.is_working(*d)).nth(5).unwrap();
        s.run("activity.edit", &json!({"id": "B10", "constraint": {"type": "start-on-or-after", "date": later.to_string()}})).unwrap();
        let v = mkperth_sched::variances(&s.project, s.schedule.as_ref().unwrap());
        assert_eq!(v[k].map(|v| (v.start, v.finish, v.duration)), Some((-5, -5, 0)));
        s.run("baseline.assign", &json!({"name": "Revised"})).unwrap();
        assert_eq!(s.project.baselines.len(), 2);
        assert_eq!(s.project.baseline_in_use.as_deref(), Some("Baseline 1"), "the first stays in use");
        s.run("baseline.use", &json!({"name": "Revised"})).unwrap();
        assert_eq!(mkperth_sched::variances(&s.project, s.schedule.as_ref().unwrap())[k].map(|v| v.start), Some(0));
        s.run("baseline.delete", &json!({"name": "Revised"})).unwrap();
        assert_eq!(s.project.baseline_in_use, None);
        assert!(s.run("baseline.use", &json!({"name": "Nope"})).is_err());
        assert!(s.run("baseline.delete", &json!({"name": "Nope"})).is_err());
        s.run("edit.undo", &Value::Null).unwrap();
        assert_eq!(s.project.baselines.len(), 2);
    }
}
