//! Primavera P6's XML interchange format: a project's WBS, activities, relationships and
//! calendars, read into MK Perth's model and written from it.
//!
//! What maps: the first project's name, planned start and must-finish-by date; its WBS as
//! summaries; each activity's ID, name, type (start and finish milestones; every other type as a
//! task), planned duration, calendar, WBS and primary and secondary constraints; relationships of
//! the four types with their lags; calendars' work weeks, holidays and worked exceptions;
//! resources (their type, calendar, units an hour and a material's unit), roles, the roles each
//! resource fills, and the project's resource assignments with their units. P6 keeps
//! durations and lags in hours: they become working days by the calendar's hours per day, rounded
//! up, with a warning when that isn't whole. Element names follow Oracle's published P6 XML schema
//! (`APIBusinessObjects`); the format's other elements are left out, each kind named once in the
//! warnings.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod tree;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use mkperth_core::{
    Accrual, Activity, ActivityKind, Assignment, Baseline, BaselineActivity, Calendar, Constraint, CostAccount, Date, DurationType, Expense,
    LevellingPriority, Period, Price, Project, Relationship, RelationshipType, Resource, ResourceKind, Role, UNIT, Weekday,
};
use mkperth_i18n::tr;
use thiserror::Error;

use tree::Element;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum P6Error {
    #[error("{}", tr!("err-p6-xml", detail = .0.as_str()))]
    Xml(String),
    #[error("{}", tr!("err-p6-not-p6"))]
    NotP6,
    #[error("{}", tr!("err-p6-too-big"))]
    TooBig,
    /// The element and its ObjectId (or position).
    #[error("{}", tr!("err-p6-field", what = .0.as_str(), field = .1.as_str()))]
    Field(String, String),
}

/// A project read from P6 XML, and what the reading had to say.
#[derive(Debug, Clone, PartialEq)]
pub struct Read {
    pub project: Project,
    pub warnings: Vec<String>,
}

/// Hours in a working day when a calendar doesn't say.
const HOURS_PER_DAY: f64 = 8.0;
/// The schema's namespace, written on the root.
const NAMESPACE: &str = "http://xmlns.oracle.com/Primavera/P6/V8.3/API/BusinessObjects";
/// Prefix of the extra keys this format keeps.
const PREFIX: &str = "p6.";

/// P6's units (hours, or a material's unit) as hundredths: finite, not below nothing.
fn units_of(text: &str) -> Option<i64> {
    text.trim().parse::<f64>().ok().filter(|u| u.is_finite() && (0.0..=1e9).contains(u)).map(|u| (u * UNIT as f64).round() as i64)
}

/// Hundredths of a unit as P6 writes them: `40`, `12.5`.
fn units_text(u: i64) -> String {
    if u % UNIT == 0 { (u / UNIT).to_string() } else { format!("{:.2}", u as f64 / UNIT as f64).trim_end_matches('0').to_string() }
}

/// P6's names for the duration types.
fn duration_type_name(d: DurationType) -> &'static str {
    match d {
        DurationType::FixedDurationAndRate => "Fixed Duration and Units/Time",
        DurationType::FixedDurationAndUnits => "Fixed Duration and Units",
        DurationType::FixedRate => "Fixed Units/Time",
        DurationType::FixedUnits => "Fixed Units",
    }
}

/// P6's names for the levelling priorities.
fn levelling_priority_name(p: LevellingPriority) -> &'static str {
    match p {
        LevellingPriority::Top => "Top",
        LevellingPriority::High => "High",
        LevellingPriority::Normal => "Normal",
        LevellingPriority::Low => "Low",
        LevellingPriority::Lowest => "Lowest",
    }
}

/// `2026-11-02T08:00:00` (or a bare date) as a date.
fn date_of(text: &str) -> Option<Date> {
    Date::parse(text.get(..10).unwrap_or(text))
}

fn weekday_of(name: &str) -> Option<Weekday> {
    Weekday::ALL.into_iter().find(|w| format!("{w:?}").eq_ignore_ascii_case(name.trim()))
}

fn relationship_of(text: &str) -> Option<RelationshipType> {
    Some(match text.trim() {
        "Finish to Start" => RelationshipType::FinishToStart,
        "Start to Start" => RelationshipType::StartToStart,
        "Finish to Finish" => RelationshipType::FinishToFinish,
        "Start to Finish" => RelationshipType::StartToFinish,
        _ => return None,
    })
}

fn relationship_name(kind: RelationshipType) -> &'static str {
    match kind {
        RelationshipType::FinishToStart => "Finish to Start",
        RelationshipType::StartToStart => "Start to Start",
        RelationshipType::FinishToFinish => "Finish to Finish",
        RelationshipType::StartToFinish => "Start to Finish",
    }
}

fn constraint_of(kind: &str, date: Option<Date>) -> Option<Constraint> {
    if kind.trim() == "As Late As Possible" {
        return Some(Constraint::AsLateAsPossible);
    }
    let d = date?;
    Some(match kind.trim() {
        "Start On" => Constraint::StartOn(d),
        "Start On or After" => Constraint::StartOnOrAfter(d),
        "Start On or Before" => Constraint::StartOnOrBefore(d),
        "Finish On" => Constraint::FinishOn(d),
        "Finish On or After" => Constraint::FinishOnOrAfter(d),
        "Finish On or Before" => Constraint::FinishOnOrBefore(d),
        "Mandatory Start" => Constraint::MandatoryStart(d),
        "Mandatory Finish" => Constraint::MandatoryFinish(d),
        _ => return None,
    })
}

/// A constraint as P6 names it, and its date.
fn constraint_name(c: Constraint) -> (&'static str, Option<Date>) {
    match c {
        Constraint::StartOn(d) => ("Start On", Some(d)),
        Constraint::StartOnOrAfter(d) => ("Start On or After", Some(d)),
        Constraint::StartOnOrBefore(d) => ("Start On or Before", Some(d)),
        Constraint::FinishOn(d) => ("Finish On", Some(d)),
        Constraint::FinishOnOrAfter(d) => ("Finish On or After", Some(d)),
        Constraint::FinishOnOrBefore(d) => ("Finish On or Before", Some(d)),
        Constraint::MandatoryStart(d) => ("Mandatory Start", Some(d)),
        Constraint::MandatoryFinish(d) => ("Mandatory Finish", Some(d)),
        Constraint::AsLateAsPossible => ("As Late As Possible", None),
    }
}

