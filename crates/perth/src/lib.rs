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

use std::collections::BTreeMap;

use mkperth_core::{
    Accrual, Activity, ActivityCode, ActivityKind, Assignment, Band, Baseline, BaselineActivity, BudgetChange, Budgeting, Calendar, ChangeStatus,
    Classification, CodeValue, Comment, Constraint, CostAccount, Date, EarnedValue, EpsNode, EtcTechnique, EvTechnique, Expense,
    ExternalRelationship, Extra, FieldKind, FieldValue, FinancialPeriod, Fund, FundingSource, Indicator, LevelBy, Levelled, LevellingOptions,
    LevellingPriority, Marker, MonthMark, ObsNode, Organisation, PeriodRecord, Pert, Price, Project, Region, Relationship, RelationshipType,
    Resource, SpendingMonth, UNIT, UserField, WbsBudget,
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
const TASK_MAPPED: [&str; 14] = [
    "id",
    "name",
    "start",
    "duration",
    "progress",
    "dependencies",
    "milestone",
    "deadline",
    "parent",
    "notes",
    "assignee",
    "optimistic",
    "most_likely",
    "pessimistic",
];
/// The longest a PERT estimate may be, in days.
const MAX_ESTIMATE: f64 = 1e6;
const PROJECT_MAPPED: [&str; 16] = [
    "name",
    "tasks",
    "calendar",
    "bands",
    "markers",
    "month_marks",
    COMMENTS,
    LEVELLING,
    BASELINES,
    COSTS,
    BUDGET,
    EARNED,
    CLASSIFICATION,
    START,
    EXTERNAL,
    ORGANISATION,
];
/// MK Perth's earned value: its techniques, the financial periods and their stored closes;
/// written only when they say something.
const EARNED: &str = "mkperth_earned_value";
/// MK Perth's budget: the original budgets and their spending plans, the change log, the
/// funding sources and the funds; written only when there is one.
const BUDGET: &str = "mkperth_budget";
/// MK Perth's costs: the currency, the cost accounts, the resources' prices and the expenses,
/// written only when they say more than Perth's own task `cost` (read as an expense).
const COSTS: &str = "mkperth_costs";
/// The name of the expense a task's Perth `cost` is read as.
pub const PERTH_COST: &str = "Perth.jl";
/// MK Perth's baselines, written only when they aren't the one Perth's own task fields hold.
const BASELINES: &str = "mkperth_baselines";
/// The name of the baseline Perth's `baseline_start` and `baseline_duration` make.
pub const PERTH_BASELINE: &str = "Perth.jl";
/// MK Perth's levelling, which Perth has no fields for: a task's priority and where levelling
/// held it, and the project's options; each written only when it says something.
const PRIORITY: &str = "mkperth_levelling_priority";
const LEVELLED: &str = "mkperth_levelled";
/// MK Perth's classification: the project's activity codes and user-defined fields, and each
/// task's values of them; written only when there are some.
const CLASSIFICATION: &str = "mkperth_classification";
/// MK Perth's start for the project, written when its tasks' starts don't give it.
const START: &str = "mkperth_start";
/// MK Perth's relationships with other projects' activities.
const EXTERNAL: &str = "mkperth_external";
/// MK Perth's organisation: the project's place in the EPS, the OBS, who answers for the
/// project; and who answers for each task.
const ORGANISATION: &str = "mkperth_organisation";
const RESPONSIBLE: &str = "mkperth_responsible";
const CODES: &str = "mkperth_codes";
const FIELDS: &str = "mkperth_fields";
const LEVELLING: &str = "mkperth_levelling";
/// MK Perth's comments, which Perth has no field for: a field of their own, written only when
/// there are some, so a file without them is written back as it was.
const COMMENTS: &str = "mkperth_comments";
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
        // PERT's three estimates, in days (0 for none); none given, none kept.
        let estimate = |field: &str| -> Result<i64, PerthError> {
            match t.get(field) {
                None | Some(Value::Null) => Ok(0),
                Some(v) => v
                    .as_f64()
                    .filter(|x| x.is_finite() && (0.0..=MAX_ESTIMATE).contains(x))
                    .map(|x| (x * UNIT as f64).round() as i64)
                    .ok_or_else(|| PerthError::Field(label.clone(), field.into())),
            }
        };
        let (optimistic, most_likely, pessimistic) = (estimate("optimistic")?, estimate("most_likely")?, estimate("pessimistic")?);
        // Its progress in percent, MK Perth's physical percent complete; 0 is none.
        let percent_complete = match t.get("progress") {
            None | Some(Value::Null) => None,
            Some(v) => {
                let p = v
                    .as_f64()
                    .filter(|x| x.is_finite() && (0.0..=100.0).contains(x))
                    .ok_or_else(|| PerthError::Field(label.clone(), "progress".into()))?;
                (p > 0.0).then(|| (p * 100.0).round() as i64)
            }
        };
        let pert = (optimistic > 0 || most_likely > 0 || pessimistic > 0).then_some(Pert { optimistic, most_likely, pessimistic });
        let levelling_priority = match t.get(PRIORITY) {
            None | Some(Value::Null) => LevellingPriority::default(),
            Some(v) => v.as_str().and_then(LevellingPriority::from_code).ok_or_else(|| PerthError::Field(label.clone(), PRIORITY.into()))?,
        };
        let levelled = match t.get(LEVELLED) {
            None | Some(Value::Null) => None,
            Some(Value::Object(o)) => {
                let start = o.get("start").and_then(Value::as_str).and_then(Date::parse);
                let resource = o.get("resource").and_then(Value::as_str);
                match (start, resource) {
                    (Some(start), Some(r)) => Some(Levelled { start, resource: r.to_string() }),
                    _ => return Err(PerthError::Field(label.clone(), LEVELLED.into())),
                }
            }
            Some(_) => return Err(PerthError::Field(label.clone(), LEVELLED.into())),
        };
        let codes = match t.get(CODES) {
            None | Some(Value::Null) => BTreeMap::new(),
            Some(Value::Object(o)) => o
                .iter()
                .take(MAX_TASKS)
                .map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string())))
                .collect::<Option<BTreeMap<_, _>>>()
                .ok_or_else(|| PerthError::Field(label.clone(), CODES.into()))?,
            Some(_) => return Err(PerthError::Field(label.clone(), CODES.into())),
        };
        let fields = match t.get(FIELDS) {
            None | Some(Value::Null) => BTreeMap::new(),
            Some(Value::Object(o)) => o
                .iter()
                .take(MAX_TASKS)
                .map(|(k, v)| field_of(v).map(|v| (k.clone(), v)))
                .collect::<Option<BTreeMap<_, _>>>()
                .ok_or_else(|| PerthError::Field(label.clone(), FIELDS.into()))?,
            Some(_) => return Err(PerthError::Field(label.clone(), FIELDS.into())),
        };
        let mut extra = Extra::new();
        for (key, value) in t {
            if !TASK_MAPPED.contains(&key.as_str()) && ![PRIORITY, LEVELLED, CODES, FIELDS, RESPONSIBLE].contains(&key.as_str()) {
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
            duration_type: Default::default(),
            resource_dependent: false,
            levelling_priority,
            levelled,
            pert,
            percent_complete,
            codes,
            fields,
            responsible: t.get(RESPONSIBLE).and_then(Value::as_str).map(str::to_string),
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
            relationships.push(Relationship {
                predecessor,
                successor: successor.clone(),
                kind,
                lag,
                lag_minutes: None,
                lag_kind: Default::default(),
            });
        }
    }
    let start = activities.iter().filter_map(|a| match a.constraint {
        Some(Constraint::StartOnOrAfter(d)) => Some(d),
        _ => None,
    });
    // MK Perth's start when the tasks don't say it: their earliest start on or after, else 1970.
    let start = match root.get(START).and_then(Value::as_str).and_then(Date::parse) {
        Some(d) => d,
        None => start.min().or_else(|| Date::from_ymd(1970, 1, 1)).ok_or(PerthError::NotAProject)?,
    };
    let date = |t: &dyn Fn(&str) -> Option<String>, k: &str| t(k).as_deref().and_then(Date::parse);
    let bands = notes(&root, "bands", |t| Some(Band { name: t("name")?, from: date(t, "from")?, to: date(t, "to")?, color: t("color")? }))?;
    let markers = notes(&root, "markers", |t| {
        let at = t("label_at")?;
        let label_at = if at.is_empty() { 0 } else { at.parse::<f64>().ok()?.clamp(0.0, 100.0) as u8 };
        Some(Marker { name: t("name")?, date: date(t, "date")?, color: t("color")?, label_at })
    })?;
    let month_marks = notes(&root, "month_marks", |t| Some(MonthMark { month: date(t, "month")?, name: t("name")?, color: t("color")? }))?;
    let comments = notes(&root, COMMENTS, |t| {
        let reply_to = t("reply_to").filter(|r| !r.is_empty());
        Some(Comment {
            id: t("id")?,
            activity: t("activity")?,
            author: t("author")?,
            at: t("at")?,
            text: t("text")?,
            reply_to,
            resolved: t("resolved").as_deref() == Some("true"),
        })
    })?;
    let levelling = match root.get(LEVELLING) {
        None | Some(Value::Null) => LevellingOptions::default(),
        Some(Value::Object(o)) => {
            let bad = || PerthError::Field(String::new(), LEVELLING.into());
            let texts = |k: &str| -> Result<Vec<String>, PerthError> {
                match o.get(k) {
                    None => Ok(Vec::new()),
                    Some(Value::Array(items)) if items.len() <= MAX_ANNOTATIONS => {
                        items.iter().map(|i| i.as_str().map(str::to_string).ok_or_else(bad)).collect()
                    }
                    Some(_) => Err(bad()),
                }
            };
            let order = match o.get("order") {
                None => LevellingOptions::default().order,
                Some(_) => texts("order")?.iter().map(|c| LevelBy::from_code(c).ok_or_else(bad)).collect::<Result<_, _>>()?,
            };
            LevellingOptions { within_float: o.get("within_float").and_then(Value::as_bool).unwrap_or(false), resources: texts("resources")?, order }
        }
        Some(_) => return Err(PerthError::Field(String::new(), LEVELLING.into())),
    };
    // Baselines: MK Perth's own when the file has them, else the one in Perth's task fields.
    let (baselines, baseline_in_use) = match root.get(BASELINES) {
        Some(Value::Object(o)) => {
            let bad = || PerthError::Field(String::new(), BASELINES.into());
            let mut list = Vec::new();
            for b in o.get("list").and_then(Value::as_array).ok_or_else(bad)?.iter().take(MAX_ANNOTATIONS) {
                let name = b.get("name").and_then(Value::as_str).ok_or_else(bad)?.to_string();
                let mut acts = Vec::new();
                for x in b.get("activities").and_then(Value::as_array).ok_or_else(bad)?.iter().take(MAX_TASKS) {
                    let text = |k: &str| x.get(k).and_then(Value::as_str);
                    let (Some(id), Some(start), Some(finish)) =
                        (text("id"), text("start").and_then(Date::parse), text("finish").and_then(Date::parse))
                    else {
                        return Err(bad());
                    };
                    let duration = x.get("duration_minutes").and_then(Value::as_i64).filter(|m| *m >= 0).ok_or_else(bad)?;
                    let cost = match x.get("cost") {
                        None | Some(Value::Null) => 0,
                        Some(c) => c.as_i64().ok_or_else(bad)?,
                    };
                    acts.push(BaselineActivity { id: id.to_string(), start, finish, duration, cost });
                }
                list.push(Baseline { name, activities: acts });
            }
            (list, o.get("in_use").and_then(Value::as_str).map(str::to_string))
        }
        Some(Value::Null) | None => match perth_baseline(&activities, &calendar) {
            Some(b) => (vec![b], Some(PERTH_BASELINE.to_string())),
            None => (Vec::new(), None),
        },
        Some(_) => return Err(PerthError::Field(String::new(), BASELINES.into())),
    };
    // Costs: MK Perth's own when the file has them; else the expenses Perth's task costs say.
    let mut prices: Vec<(String, Vec<Price>)> = Vec::new();
    let (expenses, cost_accounts, currency) = match root.get(COSTS) {
        Some(Value::Object(o)) => {
            let bad = || PerthError::Field(String::new(), COSTS.into());
            let list = |k: &str| o.get(k).and_then(Value::as_array).map(|a| a.iter().take(MAX_TASKS).collect::<Vec<_>>()).unwrap_or_default();
            let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
            let mut expenses = Vec::new();
            for e in list("expenses") {
                expenses.push(Expense {
                    activity: text(e, "activity").ok_or_else(bad)?,
                    name: text(e, "name").ok_or_else(bad)?,
                    account: text(e, "account").filter(|a| !a.is_empty()),
                    planned: e.get("planned").and_then(money_of).unwrap_or(0),
                    actual: e.get("actual").and_then(money_of).unwrap_or(0),
                    remaining: e.get("remaining").and_then(money_of),
                    accrual: text(e, "accrual").and_then(|a| Accrual::from_code(&a)).unwrap_or_default(),
                });
            }
            let mut accounts = Vec::new();
            for c in list("accounts") {
                accounts.push(CostAccount {
                    id: text(c, "id").ok_or_else(bad)?,
                    name: text(c, "name").unwrap_or_default(),
                    parent: text(c, "parent"),
                });
            }
            if let Some(Value::Object(by)) = o.get("prices") {
                for (id, ps) in by.iter().take(MAX_TASKS) {
                    let mut list = Vec::new();
                    for p in ps.as_array().ok_or_else(bad)?.iter().take(1000) {
                        let from = match p.get("from") {
                            None | Some(Value::Null) => None,
                            Some(d) => Some(d.as_str().and_then(Date::parse).ok_or_else(bad)?),
                        };
                        list.push(Price { from, per_unit: p.get("price").and_then(money_of).ok_or_else(bad)? });
                    }
                    prices.push((id.clone(), list));
                }
            }
            (expenses, accounts, text(&Value::Object(o.clone()), "currency").unwrap_or_default())
        }
        Some(Value::Null) | None => (perth_expenses(&activities), Vec::new(), String::new()),
        Some(_) => return Err(PerthError::Field(String::new(), COSTS.into())),
    };
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
    for (id, list) in prices {
        if let Some(r) = resources.iter_mut().find(|r| r.id == id) {
            r.prices = list;
        }
    }
    let organisation = match root.get(ORGANISATION) {
        Some(v @ Value::Object(_)) => organisation_of(v).ok_or_else(|| PerthError::Field(String::new(), ORGANISATION.into()))?,
        Some(Value::Null) | None => Organisation::default(),
        Some(_) => return Err(PerthError::Field(String::new(), ORGANISATION.into())),
    };
    let external = match root.get(EXTERNAL) {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .take(MAX_TASKS)
            .map(external_of)
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| PerthError::Field(String::new(), EXTERNAL.into()))?,
        Some(_) => return Err(PerthError::Field(String::new(), EXTERNAL.into())),
    };
    let classification = match root.get(CLASSIFICATION) {
        Some(v @ Value::Object(_)) => classification_of(v)?,
        Some(Value::Null) | None => Classification::default(),
        Some(_) => return Err(PerthError::Field(String::new(), CLASSIFICATION.into())),
    };
    let earned_value = match root.get(EARNED) {
        Some(v @ Value::Object(_)) => earned_of(v)?,
        Some(Value::Null) | None => EarnedValue::default(),
        Some(_) => return Err(PerthError::Field(String::new(), EARNED.into())),
    };
    let budget = match root.get(BUDGET) {
        Some(v @ Value::Object(_)) => budget_of(v)?,
        Some(Value::Null) | None => Budgeting::default(),
        Some(_) => return Err(PerthError::Field(String::new(), BUDGET.into())),
    };
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
            comments,
            levelling,
            baselines,
            baseline_in_use,
            expenses,
            cost_accounts,
            currency,
            budget,
            earned_value,
            classification,
            external,
            organisation,
            activities,
            relationships,
            extra,
        },
        warnings,
    })
}

