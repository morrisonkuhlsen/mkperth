//! Perth.jl's project JSON (the files under `~/.perth`, and its export), read into MK Perth's
//! model and written back without loss.
//!
//! What maps: a task's `id`, `name`, `duration` and `milestone`; its `start` as a start on or
//! after (Perth's scheduler only ever pushes a task later than its start); its `deadline` as a
//! secondary finish on or before (Perth caps the late finish with it); its `dependencies` —
//! `id` finish-to-start, `id+3` and `id-2` with a lag or lead, `SS:id` and `FF:id` — as
//! relationships; its `parent` as its place in the WBS; the project's `name` and `calendar` (empty for calendar days, a business-day
//! calendar's name otherwise). Everything else, the task's and the project's, travels in
//! `extra` under `perth.<field>` as the JSON it was, and is written back in Perth's order, so a
//! file read and written is the same JSON. Source: Perth.jl's `types.jl` and `schedule.jl`
//! (the user's own package, MIT).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use mkperth_core::{
    Activity, ActivityKind, Assignment, Band, Calendar, Constraint, Date, Extra, Marker, MonthMark, Project, Region, Relationship, RelationshipType,
    Resource, UNIT,
};
use mkperth_i18n::tr;
use serde_json::{Map, Value};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PerthError {
    #[error("{}", tr!("err-perth-json", detail = .0.as_str()))]
    Json(String),
    #[error("{}", tr!("err-perth-not-a-project"))]
    NotAProject,
    /// The task (its index from 1 when it has no id) and the field.
    #[error("{}", tr!("err-perth-field", task = .0.as_str(), field = .1.as_str()))]
    Field(String, String),
    #[error("{}", tr!("err-perth-too-many", max = MAX_TASKS))]
    TooMany,
}

/// The most tasks a file may hold.
pub const MAX_TASKS: usize = 1_000_000;
/// The most dependencies one task may list.
pub const MAX_DEPENDENCIES: usize = 100_000;
/// The longest a file may be, in bytes.
pub const MAX_BYTES: usize = 512 * 1024 * 1024;

/// A project read from Perth's JSON, and what the reading had to say.
#[derive(Debug, Clone, PartialEq)]
pub struct Read {
    pub project: Project,
    pub warnings: Vec<String>,
}

/// Perth's task fields in the order it writes them.
const TASK_FIELDS: [&str; 22] = [
    "id",
    "name",
    "start",
    "duration",
    "progress",
    "dependencies",
    "color",
    "assignee",
    "notes",
    "milestone",
    "cost",
    "effort",
    "parent",
    "order",
    "baseline_start",
    "baseline_duration",
    "deadline",
    "pinned",
    "status",
    "optimistic",
    "most_likely",
    "pessimistic",
];
/// Perth's project fields in the order it writes them.
const PROJECT_FIELDS: [&str; 12] =
    ["id", "name", "tasks", "calendar", "people", "bands", "markers", "month_marks", "file_path", "baseline_at", "created_at", "updated_at"];
/// The fields the model holds itself; the rest go to `extra`.
const TASK_MAPPED: [&str; 10] = ["id", "name", "start", "duration", "dependencies", "milestone", "deadline", "parent", "notes", "assignee"];
const PROJECT_MAPPED: [&str; 6] = ["name", "tasks", "calendar", "bands", "markers", "month_marks"];
/// The most bands, markers or marked months a project may have.
pub const MAX_ANNOTATIONS: usize = 10_000;
const PREFIX: &str = "perth.";
/// Keeps, in `extra`, where the project's order of keys came from, so unknown keys keep their
/// place.
const ORDER: &str = "perth.~order";

