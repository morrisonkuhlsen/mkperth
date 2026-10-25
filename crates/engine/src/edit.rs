//! The commands that change the project, and undo and redo. Each change is made on a copy and
//! kept only if it holds together (no loop of relationships, no repeated ID); the project before
//! it goes on the undo stack.

use mkperth_core::{
    Activity, ActivityKind, Calendar, Constraint, CriticalBy, Date, FloatKind, Period, ProgressMode, Project, Region, Relationship, RelationshipType,
};
use mkperth_i18n::tr;
use mkperth_sched::ScheduleError;
use serde_json::Value;

use crate::{EngineError, Session};

/// How many changes undo remembers.
pub const UNDO_DEPTH: usize = 200;
/// The longest ID or name a command takes, in characters.
const MAX_TEXT: usize = 2000;
/// The longest an activity's notes may be, in characters.
const MAX_NOTES: usize = 100_000;
/// The most dates one calendar edit adds or removes, and the most work periods in a day.
const MAX_DATES: usize = 10_000;
const MAX_PERIODS: usize = 8;

/// A command's parameter as text, if given; an error if given and not text.
fn text(params: &Value, command: &str, name: &str) -> Result<Option<String>, EngineError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.chars().count() <= MAX_TEXT => Ok(Some(s.clone())),
        Some(_) => Err(EngineError::Value(command.to_string(), name.to_string())),
    }
}

/// A command's parameter as a whole number within `range`, if given.
fn number(params: &Value, command: &str, name: &str, range: std::ops::RangeInclusive<i64>) -> Result<Option<i64>, EngineError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_i64().filter(|n| range.contains(n)).map(Some).ok_or_else(|| EngineError::Value(command.to_string(), name.to_string())),
    }
}

/// A date parameter: absent leaves it as it is, `null` clears it, `"2026-10-07"` sets it.
fn date(params: &Value, command: &str, name: &str) -> Result<Option<Option<Date>>, EngineError> {
    match params.get(name) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(v) => v.as_str().and_then(Date::parse).map(|d| Some(Some(d))).ok_or_else(|| EngineError::Value(command.to_string(), name.to_string())),
    }
}

/// A number parameter that `null` clears.
fn clearable(params: &Value, command: &str, name: &str, range: std::ops::RangeInclusive<i64>) -> Result<Option<Option<i64>>, EngineError> {
    match params.get(name) {
        Some(Value::Null) => Ok(Some(None)),
        _ => Ok(number(params, command, name, range)?.map(Some)),
    }
}

/// A list of dates (`["2026-12-24", …]`), if given.
fn dates(params: &Value, command: &str, name: &str) -> Result<Vec<Date>, EngineError> {
    let bad = || EngineError::Value(command.to_string(), name.to_string());
    match params.get(name) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) if items.len() <= MAX_DATES => items.iter().map(|d| d.as_str().and_then(Date::parse).ok_or_else(bad)).collect(),
        Some(_) => Err(bad()),
    }
}

/// A day's work periods, `[[480, 720], [780, 1020]]` in minutes after midnight: each starts before
/// it ends, within the day, after the one before; if given.
fn periods(params: &Value, command: &str) -> Result<Option<Vec<Period>>, EngineError> {
    let bad = || EngineError::Value(command.to_string(), "periods".to_string());
    let Some(value) = params.get("periods").filter(|v| !v.is_null()) else { return Ok(None) };
    let items = value.as_array().filter(|a| !a.is_empty() && a.len() <= MAX_PERIODS).ok_or_else(bad)?;
    let mut out: Vec<Period> = Vec::with_capacity(items.len());
    for item in items {
        let minute = |k: usize| item.get(k).and_then(Value::as_u64).filter(|m| *m <= 1440).and_then(|m| u16::try_from(m).ok());
        let (from, to) = (minute(0).ok_or_else(bad)?, minute(1).ok_or_else(bad)?);
        if from >= to || out.last().is_some_and(|(_, end)| *end > from) {
            return Err(bad());
        }
        out.push((from, to));
    }
    Ok(Some(out))
}

fn required(value: Option<String>, command: &str, name: &str) -> Result<String, EngineError> {
    value.filter(|s| !s.trim().is_empty()).ok_or_else(|| EngineError::Param(command.to_string(), name.to_string()))
}