/// The expenses Perth.jl's task `cost` (kept in `extra`) says: one for each task with a cost,
/// spread evenly over it.
fn perth_expenses(activities: &[Activity]) -> Vec<Expense> {
    activities
        .iter()
        .filter_map(|a| {
            let cost = a.extra.get("perth.cost").and_then(|v| v.parse::<f64>().ok()).filter(|c| c.is_finite() && *c > 0.0 && *c < 1e13)?;
            Some(Expense { activity: a.id.clone(), name: PERTH_COST.into(), planned: (cost * 100.0).round() as i64, ..Expense::default() })
        })
        .collect()
}

/// MK Perth's budget from its field: each list capped, each item checked.
fn budget_of(v: &Value) -> Result<Budgeting, PerthError> {
    let bad = || PerthError::Field(String::new(), BUDGET.into());
    let list = |k: &str| v.get(k).and_then(Value::as_array).map(|a| a.iter().take(MAX_TASKS).collect::<Vec<_>>()).unwrap_or_default();
    let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let wbs = |v: &Value| text(v, "wbs").filter(|w| !w.is_empty());
    let money = |v: &Value, k: &str| v.get(k).and_then(money_of).ok_or_else(bad);
    let mut b = Budgeting::default();
    for x in list("budgets") {
        let mut plan = Vec::new();
        for m in x.get("plan").and_then(Value::as_array).map(|a| a.iter().take(1200).collect::<Vec<_>>()).unwrap_or_default() {
            let month = text(m, "month").and_then(|t| Date::parse(&format!("{t}-01"))).ok_or_else(bad)?;
            plan.push(SpendingMonth { month, amount: money(m, "amount")? });
        }
        b.budgets.push(WbsBudget { wbs: wbs(x), original: money(x, "original")?, plan });
    }
    for c in list("log") {
        b.log.push(BudgetChange {
            number: c.get("number").and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok()).ok_or_else(bad)?,
            wbs: wbs(c),
            date: text(c, "date").and_then(|d| Date::parse(&d)).ok_or_else(bad)?,
            amount: money(c, "amount")?,
            status: text(c, "status").and_then(|s| ChangeStatus::from_code(&s)).unwrap_or_default(),
            responsible: text(c, "responsible").unwrap_or_default(),
            reason: text(c, "reason").unwrap_or_default(),
        });
    }
    for f in list("sources") {
        b.sources.push(FundingSource { id: text(f, "id").ok_or_else(bad)?, name: text(f, "name").unwrap_or_default(), parent: text(f, "parent") });
    }
    for f in list("funds") {
        b.funds.push(Fund { source: text(f, "source").ok_or_else(bad)?, amount: money(f, "amount")? });
    }
    Ok(b)
}