/// A dependency string: the predecessor's id, the kind and the lag.
pub fn parse_dependency(text: &str) -> (String, RelationshipType, i64) {
    let (kind, rest) = if let Some(r) = text.strip_prefix("SS:") {
        (RelationshipType::StartToStart, r)
    } else if let Some(r) = text.strip_prefix("FF:") {
        (RelationshipType::FinishToFinish, r)
    } else {
        (RelationshipType::FinishToStart, text)
    };
    // `id+3`, `id-2`: the last sign followed only by digits, with an id before it.
    if let Some(at) = rest.rfind(['+', '-'])
        && at > 0
        && let Some(digits) = rest.get(at + 1..)
        && !digits.is_empty()
        && digits.chars().all(|c| c.is_ascii_digit())
        && let Ok(n) = digits.parse::<i64>()
    {
        let lag = if rest.get(at..at + 1) == Some("-") { -n } else { n };
        return (rest.get(..at).unwrap_or_default().to_string(), kind, lag);
    }
    (rest.to_string(), kind, 0)
}

/// A relationship as Perth writes it; `None` for a start-to-finish, which Perth doesn't have.
pub fn write_dependency(r: &Relationship) -> Option<String> {
    let kind = match r.kind {
        RelationshipType::FinishToStart => "",
        RelationshipType::StartToStart => "SS:",
        RelationshipType::FinishToFinish => "FF:",
        RelationshipType::StartToFinish => return None,
    };
    let lag = match r.lag {
        0 => String::new(),
        l if l > 0 => format!("+{l}"),
        l => format!("{l}"),
    };
    Some(format!("{kind}{}{lag}", r.predecessor))
}

fn date_field(v: &Value, task: &str, field: &str) -> Result<Option<Date>, PerthError> {
    match v {
        Value::Null => Ok(None),
        Value::String(s) => Date::parse(s).map(Some).ok_or_else(|| PerthError::Field(task.into(), field.into())),
        _ => Err(PerthError::Field(task.into(), field.into())),
    }
}

/// One of the chart's notes (`bands`, `markers`, `month_marks`): a list of objects, each read by
/// `one` from its text and date fields.
fn notes<T>(root: &Map<String, Value>, field: &str, one: impl Fn(&dyn Fn(&str) -> Option<String>) -> Option<T>) -> Result<Vec<T>, PerthError> {
    let bad = || PerthError::Field(String::new(), field.into());
    match root.get(field) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) if items.len() <= MAX_ANNOTATIONS => items
            .iter()
            .map(|item| {
                let Value::Object(o) = item else { return Err(bad()) };
                let text = |k: &str| match o.get(k) {
                    None => Some(String::new()),
                    Some(Value::String(s)) => Some(s.clone()),
                    Some(Value::Number(n)) => Some(n.to_string()),
                    Some(_) => None,
                };
                one(&text).ok_or_else(bad)
            })
            .collect(),
        Some(_) => Err(bad()),
    }
}

