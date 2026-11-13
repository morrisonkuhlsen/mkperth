//! What MK Perth schedules with and Perth.jl has no field for, kept in fields of its own so a
//! project saved as Perth's JSON and opened again gives the same dates (#55).
//!
//! On the project, `mkperth_schedule`: the data date, the must-finish-by date, the scheduling
//! options (the lags' calendar, the float, what is critical, how progress is kept), the
//! project's calendar when Perth's name for it doesn't say it all, and the other calendars. On
//! a task, `mkperth_task`: a finish milestone, a duration in minutes, the actual dates, the
//! remaining work, the expected finish, its calendar, its duration type, whether it waits for
//! its resources, constraints Perth's `start` and `deadline` can't say, and the relationships
//! into it Perth's dependency strings can't say (start-to-finish, lags in minutes, elapsed or
//! as a share). Each is written only when it says something. Perth.jl reads its projects into
//! its own structs, so a file it saves again drops these fields, as it drops MK Perth's others.

use std::collections::{BTreeMap, BTreeSet};

use mkperth_core::{
    Activity, ActivityKind, Calendar, Constraint, CriticalBy, Date, DurationType, FloatKind, LagCalendar, LagKind, ProgressMode, Project, Region,
    Relationship, RelationshipType,
};
use serde_json::{Map, Value, json};

use crate::PerthError;

pub(crate) const SCHEDULE: &str = "mkperth_schedule";
pub(crate) const TASK: &str = "mkperth_task";

/// The longest a duration, a lag or a remaining work may be, in minutes: a hundred thousand
/// days around the clock.
const MAX_MINUTES: i64 = 100_000 * 24 * 60;
/// The most dates a calendar may list (holidays, extra days, exceptions), and calendars a
/// project may have.
const MAX_DATES: usize = 200_000;
const MAX_CALENDARS: usize = 10_000;
/// The most working periods in a day.
const MAX_PERIODS: usize = 48;

fn constraint_value(c: Constraint) -> Value {
    let (kind, date) = match c {
        Constraint::StartOnOrAfter(d) => ("start-on-or-after", Some(d)),
        Constraint::StartOnOrBefore(d) => ("start-on-or-before", Some(d)),
        Constraint::FinishOnOrAfter(d) => ("finish-on-or-after", Some(d)),
        Constraint::FinishOnOrBefore(d) => ("finish-on-or-before", Some(d)),
        Constraint::StartOn(d) => ("start-on", Some(d)),
        Constraint::FinishOn(d) => ("finish-on", Some(d)),
        Constraint::MandatoryStart(d) => ("mandatory-start", Some(d)),
        Constraint::MandatoryFinish(d) => ("mandatory-finish", Some(d)),
        Constraint::AsLateAsPossible => ("as-late-as-possible", None),
    };
    json!({"type": kind, "date": date.map(|d| d.to_string())})
}

fn constraint_of(v: &Value) -> Option<Constraint> {
    let date = v.get("date").and_then(Value::as_str).and_then(Date::parse);
    Some(match v.get("type")?.as_str()? {
        "start-on-or-after" => Constraint::StartOnOrAfter(date?),
        "start-on-or-before" => Constraint::StartOnOrBefore(date?),
        "finish-on-or-after" => Constraint::FinishOnOrAfter(date?),
        "finish-on-or-before" => Constraint::FinishOnOrBefore(date?),
        "start-on" => Constraint::StartOn(date?),
        "finish-on" => Constraint::FinishOn(date?),
        "mandatory-start" => Constraint::MandatoryStart(date?),
        "mandatory-finish" => Constraint::MandatoryFinish(date?),
        "as-late-as-possible" => Constraint::AsLateAsPossible,
        _ => return None,
    })
}

/// A calendar whole: its week, hours, holidays, extra days, exceptions and region.
pub(crate) fn calendar_value(c: &Calendar) -> Value {
    let periods = |ps: &[(u16, u16)]| Value::Array(ps.iter().map(|(a, b)| json!([a, b])).collect());
    let dates = |ds: &BTreeSet<Date>| Value::Array(ds.iter().map(|d| Value::String(d.to_string())).collect());
    let exceptions: Map<String, Value> = c.exceptions.iter().map(|(d, ps)| (d.to_string(), periods(ps))).collect();
    json!({
        "name": c.name,
        "week": c.week,
        "periods": c.periods.iter().map(|p| periods(p)).collect::<Vec<_>>(),
        "holidays": dates(&c.holidays),
        "extra_days": dates(&c.extra_days),
        "exceptions": exceptions,
        "region": c.region.map(|r| match r { Region::Brazil => "Brazil" }),
    })
}