/// A field's value as written: its kind and the value (`{"cost": 1250.5}`, `{"date": "2026-11-20"}`).
fn field_of(v: &Value) -> Option<FieldValue> {
    let (kind, value) = v.as_object().filter(|o| o.len() == 1)?.iter().next()?;
    let number = || value.as_f64().filter(|x| x.is_finite() && x.abs() < 1e12);
    Some(match FieldKind::from_code(kind)? {
        FieldKind::Text => FieldValue::Text(value.as_str()?.to_string()),
        FieldKind::Number => FieldValue::Number((number()? * 1e6).round() as i64),
        FieldKind::Integer => FieldValue::Integer(value.as_i64()?),
        FieldKind::Cost => FieldValue::Cost(money_of(value)?),
        FieldKind::Date => FieldValue::Date(value.as_str().and_then(Date::parse)?),
        FieldKind::Indicator => FieldValue::Indicator(value.as_str().and_then(Indicator::from_code)?),
    })
}

/// A field's value as its kind and the value.
fn field_value(v: &FieldValue) -> Value {
    let value = match v {
        FieldValue::Text(t) => Value::String(t.clone()),
        FieldValue::Number(n) => Value::from(*n as f64 / 1e6),
        FieldValue::Integer(n) => Value::from(*n),
        FieldValue::Cost(c) => money_value(*c),
        FieldValue::Date(d) => Value::String(d.to_string()),
        FieldValue::Indicator(i) => Value::String(i.code().into()),
    };
    let mut o = Map::new();
    o.insert(v.kind().code().into(), value);
    Value::Object(o)
}