/// `08:00:00` (or `08:00`) as minutes after midnight; midnight at a period's end is 24:00.
fn minutes_of(text: &str) -> Option<u16> {
    let mut parts = text.trim().split(':');
    let h: u16 = parts.next()?.parse().ok()?;
    let m: u16 = parts.next().map_or(Some(0), |m| m.parse().ok())?;
    (h <= 24 && m < 60).then(|| h * 60 + m).filter(|t| *t <= 1440)
}

/// The work periods of a `WorkTime` list; a `WorkTime` without times works `hours` from 08:00.
fn periods_of(e: &Element, hours: f64) -> Vec<Period> {
    let mut out = Vec::new();
    for w in e.all("WorkTime") {
        let period = match (w.text_of("Start").and_then(minutes_of), w.text_of("Finish").and_then(minutes_of)) {
            (Some(a), Some(b)) => Some((a, if b <= a { 1440 } else { b })),
            _ => Some((480, (480.0 + hours * 60.0).min(1440.0) as u16)),
        };
        if let Some(p) = period.filter(|(a, b)| a < b) {
            out.push(p);
        }
    }
    out.sort_unstable();
    out
}

/// A calendar and its hours per day.
fn calendar_of(e: &Element) -> (Calendar, f64) {
    let name = e.text_of("Name").unwrap_or("Calendar").to_string();
    let hours = e.text_of("HoursPerDay").and_then(|h| h.parse::<f64>().ok()).filter(|h| *h > 0.0 && *h <= 24.0).unwrap_or(HOURS_PER_DAY);
    let mut week = [false; 7];
    let mut periods: [Vec<Period>; 7] = Default::default();
    let mut any_day = false;
    if let Some(standard) = e.child("StandardWorkWeek") {
        for day in standard.all("StandardWorkHours") {
            any_day = true;
            if let Some(w) = day.text_of("DayOfWeek").and_then(weekday_of) {
                let p = periods_of(day, hours);
                if let (Some(slot), Some(ps)) = (week.get_mut(w.index()), periods.get_mut(w.index())) {
                    *slot = !p.is_empty();
                    *ps = p;
                }
            }
        }
    }
    if !any_day {
        week = [true, true, true, true, true, false, false];
    }
    // An exception with work times works them, whatever the week says; one without is a day off.
    let (mut holidays, mut exceptions) = (BTreeSet::new(), BTreeMap::new());
    if let Some(list) = e.child("HolidayOrExceptions") {
        for x in list.all("HolidayOrException") {
            let Some(d) = x.text_of("Date").and_then(date_of) else { continue };
            let p = periods_of(x, hours);
            if p.is_empty() {
                holidays.insert(d);
            } else {
                exceptions.insert(d, p);
            }
        }
    }
    (Calendar { name, week, holidays, extra_days: BTreeSet::new(), exceptions, region: None, periods }, hours)
}