/// Reads Perth's project JSON.
pub fn read(text: &str) -> Result<Read, PerthError> {
    if text.len() > MAX_BYTES {
        return Err(PerthError::TooMany);
    }
    let root: Value = serde_json::from_str(text).map_err(|e| PerthError::Json(e.to_string()))?;
    let Value::Object(root) = root else { return Err(PerthError::NotAProject) };
    let Some(Value::Array(tasks)) = root.get("tasks") else { return Err(PerthError::NotAProject) };
    if tasks.len() > MAX_TASKS {
        return Err(PerthError::TooMany);
    }
    let mut warnings = Vec::new();
    let mut extra = Extra::new();
    for (key, value) in &root {
        if !PROJECT_MAPPED.contains(&key.as_str()) {
            extra.insert(format!("{PREFIX}{key}"), value.to_string());
        }
    }
    extra.insert(ORDER.into(), root.keys().cloned().collect::<Vec<_>>().join("\u{1f}"));
    let name = match root.get("name") {
        Some(Value::String(s)) => s.clone(),
        None => String::new(),
        Some(_) => return Err(PerthError::Field(String::new(), "name".into())),
    };
    let calendar_name = match root.get("calendar") {
        Some(Value::String(s)) => s.clone(),
        None => {
            extra.insert(format!("{PREFIX}~no-calendar"), String::new());
            String::new()
        }
        Some(_) => return Err(PerthError::Field(String::new(), "calendar".into())),
    };
    // Empty: calendar days. A name: business days, with the region's holidays when MK Perth
    // knows them (Perth.jl takes the name to BusinessDays.jl).
    let calendar = if calendar_name.is_empty() {
        Calendar { name: String::new(), ..Calendar::every_day() }
    } else {
        let region = Region::named(&calendar_name);
        if region.is_none() && calendar_name != "WeekendsOnly" {
            warnings.push(tr!("warn-perth-calendar", name = calendar_name.as_str()));
        }
        Calendar { name: calendar_name, region, ..Calendar::default() }
    };

    let mut activities = Vec::with_capacity(tasks.len());
    let mut dependencies: Vec<(String, Vec<String>)> = Vec::with_capacity(tasks.len());
    // Each task's assignee: a person or resource by name, the same one wherever it is named.
    let mut assignees: Vec<(String, String, Option<f64>)> = Vec::new();
    for (k, task) in tasks.iter().enumerate() {
        let Value::Object(t) = task else { return Err(PerthError::Field(format!("#{}", k + 1), "task".into())) };
        let label = match t.get("id") {
            Some(Value::String(id)) => id.clone(),
            _ => return Err(PerthError::Field(format!("#{}", k + 1), "id".into())),
        };
        let text_of = |field: &str| match t.get(field) {
            Some(Value::String(s)) => Ok(s.clone()),
            None => Ok(String::new()),
            Some(_) => Err(PerthError::Field(label.clone(), field.into())),
        };
        let duration = match t.get("duration") {
            Some(v) => v.as_i64().ok_or_else(|| PerthError::Field(label.clone(), "duration".into()))?,
            None => 1,
        };
        let milestone = match t.get("milestone") {
            Some(Value::Bool(b)) => *b,
            None => false,
            Some(_) => return Err(PerthError::Field(label.clone(), "milestone".into())),
        };
        let start = match t.get("start") {
            Some(v) => date_field(v, &label, "start")?,
            None => None,
        };
        let deadline = match t.get("deadline") {
            Some(v) => date_field(v, &label, "deadline")?,
            None => None,
        };
        let deps = match t.get("dependencies") {
            Some(Value::Array(items)) => {
                if items.len() > MAX_DEPENDENCIES {
                    return Err(PerthError::TooMany);
                }
                items
                    .iter()
                    .map(|d| d.as_str().map(str::to_string).ok_or_else(|| PerthError::Field(label.clone(), "dependencies".into())))
                    .collect::<Result<Vec<_>, _>>()?
            }
            None => Vec::new(),
            Some(_) => return Err(PerthError::Field(label.clone(), "dependencies".into())),
        };
        let mut extra = Extra::new();
        for (key, value) in t {
            if !TASK_MAPPED.contains(&key.as_str()) {
                extra.insert(format!("{PREFIX}{key}"), value.to_string());
            }
        }
        // Which of the mapped fields the file had, so they're written back only then.
        let present: Vec<&str> = TASK_MAPPED.iter().copied().filter(|f| t.contains_key(*f)).collect();
        extra.insert(format!("{PREFIX}~present"), present.join(","));
        extra.insert(format!("{PREFIX}~order"), t.keys().cloned().collect::<Vec<_>>().join("\u{1f}"));
        // Perth takes every task as at least a day long.
        if duration < 1 && !milestone {
            warnings.push(tr!("warn-perth-duration", task = label.as_str()));
        }
        let parent = text_of("parent")?;
        let assignee = text_of("assignee")?;
        if !assignee.trim().is_empty() {
            // Perth's effort, in hours, when it gives one.
            let effort = t.get("effort").and_then(Value::as_f64).filter(|e| e.is_finite() && *e > 0.0 && *e <= 1e9);
            assignees.push((label.clone(), assignee, effort));
        }
        activities.push(Activity {
            id: label.clone(),
            name: text_of("name")?,
            parent: (!parent.is_empty()).then_some(parent),
            kind: if milestone { ActivityKind::StartMilestone } else { ActivityKind::Task },
            duration: if milestone { duration } else { duration.max(1) },
            duration_minutes: None,
            actual_start: None,
            actual_finish: None,
            remaining_minutes: None,
            expected_finish: None,
            constraint: start.map(Constraint::StartOnOrAfter),
            secondary_constraint: deadline.map(Constraint::FinishOnOrBefore),
            calendar: None,
            notes: text_of("notes")?,
            extra,
        });
        dependencies.push((label, deps));
    }
    // Relationships, in the order the tasks list them; one to a task that isn't there is left
    // out, as Perth's own scheduler ignores it.
    let ids: std::collections::HashSet<&str> = activities.iter().map(|a| a.id.as_str()).collect();
    let mut relationships = Vec::new();
    for (successor, deps) in &dependencies {
        for d in deps {
            let (predecessor, kind, lag) = parse_dependency(d);
            if !ids.contains(predecessor.as_str()) {
                warnings.push(tr!("warn-perth-dangling", task = successor.as_str(), dependency = d.as_str()));
                continue;
            }
            relationships.push(Relationship { predecessor, successor: successor.clone(), kind, lag, lag_minutes: None });
        }
    }
    let start = activities.iter().filter_map(|a| match a.constraint {
        Some(Constraint::StartOnOrAfter(d)) => Some(d),
        _ => None,
    });
    let start = start.min().or_else(|| Date::from_ymd(1970, 1, 1)).ok_or(PerthError::NotAProject)?;
    let date = |t: &dyn Fn(&str) -> Option<String>, k: &str| t(k).as_deref().and_then(Date::parse);
    let bands = notes(&root, "bands", |t| Some(Band { name: t("name")?, from: date(t, "from")?, to: date(t, "to")?, color: t("color")? }))?;
    let markers = notes(&root, "markers", |t| {
        let at = t("label_at")?;
        let label_at = if at.is_empty() { 0 } else { at.parse::<f64>().ok()?.clamp(0.0, 100.0) as u8 };
        Some(Marker { name: t("name")?, date: date(t, "date")?, color: t("color")?, label_at })
    })?;
    let month_marks = notes(&root, "month_marks", |t| Some(MonthMark { month: date(t, "month")?, name: t("name")?, color: t("color")? }))?;
    // An assignee is a resource named as written (one for each name), assigned for the task's
    // effort, else its working hours at one an hour.
    let mut resources: Vec<Resource> = Vec::new();
    let mut assignments = Vec::with_capacity(assignees.len());
    for (task, name, effort) in assignees {
        if !resources.iter().any(|r| r.id == name) {
            resources.push(Resource { id: name.clone(), name: name.clone(), ..Resource::default() });
        }
        let minutes = activities.iter().find(|a| a.id == task).map_or(0, |a| a.working_minutes(calendar.day_minutes()));
        let planned = effort.map_or(minutes.saturating_mul(UNIT) / 60, |hours| (hours * UNIT as f64).round() as i64);
        assignments.push(Assignment { activity: task, resource: Some(name), planned, ..Assignment::default() });
    }
    Ok(Read {
        project: Project {
            name,
            start,
            calendar,
            calendars: Vec::new(),
            lag_calendar: Default::default(),
            float_kind: Default::default(),
            critical_by: Default::default(),
            data_date: None,
            progress_mode: Default::default(),
            must_finish_by: None,
            bands,
            markers,
            month_marks,
            resources,
            roles: Vec::new(),
            assignments,
            activities,
            relationships,
            extra,
        },
        warnings,
    })
}