/// A calendar read back; `None` for anything that isn't one.
pub(crate) fn calendar_of(v: &Value) -> Option<Calendar> {
    let periods = |v: &Value| -> Option<Vec<(u16, u16)>> {
        let items = v.as_array()?;
        if items.len() > MAX_PERIODS {
            return None;
        }
        let mut out = Vec::with_capacity(items.len());
        for p in items {
            let a = u16::try_from(p.get(0)?.as_u64()?).ok()?;
            let b = u16::try_from(p.get(1)?.as_u64()?).ok()?;
            // Within the day, in order, not overlapping the one before.
            if a >= b || b > 24 * 60 || out.last().is_some_and(|(_, prev): &(u16, u16)| *prev > a) {
                return None;
            }
            out.push((a, b));
        }
        Some(out)
    };
    let dates = |v: Option<&Value>| -> Option<BTreeSet<Date>> {
        match v {
            None | Some(Value::Null) => Some(BTreeSet::new()),
            Some(Value::Array(items)) if items.len() <= MAX_DATES => items.iter().map(|d| d.as_str().and_then(Date::parse)).collect(),
            Some(_) => None,
        }
    };
    let mut week = [false; 7];
    let given = v.get("week")?.as_array()?;
    if given.len() != 7 {
        return None;
    }
    for (slot, day) in week.iter_mut().zip(given) {
        *slot = day.as_bool()?;
    }
    let mut days: [Vec<(u16, u16)>; 7] = Default::default();
    if let Some(given) = v.get("periods") {
        let given = given.as_array()?;
        if given.len() != 7 {
            return None;
        }
        for (slot, day) in days.iter_mut().zip(given) {
            *slot = periods(day)?;
        }
    }
    let mut exceptions = BTreeMap::new();
    match v.get("exceptions") {
        None | Some(Value::Null) => {}
        Some(Value::Object(o)) if o.len() <= MAX_DATES => {
            for (d, ps) in o {
                exceptions.insert(Date::parse(d)?, periods(ps)?);
            }
        }
        Some(_) => return None,
    }
    let region = match v.get("region") {
        None | Some(Value::Null) => None,
        Some(r) => Some(Region::named(r.as_str()?)?),
    };
    Some(Calendar {
        name: v.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
        week,
        holidays: dates(v.get("holidays"))?,
        extra_days: dates(v.get("extra_days"))?,
        exceptions,
        region,
        periods: days,
    })
}

fn lag_calendar_code(l: LagCalendar) -> &'static str {
    match l {
        LagCalendar::Predecessor => "predecessor",
        LagCalendar::Successor => "successor",
        LagCalendar::TwentyFourHour => "24-hour",
        LagCalendar::Project => "project",
    }
}

fn float_code(f: FloatKind) -> &'static str {
    match f {
        FloatKind::Finish => "finish",
        FloatKind::Start => "start",
        FloatKind::Smallest => "smallest",
    }
}

fn progress_code(p: ProgressMode) -> &'static str {
    match p {
        ProgressMode::RetainedLogic => "retained-logic",
        ProgressMode::ProgressOverride => "progress-override",
    }
}

/// The project's `mkperth_schedule`, when it says something. `exact` is whether Perth's name for
/// the project's calendar gives it back whole.
pub(crate) fn project_value(p: &Project, exact: bool) -> Option<Value> {
    let mut o = Map::new();
    if let Some(d) = p.data_date {
        o.insert("data_date".into(), Value::String(d.to_string()));
    }
    if let Some(d) = p.must_finish_by {
        o.insert("must_finish_by".into(), Value::String(d.to_string()));
    }
    if p.lag_calendar != LagCalendar::default() {
        o.insert("lag_calendar".into(), Value::from(lag_calendar_code(p.lag_calendar)));
    }
    if p.float_kind != FloatKind::default() {
        o.insert("float".into(), Value::from(float_code(p.float_kind)));
    }
    if p.critical_by != CriticalBy::default() {
        let v = match p.critical_by {
            CriticalBy::TotalFloat(m) => json!({"total_float_minutes": m}),
            CriticalBy::LongestPath => Value::from("longest-path"),
        };
        o.insert("critical".into(), v);
    }
    if p.progress_mode != ProgressMode::default() {
        o.insert("progress".into(), Value::from(progress_code(p.progress_mode)));
    }
    if !exact {
        o.insert("calendar".into(), calendar_value(&p.calendar));
    }
    if !p.calendars.is_empty() {
        o.insert("calendars".into(), Value::Array(p.calendars.iter().map(calendar_value).collect()));
    }
    (!o.is_empty()).then_some(Value::Object(o))
}