/// Reads P6 XML.
pub fn read(text: &str) -> Result<Read, P6Error> {
    let root = tree::parse(text)?;
    if root.name != "APIBusinessObjects" {
        return Err(P6Error::NotP6);
    }
    let mut warnings = Vec::new();
    let projects: Vec<&Element> = root.all("Project").collect();
    let Some(p) = projects.first() else { return Err(P6Error::NotP6) };
    if projects.len() > 1 {
        warnings.push(tr!("warn-p6-projects", n = projects.len()));
    }
    // Calendars, global and the project's own, by ObjectId.
    let mut calendars: Vec<(String, Calendar, f64)> = Vec::new();
    for e in root.all("Calendar").chain(p.all("Calendar")) {
        let id = e.text_of("ObjectId").unwrap_or_default().to_string();
        let (mut cal, hours) = calendar_of(e);
        // Names are how activities name their calendar here: keep them unique.
        while calendars.iter().any(|(_, c, _)| c.name == cal.name) {
            cal.name.push('*');
        }
        calendars.push((id, cal, hours));
    }
    let default_id = p.text_of("ActivityDefaultCalendarObjectId").unwrap_or_default();
    let default = calendars.iter().position(|(id, _, _)| id == default_id).or(if calendars.is_empty() { None } else { Some(0) });
    let (project_calendar, project_hours) = match default {
        Some(k) => calendars.get(k).map(|(_, c, h)| (c.clone(), *h)).unwrap_or((Calendar::default(), HOURS_PER_DAY)),
        None => (Calendar::default(), HOURS_PER_DAY),
    };
    let calendar_by_id: HashMap<&str, (&str, f64)> = calendars.iter().map(|(id, c, h)| (id.as_str(), (c.name.as_str(), *h))).collect();
    // Hours as working minutes, kept exact (P6 schedules them on the calendar's work periods);
    // in days by its hours per day too, for showing.
    let to_minutes = |hours: f64, per_day: f64| -> (i64, Option<i64>) {
        let minutes = (hours * 60.0).round().clamp(-1e12, 1e12) as i64;
        let day = ((per_day * 60.0).round() as i64).max(1);
        (minutes / day + i64::from(minutes > 0 && minutes % day != 0), Some(minutes))
    };

    // The WBS as summaries, IDs from their codes, kept unique.
    let mut activities: Vec<Activity> = Vec::new();
    let mut wbs_id: HashMap<String, String> = HashMap::new();
    let wbs: Vec<&Element> = p.all("WBS").chain(root.all("WBS")).collect();
    for w in &wbs {
        let object = w.text_of("ObjectId").unwrap_or_default().to_string();
        let mut id = w.text_of("Code").or_else(|| w.text_of("Name")).unwrap_or("WBS").to_string();
        while activities.iter().any(|a| a.id == id) {
            id.push('*');
        }
        let mut a = Activity { id: id.clone(), name: w.text_of("Name").unwrap_or_default().to_string(), ..Activity::default() };
        a.extra.insert(format!("{PREFIX}wbs"), object.clone());
        activities.push(a);
        wbs_id.insert(object, id);
    }
    for (k, w) in wbs.iter().enumerate() {
        let parent = w.text_of("ParentObjectId").and_then(|p| wbs_id.get(p)).cloned();
        if let Some(a) = activities.get_mut(k) {
            a.parent = parent.filter(|p| *p != a.id);
        }
    }

    // Activities, by ObjectId for the relationships.
    let mut by_object: HashMap<String, String> = HashMap::new();
    for (n, e) in p.all("Activity").enumerate() {
        let field = |name: &str| P6Error::Field(format!("Activity #{}", n + 1), name.to_string());
        let id = e.text_of("Id").ok_or_else(|| field("Id"))?.to_string();
        if activities.iter().any(|a| a.id == id) {
            return Err(P6Error::Field(format!("Activity {id}"), "Id".into()));
        }
        let (calendar, per_day) = match e.text_of("CalendarObjectId").and_then(|c| calendar_by_id.get(c)) {
            Some((name, hours)) if Some(*name) != Some(project_calendar.name.as_str()) => (Some(name.to_string()), *hours),
            Some((_, hours)) => (None, *hours),
            None => (None, project_hours),
        };
        let hours = match e.text_of("PlannedDuration") {
            Some(h) => h.parse::<f64>().ok().filter(|h| h.is_finite() && *h >= 0.0).ok_or_else(|| field("PlannedDuration"))?,
            None => 0.0,
        };
        let kind = match e.text_of("Type").unwrap_or("Task Dependent") {
            "Start Milestone" => ActivityKind::StartMilestone,
            "Finish Milestone" => ActivityKind::FinishMilestone,
            _ => ActivityKind::Task,
        };
        let constraint = |kind: &str, date: &str| e.text_of(kind).and_then(|k| constraint_of(k, e.text_of(date).and_then(date_of)));
        let mut a = Activity {
            id: id.clone(),
            name: e.text_of("Name").unwrap_or_default().to_string(),
            kind,
            duration: to_minutes(hours, per_day).0,
            duration_minutes: to_minutes(hours, per_day).1,
            constraint: constraint("PrimaryConstraintType", "PrimaryConstraintDate"),
            secondary_constraint: constraint("SecondaryConstraintType", "SecondaryConstraintDate"),
            calendar,
            parent: e.text_of("WBSObjectId").and_then(|w| wbs_id.get(w)).cloned(),
            duration_type: e
                .text_of("DurationType")
                .and_then(|d| DurationType::ALL.into_iter().find(|x| duration_type_name(*x) == d))
                .unwrap_or_default(),
            resource_dependent: e.text_of("Type") == Some("Resource Dependent"),
            levelling_priority: e
                .text_of("LevelingPriority")
                .and_then(|l| LevellingPriority::ALL.into_iter().find(|x| levelling_priority_name(*x) == l))
                .unwrap_or_default(),
            ..Activity::default()
        };
        // Progress: the actual dates, and the work left of an activity under way.
        a.actual_start = e.text_of("ActualStartDate").and_then(date_of);
        a.actual_finish = e.text_of("ActualFinishDate").and_then(date_of);
        a.expected_finish = e.text_of("ExpectedFinishDate").and_then(date_of);
        if a.actual_start.is_some() && a.actual_finish.is_none() {
            a.remaining_minutes = match e.text_of("RemainingDuration") {
                Some(h) => {
                    to_minutes(h.parse::<f64>().ok().filter(|h| h.is_finite() && *h >= 0.0).ok_or_else(|| field("RemainingDuration"))?, per_day).1
                }
                None => None,
            };
        }
        if let Some(object) = e.text_of("ObjectId") {
            a.extra.insert(format!("{PREFIX}object"), object.to_string());
            by_object.insert(object.to_string(), id);
        }
        activities.push(a);
    }

    let mut relationships = Vec::new();
    let mut dangling = 0usize;
    for e in p.all("Relationship").chain(root.all("Relationship")) {
        let end = |name: &str| e.text_of(name).and_then(|o| by_object.get(o)).cloned();
        let (Some(predecessor), Some(successor)) = (end("PredecessorActivityObjectId"), end("SuccessorActivityObjectId")) else {
            dangling += 1;
            continue;
        };
        let kind = e.text_of("Type").and_then(relationship_of).unwrap_or_default();
        // The lag counts on the predecessor's calendar, P6's default.
        let per_day = activities
            .iter()
            .find(|a| a.id == predecessor)
            .and_then(|a| a.calendar.as_deref())
            .map_or(project_hours, |name| calendars.iter().find(|(_, c, _)| c.name == name).map_or(project_hours, |(_, _, h)| *h));
        let (lag, lag_minutes) = match e.text_of("Lag") {
            Some(h) => {
                let hours = h.parse::<f64>().ok().filter(|h| h.is_finite()).ok_or_else(|| P6Error::Field("Relationship".into(), "Lag".into()))?;
                let (days, minutes) = to_minutes(hours, per_day);
                // A lead's part day doesn't round to a whole one.
                (if hours < 0.0 { minutes.map_or(days, |m| m / ((per_day * 60.0).round() as i64).max(1)) } else { days }, minutes)
            }
            None => (0, None),
        };
        relationships.push(Relationship { predecessor, successor, kind, lag, lag_minutes });
    }
    if dangling > 0 {
        warnings.push(tr!("warn-p6-dangling", n = dangling));
    }
    // Roles and resources, the file's own; the roles each resource fills.
    let field = |what: &str, n: usize, name: &str| P6Error::Field(format!("{what} #{}", n + 1), name.to_string());
    let mut roles: Vec<Role> = Vec::new();
    let mut role_by_object: HashMap<&str, String> = HashMap::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (n, e) in root.all("Role").enumerate() {
        let id = e.text_of("Id").ok_or_else(|| field("Role", n, "Id"))?.to_string();
        if !seen.insert(id.clone()) {
            return Err(P6Error::Field(format!("Role {id}"), "Id".into()));
        }
        if let Some(object) = e.text_of("ObjectId") {
            role_by_object.insert(object, id.clone());
        }
        roles.push(Role { name: e.text_of("Name").unwrap_or(&id).to_string(), id, ..Role::default() });
    }
    let unit_of_measure: HashMap<&str, &str> =
        root.all("UnitOfMeasure").filter_map(|u| Some((u.text_of("ObjectId")?, u.text_of("Abbreviation").or_else(|| u.text_of("Name"))?))).collect();
    let mut resources: Vec<Resource> = Vec::new();
    let mut resource_by_object: HashMap<&str, String> = HashMap::new();
    seen.clear();
    for (n, e) in root.all("Resource").enumerate() {
        let id = e.text_of("Id").ok_or_else(|| field("Resource", n, "Id"))?.to_string();
        if !seen.insert(id.clone()) {
            return Err(P6Error::Field(format!("Resource {id}"), "Id".into()));
        }
        if let Some(object) = e.text_of("ObjectId") {
            resource_by_object.insert(object, id.clone());
        }
        let kind = e.text_of("ResourceType").and_then(ResourceKind::from_code).unwrap_or_default();
        let max_units = match e.text_of("MaxUnitsPerTime") {
            Some(t) => units_of(t).ok_or_else(|| P6Error::Field(format!("Resource {id}"), "MaxUnitsPerTime".into()))?,
            None => UNIT,
        };
        // Its calendar by name, unless it is the project's.
        let calendar = e
            .text_of("CalendarObjectId")
            .and_then(|c| calendar_by_id.get(c))
            .map(|(name, _)| name.to_string())
            .filter(|n| *n != project_calendar.name);
        let unit = e.text_of("UnitOfMeasureObjectId").and_then(|u| unit_of_measure.get(u)).map_or_else(String::new, |u| u.to_string());
        resources.push(Resource { name: e.text_of("Name").unwrap_or(&id).to_string(), id, kind, unit, calendar, max_units, ..Resource::default() });
    }
    // Prices over time: each resource's rates by the day they take effect; the earliest holds
    // from the start of time, as MK Perth keeps a first price.
    for e in root.all("ResourceRate") {
        let (Some(resource), Some(price)) =
            (e.text_of("ResourceObjectId").and_then(|r| resource_by_object.get(r)), e.text_of("PricePerUnit").and_then(money_of))
        else {
            continue;
        };
        if let Some(r) = resources.iter_mut().find(|x| x.id == *resource).filter(|r| r.prices.len() < 1000) {
            r.prices.push(Price { from: e.text_of("EffectiveDate").and_then(date_of), per_unit: price });
        }
    }
    for r in &mut resources {
        r.prices.sort_by_key(|p| p.from);
        if let Some(first) = r.prices.first_mut() {
            first.from = None;
        }
    }
    // Cost accounts, by their object numbers for their parents and the expenses.
    let mut account_by_object: HashMap<&str, String> = HashMap::new();
    for e in root.all("CostAccount") {
        if let (Some(o), Some(id)) = (e.text_of("ObjectId"), e.text_of("Id")) {
            account_by_object.insert(o, id.to_string());
        }
    }
    let cost_accounts: Vec<CostAccount> = root
        .all("CostAccount")
        .filter_map(|e| {
            Some(CostAccount {
                id: e.text_of("Id")?.to_string(),
                name: e.text_of("Name").unwrap_or_default().to_string(),
                parent: e.text_of("ParentObjectId").and_then(|o| account_by_object.get(o)).cloned(),
            })
        })
        .collect();
    let mut expenses: Vec<Expense> = Vec::new();
    for e in p.all("ActivityExpense").chain(root.all("ActivityExpense")) {
        let Some(activity) = e.text_of("ActivityObjectId").and_then(|o| by_object.get(o)).cloned() else { continue };
        let mut name = e.text_of("ExpenseItem").unwrap_or("Expense").to_string();
        // Unique on its activity, as MK Perth names them.
        let base = name.clone();
        let mut k = 1;
        while expenses.iter().any(|x| x.activity == activity && x.name == name) && k < 10_000 {
            k += 1;
            name = format!("{base} ({k})");
        }
        let accrual = match e.text_of("AccrualType") {
            Some("Start of Activity") => Accrual::Start,
            Some("End of Activity") => Accrual::End,
            _ => Accrual::Uniform,
        };
        expenses.push(Expense {
            activity,
            name,
            account: e.text_of("CostAccountObjectId").and_then(|o| account_by_object.get(o)).cloned(),
            planned: e.text_of("PlannedCost").and_then(money_of).unwrap_or(0),
            actual: e.text_of("ActualCost").and_then(money_of).unwrap_or(0),
            remaining: e.text_of("RemainingCost").and_then(money_of),
            accrual,
        });
    }
    for e in root.all("ResourceRole") {
        let resource = e.text_of("ResourceObjectId").and_then(|r| resource_by_object.get(r));
        let role = e.text_of("RoleObjectId").and_then(|r| role_by_object.get(r));
        if let (Some(resource), Some(role)) = (resource, role)
            && let Some(r) = resources.iter_mut().find(|x| x.id == *resource)
            && !r.roles.contains(role)
        {
            r.roles.push(role.clone());
        }
    }
    // The project's assignments: an activity, a resource or a role (or both), and their units.
    let mut assignments = Vec::new();
    let mut unplaced = 0usize;
    for e in p.all("ResourceAssignment").chain(root.all("ResourceAssignment")) {
        let activity = e.text_of("ActivityObjectId").and_then(|o| by_object.get(o)).cloned();
        let resource = e.text_of("ResourceObjectId").and_then(|r| resource_by_object.get(r)).cloned();
        let role = e.text_of("RoleObjectId").and_then(|r| role_by_object.get(r)).cloned();
        let Some(activity) = activity.filter(|_| resource.is_some() || role.is_some()) else {
            unplaced += 1;
            continue;
        };
        let units = |name: &str| match e.text_of(name) {
            Some(t) => units_of(t).map(Some).ok_or_else(|| P6Error::Field(format!("ResourceAssignment {activity}"), name.to_string())),
            None => Ok(None),
        };
        let (planned, actual) = (units("PlannedUnits")?.unwrap_or(0), units("ActualUnits")?.unwrap_or(0));
        // What is left, kept only when it isn't the budget less what is done.
        let remaining = units("RemainingUnits")?.filter(|r| *r != planned.saturating_sub(actual).max(0));
        let rate = units("PlannedUnitsPerTime")?;
        assignments.push(Assignment { activity, resource, role, planned, actual, remaining, rate, ..Assignment::default() });
    }
    if unplaced > 0 {
        warnings.push(tr!("warn-p6-assignments", n = unplaced));
    }

    // What else the project and the file hold, each kind named once.
    const READ: [&str; 10] =
        ["Project", "Calendar", "WBS", "Activity", "Relationship", "ProjectCodeType", "Resource", "Role", "ResourceRole", "UnitOfMeasure"];
    const PROJECT_FIELDS_READ: [&str; 12] = [
        "ObjectId",
        "Id",
        "Name",
        "PlannedStartDate",
        "DataDate",
        "MustFinishByDate",
        "ActivityDefaultCalendarObjectId",
        "Calendar",
        "WBS",
        "Activity",
        "Relationship",
        "ResourceAssignment",
    ];
    let mut skipped: BTreeSet<&str> = root.children.iter().map(|c| c.name.as_str()).filter(|n| !READ.contains(n)).collect();
    skipped.extend(p.children.iter().filter(|c| !c.children.is_empty()).map(|c| c.name.as_str()).filter(|n| !PROJECT_FIELDS_READ.contains(n)));
    if !skipped.is_empty() {
        warnings.push(tr!("warn-p6-skipped", kinds = skipped.into_iter().collect::<Vec<_>>().join(", ")));
    }
    let start = p.text_of("PlannedStartDate").and_then(date_of).or_else(|| p.text_of("DataDate").and_then(date_of)).unwrap_or(Date::EPOCH);
    let mut extra = mkperth_core::Extra::new();
    if let Some(id) = p.text_of("Id") {
        extra.insert(format!("{PREFIX}id"), id.to_string());
    }
    let (baselines, baseline_in_use) = baselines_of(&root, p);
    Ok(Read {
        project: Project {
            name: p.text_of("Name").or_else(|| p.text_of("Id")).unwrap_or_default().to_string(),
            start,
            calendar: project_calendar.clone(),
            calendars: calendars.into_iter().map(|(_, c, _)| c).filter(|c| c.name != project_calendar.name).collect(),
            lag_calendar: Default::default(),
            float_kind: Default::default(),
            critical_by: Default::default(),
            data_date: p.text_of("DataDate").and_then(date_of),
            progress_mode: Default::default(),
            must_finish_by: p.text_of("MustFinishByDate").and_then(date_of),
            bands: Vec::new(),
            markers: Vec::new(),
            month_marks: Vec::new(),
            resources,
            roles,
            assignments,
            comments: Vec::new(),
            levelling: Default::default(),
            baselines,
            baseline_in_use,
            expenses,
            cost_accounts,
            currency: String::new(),
            budget: Default::default(),
            activities,
            relationships,
            extra,
        },
        warnings,
    })
}