fn kind_of(code: &str) -> Option<ActivityKind> {
    match code {
        "task" => Some(ActivityKind::Task),
        "start-milestone" => Some(ActivityKind::StartMilestone),
        "finish-milestone" => Some(ActivityKind::FinishMilestone),
        _ => None,
    }
}

/// A constraint from `{"type": "start-on-or-after", "date": "2026-11-02"}`; `null` for none.
fn constraint_of(value: &Value, command: &str) -> Result<Option<Constraint>, EngineError> {
    if value.is_null() {
        return Ok(None);
    }
    let bad = || EngineError::Value(command.to_string(), "constraint".to_string());
    let kind = value.get("type").and_then(Value::as_str).ok_or_else(bad)?;
    if kind == "as-late-as-possible" {
        return Ok(Some(Constraint::AsLateAsPossible));
    }
    let date = value.get("date").and_then(Value::as_str).and_then(Date::parse).ok_or_else(bad)?;
    Ok(Some(match kind {
        "start-on-or-after" => Constraint::StartOnOrAfter(date),
        "start-on-or-before" => Constraint::StartOnOrBefore(date),
        "finish-on-or-after" => Constraint::FinishOnOrAfter(date),
        "finish-on-or-before" => Constraint::FinishOnOrBefore(date),
        "start-on" => Constraint::StartOn(date),
        "finish-on" => Constraint::FinishOn(date),
        "mandatory-start" => Constraint::MandatoryStart(date),
        "mandatory-finish" => Constraint::MandatoryFinish(date),
        _ => return Err(bad()),
    }))
}

/// An ID not in the project: `A10`, `A20`, … after the highest such.
fn new_id(project: &Project) -> String {
    let highest = project.activities.iter().filter_map(|a| a.id.strip_prefix('A').and_then(|n| n.parse::<u64>().ok())).max().unwrap_or(0);
    let mut n = (highest / 10).saturating_add(1).saturating_mul(10);
    loop {
        let id = format!("A{n}");
        if !project.activities.iter().any(|a| a.id == id) {
            return id;
        }
        n = n.saturating_add(10);
    }
}

/// The activities a chain names (`activities`: two IDs or more), if given.
fn chain(params: &Value, command: &str) -> Result<Option<Vec<String>>, EngineError> {
    match params.get("activities") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(items)) if (2..=mkperth_sched::MAX_ACTIVITIES).contains(&items.len()) => items
            .iter()
            .map(|i| i.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()
            .map(Some)
            .ok_or_else(|| EngineError::Value(command.into(), "activities".into())),
        Some(_) => Err(EngineError::Value(command.into(), "activities".into())),
    }
}

fn find(project: &Project, id: &str) -> Result<usize, EngineError> {
    project.activities.iter().position(|a| a.id == id).ok_or_else(|| EngineError::UnknownActivity(id.to_string()))
}