/// Reads the project's `mkperth_schedule` into `p`.
pub(crate) fn read_project(v: &Value, p: &mut Project) -> Result<(), PerthError> {
    let bad = || PerthError::Field(String::new(), SCHEDULE.into());
    let Value::Object(o) = v else { return Err(bad()) };
    let date = |k: &str| -> Result<Option<Date>, PerthError> {
        match o.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(d) => d.as_str().and_then(Date::parse).map(Some).ok_or_else(bad),
        }
    };
    p.data_date = date("data_date")?;
    p.must_finish_by = date("must_finish_by")?;
    let text = |k: &str| o.get(k).and_then(Value::as_str);
    if o.contains_key("lag_calendar") {
        p.lag_calendar = match text("lag_calendar") {
            Some("predecessor") => LagCalendar::Predecessor,
            Some("successor") => LagCalendar::Successor,
            Some("24-hour") => LagCalendar::TwentyFourHour,
            Some("project") => LagCalendar::Project,
            _ => return Err(bad()),
        };
    }
    if o.contains_key("float") {
        p.float_kind = match text("float") {
            Some("finish") => FloatKind::Finish,
            Some("start") => FloatKind::Start,
            Some("smallest") => FloatKind::Smallest,
            _ => return Err(bad()),
        };
    }
    match o.get("critical") {
        None | Some(Value::Null) => {}
        Some(Value::String(s)) if s == "longest-path" => p.critical_by = CriticalBy::LongestPath,
        Some(v) => {
            let m = v.get("total_float_minutes").and_then(Value::as_i64).filter(|m| m.abs() <= MAX_MINUTES).ok_or_else(bad)?;
            p.critical_by = CriticalBy::TotalFloat(m);
        }
    }
    if o.contains_key("progress") {
        p.progress_mode = match text("progress") {
            Some("retained-logic") => ProgressMode::RetainedLogic,
            Some("progress-override") => ProgressMode::ProgressOverride,
            _ => return Err(bad()),
        };
    }
    if let Some(c) = o.get("calendar").filter(|c| !c.is_null()) {
        p.calendar = calendar_of(c).ok_or_else(bad)?;
    }
    match o.get("calendars") {
        None | Some(Value::Null) => {}
        Some(Value::Array(items)) if items.len() <= MAX_CALENDARS => {
            p.calendars = items.iter().map(calendar_of).collect::<Option<Vec<_>>>().ok_or_else(bad)?;
        }
        Some(_) => return Err(bad()),
    }
    Ok(())
}

/// Whether Perth's dependency string says `r` exactly.
pub(crate) fn perth_says(r: &Relationship) -> bool {
    r.kind != RelationshipType::StartToFinish && r.lag_minutes.is_none() && r.lag_kind == LagKind::Working
}

fn relationship_value(r: &Relationship) -> Value {
    let mut o = json!({"predecessor": r.predecessor, "type": r.kind.code(), "lag": r.lag});
    if let Some(m) = r.lag_minutes {
        o["lag_minutes"] = Value::from(m);
    }
    match r.lag_kind {
        LagKind::Working => {}
        LagKind::Elapsed => o["lag_kind"] = Value::from("elapsed"),
        LagKind::Percent(n) => {
            o["lag_kind"] = Value::from("percent");
            o["lag_percent"] = Value::from(n);
        }
    }
    o
}

fn relationship_of(v: &Value, successor: &str) -> Option<Relationship> {
    let lag = v.get("lag").map_or(Some(0), Value::as_i64).filter(|l| l.abs() <= MAX_MINUTES)?;
    let lag_minutes = match v.get("lag_minutes") {
        None | Some(Value::Null) => None,
        Some(m) => Some(m.as_i64().filter(|m| m.abs() <= MAX_MINUTES)?),
    };
    let lag_kind = match v.get("lag_kind").and_then(Value::as_str) {
        None => LagKind::Working,
        Some("elapsed") => LagKind::Elapsed,
        Some("percent") => LagKind::Percent(v.get("lag_percent")?.as_i64().filter(|n| (0..=1_000_000).contains(n))?),
        Some(_) => return None,
    };
    Some(Relationship {
        predecessor: v.get("predecessor")?.as_str()?.to_string(),
        successor: successor.to_string(),
        kind: RelationshipType::from_code(v.get("type")?.as_str()?)?,
        lag,
        lag_minutes,
        lag_kind,
    })
}