/// The project's baselines, P6's `BaselineProject` elements: each one's activities with their
/// planned dates and duration; and the one the project compares with
/// (`CurrentBaselineProjectObjectId`).
fn baselines_of(root: &Element, p: &Element) -> (Vec<Baseline>, Option<String>) {
    let mut by_object: HashMap<&str, String> = HashMap::new();
    let mut list = Vec::new();
    for b in root.all("BaselineProject").take(1000) {
        let name = b.text_of("Name").or_else(|| b.text_of("Id")).unwrap_or("Baseline").to_string();
        let activities = b
            .all("Activity")
            .filter_map(|a| {
                let id = a.text_of("Id")?.to_string();
                let start = a.text_of("PlannedStartDate").or_else(|| a.text_of("StartDate")).and_then(date_of)?;
                let finish = a.text_of("PlannedFinishDate").or_else(|| a.text_of("FinishDate")).and_then(date_of)?;
                let hours = a
                    .text_of("PlannedDuration")
                    .and_then(|h| h.parse::<f64>().ok())
                    .filter(|h| h.is_finite() && *h >= 0.0 && *h < 1e9)
                    .unwrap_or(0.0);
                let cost = a
                    .text_of("PlannedTotalCost")
                    .and_then(|c| c.parse::<f64>().ok())
                    .filter(|c| c.is_finite() && c.abs() < 1e15)
                    .map_or(0, |c| (c * 100.0).round() as i64);
                Some(BaselineActivity { id, start, finish, duration: (hours * 60.0).round() as i64, cost })
            })
            .collect();
        if let Some(o) = b.text_of("ObjectId") {
            by_object.insert(o, name.clone());
        }
        list.push(Baseline { name, activities });
    }
    let in_use = p.text_of("CurrentBaselineProjectObjectId").and_then(|o| by_object.get(o).cloned());
    (list, in_use)
}