impl Session {
    /// What undo would undo, if anything.
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|(label, _)| label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|(label, _)| label.as_str())
    }

    /// Makes a change on a copy of the project; keeps it, with the old project on the undo stack,
    /// if `change` succeeds and leaves no loop of relationships or of parents.
    pub(crate) fn change(&mut self, label: String, change: impl FnOnce(&mut Project) -> Result<(), EngineError>) -> Result<(), EngineError> {
        let mut project = self.project.clone();
        change(&mut project)?;
        let schedule = mkperth_sched::schedule(&project);
        if let Err(e @ (ScheduleError::Loop(_) | ScheduleError::ParentLoop(_) | ScheduleError::MilestoneParent(_))) = schedule {
            return Err(e.into());
        }
        let old = std::mem::replace(&mut self.project, project);
        self.undo.push((label, old));
        if self.undo.len() > UNDO_DEPTH {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.schedule = schedule;
        self.modified = true;
        Ok(())
    }

    pub(crate) fn undo_change(&mut self) -> Result<(), EngineError> {
        let (label, project) = self.undo.pop().ok_or(EngineError::NothingToUndo)?;
        let now = std::mem::replace(&mut self.project, project);
        self.redo.push((label, now));
        self.modified = true;
        self.reschedule();
        Ok(())
    }

    pub(crate) fn redo_change(&mut self) -> Result<(), EngineError> {
        let (label, project) = self.redo.pop().ok_or(EngineError::NothingToRedo)?;
        let now = std::mem::replace(&mut self.project, project);
        self.undo.push((label, now));
        self.modified = true;
        self.reschedule();
        Ok(())
    }

    /// The editing commands; `None` when `id` isn't one of them.
    pub(crate) fn run_edit(&mut self, id: &str, params: &Value) -> Option<Result<(), EngineError>> {
        Some(match id {
            "activity.add" => self.add_activity(params),
            "activity.edit" => self.edit_activity(params),
            "activity.delete" => self.delete_activity(params),
            "relationship.add" => self.add_relationship(params),
            "relationship.delete" => self.delete_relationship(params),
            "project.options" => self.set_options(params),
            "calendar.edit" => self.edit_calendar(params),
            "edit.undo" => self.undo_change(),
            "edit.redo" => self.redo_change(),
            _ => return None,
        })
    }

    /// The scheduling options: how total float is measured (`float`: finish, start, smallest),
    /// what is critical (`critical`: float, with `threshold` working days, or longest-path), the
    /// `data_date` progress is told at, and how out-of-sequence progress is scheduled
    /// (`progress`: retained-logic or progress-override).
    fn set_options(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "project.options";
        let bad = |name: &str| EngineError::Value(C.into(), name.into());
        let float = text(params, C, "float")?
            .map(|f| match f.as_str() {
                "finish" => Ok(FloatKind::Finish),
                "start" => Ok(FloatKind::Start),
                "smallest" => Ok(FloatKind::Smallest),
                _ => Err(bad("float")),
            })
            .transpose()?;
        let threshold = number(params, C, "threshold", -mkperth_sched::MAX_SPAN..=mkperth_sched::MAX_SPAN)?;
        let critical = text(params, C, "critical")?
            .map(|c| match c.as_str() {
                "float" => Ok(CriticalBy::TotalFloat(threshold.unwrap_or(0))),
                "longest-path" => Ok(CriticalBy::LongestPath),
                _ => Err(bad("critical")),
            })
            .transpose()?;
        let data_date = date(params, C, "data_date")?;
        // The currency's symbol, written before amounts (empty for none).
        let currency = match params.get("currency") {
            None | Some(Value::Null) => None,
            Some(Value::String(c)) if c.chars().count() <= 8 => Some(c.trim().to_string()),
            Some(_) => return Err(bad("currency")),
        };
        let progress = text(params, C, "progress")?
            .map(|m| match m.as_str() {
                "retained-logic" => Ok(ProgressMode::RetainedLogic),
                "progress-override" => Ok(ProgressMode::ProgressOverride),
                _ => Err(bad("progress")),
            })
            .transpose()?;
        self.change(tr!("undo-options"), |p| {
            if let Some(f) = float {
                p.float_kind = f;
            }
            if let Some(d) = data_date {
                p.data_date = d;
            }
            if let Some(c) = currency {
                p.currency = c;
            }
            if let Some(m) = progress {
                p.progress_mode = m;
            }
            match (critical, threshold) {
                (Some(c), _) => p.critical_by = c,
                (None, Some(t)) => p.critical_by = CriticalBy::TotalFloat(t),
                (None, None) => {}
            }
            Ok(())
        })
    }

    /// Changes a calendar: the project's own, or the one named `calendar`. `week` (seven
    /// booleans, Monday first), `periods` (each worked weekday's work periods), dates added to or
    /// taken from its holidays (`holidays_add`, `holidays_remove`) and its worked days off the
    /// week (`worked_add`, `worked_remove`), and the `region` whose holidays it keeps
    /// (`"brazil"`, or `null` for none). A week with no working day is refused.
    fn edit_calendar(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "calendar.edit";
        let bad = |name: &str| EngineError::Value(C.into(), name.into());
        let name = text(params, C, "calendar")?;
        let week = match params.get("week") {
            None | Some(Value::Null) => None,
            Some(v) => {
                let days: Vec<bool> =
                    v.as_array().filter(|a| a.len() == 7).map(|a| a.iter().filter_map(Value::as_bool).collect()).ok_or_else(|| bad("week"))?;
                let week: [bool; 7] = days.try_into().map_err(|_| bad("week"))?;
                if !week.contains(&true) {
                    return Err(bad("week"));
                }
                Some(week)
            }
        };
        let periods = periods(params, C)?;
        let (holidays_add, holidays_remove) = (dates(params, C, "holidays_add")?, dates(params, C, "holidays_remove")?);
        let (worked_add, worked_remove) = (dates(params, C, "worked_add")?, dates(params, C, "worked_remove")?);
        let region = match params.get("region") {
            None => None,
            Some(Value::Null) => Some(None),
            Some(v) => Some(Some(v.as_str().and_then(Region::named).ok_or_else(|| bad("region"))?)),
        };
        let nothing = week.is_none()
            && periods.is_none()
            && region.is_none()
            && [&holidays_add, &holidays_remove, &worked_add, &worked_remove].iter().all(|d| d.is_empty());
        if nothing {
            return Err(EngineError::Param(C.into(), "week".into()));
        }
        let shown = name.clone().unwrap_or_else(|| self.project.calendar.name.clone());
        self.change(tr!("undo-edit-calendar", name = shown.as_str()), |p| {
            let calendar: &mut Calendar = match name.as_deref() {
                None => &mut p.calendar,
                Some(n) if n == p.calendar.name => &mut p.calendar,
                Some(n) => p.calendars.iter_mut().find(|c| c.name == n).ok_or_else(|| bad("calendar"))?,
            };
            if let Some(w) = week {
                calendar.week = w;
            }
            if let Some(ps) = periods {
                calendar.periods = std::array::from_fn(|_| ps.clone());
            }
            for d in holidays_remove {
                calendar.holidays.remove(&d);
            }
            calendar.holidays.extend(holidays_add);
            for d in worked_remove {
                calendar.extra_days.remove(&d);
            }
            calendar.extra_days.extend(worked_add);
            if let Some(r) = region {
                calendar.region = r;
            }
            Ok(())
        })
    }

    fn add_activity(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "activity.add";
        let id = text(params, C, "id")?.filter(|s| !s.trim().is_empty());
        let name = text(params, C, "name")?;
        let duration = number(params, C, "duration", 0..=mkperth_sched::MAX_SPAN)?.unwrap_or(5);
        let after = text(params, C, "after")?;
        let parent = text(params, C, "parent")?;
        let id = id.unwrap_or_else(|| new_id(&self.project));
        let label = tr!("undo-add-activity", id = id.as_str());
        self.change(label, |p| {
            if p.activities.iter().any(|a| a.id == id) {
                return Err(EngineError::DuplicateId(id.clone()));
            }
            // After an activity: in its place in the WBS, under the same summary.
            let (at, inherited) = match after.as_deref() {
                Some(a) => {
                    let k = find(p, a)?;
                    (k + 1, p.activities.get(k).and_then(|x| x.parent.clone()))
                }
                None => (p.activities.len(), None),
            };
            if let Some(parent) = parent.as_deref().filter(|s| !s.is_empty()) {
                find(p, parent)?;
            }
            let activity = Activity {
                id: id.clone(),
                name: name.unwrap_or_else(|| tr!("new-activity")),
                duration,
                parent: parent.filter(|s| !s.is_empty()).or(inherited),
                ..Activity::default()
            };
            p.activities.insert(at.min(p.activities.len()), activity);
            Ok(())
        })
    }

    fn edit_activity(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "activity.edit";
        let id = required(text(params, C, "id")?, C, "id")?;
        let new_id = text(params, C, "new_id")?.map(|s| s.trim().to_string());
        let name = text(params, C, "name")?;
        let duration = number(params, C, "duration", 0..=mkperth_sched::MAX_SPAN)?;
        let minutes = number(params, C, "duration_minutes", 0..=mkperth_sched::MAX_SPAN * 1440)?;
        let kind = text(params, C, "kind")?.map(|k| kind_of(&k).ok_or_else(|| EngineError::Value(C.into(), "kind".into()))).transpose()?;
        let duration_type = text(params, C, "duration_type")?
            .map(|d| mkperth_core::DurationType::from_code(&d).ok_or_else(|| EngineError::Value(C.into(), "duration_type".into())))
            .transpose()?;
        let calendar = text(params, C, "calendar")?;
        // PERT's estimates in days (`{"optimistic", "most_likely", "pessimistic"}`, O ≤ M ≤ P), or
        // `null` for none: for the analysis only, never the dates.
        let pert = match params.get("pert") {
            None => None,
            Some(Value::Null) => Some(None),
            Some(v) => {
                let bad = || EngineError::Value(C.into(), "pert".into());
                let field = |k: &str| -> Result<i64, EngineError> {
                    v.get(k)
                        .and_then(Value::as_f64)
                        .filter(|d| d.is_finite() && (0.0..=mkperth_sched::MAX_SPAN as f64).contains(d))
                        .map(|d| (d * mkperth_core::UNIT as f64).round() as i64)
                        .ok_or_else(bad)
                };
                let p =
                    mkperth_core::Pert { optimistic: field("optimistic")?, most_likely: field("most_likely")?, pessimistic: field("pessimistic")? };
                if !p.ordered() {
                    return Err(bad());
                }
                Some((p.pessimistic > 0).then_some(p))
            }
        };
        // The summary it goes under; empty for the top of the WBS.
        let parent = text(params, C, "parent")?.map(|s| s.trim().to_string());
        let priority = text(params, C, "levelling_priority")?
            .map(|d| mkperth_core::LevellingPriority::from_code(&d).ok_or_else(|| EngineError::Value(C.into(), "levelling_priority".into())))
            .transpose()?;
        let resource_dependent = match params.get("resource_dependent") {
            None | Some(Value::Null) => None,
            Some(v) => Some(v.as_bool().ok_or_else(|| EngineError::Value(C.into(), "resource_dependent".into()))?),
        };
        let notes = match params.get("notes") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) if s.chars().count() <= MAX_NOTES => Some(s.clone()),
            Some(_) => return Err(EngineError::Value(C.into(), "notes".into())),
        };
        let constraint = params.get("constraint").map(|c| constraint_of(c, C)).transpose()?;
        let secondary = params.get("secondary_constraint").map(|c| constraint_of(c, C)).transpose()?;
        let expected = date(params, C, "expected_finish")?;
        // Progress: the actual dates, and the work left in days or in working minutes.
        let actual_start = date(params, C, "actual_start")?;
        let actual_finish = date(params, C, "actual_finish")?;
        let remaining = clearable(params, C, "remaining", 0..=mkperth_sched::MAX_SPAN)?;
        let remaining_minutes = clearable(params, C, "remaining_minutes", 0..=mkperth_sched::MAX_SPAN * 1440)?;
        let label = tr!("undo-edit-activity", id = id.as_str());
        self.change(label, |p| {
            let k = find(p, &id)?;
            if let Some(cal) = calendar.as_deref().filter(|c| !c.is_empty() && *c != p.calendar.name)
                && !p.calendars.iter().any(|x| x.name == cal)
            {
                return Err(EngineError::Value(C.into(), "calendar".into()));
            }
            if let Some(new) = new_id.as_deref().filter(|n| *n != id) {
                if new.is_empty() {
                    return Err(EngineError::Value(C.into(), "new_id".into()));
                }
                if p.activities.iter().any(|a| a.id == new) {
                    return Err(EngineError::DuplicateId(new.to_string()));
                }
                // The ID changes everywhere it's named.
                for r in &mut p.relationships {
                    for end in [&mut r.predecessor, &mut r.successor] {
                        if *end == id {
                            *end = new.to_string();
                        }
                    }
                }
                for a in &mut p.activities {
                    if a.parent.as_deref() == Some(id.as_str()) {
                        a.parent = Some(new.to_string());
                    }
                }
                for a in p.assignments.iter_mut().filter(|a| a.activity == id) {
                    a.activity = new.to_string();
                }
                for c in p.comments.iter_mut().filter(|c| c.activity == id) {
                    c.activity = new.to_string();
                }
                for e in p.expenses.iter_mut().filter(|e| e.activity == id) {
                    e.activity = new.to_string();
                }
                for b in p.budget.budgets.iter_mut().filter(|b| b.wbs.as_deref() == Some(id.as_str())) {
                    b.wbs = Some(new.to_string());
                }
                for c in p.budget.log.iter_mut().filter(|c| c.wbs.as_deref() == Some(id.as_str())) {
                    c.wbs = Some(new.to_string());
                }
            }
            if let Some(par) = parent.as_deref().filter(|s| !s.is_empty()) {
                if par == id {
                    return Err(EngineError::Value(C.into(), "parent".into()));
                }
                find(p, par)?;
            }
            // Its assignments' units an hour, as they are before a new duration.
            let resized = duration.is_some() || minutes.is_some();
            if resized {
                let before = crate::resources::activity_minutes(p, k);
                crate::resources::before_duration(p, k, before);
            }
            let Some(a) = p.activities.get_mut(k) else { return Err(EngineError::UnknownActivity(id.clone())) };
            if let Some(new) = new_id.filter(|n| !n.is_empty()) {
                a.id = new;
            }
            if let Some(d) = duration_type {
                a.duration_type = d;
            }
            if let Some(r) = resource_dependent {
                a.resource_dependent = r;
            }
            if let Some(par) = parent {
                a.parent = (!par.is_empty()).then_some(par);
            }
            if let Some(p) = pert {
                a.pert = p;
            }
            if let Some(l) = priority {
                a.levelling_priority = l;
            }
            if let Some(name) = name {
                a.name = name;
            }
            // Whole days, or working minutes for a part day (its days rounded up, for showing).
            if let Some(d) = duration {
                a.duration = d;
                a.duration_minutes = None;
            }
            if let Some(m) = minutes {
                a.duration_minutes = Some(m);
                a.duration = (m + 479) / 480;
            }
            if let Some(kind) = kind {
                a.kind = kind;
            }
            if let Some(cal) = calendar {
                a.calendar = (!cal.is_empty()).then_some(cal);
            }
            if let Some(c) = constraint {
                a.constraint = c;
            }
            if let Some(c) = secondary {
                a.secondary_constraint = c;
            }
            // A secondary stands only beside a primary it pairs with.
            if let Some(s) = a.secondary_constraint
                && !a.constraint.is_some_and(|p| Constraint::pairs(p, s))
            {
                return Err(EngineError::ConstraintPair(a.id.clone()));
            }
            if let Some(d) = expected {
                a.expected_finish = d;
            }
            if let Some(n) = notes {
                a.notes = n;
            }
            if let Some(d) = actual_start {
                a.actual_start = d;
            }
            if let Some(d) = actual_finish {
                a.actual_finish = d;
            }
            if let (Some(s), Some(f)) = (a.actual_start, a.actual_finish)
                && f < s
            {
                return Err(EngineError::Value(C.into(), "actual_finish".into()));
            }
            if let Some(r) = remaining {
                let day = p
                    .calendars
                    .iter()
                    .chain(std::iter::once(&p.calendar))
                    .find(|c| Some(&c.name) == a.calendar.as_ref())
                    .unwrap_or(&p.calendar)
                    .day_minutes();
                a.remaining_minutes = r.map(|d| d.saturating_mul(day));
            }
            if let Some(m) = remaining_minutes {
                a.remaining_minutes = m;
            }
            // Its assignments by its duration type: their units follow, or their units an hour.
            if resized {
                crate::resources::after_duration(p, k);
            }
            Ok(())
        })
    }

    /// Deletes the activity `id`, or every one of `ids` as one change (one undo).
    fn delete_activity(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "activity.delete";
        let ids: Vec<String> = match params.get("ids") {
            None | Some(Value::Null) => vec![required(text(params, C, "id")?, C, "id")?],
            Some(Value::Array(items)) if !items.is_empty() && items.len() <= 100_000 => items
                .iter()
                .map(|i| i.as_str().filter(|s| !s.trim().is_empty()).map(str::to_string))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| EngineError::Value(C.into(), "ids".into()))?,
            Some(_) => return Err(EngineError::Value(C.into(), "ids".into())),
        };
        let label = match ids.as_slice() {
            [one] => tr!("undo-delete-activity", id = one.as_str()),
            many => tr!("undo-delete-activities", n = many.len()),
        };
        self.change(label, |p| {
            for id in &ids {
                let k = find(p, id)?;
                let removed = p.activities.remove(k);
                // Its relationships and assignments go with it; its children move up to its parent.
                p.relationships.retain(|r| r.predecessor != *id && r.successor != *id);
                p.assignments.retain(|a| a.activity != *id);
                p.comments.retain(|c| c.activity != *id);
                p.expenses.retain(|e| e.activity != *id);
                p.budget.budgets.retain(|b| b.wbs.as_deref() != Some(id.as_str()));
                p.budget.log.retain(|c| c.wbs.as_deref() != Some(id.as_str()));
                for a in &mut p.activities {
                    if a.parent.as_deref() == Some(id.as_str()) {
                        a.parent = removed.parent.clone();
                    }
                }
            }
            Ok(())
        })
    }

    fn add_relationship(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "relationship.add";
        let kind = match text(params, C, "kind")? {
            None => RelationshipType::FinishToStart,
            Some(k) => RelationshipType::from_code(&k).ok_or_else(|| EngineError::Value(C.into(), "kind".into()))?,
        };
        let lag = number(params, C, "lag", -mkperth_sched::MAX_SPAN..=mkperth_sched::MAX_SPAN)?.unwrap_or(0);
        // A chain: each of `activities` the predecessor of the next, as one change; ties already
        // there are kept as they are.
        if let Some(ids) = chain(params, C)? {
            return self.change(tr!("undo-link", n = ids.len()), |p| {
                for id in &ids {
                    find(p, id)?;
                }
                let mut added = 0;
                for pair in ids.windows(2) {
                    let (Some(a), Some(b)) = (pair.first(), pair.get(1)) else { continue };
                    if a == b {
                        return Err(ScheduleError::SelfRelationship(a.clone()).into());
                    }
                    if !p.relationships.iter().any(|r| r.predecessor == *a && r.successor == *b) {
                        p.relationships.push(Relationship { predecessor: a.clone(), successor: b.clone(), kind, lag, lag_minutes: None });
                        added += 1;
                    }
                }
                if added == 0 {
                    let (first, last) = (ids.first().cloned().unwrap_or_default(), ids.last().cloned().unwrap_or_default());
                    return Err(EngineError::DuplicateRelationship(first, last));
                }
                Ok(())
            });
        }
        let predecessor = required(text(params, C, "predecessor")?, C, "predecessor")?;
        let successor = required(text(params, C, "successor")?, C, "successor")?;
        let label = tr!("undo-add-relationship", predecessor = predecessor.as_str(), successor = successor.as_str());
        self.change(label, |p| {
            find(p, &predecessor)?;
            find(p, &successor)?;
            if predecessor == successor {
                return Err(ScheduleError::SelfRelationship(predecessor.clone()).into());
            }
            if p.relationships.iter().any(|r| r.predecessor == predecessor && r.successor == successor) {
                return Err(EngineError::DuplicateRelationship(predecessor.clone(), successor.clone()));
            }
            p.relationships.push(Relationship { predecessor: predecessor.clone(), successor: successor.clone(), kind, lag, lag_minutes: None });
            Ok(())
        })
    }

    fn delete_relationship(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "relationship.delete";
        // Every tie between any two of `activities`, as one change.
        if let Some(ids) = chain(params, C)? {
            return self.change(tr!("undo-unlink", n = ids.len()), |p| {
                let before = p.relationships.len();
                p.relationships.retain(|r| !(ids.contains(&r.predecessor) && ids.contains(&r.successor)));
                if p.relationships.len() == before {
                    let (first, last) = (ids.first().cloned().unwrap_or_default(), ids.last().cloned().unwrap_or_default());
                    return Err(EngineError::UnknownRelationship(first, last));
                }
                Ok(())
            });
        }
        let predecessor = required(text(params, C, "predecessor")?, C, "predecessor")?;
        let successor = required(text(params, C, "successor")?, C, "successor")?;
        let label = tr!("undo-delete-relationship", predecessor = predecessor.as_str(), successor = successor.as_str());
        self.change(label, |p| {
            let before = p.relationships.len();
            p.relationships.retain(|r| !(r.predecessor == predecessor && r.successor == successor));
            if p.relationships.len() == before {
                return Err(EngineError::UnknownRelationship(predecessor.clone(), successor.clone()));
            }
            Ok(())
        })
    }
}