/// A relationship with another project's activity, as written: the file, the activity here and
/// there, which way it runs (`in`, the other's driving this one), its type and lag in days.
fn external_of(v: &Value) -> Option<ExternalRelationship> {
    let text = |k: &str| v.get(k).and_then(Value::as_str);
    let kind = match text("type")? {
        "FS" => RelationshipType::FinishToStart,
        "SS" => RelationshipType::StartToStart,
        "FF" => RelationshipType::FinishToFinish,
        "SF" => RelationshipType::StartToFinish,
        _ => return None,
    };
    Some(ExternalRelationship {
        project: text("project")?.to_string(),
        here: text("activity")?.to_string(),
        there: text("other")?.to_string(),
        incoming: match text("direction")? {
            "in" => true,
            "out" => false,
            _ => return None,
        },
        kind,
        lag: v.get("lag").and_then(Value::as_i64).filter(|l| l.abs() <= 1_000_000)?,
        lag_minutes: None,
    })
}

/// MK Perth's organisation from its field.
fn organisation_of(v: &Value) -> Option<Organisation> {
    let list = |k: &str| v.get(k).and_then(Value::as_array).map(|a| a.iter().take(MAX_TASKS).collect::<Vec<_>>()).unwrap_or_default();
    let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    Some(Organisation {
        eps: list("eps")
            .into_iter()
            .map(|e| Some(EpsNode { id: text(e, "id")?, name: text(e, "name").unwrap_or_default() }))
            .collect::<Option<_>>()?,
        obs: list("obs")
            .into_iter()
            .map(|o| Some(ObsNode { name: text(o, "name")?, description: text(o, "description").unwrap_or_default(), parent: text(o, "parent") }))
            .collect::<Option<_>>()?,
        responsible: text(v, "responsible"),
    })
}