/// The calendar as Perth.jl names it: empty for calendar days, a BusinessDays.jl name for working
/// days ("Brazil" with its holidays, "WeekendsOnly" without). A calendar those can't say (other
/// working weekdays, its own holidays or hours) is written as the nearest, with a warning.
fn calendar_name(c: &Calendar, warnings: &mut Vec<String>) -> String {
    let weekdays = [true, true, true, true, true, false, false];
    let plain = c.holidays.is_empty() && c.extra_days.is_empty() && c.exceptions.is_empty();
    let (name, exact) = match c.region {
        Some(Region::Brazil) => ("Brazil".to_string(), c.week == weekdays && plain),
        None if c.week == [true; 7] => (String::new(), plain),
        None => ("WeekendsOnly".to_string(), c.week == weekdays && plain),
    };
    // A region's other name Perth.jl knows is kept as it was written (BRSettlement, say).
    let name = match c.region {
        Some(r) if Region::named(&c.name) == Some(r) => c.name.clone(),
        _ => name,
    };
    if !exact {
        warnings.push(tr!("warn-perth-calendar-near", name = c.name.as_str(), perth = if name.is_empty() { "\"\"" } else { name.as_str() }));
    }
    name
}

/// A field kept as JSON text, back as a value (`null` if it no longer parses).
fn kept(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or(Value::Null)
}

