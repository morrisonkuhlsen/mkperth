//! The commands that change the project, and undo and redo. Each change is made on a copy and
//! kept only if it holds together (no loop of relationships, no repeated ID); the project before
//! it goes on the undo stack.

use mkperth_core::{Activity, ActivityKind, Constraint, CriticalBy, Date, FloatKind, ProgressMode, Project, Relationship, RelationshipType};
use mkperth_i18n::tr;
use mkperth_sched::ScheduleError;
use serde_json::Value;

use crate::{EngineError, Session};

/// How many changes undo remembers.
pub const UNDO_DEPTH: usize = 200;
/// The longest ID or name a command takes, in characters.
const MAX_TEXT: usize = 2000;

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
    fn change(&mut self, label: String, change: impl FnOnce(&mut Project) -> Result<(), EngineError>) -> Result<(), EngineError> {
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
        let calendar = text(params, C, "calendar")?;
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
            }
            let Some(a) = p.activities.get_mut(k) else { return Err(EngineError::UnknownActivity(id.clone())) };
            if let Some(new) = new_id.filter(|n| !n.is_empty()) {
                a.id = new;
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
            Ok(())
        })
    }

    fn delete_activity(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "activity.delete";
        let id = required(text(params, C, "id")?, C, "id")?;
        let label = tr!("undo-delete-activity", id = id.as_str());
        self.change(label, |p| {
            let k = find(p, &id)?;
            let removed = p.activities.remove(k);
            // Its relationships go with it; its children move up to its parent.
            p.relationships.retain(|r| r.predecessor != id && r.successor != id);
            for a in &mut p.activities {
                if a.parent.as_deref() == Some(id.as_str()) {
                    a.parent = removed.parent.clone();
                }
            }
            Ok(())
        })
    }

    fn add_relationship(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "relationship.add";
        let predecessor = required(text(params, C, "predecessor")?, C, "predecessor")?;
        let successor = required(text(params, C, "successor")?, C, "successor")?;
        let kind = match text(params, C, "kind")? {
            None => RelationshipType::FinishToStart,
            Some(k) => RelationshipType::from_code(&k).ok_or_else(|| EngineError::Value(C.into(), "kind".into()))?,
        };
        let lag = number(params, C, "lag", -mkperth_sched::MAX_SPAN..=mkperth_sched::MAX_SPAN)?.unwrap_or(0);
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