fn organisation_value(o: &Organisation) -> Value {
    let eps: Vec<Value> = o.eps.iter().map(|e| serde_json::json!({"id": e.id, "name": e.name})).collect();
    let obs: Vec<Value> = o.obs.iter().map(|x| serde_json::json!({"name": x.name, "description": x.description, "parent": x.parent})).collect();
    serde_json::json!({"eps": eps, "obs": obs, "responsible": o.responsible})
}

fn external_value(x: &ExternalRelationship) -> Value {
    let kind = match x.kind {
        RelationshipType::FinishToStart => "FS",
        RelationshipType::StartToStart => "SS",
        RelationshipType::FinishToFinish => "FF",
        RelationshipType::StartToFinish => "SF",
    };
    serde_json::json!({"project": x.project, "activity": x.here, "other": x.there,
        "direction": if x.incoming { "in" } else { "out" }, "type": kind, "lag": x.lag})
}

/// MK Perth's classification from its field.
fn classification_of(v: &Value) -> Result<Classification, PerthError> {
    let bad = || PerthError::Field(String::new(), CLASSIFICATION.into());
    fn list<'a>(v: &'a Value, k: &str) -> Vec<&'a Value> {
        v.get(k).and_then(Value::as_array).map(|a| a.iter().take(MAX_TASKS).collect()).unwrap_or_default()
    }
    let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let mut c = Classification::default();
    for code in list(v, "codes") {
        let mut values = Vec::new();
        for x in list(code, "values") {
            values.push(CodeValue {
                code: text(x, "code").ok_or_else(bad)?,
                description: text(x, "description").unwrap_or_default(),
                parent: text(x, "parent"),
            });
        }
        c.codes.push(ActivityCode { name: text(code, "name").ok_or_else(bad)?, values });
    }
    for f in list(v, "fields") {
        c.fields.push(UserField {
            name: text(f, "name").ok_or_else(bad)?,
            kind: text(f, "kind").and_then(|k| FieldKind::from_code(&k)).ok_or_else(bad)?,
        });
    }
    Ok(c)
}