/// P6's money (`1500.5`) as hundredths of the currency.
fn money_of(text: &str) -> Option<i64> {
    text.trim().parse::<f64>().ok().filter(|m| m.is_finite() && m.abs() < 1e15).map(|m| (m * 100.0).round() as i64)
}

/// Hundredths of the currency as P6 writes money: `1500.5`.
fn money(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let c = cents.unsigned_abs();
    let text = format!("{sign}{}.{:02}", c / 100, c % 100);
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Text for XML: `&`, `<`, `>`, quotes escaped.
fn esc(text: &str) -> String {
    quick_xml::escape::escape(text).into_owned()
}

/// Working minutes as P6's hours: whole, or with the decimals they need.
fn hours(minutes: i64) -> String {
    if minutes % 60 == 0 { (minutes / 60).to_string() } else { format!("{:.4}", minutes as f64 / 60.0).trim_end_matches('0').to_string() }
}

/// A day's work periods as `WorkTime` elements, indented.
fn work_times(periods: &[Period], indent: &str) -> String {
    let at = |m: u16| format!("{:02}:{:02}:00", m / 60, m % 60);
    periods
        .iter()
        .map(|(a, b)| {
            format!("{indent}<WorkTime>\n{indent}  <Start>{}</Start>\n{indent}  <Finish>{}</Finish>\n{indent}</WorkTime>\n", at(*a), at(*b))
        })
        .collect()
}

fn timestamp(d: Date, evening: bool) -> String {
    format!("{d}T{}", if evening { "17:00:00" } else { "08:00:00" })
}

/// Writes `project` as P6 XML (one project, its calendars, WBS, activities and relationships),
/// durations and lags in hours by each calendar's working day, and the calendars' work periods. Returns the text and what P6's format couldn't
/// hold.
pub fn write(project: &Project) -> (String, Vec<String>) {
    let mut warnings = Vec::new();
    // P6 keeps notes in notebook topics this file doesn't write.
    let noted = project.activities.iter().filter(|a| !a.notes.trim().is_empty()).count();
    if noted > 0 {
        warnings.push(tr!("warn-p6-notes", n = noted));
    }
    if !project.comments.is_empty() {
        warnings.push(tr!("warn-p6-comments", n = project.comments.len()));
    }
    if !(project.bands.is_empty() && project.markers.is_empty() && project.month_marks.is_empty()) {
        warnings.push(tr!("warn-p6-chart-notes"));
    }
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!("<APIBusinessObjects xmlns=\"{NAMESPACE}\">\n"));
    let calendars: Vec<&Calendar> = std::iter::once(&project.calendar).chain(project.calendars.iter()).collect();
    let calendar_object = |name: &str| calendars.iter().position(|c| c.name == name).map_or(1, |k| k + 1);
    // An activity's working day, by its calendar, to say days of duration and lag in hours.
    let day_of = |a: &Activity| {
        a.calendar.as_deref().and_then(|n| calendars.iter().find(|c| c.name == n)).map_or(project.calendar.day_minutes(), |c| c.day_minutes())
    };
    for (k, c) in calendars.iter().enumerate() {
        out.push_str(&format!(
            "  <Calendar>\n    <ObjectId>{}</ObjectId>\n    <Name>{}</Name>\n    <HoursPerDay>{}</HoursPerDay>\n    <StandardWorkWeek>\n",
            k + 1,
            // P6 names every calendar.
            esc(match c.name.as_str() {
                "" if c.week == [true; 7] => "Calendar days",
                "" => "Standard",
                name => name,
            }),
            hours(c.day_minutes())
        ));
        for w in Weekday::ALL {
            out.push_str(&format!("      <StandardWorkHours>\n        <DayOfWeek>{w:?}</DayOfWeek>\n"));
            if c.week.get(w.index()).copied().unwrap_or(false) {
                out.push_str(&work_times(c.weekday_periods(w.index()), "        "));
            }
            out.push_str("      </StandardWorkHours>\n");
        }
        out.push_str("    </StandardWorkWeek>\n    <HolidayOrExceptions>\n");
        // A region's holidays are rules here and dates in P6: ten years of them from the start.
        let mut holidays = c.holidays.clone();
        if let Some(region) = c.region {
            let year = project.start.ymd().0;
            let (from, to) = (Date::from_ymd(year, 1, 1), Date::from_ymd(year.saturating_add(9), 12, 31));
            if let (Some(mut d), Some(to)) = (from, to) {
                while d <= to {
                    if region.is_holiday(d) && c.week.get(d.weekday().index()).copied().unwrap_or(false) {
                        holidays.insert(d);
                    }
                    match d.succ() {
                        Some(next) => d = next,
                        None => break,
                    }
                }
            }
        }
        // A day worked on its own periods is written once, as worked.
        for d in holidays.iter().filter(|d| !c.extra_days.contains(d) && !c.exceptions.get(d).is_some_and(|p| !p.is_empty())) {
            out.push_str(&format!("      <HolidayOrException>\n        <Date>{d}T00:00:00</Date>\n      </HolidayOrException>\n"));
        }
        let worked: BTreeSet<Date> = c.extra_days.iter().chain(c.exceptions.iter().filter(|(_, p)| !p.is_empty()).map(|(d, _)| d)).copied().collect();
        for d in &worked {
            out.push_str(&format!(
                "      <HolidayOrException>\n        <Date>{d}T00:00:00</Date>\n{}      </HolidayOrException>\n",
                work_times(c.day_periods(*d), "        ")
            ));
        }
        out.push_str("    </HolidayOrExceptions>\n  </Calendar>\n");
    }
    // A material's units of measure, its roles and its resources: the file's, before the project.
    let mut measures: Vec<&str> =
        project.resources.iter().filter(|r| r.kind == ResourceKind::Material && !r.unit.is_empty()).map(|r| r.unit.as_str()).collect();
    measures.sort_unstable();
    measures.dedup();
    for (k, u) in measures.iter().enumerate() {
        out.push_str(&format!(
            "  <UnitOfMeasure>\n    <ObjectId>{}</ObjectId>\n    <Abbreviation>{}</Abbreviation>\n    <Name>{}</Name>\n  </UnitOfMeasure>\n",
            k + 1,
            esc(u),
            esc(u)
        ));
    }
    let role_object = |id: &str| project.roles.iter().position(|r| r.id == id).map(|k| k + 1);
    let resource_object = |id: &str| project.resources.iter().position(|r| r.id == id).map(|k| k + 1);
    for (k, r) in project.roles.iter().enumerate() {
        out.push_str(&format!(
            "  <Role>\n    <ObjectId>{}</ObjectId>\n    <Id>{}</Id>\n    <Name>{}</Name>\n  </Role>\n",
            k + 1,
            esc(&r.id),
            esc(&r.name)
        ));
    }
    for (k, r) in project.resources.iter().enumerate() {
        let kind = match r.kind {
            ResourceKind::Labor => "Labor",
            ResourceKind::Nonlabor => "Nonlabor",
            ResourceKind::Material => "Material",
        };
        out.push_str(&format!(
            "  <Resource>\n    <ObjectId>{}</ObjectId>\n    <Id>{}</Id>\n    <Name>{}</Name>\n    <ResourceType>{kind}</ResourceType>\n    <CalendarObjectId>{}</CalendarObjectId>\n    <MaxUnitsPerTime>{}</MaxUnitsPerTime>\n",
            k + 1,
            esc(&r.id),
            esc(&r.name),
            r.calendar.as_deref().map_or(1, calendar_object),
            units_text(r.max_units)
        ));
        if let Some(u) = measures.iter().position(|u| *u == r.unit).filter(|_| r.kind == ResourceKind::Material) {
            out.push_str(&format!("    <UnitOfMeasureObjectId>{}</UnitOfMeasureObjectId>\n", u + 1));
        }
        out.push_str("  </Resource>\n");
    }
    // Each resource's prices as its rates; a first price from the start of time takes effect on
    // the project's start.
    let mut rates = 0usize;
    for r in &project.resources {
        for price in &r.prices {
            rates += 1;
            out.push_str(&format!(
                "  <ResourceRate>\n    <ObjectId>{rates}</ObjectId>\n    <ResourceObjectId>{}</ResourceObjectId>\n    <EffectiveDate>{}</EffectiveDate>\n    <PricePerUnit>{}</PricePerUnit>\n  </ResourceRate>\n",
                resource_object(&r.id).unwrap_or(0),
                timestamp(price.from.unwrap_or(project.start), false),
                money(price.per_unit)
            ));
        }
    }
    let account_object = |id: &str| project.cost_accounts.iter().position(|c| c.id == id).map(|k| k + 1);
    for (k, c) in project.cost_accounts.iter().enumerate() {
        out.push_str(&format!(
            "  <CostAccount>\n    <ObjectId>{}</ObjectId>\n    <Id>{}</Id>\n    <Name>{}</Name>\n",
            k + 1,
            esc(&c.id),
            esc(&c.name)
        ));
        if let Some(parent) = c.parent.as_deref().and_then(account_object) {
            out.push_str(&format!("    <ParentObjectId>{parent}</ParentObjectId>\n"));
        }
        out.push_str("  </CostAccount>\n");
    }
    let mut pairs = 0usize;
    for r in &project.resources {
        for role in r.roles.iter().filter_map(|id| role_object(id)) {
            pairs += 1;
            out.push_str(&format!(
                "  <ResourceRole>\n    <ObjectId>{pairs}</ObjectId>\n    <ResourceObjectId>{}</ResourceObjectId>\n    <RoleObjectId>{role}</RoleObjectId>\n  </ResourceRole>\n",
                resource_object(&r.id).unwrap_or(0)
            ));
        }
    }
    out.push_str(&format!(
        "  <Project>\n    <ObjectId>1</ObjectId>\n    <Id>{}</Id>\n    <Name>{}</Name>\n    <PlannedStartDate>{}</PlannedStartDate>\n    <ActivityDefaultCalendarObjectId>1</ActivityDefaultCalendarObjectId>\n",
        esc(project.extra.get("p6.id").map_or(project.name.as_str(), String::as_str)),
        esc(&project.name),
        timestamp(project.start, false)
    ));
    if let Some(d) = project.must_finish_by {
        out.push_str(&format!("    <MustFinishByDate>{}</MustFinishByDate>\n", timestamp(d, true)));
    }
    if let Some(d) = project.data_date {
        out.push_str(&format!("    <DataDate>{}</DataDate>\n", timestamp(d, false)));
    }
    // A baseline's object number: 2 and on, after the project's 1.
    let baseline_object = |name: &str| project.baselines.iter().position(|b| b.name == name).map(|k| k + 2);
    if let Some(k) = project.baseline_in_use.as_deref().and_then(baseline_object) {
        out.push_str(&format!("    <CurrentBaselineProjectObjectId>{k}</CurrentBaselineProjectObjectId>\n"));
    }
    // Summaries are WBS elements; every other activity an Activity under its WBS. An activity's
    // object number is its place in the list, from 1000.
    let summary = |id: &str| project.activities.iter().any(|a| a.parent.as_deref() == Some(id));
    let object = |id: &str| project.activities.iter().position(|a| a.id == id).map_or(0, |k| k + 1000);
    for a in project.activities.iter().filter(|a| summary(&a.id)) {
        out.push_str(&format!(
            "    <WBS>\n      <ObjectId>{}</ObjectId>\n      <Code>{}</Code>\n      <Name>{}</Name>\n",
            object(&a.id),
            esc(&a.id),
            esc(&a.name)
        ));
        if let Some(p) = &a.parent {
            out.push_str(&format!("      <ParentObjectId>{}</ParentObjectId>\n", object(p)));
        }
        out.push_str("      <ProjectObjectId>1</ProjectObjectId>\n    </WBS>\n");
    }
    for a in project.activities.iter().filter(|a| !summary(&a.id)) {
        let kind = match a.kind {
            ActivityKind::Task if a.resource_dependent => "Resource Dependent",
            ActivityKind::Task => "Task Dependent",
            ActivityKind::StartMilestone => "Start Milestone",
            ActivityKind::FinishMilestone => "Finish Milestone",
        };
        out.push_str(&format!(
            "    <Activity>\n      <ObjectId>{}</ObjectId>\n      <Id>{}</Id>\n      <Name>{}</Name>\n      <Type>{kind}</Type>\n      <PlannedDuration>{}</PlannedDuration>\n      <CalendarObjectId>{}</CalendarObjectId>\n      <ProjectObjectId>1</ProjectObjectId>\n",
            object(&a.id),
            esc(&a.id),
            esc(&a.name),
            hours(a.working_minutes(day_of(a))),
            a.calendar.as_deref().map_or(1, calendar_object)
        ));
        if let Some(p) = &a.parent {
            out.push_str(&format!("      <WBSObjectId>{}</WBSObjectId>\n", object(p)));
        }
        let status = match (a.actual_start, a.actual_finish) {
            (_, Some(_)) => "Completed",
            (Some(_), None) => "In Progress",
            (None, None) => "Not Started",
        };
        out.push_str(&format!("      <Status>{status}</Status>\n"));
        out.push_str(&format!("      <DurationType>{}</DurationType>\n", duration_type_name(a.duration_type)));
        if a.levelling_priority != LevellingPriority::default() {
            out.push_str(&format!("      <LevelingPriority>{}</LevelingPriority>\n", levelling_priority_name(a.levelling_priority)));
        }
        if let Some(d) = a.actual_start {
            out.push_str(&format!("      <ActualStartDate>{}</ActualStartDate>\n", timestamp(d, false)));
        }
        if let Some(d) = a.actual_finish {
            out.push_str(&format!("      <ActualFinishDate>{}</ActualFinishDate>\n", timestamp(d, true)));
        }
        if let Some(d) = a.expected_finish {
            out.push_str(&format!("      <ExpectedFinishDate>{}</ExpectedFinishDate>\n", timestamp(d, true)));
        }
        if let (Some(_), None, Some(m)) = (a.actual_start, a.actual_finish, a.remaining_minutes) {
            out.push_str(&format!("      <RemainingDuration>{}</RemainingDuration>\n", hours(m)));
        }
        for (which, c) in [("Primary", a.constraint), ("Secondary", a.secondary_constraint)] {
            if let Some(c) = c {
                let (name, date) = constraint_name(c);
                out.push_str(&format!("      <{which}ConstraintType>{name}</{which}ConstraintType>\n"));
                if let Some(d) = date {
                    let evening = name.starts_with("Finish") || name == "Mandatory Finish";
                    out.push_str(&format!("      <{which}ConstraintDate>{}</{which}ConstraintDate>\n", timestamp(d, evening)));
                }
            }
        }
        out.push_str("    </Activity>\n");
    }
    for (k, r) in project.relationships.iter().enumerate() {
        out.push_str(&format!(
            "    <Relationship>\n      <ObjectId>{}</ObjectId>\n      <PredecessorActivityObjectId>{}</PredecessorActivityObjectId>\n      <SuccessorActivityObjectId>{}</SuccessorActivityObjectId>\n      <Type>{}</Type>\n      <Lag>{}</Lag>\n    </Relationship>\n",
            k + 1,
            object(&r.predecessor),
            object(&r.successor),
            relationship_name(r.kind),
            hours(r.lag_minutes.unwrap_or_else(|| {
                let day = project.activities.iter().find(|a| a.id == r.predecessor).map_or(project.calendar.day_minutes(), day_of);
                r.lag.saturating_mul(day)
            }))
        ));
    }
    // Expenses on activities; a summary is a WBS element and takes none.
    for (k, e) in project.expenses.iter().enumerate().filter(|(_, e)| !summary(&e.activity)) {
        let accrual = match e.accrual {
            Accrual::Start => "Start of Activity",
            Accrual::End => "End of Activity",
            Accrual::Uniform => "Uniform over Activity",
        };
        out.push_str(&format!(
            "    <ActivityExpense>\n      <ObjectId>{}</ObjectId>\n      <ActivityObjectId>{}</ActivityObjectId>\n      <ExpenseItem>{}</ExpenseItem>\n      <PlannedCost>{}</PlannedCost>\n      <ActualCost>{}</ActualCost>\n      <AccrualType>{accrual}</AccrualType>\n      <ProjectObjectId>1</ProjectObjectId>\n",
            k + 1,
            object(&e.activity),
            esc(&e.name),
            money(e.planned),
            money(e.actual)
        ));
        if let Some(r) = e.remaining {
            out.push_str(&format!("      <RemainingCost>{}</RemainingCost>\n", money(r)));
        }
        if let Some(a) = e.account.as_deref().and_then(account_object) {
            out.push_str(&format!("      <CostAccountObjectId>{a}</CostAccountObjectId>\n"));
        }
        out.push_str("    </ActivityExpense>\n");
    }
    // Assignments to activities; a summary is a WBS element in P6 and takes none.
    let mut on_summaries = 0usize;
    for (k, x) in project.assignments.iter().enumerate() {
        if summary(&x.activity) {
            on_summaries += 1;
            continue;
        }
        out.push_str(&format!(
            "    <ResourceAssignment>\n      <ObjectId>{}</ObjectId>\n      <ActivityObjectId>{}</ActivityObjectId>\n",
            k + 1,
            object(&x.activity)
        ));
        if let Some(r) = x.resource.as_deref().and_then(resource_object) {
            out.push_str(&format!("      <ResourceObjectId>{r}</ResourceObjectId>\n"));
        }
        if let Some(r) = x.role.as_deref().and_then(role_object) {
            out.push_str(&format!("      <RoleObjectId>{r}</RoleObjectId>\n"));
        }
        out.push_str(&format!("      <PlannedUnits>{}</PlannedUnits>\n", units_text(x.planned)));
        // Units an hour when set; else they are the budget over the duration, as P6 works out.
        if let Some(r) = x.rate {
            out.push_str(&format!("      <PlannedUnitsPerTime>{}</PlannedUnitsPerTime>\n", units_text(r)));
        }
        out.push_str(&format!(
            "      <ActualUnits>{}</ActualUnits>\n      <RemainingUnits>{}</RemainingUnits>\n      <ProjectObjectId>1</ProjectObjectId>\n    </ResourceAssignment>\n",
            units_text(x.actual),
            units_text(x.remaining_units())
        ));
    }
    if on_summaries > 0 {
        warnings.push(tr!("warn-p6-summary-assignments", n = on_summaries));
    }
    out.push_str("  </Project>\n");
    // Each baseline as P6 keeps one: a project of its own, its activities at their planned dates.
    let mut next = 1000 + project.activities.len();
    for (k, b) in project.baselines.iter().enumerate() {
        out.push_str(&format!(
            "  <BaselineProject>\n    <ObjectId>{}</ObjectId>\n    <Id>{}</Id>\n    <Name>{}</Name>\n",
            k + 2,
            esc(&b.name),
            esc(&b.name)
        ));
        for a in &b.activities {
            next += 1;
            out.push_str(&format!(
                "    <Activity>\n      <ObjectId>{next}</ObjectId>\n      <Id>{}</Id>\n      <PlannedStartDate>{}</PlannedStartDate>\n      <PlannedFinishDate>{}</PlannedFinishDate>\n      <PlannedDuration>{}</PlannedDuration>\n      <PlannedTotalCost>{}</PlannedTotalCost>\n      <ProjectObjectId>{}</ProjectObjectId>\n    </Activity>\n",
                esc(&a.id),
                timestamp(a.start, false),
                timestamp(a.finish, true),
                hours(a.duration),
                money(a.cost),
                k + 2
            ));
        }
        out.push_str("  </BaselineProject>\n");
    }
    out.push_str("</APIBusinessObjects>\n");
    (out, warnings)
}

#[cfg(test)]
mod tests;