/// A task's `mkperth_task`, when it says something. `primary` and `secondary` are its
/// constraints Perth's `start` and `deadline` couldn't hold.
pub(crate) fn task_value(a: &Activity, primary: Option<Constraint>, secondary: Option<Constraint>, relationships: &[&Relationship]) -> Option<Value> {
    let mut o = Map::new();
    if a.kind == ActivityKind::FinishMilestone {
        o.insert("kind".into(), Value::from("finish-milestone"));
    }
    let mut minutes = |k: &str, m: Option<i64>| {
        if let Some(m) = m {
            o.insert(k.into(), Value::from(m));
        }
    };
    minutes("duration_minutes", a.duration_minutes);
    minutes("remaining_minutes", a.remaining_minutes);
    for (k, d) in [("actual_start", a.actual_start), ("actual_finish", a.actual_finish), ("expected_finish", a.expected_finish)] {
        if let Some(d) = d {
            o.insert(k.into(), Value::String(d.to_string()));
        }
    }
    if let Some(c) = &a.calendar {
        o.insert("calendar".into(), Value::String(c.clone()));
    }
    if a.duration_type != DurationType::default() {
        o.insert("duration_type".into(), Value::from(a.duration_type.code()));
    }
    if a.resource_dependent {
        o.insert("resource_dependent".into(), Value::Bool(true));
    }
    if let Some(c) = primary {
        o.insert("constraint".into(), constraint_value(c));
    }
    if let Some(c) = secondary {
        o.insert("secondary_constraint".into(), constraint_value(c));
    }
    if !relationships.is_empty() {
        o.insert("relationships".into(), Value::Array(relationships.iter().map(|r| relationship_value(r)).collect()));
    }
    (!o.is_empty()).then_some(Value::Object(o))
}

/// Reads a task's `mkperth_task` into `a`; returns the relationships into it it keeps, which
/// take the place of the dependency strings' for the same predecessor and type.
pub(crate) fn read_task(v: &Value, a: &mut Activity) -> Result<Vec<Relationship>, PerthError> {
    let bad = || PerthError::Field(a.id.clone(), TASK.into());
    let Value::Object(o) = v else { return Err(bad()) };
    let date = |k: &str| -> Result<Option<Date>, PerthError> {
        match o.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(d) => d.as_str().and_then(Date::parse).map(Some).ok_or_else(bad),
        }
    };
    let minutes = |k: &str, least: i64| -> Result<Option<i64>, PerthError> {
        match o.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(m) => m.as_i64().filter(|m| (least..=MAX_MINUTES).contains(m)).map(Some).ok_or_else(bad),
        }
    };
    match o.get("kind").and_then(Value::as_str) {
        None => {}
        Some("finish-milestone") => a.kind = ActivityKind::FinishMilestone,
        Some(_) => return Err(bad()),
    }
    a.duration_minutes = minutes("duration_minutes", 0)?;
    a.remaining_minutes = minutes("remaining_minutes", 0)?;
    a.actual_start = date("actual_start")?;
    a.actual_finish = date("actual_finish")?;
    a.expected_finish = date("expected_finish")?;
    if let Some(c) = o.get("calendar").filter(|c| !c.is_null()) {
        a.calendar = Some(c.as_str().ok_or_else(bad)?.to_string());
    }
    if let Some(d) = o.get("duration_type").filter(|d| !d.is_null()) {
        a.duration_type = d.as_str().and_then(DurationType::from_code).ok_or_else(bad)?;
    }
    a.resource_dependent = o.get("resource_dependent").and_then(Value::as_bool).unwrap_or(false);
    if let Some(c) = o.get("constraint").filter(|c| !c.is_null()) {
        a.constraint = Some(constraint_of(c).ok_or_else(bad)?);
    }
    if let Some(c) = o.get("secondary_constraint").filter(|c| !c.is_null()) {
        a.secondary_constraint = Some(constraint_of(c).ok_or_else(bad)?);
    }
    match o.get("relationships") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) if items.len() <= crate::MAX_DEPENDENCIES => {
            items.iter().map(|r| relationship_of(r, &a.id)).collect::<Option<Vec<_>>>().ok_or_else(bad)
        }
        Some(_) => Err(bad()),
    }
}