/// MK Perth's classification as its field.
fn classification_value(c: &Classification) -> Value {
    let codes: Vec<Value> = c
        .codes
        .iter()
        .map(|code| {
            let values: Vec<Value> =
                code.values.iter().map(|v| serde_json::json!({"code": v.code, "description": v.description, "parent": v.parent})).collect();
            serde_json::json!({"name": code.name, "values": values})
        })
        .collect();
    let fields: Vec<Value> = c.fields.iter().map(|f| serde_json::json!({"name": f.name, "kind": f.kind.code()})).collect();
    serde_json::json!({"codes": codes, "fields": fields})
}

/// MK Perth's earned value from its field.
fn earned_of(v: &Value) -> Result<EarnedValue, PerthError> {
    let bad = || PerthError::Field(String::new(), EARNED.into());
    let list = |k: &str| v.get(k).and_then(Value::as_array).map(|a| a.iter().take(MAX_TASKS).collect::<Vec<_>>()).unwrap_or_default();
    let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let day = |v: &Value, k: &str| text(v, k).and_then(|d| Date::parse(&d)).ok_or_else(bad);
    let money = |v: &Value, k: &str| v.get(k).and_then(money_of).ok_or_else(bad);
    let share = v.get("share").and_then(Value::as_f64).filter(|x| x.is_finite()).map_or(0, |x| (x * 100.0).round() as i64);
    let mut e = EarnedValue {
        technique: text(v, "technique").map_or(Some(EvTechnique::default()), |t| EvTechnique::from_code(&t, share)).ok_or_else(bad)?,
        etc: text(v, "etc").map_or(Some(EtcTechnique::default()), |t| EtcTechnique::from_code(&t)).ok_or_else(bad)?,
        ..EarnedValue::default()
    };
    for p in list("periods") {
        e.periods.push(FinancialPeriod { name: text(p, "name").ok_or_else(bad)?, start: day(p, "start")?, finish: day(p, "finish")? });
    }
    let units =
        |v: &Value, k: &str| v.get(k).and_then(Value::as_f64).filter(|x| x.is_finite() && x.abs() < 1e12).map_or(0, |x| (x * 100.0).round() as i64);
    // Records written before #34 held amounts to date and one actual cost: made each period's
    // own by what the period before held, the actual cost kept as an expense.
    let mut to_date: Vec<(String, String, i64, i64, i64)> = Vec::new();
    for r in list("records") {
        let (period, activity) = (text(r, "period").ok_or_else(bad)?, text(r, "activity").ok_or_else(bad)?);
        if r.get("labor_cost").is_none() && r.get("actual").is_some() {
            to_date.push((period, activity, money(r, "planned")?, money(r, "earned")?, money(r, "actual")?));
            continue;
        }
        let cost = |k: &str| r.get(k).and_then(money_of).unwrap_or(0);
        e.records.push(PeriodRecord {
            period,
            activity,
            planned: money(r, "planned")?,
            earned: money(r, "earned")?,
            labor_cost: cost("labor_cost"),
            nonlabor_cost: cost("nonlabor_cost"),
            material_cost: cost("material_cost"),
            expense_cost: cost("expense_cost"),
            labor_units: units(r, "labor_units"),
            nonlabor_units: units(r, "nonlabor_units"),
        });
    }
    let start_of = |name: &str| e.periods.iter().find(|q| q.name == name).map(|q| q.start);
    to_date.sort_by(|a, b| (a.1.as_str(), start_of(&a.0)).cmp(&(b.1.as_str(), start_of(&b.0))));
    let mut last: Option<(String, i64, i64, i64)> = None;
    for (period, activity, planned, earned, actual) in to_date {
        let (p0, e0, a0) = match &last {
            Some((who, p, e, a)) if *who == activity => (*p, *e, *a),
            _ => (0, 0, 0),
        };
        e.records.push(PeriodRecord {
            period,
            activity: activity.clone(),
            planned: planned - p0,
            earned: earned - e0,
            expense_cost: actual - a0,
            ..PeriodRecord::default()
        });
        last = Some((activity, planned, earned, actual));
    }
    Ok(e)
}

/// MK Perth's earned value as its field.
fn earned_value_value(e: &EarnedValue) -> Value {
    let share = match e.technique {
        EvTechnique::Custom(s) => Value::from(s as f64 / 100.0),
        _ => Value::Null,
    };
    let periods: Vec<Value> =
        e.periods.iter().map(|p| serde_json::json!({"name": p.name, "start": p.start.to_string(), "finish": p.finish.to_string()})).collect();
    let records: Vec<Value> = e
        .records
        .iter()
        .map(|r| {
            serde_json::json!({"period": r.period, "activity": r.activity, "planned": money_value(r.planned),
                "earned": money_value(r.earned), "labor_cost": money_value(r.labor_cost), "nonlabor_cost": money_value(r.nonlabor_cost),
                "material_cost": money_value(r.material_cost), "expense_cost": money_value(r.expense_cost),
                "labor_units": r.labor_units as f64 / 100.0, "nonlabor_units": r.nonlabor_units as f64 / 100.0})
        })
        .collect();
    serde_json::json!({"technique": e.technique.code(), "share": share, "etc": e.etc.code(), "periods": periods, "records": records})
}