/// Writes `project` as Perth's JSON, pretty or compact. Returns the text and what couldn't be
/// written (start-to-finish relationships, constraints Perth has no field for).
pub fn write(project: &Project) -> (String, Vec<String>) {
    let mut warnings = Vec::new();
    let mut tasks = Vec::with_capacity(project.activities.len());
    for a in &project.activities {
        let mut known = Map::new();
        known.insert("id".into(), Value::String(a.id.clone()));
        known.insert("name".into(), Value::String(a.name.clone()));
        match a.constraint {
            Some(Constraint::StartOnOrAfter(d)) => {
                known.insert("start".into(), Value::String(d.to_string()));
            }
            None => {}
            Some(_) => warnings.push(tr!("warn-perth-constraint", task = a.id.as_str())),
        }
        known.insert("duration".into(), Value::from(a.duration));
        let deps: Vec<Value> = project
            .relationships
            .iter()
            .filter(|r| r.successor == a.id)
            .filter_map(|r| {
                let written = write_dependency(r);
                if written.is_none() {
                    warnings.push(tr!("warn-perth-sf", task = a.id.as_str(), predecessor = r.predecessor.as_str()));
                }
                written.map(Value::String)
            })
            .collect();
        known.insert("dependencies".into(), Value::Array(deps));
        known.insert("milestone".into(), Value::Bool(a.kind.is_milestone()));
        known.insert("parent".into(), Value::String(a.parent.clone().unwrap_or_default()));
        known.insert("notes".into(), Value::String(a.notes.clone()));
        // Perth names one assignee: the resources on the task, by name.
        let names: Vec<&str> = project
            .assignments
            .iter()
            .filter(|x| x.activity == a.id)
            .filter_map(|x| x.resource.as_deref())
            .map(|id| project.resources.iter().find(|r| r.id == id).map_or(id, |r| r.name.as_str()))
            .collect();
        if names.len() > 1 {
            warnings.push(tr!("warn-perth-assignees", task = a.id.as_str()));
        }
        known.insert("assignee".into(), Value::String(names.join(", ")));
        match a.secondary_constraint {
            Some(Constraint::FinishOnOrBefore(d)) => {
                known.insert("deadline".into(), Value::String(d.to_string()));
            }
            None => {
                known.insert("deadline".into(), Value::Null);
            }
            Some(_) => warnings.push(tr!("warn-perth-constraint", task = a.id.as_str())),
        }
        // A mapped field is written when the file had it, when it says something, or always for
        // a task made here; so a file read and written keeps the fields it had and no others.
        let present = a.extra.get("perth.~present").map(|p| p.split(',').map(str::to_string).collect::<Vec<_>>());
        let says_something = |key: &str, value: &Value| match key {
            "dependencies" => value.as_array().is_some_and(|d| !d.is_empty()),
            "milestone" => value.as_bool() == Some(true),
            "deadline" => !value.is_null(),
            "parent" | "notes" | "assignee" => value.as_str().is_some_and(|p| !p.is_empty()),
            _ => true,
        };
        let mut fields: Vec<(String, Value)> = Vec::new();
        for (key, value) in known {
            if present.as_ref().is_none_or(|p| p.contains(&key) || (key != "duration" && key != "name" && says_something(&key, &value))) {
                fields.push((key, value));
            }
        }
        for (key, value) in &a.extra {
            if let Some(field) = key.strip_prefix(PREFIX)
                && !field.starts_with('~')
            {
                fields.push((field.to_string(), kept(value)));
            }
        }
        // Perth's order, then the order the file had for anything else.
        let order: Vec<String> = a.extra.get("perth.~order").map_or_else(Vec::new, |o| o.split('\u{1f}').map(str::to_string).collect());
        tasks.push(Value::Object(ordered(fields, &order, &TASK_FIELDS)));
    }
    let mut fields: Vec<(String, Value)> = vec![("name".into(), Value::String(project.name.clone())), ("tasks".into(), Value::Array(tasks))];
    if !project.extra.contains_key("perth.~no-calendar") {
        fields.push(("calendar".into(), Value::String(calendar_name(&project.calendar, &mut warnings))));
    }
    // The chart's notes, in Perth's order of keys: when there are some, or the file had the field.
    let had = |field: &str| project.extra.get(ORDER).is_some_and(|o| o.split('\u{1f}').any(|k| k == field));
    let bands: Vec<Value> = project
        .bands
        .iter()
        .map(|b| serde_json::json!({"name": b.name, "from": b.from.to_string(), "to": b.to.to_string(), "color": b.color}))
        .collect();
    let markers: Vec<Value> = project
        .markers
        .iter()
        .map(|m| serde_json::json!({"name": m.name, "date": m.date.to_string(), "color": m.color, "label_at": m.label_at}))
        .collect();
    let months: Vec<Value> =
        project.month_marks.iter().map(|m| serde_json::json!({"month": m.month.to_string(), "name": m.name, "color": m.color})).collect();
    for (field, list) in [("bands", bands), ("markers", markers), ("month_marks", months)] {
        if !list.is_empty() || had(field) {
            fields.push((field.into(), Value::Array(list)));
        }
    }
    for (key, value) in &project.extra {
        if let Some(field) = key.strip_prefix(PREFIX)
            && !field.starts_with('~')
        {
            fields.push((field.to_string(), kept(value)));
        }
    }
    let order: Vec<String> = project.extra.get(ORDER).map_or_else(Vec::new, |o| o.split('\u{1f}').map(str::to_string).collect());
    let root = Value::Object(ordered(fields, &order, &PROJECT_FIELDS));
    (serde_json::to_string(&root).unwrap_or_default(), warnings)
}

/// The fields in the file's order when it had one, else in Perth's; anything left at the end.
fn ordered(mut fields: Vec<(String, Value)>, file: &[String], perth: &[&str]) -> Map<String, Value> {
    let mut out = Map::new();
    let keys: Vec<String> = if file.is_empty() { perth.iter().map(|s| s.to_string()).collect() } else { file.to_vec() };
    for key in keys {
        if let Some(at) = fields.iter().position(|(k, _)| *k == key) {
            let (k, v) = fields.remove(at);
            out.insert(k, v);
        }
    }
    for (k, v) in fields {
        out.insert(k, v);
    }
    out
}

#[cfg(test)]
mod tests;