/// MK Perth's budget as its field.
fn budget_value(b: &Budgeting) -> Value {
    let month = |d: Date| {
        let (y, m, _) = d.ymd();
        format!("{y:04}-{m:02}")
    };
    let budgets: Vec<Value> = b
        .budgets
        .iter()
        .map(|x| {
            let plan: Vec<Value> = x.plan.iter().map(|m| serde_json::json!({"month": month(m.month), "amount": money_value(m.amount)})).collect();
            serde_json::json!({"wbs": x.wbs, "original": money_value(x.original), "plan": plan})
        })
        .collect();
    let log: Vec<Value> = b
        .log
        .iter()
        .map(|c| {
            serde_json::json!({"number": c.number, "wbs": c.wbs, "date": c.date.to_string(), "amount": money_value(c.amount),
                "status": c.status.code(), "responsible": c.responsible, "reason": c.reason})
        })
        .collect();
    let sources: Vec<Value> = b.sources.iter().map(|f| serde_json::json!({"id": f.id, "name": f.name, "parent": f.parent})).collect();
    let funds: Vec<Value> = b.funds.iter().map(|f| serde_json::json!({"source": f.source, "amount": money_value(f.amount)})).collect();
    serde_json::json!({"budgets": budgets, "log": log, "sources": sources, "funds": funds})
}

/// Hundredths of the currency as a JSON number: whole when it is.
fn money_value(cents: i64) -> Value {
    if cents % 100 == 0 { Value::from(cents / 100) } else { Value::from(cents as f64 / 100.0) }
}

/// A JSON number as hundredths of the currency.
fn money_of(v: &Value) -> Option<i64> {
    v.as_f64().filter(|m| m.is_finite() && m.abs() < 1e13).map(|m| (m * 100.0).round() as i64)
}

/// The baseline Perth.jl keeps in its tasks (`baseline_start` and `baseline_duration`, in days,
/// kept in `extra`), as one of MK Perth's: a task with a baseline start, finishing on the last of
/// its days on `calendar`. `None` when no task has one.
fn perth_baseline(activities: &[Activity], calendar: &Calendar) -> Option<Baseline> {
    let day = calendar.day_minutes();
    let list: Vec<BaselineActivity> = activities
        .iter()
        .filter_map(|a| {
            let start = a.extra.get("perth.baseline_start").and_then(|v| serde_json::from_str::<String>(v).ok()).and_then(|s| Date::parse(&s))?;
            let days =
                a.extra.get("perth.baseline_duration").and_then(|v| v.parse::<f64>().ok()).filter(|d| d.is_finite() && *d >= 0.0).unwrap_or(0.0);
            let days = (days.round() as i64).min(100_000);
            // The last of its working days: the start, then a working day more for each after it.
            let (mut finish, mut left, mut steps) = (start, days - 1, 0);
            while left > 0 && steps < 400_000 {
                match finish.succ() {
                    Some(next) => finish = next,
                    None => break,
                }
                steps += 1;
                left -= i64::from(calendar.is_working(finish));
            }
            // Its cost the task's: Perth keeps none apart for its baseline.
            let cost = a
                .extra
                .get("perth.cost")
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|c| c.is_finite() && c.abs() < 1e13)
                .map_or(0, |c| (c * 100.0).round() as i64);
            Some(BaselineActivity { id: a.id.clone(), start, finish, duration: days.saturating_mul(day), cost })
        })
        .collect();
    (!list.is_empty()).then(|| Baseline { name: PERTH_BASELINE.into(), activities: list })
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
    let odd_lags = project.relationships.iter().filter(|r| r.lag_kind != mkperth_core::LagKind::Working).count();
    if odd_lags > 0 {
        warnings.push(tr!("warn-lag-kinds", n = odd_lags));
    }
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
        let percent = a.percent_complete.unwrap_or(0);
        known.insert("progress".into(), if percent % 100 == 0 { Value::from(percent / 100) } else { Value::from(percent as f64 / 100.0) });
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
        // PERT's estimates in days: whole as Perth writes them, else with their decimals.
        let days = |h: i64| if h % UNIT == 0 { Value::from(h / UNIT) } else { Value::from(h as f64 / UNIT as f64) };
        let p = a.pert.unwrap_or_default();
        for (field, h) in [("optimistic", p.optimistic), ("most_likely", p.most_likely), ("pessimistic", p.pessimistic)] {
            known.insert(field.into(), days(h));
        }
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
            "optimistic" | "most_likely" | "pessimistic" | "progress" => value.as_f64().is_some_and(|d| d != 0.0),
            _ => true,
        };
        let mut fields: Vec<(String, Value)> = Vec::new();
        for (key, value) in known {
            if present.as_ref().is_none_or(|p| p.contains(&key) || (key != "duration" && key != "name" && says_something(&key, &value))) {
                fields.push((key, value));
            }
        }
        if a.levelling_priority != LevellingPriority::default() {
            fields.push((PRIORITY.into(), Value::String(a.levelling_priority.code().into())));
        }
        if let Some(l) = &a.levelled {
            fields.push((LEVELLED.into(), serde_json::json!({"start": l.start.to_string(), "resource": l.resource})));
        }
        if let Some(r) = &a.responsible {
            fields.push((RESPONSIBLE.into(), Value::String(r.clone())));
        }
        if !a.codes.is_empty() {
            fields.push((CODES.into(), Value::Object(a.codes.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect())));
        }
        if !a.fields.is_empty() {
            fields.push((FIELDS.into(), Value::Object(a.fields.iter().map(|(k, v)| (k.clone(), field_value(v))).collect())));
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
    let comments: Vec<Value> = project
        .comments
        .iter()
        .map(|c| {
            serde_json::json!({"id": c.id, "activity": c.activity, "author": c.author, "at": c.at, "text": c.text,
                "reply_to": c.reply_to.clone().unwrap_or_default(), "resolved": if c.resolved { "true" } else { "" }})
        })
        .collect();
    if !comments.is_empty() {
        fields.push((COMMENTS.into(), Value::Array(comments)));
    }
    // The costs, when they say more than Perth's task costs already do.
    let priced: Vec<&Resource> = project.resources.iter().filter(|r| !r.prices.is_empty()).collect();
    if !project.currency.is_empty()
        || !project.cost_accounts.is_empty()
        || !priced.is_empty()
        || project.expenses != perth_expenses(&project.activities)
    {
        let expenses: Vec<Value> = project
            .expenses
            .iter()
            .map(|e| {
                serde_json::json!({"activity": e.activity, "name": e.name, "account": e.account.clone().unwrap_or_default(),
                    "planned": money_value(e.planned), "actual": money_value(e.actual), "remaining": e.remaining.map(money_value),
                    "accrual": e.accrual.code()})
            })
            .collect();
        let accounts: Vec<Value> =
            project.cost_accounts.iter().map(|c| serde_json::json!({"id": c.id, "name": c.name, "parent": c.parent})).collect();
        let mut prices = Map::new();
        for r in priced {
            let list: Vec<Value> =
                r.prices.iter().map(|p| serde_json::json!({"from": p.from.map(|d| d.to_string()), "price": money_value(p.per_unit)})).collect();
            prices.insert(r.id.clone(), Value::Array(list));
        }
        fields.push((COSTS.into(), serde_json::json!({"currency": project.currency, "accounts": accounts, "prices": prices, "expenses": expenses})));
    }
    if !project.budget.is_empty() {
        fields.push((BUDGET.into(), budget_value(&project.budget)));
    }
    if !project.classification.is_empty() {
        fields.push((CLASSIFICATION.into(), classification_value(&project.classification)));
    }
    if !project.external.is_empty() {
        fields.push((EXTERNAL.into(), Value::Array(project.external.iter().map(external_value).collect())));
    }
    if !project.organisation.is_empty() {
        fields.push((ORGANISATION.into(), organisation_value(&project.organisation)));
    }
    // The start, when reading the tasks wouldn't give it back.
    let implied = project
        .activities
        .iter()
        .filter_map(|a| match a.constraint {
            Some(Constraint::StartOnOrAfter(d)) => Some(d),
            _ => None,
        })
        .min()
        .or_else(|| Date::from_ymd(1970, 1, 1));
    if implied != Some(project.start) {
        fields.push((START.into(), Value::String(project.start.to_string())));
    }
    if !project.earned_value.is_empty() {
        fields.push((EARNED.into(), earned_value_value(&project.earned_value)));
    }
    // The baselines, unless they are just the one Perth's task fields already say.
    let from_tasks = perth_baseline(&project.activities, &project.calendar);
    let said = (from_tasks.iter().cloned().collect::<Vec<_>>(), from_tasks.as_ref().map(|_| PERTH_BASELINE.to_string()));
    if !project.baselines.is_empty() && (project.baselines.clone(), project.baseline_in_use.clone()) != said {
        let list: Vec<Value> = project
            .baselines
            .iter()
            .map(|b| {
                let acts: Vec<Value> = b
                    .activities
                    .iter()
                    .map(|x| serde_json::json!({"id": x.id, "start": x.start.to_string(), "finish": x.finish.to_string(), "duration_minutes": x.duration, "cost": x.cost}))
                    .collect();
                serde_json::json!({"name": b.name, "activities": acts})
            })
            .collect();
        fields.push((BASELINES.into(), serde_json::json!({"in_use": project.baseline_in_use, "list": list})));
    }
    if project.levelling != LevellingOptions::default() {
        let l = &project.levelling;
        let order: Vec<&str> = l.order.iter().map(|b| b.code()).collect();
        fields.push((LEVELLING.into(), serde_json::json!({"within_float": l.within_float, "resources": l.resources, "order": order})));
    }
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
