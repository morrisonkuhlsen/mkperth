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

pub mod tree;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use mkperth_core::{
    Accrual, Activity, ActivityCode, ActivityKind, Assignment, Baseline, BaselineActivity, BudgetChange, Budgeting, Calendar, ChangeStatus,
    Classification, CodeValue, Constraint, CostAccount, Date, DurationType, EarnedValue, EpsNode, EtcTechnique, EvTechnique, Expense, FieldKind,
    FieldValue, FinancialPeriod, Fund, FundingSource, Indicator, LevellingPriority, Limit, ObsNode, Organisation, Period, PeriodRecord, Price,
    Project, Relationship, RelationshipType, Resource, ResourceKind, Role, SpendingMonth, UNIT, UserField, WbsBudget, Weekday,
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

    // How the activities measure their percent complete, counted: the project's technique when
    // it is by each activity's percent complete.
    let mut percent_kinds: BTreeMap<String, usize> = BTreeMap::new();
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
        // P6's physical percent complete, from 0 to 100; 0 is none.
        a.percent_complete = e
            .text_of("PhysicalPercentComplete")
            .and_then(|x| x.trim().parse::<f64>().ok())
            .filter(|x| x.is_finite() && *x > 0.0 && *x <= 100.0)
            .map(|x| (x * 100.0).round() as i64);
        if let Some(kind) = e.text_of("PercentCompleteType") {
            *percent_kinds.entry(kind.to_string()).or_insert(0usize) += 1;
        }
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
        relationships.push(Relationship { predecessor, successor, kind, lag, lag_minutes, lag_kind: Default::default() });
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
    // Prices and limits over time: each resource's rates by the day they take effect, each with
    // its price and its most units an hour. The earliest price holds from the start of time, as
    // MK Perth keeps a first price; a limit is kept from the day it differs from the one before
    // (the resource's own, before any). A rate that repeats the one before it says nothing new.
    // Each rate: its resource's place, its day, its price and its limit.
    type Rate = (usize, Option<Date>, Option<i64>, Option<i64>);
    let mut rates: Vec<Rate> = Vec::new();
    for e in root.all("ResourceRate") {
        let Some(k) = e.text_of("ResourceObjectId").and_then(|r| resource_by_object.get(r)).and_then(|id| resources.iter().position(|x| x.id == *id))
        else {
            continue;
        };
        let max = e.text_of("MaxUnitsPerTime").and_then(units_of);
        rates.push((k, e.text_of("EffectiveDate").and_then(date_of), e.text_of("PricePerUnit").and_then(money_of), max));
    }
    rates.sort_by_key(|(k, from, _, _)| (*k, *from));
    for (k, from, price, max) in rates {
        let Some(r) = resources.get_mut(k).filter(|r| r.prices.len() < 1000 && r.limits.len() < 1000) else { continue };
        if let Some(price) = price
            && r.prices.last().is_none_or(|p| p.per_unit != price)
        {
            r.prices.push(Price { from: if r.prices.is_empty() { None } else { from }, per_unit: price });
        }
        if let (Some(max), Some(from)) = (max, from)
            && max != r.limits.last().map_or(r.max_units, |l| l.max_units)
        {
            r.limits.push(Limit { from, max_units: max });
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
    // The budget: the project's and its WBS's first approved, the log of changes, the spending
    // plans month by month, the funding sources and what each gives.
    let mut budget = Budgeting::default();
    let budget_of = |b: &mut Budgeting, wbs: Option<String>| -> usize {
        match b.budgets.iter().position(|x| x.wbs == wbs) {
            Some(k) => k,
            None => {
                b.budgets.push(WbsBudget { wbs, ..WbsBudget::default() });
                b.budgets.len() - 1
            }
        }
    };
    if let Some(m) = p.text_of("OriginalBudget").and_then(money_of) {
        let k = budget_of(&mut budget, None);
        if let Some(b) = budget.budgets.get_mut(k) {
            b.original = m;
        }
    }
    for w in &wbs {
        let (Some(m), Some(id)) = (w.text_of("OriginalBudget").and_then(money_of), w.text_of("ObjectId").and_then(|o| wbs_id.get(o))) else {
            continue;
        };
        let k = budget_of(&mut budget, Some(id.clone()));
        if let Some(b) = budget.budgets.get_mut(k) {
            b.original = m;
        }
    }
    let wbs_of = |e: &Element| e.text_of("WBSObjectId").and_then(|o| wbs_id.get(o)).cloned();
    for e in p.all("ProjectBudgetChangeLog").chain(root.all("ProjectBudgetChangeLog")) {
        let Some(date) = e.text_of("Date").and_then(date_of) else { continue };
        let status = match e.text_of("Status") {
            Some("Approved") => ChangeStatus::Approved,
            Some("Not Approved") => ChangeStatus::NotApproved,
            _ => ChangeStatus::Pending,
        };
        // Its number, unless another change has it: then after the highest.
        let given = e.text_of("ChangeNumber").and_then(|n| n.trim().parse::<u32>().ok()).filter(|n| !budget.log.iter().any(|c| c.number == *n));
        let Some(number) = given.or_else(|| budget.log.iter().map(|c| c.number).max().unwrap_or(0).checked_add(1)) else { continue };
        budget.log.push(BudgetChange {
            number,
            wbs: wbs_of(e),
            date,
            amount: e.text_of("Amount").and_then(money_of).unwrap_or(0),
            status,
            responsible: e.text_of("Responsible").unwrap_or_default().to_string(),
            reason: e.text_of("Reason").unwrap_or_default().to_string(),
        });
    }
    // The project's plan, or a WBS's when it names one; `WBSSpendingPlan`, as MK Perth wrote a
    // WBS's before #31, still read.
    let plans = p.all("ProjectSpendingPlan").chain(root.all("ProjectSpendingPlan")).map(|e| (e, wbs_of(e)));
    let wbs_plans = p.all("WBSSpendingPlan").chain(root.all("WBSSpendingPlan")).filter_map(|e| Some((e, Some(wbs_of(e)?))));
    for (e, wbs) in plans.chain(wbs_plans) {
        let (Some((y, m, _)), Some(amount)) = (e.text_of("Date").and_then(date_of).map(|d| d.ymd()), e.text_of("SpendingPlan").and_then(money_of))
        else {
            continue;
        };
        let Some(month) = Date::from_ymd(y, m, 1) else { continue };
        let k = budget_of(&mut budget, wbs);
        let Some(b) = budget.budgets.get_mut(k) else { continue };
        // Two entries in one month add up.
        match b.plan.iter_mut().find(|x| x.month == month) {
            Some(x) => x.amount = x.amount.saturating_add(amount),
            None => b.plan.push(SpendingMonth { month, amount }),
        }
        b.plan.sort_by_key(|x| x.month);
    }
    let mut source_by_object: HashMap<&str, String> = HashMap::new();
    for e in root.all("FundingSource").chain(p.all("FundingSource")) {
        let (Some(o), Some(name)) = (e.text_of("ObjectId"), e.text_of("Name")) else { continue };
        let mut id = name.to_string();
        while budget.sources.iter().any(|f| f.id == id) {
            id.push('*');
        }
        let description = e.text_of("Description").filter(|d| !d.trim().is_empty()).unwrap_or(name);
        budget.sources.push(FundingSource { id: id.clone(), name: description.to_string(), parent: None });
        source_by_object.insert(o, id);
    }
    for (k, e) in
        root.all("FundingSource").chain(p.all("FundingSource")).filter(|e| e.text_of("ObjectId").is_some() && e.text_of("Name").is_some()).enumerate()
    {
        let parent = e.text_of("ParentObjectId").and_then(|o| source_by_object.get(o)).cloned();
        if let Some(f) = budget.sources.get_mut(k) {
            f.parent = parent.filter(|x| *x != f.id);
        }
    }
    for e in p.all("ProjectFunding").chain(root.all("ProjectFunding")) {
        let (Some(source), Some(amount)) =
            (e.text_of("FundingSourceObjectId").and_then(|o| source_by_object.get(o)), e.text_of("Amount").and_then(money_of))
        else {
            continue;
        };
        match budget.funds.iter_mut().find(|f| f.source == *source) {
            Some(f) => f.amount = f.amount.saturating_add(amount),
            None => budget.funds.push(Fund { source: source.clone(), amount }),
        }
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

    let classification = classification_of(&root, p, &by_object, &mut activities, &mut warnings);
    let organisation = organisation_of(&root, p, &wbs, &wbs_id, &mut activities);

    // What else the project and the file hold, each kind named once.
    const READ: [&str; 21] = [
        "EPS",
        "OBS",
        "ActivityCodeType",
        "ActivityCode",
        "UDFType",
        "FinancialPeriod",
        "Project",
        "Calendar",
        "WBS",
        "Activity",
        "Relationship",
        "ProjectCodeType",
        "Resource",
        "Role",
        "ResourceRole",
        "UnitOfMeasure",
        "ResourceRate",
        "CostAccount",
        "ActivityExpense",
        "FundingSource",
        "BaselineProject",
    ];
    const PROJECT_FIELDS_READ: [&str; 24] = [
        "ActivityPeriodActual",
        "ActivityCodeType",
        "ActivityCode",
        "ActivityCodeAssignment",
        "UDFType",
        "UDFValue",
        "ActivityExpense",
        "ProjectBudgetChangeLog",
        "ProjectSpendingPlan",
        "WBSSpendingPlan",
        "ProjectFunding",
        "BaselineProject",
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
    let earned_value = earned_value_of(&root, p, &percent_kinds, &by_object);
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
            budget,
            earned_value,
            classification,
            external: Vec::new(),
            organisation,
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

/// The OBS, the EPS from the top down to the node holding the project (following each node's
/// parent, never further than there are nodes), who answers for the project and each WBS.
fn organisation_of(root: &Element, p: &Element, wbs: &[&Element], wbs_id: &HashMap<String, String>, activities: &mut [Activity]) -> Organisation {
    let mut o = Organisation::default();
    let mut obs_name: HashMap<&str, String> = HashMap::new();
    let nodes: Vec<&Element> = root.all("OBS").chain(p.all("OBS")).collect();
    for e in &nodes {
        let (Some(id), Some(name)) = (e.text_of("ObjectId"), e.text_of("Name")) else { continue };
        let mut name = name.to_string();
        while o.obs.iter().any(|x| x.name == name) {
            name.push('*');
        }
        obs_name.insert(id, name.clone());
        o.obs.push(ObsNode { name, description: e.text_of("Description").unwrap_or_default().to_string(), parent: None });
    }
    for e in &nodes {
        let (Some(name), Some(parent)) =
            (e.text_of("ObjectId").and_then(|i| obs_name.get(i)), e.text_of("ParentObjectId").and_then(|i| obs_name.get(i)))
        else {
            continue;
        };
        if name != parent
            && let Some(x) = o.obs.iter_mut().find(|x| x.name == *name)
        {
            x.parent = Some(parent.clone());
        }
    }
    let eps: Vec<&Element> = root.all("EPS").chain(p.all("EPS")).collect();
    let mut at = p.text_of("ParentEPSObjectId");
    while let Some(id) = at.filter(|_| o.eps.len() < eps.len()) {
        let Some(e) = eps.iter().find(|e| e.text_of("ObjectId") == Some(id)) else { break };
        o.eps.insert(0, EpsNode { id: e.text_of("Id").unwrap_or_default().to_string(), name: e.text_of("Name").unwrap_or_default().to_string() });
        at = e.text_of("ParentObjectId");
    }
    o.responsible = p.text_of("OBSObjectId").and_then(|i| obs_name.get(i)).cloned();
    for w in wbs {
        let (Some(id), Some(r)) = (w.text_of("ObjectId").and_then(|x| wbs_id.get(x)), w.text_of("OBSObjectId").and_then(|i| obs_name.get(i))) else {
            continue;
        };
        if let Some(a) = activities.iter_mut().find(|a| a.id == *id) {
            a.responsible = Some(r.clone());
        }
    }
    o
}

/// P6's names for what a user-defined field holds.
const FIELD_KINDS: [(&str, FieldKind); 6] = [
    ("Text", FieldKind::Text),
    ("Double", FieldKind::Number),
    ("Integer", FieldKind::Integer),
    ("Cost", FieldKind::Cost),
    ("Start Date", FieldKind::Date),
    ("Indicator", FieldKind::Indicator),
];

/// The activity codes and the activities' user-defined fields, with each activity's values:
/// codes and their values by object number, assignments and values naming them. A field P6
/// computes from a code (`Code`) or keeps for something other than activities is left out.
fn classification_of(
    root: &Element,
    p: &Element,
    by_object: &HashMap<String, String>,
    activities: &mut [Activity],
    warnings: &mut Vec<String>,
) -> Classification {
    let mut c = Classification::default();
    let mut type_by_object: HashMap<&str, usize> = HashMap::new();
    for e in root.all("ActivityCodeType").chain(p.all("ActivityCodeType")) {
        let (Some(o), Some(name)) = (e.text_of("ObjectId"), e.text_of("Name")) else { continue };
        let mut name = name.to_string();
        while c.code(&name).is_some() {
            name.push('*');
        }
        type_by_object.insert(o, c.codes.len());
        c.codes.push(ActivityCode { name, values: Vec::new() });
    }
    // Values by object number: their code's place and their own short code.
    let mut value_by_object: HashMap<&str, (usize, String)> = HashMap::new();
    let codes: Vec<&Element> = root.all("ActivityCode").chain(p.all("ActivityCode")).collect();
    for e in &codes {
        let (Some(o), Some(&k), Some(value)) =
            (e.text_of("ObjectId"), e.text_of("CodeTypeObjectId").and_then(|t| type_by_object.get(t)), e.text_of("CodeValue"))
        else {
            continue;
        };
        let Some(code) = c.codes.get_mut(k) else { continue };
        if code.values.iter().any(|v| v.code == value) {
            continue;
        }
        code.values.push(CodeValue { code: value.to_string(), description: e.text_of("Description").unwrap_or_default().to_string(), parent: None });
        value_by_object.insert(o, (k, value.to_string()));
    }
    for e in &codes {
        let (Some((k, value)), Some((pk, parent))) =
            (e.text_of("ObjectId").and_then(|o| value_by_object.get(o)), e.text_of("ParentObjectId").and_then(|o| value_by_object.get(o)))
        else {
            continue;
        };
        if k == pk
            && value != parent
            && let Some(v) = c.codes.get_mut(*k).and_then(|code| code.values.iter_mut().find(|v| v.code == *value))
        {
            v.parent = Some(parent.clone());
        }
    }
    let index: HashMap<&str, usize> = activities.iter().enumerate().map(|(i, a)| (a.id.as_str(), i)).collect();
    let activity_of = |e: &Element, field: &str| e.text_of(field).and_then(|o| by_object.get(o)).and_then(|id| index.get(id.as_str()).copied());
    let mut assigned: Vec<(usize, String, String)> = Vec::new();
    for e in p.all("ActivityCodeAssignment").chain(root.all("ActivityCodeAssignment")) {
        let (Some(i), Some((k, value))) =
            (activity_of(e, "ActivityObjectId"), e.text_of("ActivityCodeObjectId").and_then(|o| value_by_object.get(o)))
        else {
            continue;
        };
        if let Some(code) = c.codes.get(*k) {
            assigned.push((i, code.name.clone(), value.clone()));
        }
    }
    let mut field_by_object: HashMap<&str, usize> = HashMap::new();
    let mut left_out = 0usize;
    for e in root.all("UDFType").chain(p.all("UDFType")) {
        let (Some(o), Some(title)) = (e.text_of("ObjectId"), e.text_of("Title")) else { continue };
        let kind = e.text_of("DataType").and_then(|d| FIELD_KINDS.iter().find(|(n, _)| *n == d)).map(|(_, k)| *k);
        let kind = match (e.text_of("SubjectArea"), kind, e.text_of("DataType")) {
            (Some("Activity") | None, Some(k), _) => k,
            // A finish date is a date too.
            (Some("Activity") | None, None, Some("Finish Date")) => FieldKind::Date,
            _ => {
                left_out += 1;
                continue;
            }
        };
        let mut name = title.to_string();
        while c.field(&name).is_some() {
            name.push('*');
        }
        field_by_object.insert(o, c.fields.len());
        c.fields.push(UserField { name, kind });
    }
    if left_out > 0 {
        warnings.push(tr!("warn-p6-fields", n = left_out));
    }
    let mut values: Vec<(usize, String, FieldValue)> = Vec::new();
    for e in p.all("UDFValue").chain(root.all("UDFValue")) {
        let (Some(i), Some(field)) =
            (activity_of(e, "ForeignObjectId"), e.text_of("UDFTypeObjectId").and_then(|o| field_by_object.get(o)).and_then(|k| c.fields.get(*k)))
        else {
            continue;
        };
        let number = |name: &str| e.text_of(name).and_then(|x| x.trim().parse::<f64>().ok()).filter(|x| x.is_finite() && x.abs() < 1e12);
        let value = match field.kind {
            FieldKind::Text => e.text_of("Text").map(|t| FieldValue::Text(t.to_string())),
            FieldKind::Number => number("Double").map(|x| FieldValue::Number((x * 1e6).round() as i64)),
            FieldKind::Integer => e.text_of("Integer").and_then(|x| x.trim().parse::<i64>().ok()).map(FieldValue::Integer),
            FieldKind::Cost => e.text_of("Cost").and_then(money_of).map(FieldValue::Cost),
            FieldKind::Date => e.text_of("StartDate").or_else(|| e.text_of("FinishDate")).and_then(date_of).map(FieldValue::Date),
            FieldKind::Indicator => e
                .text_of("Indicator")
                .and_then(|x| Indicator::ALL.into_iter().find(|i| i.code().eq_ignore_ascii_case(x.trim())))
                .map(FieldValue::Indicator),
        };
        if let Some(v) = value {
            values.push((i, field.name.clone(), v));
        }
    }
    for (i, code, value) in assigned {
        if let Some(a) = activities.get_mut(i) {
            a.codes.insert(code, value);
        }
    }
    for (i, field, value) in values {
        if let Some(a) = activities.get_mut(i) {
            a.fields.insert(field, value);
        }
    }
    c
}

/// P6's name for a field's kind.
fn field_kind_name(kind: FieldKind) -> &'static str {
    FIELD_KINDS.iter().find(|(_, k)| *k == kind).map_or("Text", |(n, _)| n)
}

/// P6's names for the earned value techniques: how earned value is computed, and the
/// estimate to complete.
const EV_ACTIVITY: &str = "Activity Percent Complete";
const EV_TECHNIQUES: [(&str, EvTechnique); 3] =
    [("0 / 100", EvTechnique::ZeroHundred), ("50 / 50", EvTechnique::FiftyFifty), ("Custom Percent Complete", EvTechnique::Custom(0))];
const ETC_TECHNIQUES: [(&str, EtcTechnique); 4] = [
    ("ETC = Remaining Cost for Activity", EtcTechnique::Remaining),
    ("Performance Factor = 1", EtcTechnique::Planned),
    ("Performance Factor = 1 / Cost Performance Index", EtcTechnique::Cpi),
    ("Performance Factor = 1 / (Cost Performance Index * Schedule Performance Index)", EtcTechnique::CpiSpi),
];
/// The names MK Perth wrote before they were checked against Oracle's reference (#31), still read.
const ETC_TECHNIQUES_BEFORE: [(&str, EtcTechnique); 3] = [
    ("PF = 1", EtcTechnique::Planned),
    ("PF = 1 / Cost Performance Index", EtcTechnique::Cpi),
    ("PF = 1 / (Cost Performance Index * Schedule Performance Index)", EtcTechnique::CpiSpi),
];

/// The percent complete type P6 gives an activity measured by `technique`.
fn percent_kind(technique: EvTechnique) -> &'static str {
    match technique {
        EvTechnique::Duration => "Duration",
        EvTechnique::Units => "Units",
        _ => "Physical",
    }
}

/// The project's earned value techniques and its financial periods. By each activity's percent
/// complete, the technique is the type most of them have.
fn earned_value_of(root: &Element, p: &Element, kinds: &BTreeMap<String, usize>, by_object: &HashMap<String, String>) -> EarnedValue {
    let technique = match p.text_of("EarnedValueComputeType") {
        Some(EV_ACTIVITY) | None => match kinds.iter().max_by_key(|(_, n)| **n).map(|(k, _)| k.as_str()) {
            Some("Duration") => EvTechnique::Duration,
            Some("Units") => EvTechnique::Units,
            _ => EvTechnique::Physical,
        },
        Some(name) => match EV_TECHNIQUES.iter().find(|(n, _)| *n == name).map(|(_, t)| *t) {
            Some(EvTechnique::Custom(_)) => {
                let share = p.text_of("EarnedValueUserPercent").and_then(|x| x.trim().parse::<f64>().ok()).filter(|x| x.is_finite());
                EvTechnique::Custom(share.map_or(0, |x| (x * 100.0).round().clamp(0.0, 10_000.0) as i64))
            }
            Some(t) => t,
            None => EvTechnique::Physical,
        },
    };
    let etc = p
        .text_of("EarnedValueETCComputeType")
        .and_then(|n| ETC_TECHNIQUES.iter().chain(ETC_TECHNIQUES_BEFORE.iter()).find(|(x, _)| *x == n))
        .map(|(_, t)| *t)
        .unwrap_or_default();
    let mut periods: Vec<FinancialPeriod> = Vec::new();
    let mut period_by_object: HashMap<&str, String> = HashMap::new();
    for e in root.all("FinancialPeriod").chain(p.all("FinancialPeriod")) {
        let (Some(name), Some(start), Some(finish)) =
            (e.text_of("Name"), e.text_of("StartDate").and_then(date_of), e.text_of("EndDate").and_then(date_of))
        else {
            continue;
        };
        if finish >= start && !periods.iter().any(|q| q.name == name || (q.start <= finish && start <= q.finish)) {
            periods.push(FinancialPeriod { name: name.to_string(), start, finish });
            if let Some(o) = e.text_of("ObjectId") {
                period_by_object.insert(o, name.to_string());
            }
        }
    }
    periods.sort_by_key(|q| q.start);
    // Each activity's own amounts in each period.
    let mut records: Vec<PeriodRecord> = Vec::new();
    for e in p.all("ActivityPeriodActual").chain(root.all("ActivityPeriodActual")) {
        let (Some(activity), Some(period)) = (
            e.text_of("ActivityObjectId").and_then(|o| by_object.get(o)),
            e.text_of("FinancialPeriodObjectId").and_then(|o| period_by_object.get(o)),
        ) else {
            continue;
        };
        let cost = |k: &str| e.text_of(k).and_then(money_of).unwrap_or(0);
        let units = |k: &str| {
            e.text_of(k)
                .and_then(|x| x.trim().parse::<f64>().ok())
                .filter(|x| x.is_finite() && x.abs() < 1e12)
                .map_or(0, |x| (x * UNIT as f64).round() as i64)
        };
        records.retain(|r| !(r.activity == *activity && r.period == *period));
        records.push(PeriodRecord {
            period: period.clone(),
            activity: activity.clone(),
            planned: cost("PlannedValueCost"),
            earned: cost("EarnedValueCost"),
            labor_cost: cost("ActualLaborCost"),
            nonlabor_cost: cost("ActualNonLaborCost"),
            material_cost: cost("ActualMaterialCost"),
            expense_cost: cost("ActualExpenseCost"),
            labor_units: units("ActualLaborUnits"),
            nonlabor_units: units("ActualNonLaborUnits"),
        });
    }
    EarnedValue { technique, etc, periods, records }
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
    if !project.external.is_empty() {
        warnings.push(tr!("warn-p6-external", n = project.external.len()));
    }
    let odd_lags = project.relationships.iter().filter(|r| r.lag_kind != mkperth_core::LagKind::Working).count();
    if odd_lags > 0 {
        warnings.push(tr!("warn-lag-kinds", n = odd_lags));
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
            "  <Calendar>\n    <ObjectId>{}</ObjectId>\n    <Name>{}</Name>\n    <Type>Global</Type>\n    <HoursPerDay>{}</HoursPerDay>\n    <StandardWorkWeek>\n",
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
    // Each resource's prices and limits as its rates, a rate for each day either changes, with
    // the price and the most units an hour then; a first price from the start of time takes
    // effect on the project's start.
    let mut rates = 0usize;
    for r in &project.resources {
        let days: BTreeSet<Date> = r.prices.iter().map(|p| p.from.unwrap_or(project.start)).chain(r.limits.iter().map(|l| l.from)).collect();
        for day in days {
            rates += 1;
            out.push_str(&format!(
                "  <ResourceRate>\n    <ObjectId>{rates}</ObjectId>\n    <ResourceObjectId>{}</ResourceObjectId>\n    <EffectiveDate>{}</EffectiveDate>\n",
                resource_object(&r.id).unwrap_or(0),
                timestamp(day, false)
            ));
            if !r.prices.is_empty() {
                out.push_str(&format!("    <PricePerUnit>{}</PricePerUnit>\n", money(r.price_on(day))));
            }
            out.push_str(&format!("    <MaxUnitsPerTime>{}</MaxUnitsPerTime>\n  </ResourceRate>\n", units_text(r.limit_on(day))));
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
    // Funding sources: P6 names them and describes them; their IDs go in the name.
    let source_object = |id: &str| project.budget.sources.iter().position(|f| f.id == id).map(|k| k + 1);
    for (k, f) in project.budget.sources.iter().enumerate() {
        out.push_str(&format!(
            "  <FundingSource>\n    <ObjectId>{}</ObjectId>\n    <Name>{}</Name>\n    <Description>{}</Description>\n",
            k + 1,
            esc(&f.id),
            esc(&f.name)
        ));
        if let Some(parent) = f.parent.as_deref().and_then(source_object) {
            out.push_str(&format!("    <ParentObjectId>{parent}</ParentObjectId>\n"));
        }
        out.push_str("  </FundingSource>\n");
    }
    // The OBS, and the EPS from the top down to the node holding the project.
    let org = &project.organisation;
    let obs_object = |name: &str| org.obs.iter().position(|o| o.name == name).map(|k| k + 1);
    for (k, o) in org.obs.iter().enumerate() {
        out.push_str(&format!(
            "  <OBS>\n    <ObjectId>{}</ObjectId>\n    <Name>{}</Name>\n    <Description>{}</Description>\n",
            k + 1,
            esc(&o.name),
            esc(&o.description)
        ));
        if let Some(parent) = o.parent.as_deref().and_then(obs_object) {
            out.push_str(&format!("    <ParentObjectId>{parent}</ParentObjectId>\n"));
        }
        out.push_str("  </OBS>\n");
    }
    let responsible = org.responsible.as_deref().and_then(obs_object);
    for (k, e) in org.eps.iter().enumerate() {
        out.push_str(&format!("  <EPS>\n    <ObjectId>{}</ObjectId>\n    <Id>{}</Id>\n    <Name>{}</Name>\n", k + 1, esc(&e.id), esc(&e.name)));
        if k > 0 {
            out.push_str(&format!("    <ParentObjectId>{k}</ParentObjectId>\n"));
        }
        if let Some(r) = responsible.or(if org.obs.is_empty() { None } else { Some(1) }) {
            out.push_str(&format!("    <ObsObjectId>{r}</ObsObjectId>\n"));
        }
        out.push_str("  </EPS>\n");
    }
    // The activity codes, the project's own, and their values in their hierarchy; the fields.
    let mut value_object: HashMap<(&str, &str), usize> = HashMap::new();
    for (k, code) in project.classification.codes.iter().enumerate() {
        out.push_str(&format!(
            "  <ActivityCodeType>\n    <ObjectId>{}</ObjectId>\n    <Name>{}</Name>\n    <Scope>Project</Scope>\n    <ProjectObjectId>1</ProjectObjectId>\n    <SequenceNumber>{k}</SequenceNumber>\n  </ActivityCodeType>\n",
            k + 1,
            esc(&code.name)
        ));
    }
    for code in &project.classification.codes {
        for v in &code.values {
            let n = value_object.len() + 1;
            value_object.insert((code.name.as_str(), v.code.as_str()), n);
        }
    }
    for (k, code) in project.classification.codes.iter().enumerate() {
        for (seq, v) in code.values.iter().enumerate() {
            let n = value_object.get(&(code.name.as_str(), v.code.as_str())).copied().unwrap_or(0);
            out.push_str(&format!(
                "  <ActivityCode>\n    <ObjectId>{n}</ObjectId>\n    <CodeTypeObjectId>{}</CodeTypeObjectId>\n    <CodeValue>{}</CodeValue>\n    <Description>{}</Description>\n    <SequenceNumber>{seq}</SequenceNumber>\n",
                k + 1,
                esc(&v.code),
                esc(&v.description)
            ));
            if let Some(parent) = v.parent.as_deref().and_then(|p| value_object.get(&(code.name.as_str(), p))) {
                out.push_str(&format!("    <ParentObjectId>{parent}</ParentObjectId>\n"));
            }
            out.push_str("  </ActivityCode>\n");
        }
    }
    for (k, f) in project.classification.fields.iter().enumerate() {
        out.push_str(&format!(
            "  <UDFType>\n    <ObjectId>{}</ObjectId>\n    <Title>{}</Title>\n    <DataType>{}</DataType>\n    <SubjectArea>Activity</SubjectArea>\n  </UDFType>\n",
            k + 1,
            esc(&f.name),
            field_kind_name(f.kind)
        ));
    }
    for (k, q) in project.earned_value.periods.iter().enumerate() {
        out.push_str(&format!(
            "  <FinancialPeriod>\n    <ObjectId>{}</ObjectId>\n    <Name>{}</Name>\n    <StartDate>{}T00:00:00</StartDate>\n    <EndDate>{}T23:59:59</EndDate>\n  </FinancialPeriod>\n",
            k + 1,
            esc(&q.name),
            q.start,
            q.finish
        ));
    }
    // A resource's roles: the pair is the key, with no object number of its own.
    for r in &project.resources {
        for role in r.roles.iter().filter_map(|id| role_object(id)) {
            out.push_str(&format!(
                "  <ResourceRole>\n    <ResourceObjectId>{}</ResourceObjectId>\n    <RoleObjectId>{role}</RoleObjectId>\n  </ResourceRole>\n",
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
    let ev = &project.earned_value;
    let compute = match ev.technique {
        EvTechnique::Physical | EvTechnique::Duration | EvTechnique::Units => EV_ACTIVITY,
        t => EV_TECHNIQUES.iter().find(|(_, x)| std::mem::discriminant(x) == std::mem::discriminant(&t)).map_or(EV_ACTIVITY, |(n, _)| n),
    };
    out.push_str(&format!("    <EarnedValueComputeType>{compute}</EarnedValueComputeType>\n"));
    if let EvTechnique::Custom(share) = ev.technique {
        out.push_str(&format!("    <EarnedValueUserPercent>{}</EarnedValueUserPercent>\n", share as f64 / 100.0));
    }
    let etc = ETC_TECHNIQUES.iter().find(|(_, t)| *t == ev.etc).map_or("Performance Factor = 1", |(n, _)| n);
    out.push_str(&format!("    <EarnedValueETCComputeType>{etc}</EarnedValueETCComputeType>\n"));
    if let Some(b) = project.budget.of(None) {
        out.push_str(&format!("    <OriginalBudget>{}</OriginalBudget>\n", money(b.original)));
    }
    if !org.eps.is_empty() {
        out.push_str(&format!("    <ParentEPSObjectId>{}</ParentEPSObjectId>\n", org.eps.len()));
    }
    if let Some(r) = responsible {
        out.push_str(&format!("    <OBSObjectId>{r}</OBSObjectId>\n"));
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
        if let Some(b) = project.budget.of(Some(a.id.as_str())) {
            out.push_str(&format!("      <OriginalBudget>{}</OriginalBudget>\n", money(b.original)));
        }
        if let Some(r) = a.responsible.as_deref().and_then(obs_object) {
            out.push_str(&format!("      <OBSObjectId>{r}</OBSObjectId>\n"));
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
        out.push_str(&format!("      <PercentCompleteType>{}</PercentCompleteType>\n", percent_kind(project.earned_value.technique)));
        if let Some(x) = a.percent_complete {
            let percent = format!("{:.2}", x as f64 / 100.0);
            out.push_str(&format!(
                "      <PhysicalPercentComplete>{}</PhysicalPercentComplete>\n",
                percent.trim_end_matches('0').trim_end_matches('.')
            ));
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
    // The budget: its log of changes, the spending plans month by month, the funds. P6 keeps a
    // budget on the project and its WBS only: one on another activity is left out.
    let budgeted = |wbs: Option<&str>| wbs.is_none_or(summary);
    let left_out = project.budget.budgets.iter().filter(|b| !budgeted(b.wbs.as_deref())).count()
        + project.budget.log.iter().filter(|c| !budgeted(c.wbs.as_deref())).count();
    if left_out > 0 {
        warnings.push(tr!("warn-p6-budget-not-wbs", n = left_out));
    }
    for (k, c) in project.budget.log.iter().enumerate().filter(|(_, c)| budgeted(c.wbs.as_deref())) {
        let status = match c.status {
            ChangeStatus::Pending => "Pending",
            ChangeStatus::Approved => "Approved",
            ChangeStatus::NotApproved => "Not Approved",
        };
        out.push_str(&format!("    <ProjectBudgetChangeLog>\n      <ObjectId>{}</ObjectId>\n      <ProjectObjectId>1</ProjectObjectId>\n", k + 1));
        if let Some(w) = &c.wbs {
            out.push_str(&format!("      <WBSObjectId>{}</WBSObjectId>\n", object(w)));
        }
        out.push_str(&format!(
            "      <ChangeNumber>{}</ChangeNumber>\n      <Date>{}T00:00:00</Date>\n      <Amount>{}</Amount>\n      <Status>{status}</Status>\n      <Responsible>{}</Responsible>\n      <Reason>{}</Reason>\n    </ProjectBudgetChangeLog>\n",
            c.number,
            c.date,
            money(c.amount),
            esc(&c.responsible),
            esc(&c.reason)
        ));
    }
    let mut plans = 0usize;
    for b in project.budget.budgets.iter().filter(|b| budgeted(b.wbs.as_deref())) {
        for m in &b.plan {
            plans += 1;
            match &b.wbs {
                None => out.push_str(&format!(
                    "    <ProjectSpendingPlan>\n      <ObjectId>{plans}</ObjectId>\n      <ProjectObjectId>1</ProjectObjectId>\n      <Date>{}T00:00:00</Date>\n      <SpendingPlan>{}</SpendingPlan>\n    </ProjectSpendingPlan>\n",
                    m.month,
                    money(m.amount)
                )),
                // A WBS's plan is the project's kind, naming the WBS.
                Some(w) => out.push_str(&format!(
                    "    <ProjectSpendingPlan>\n      <ObjectId>{plans}</ObjectId>\n      <ProjectObjectId>1</ProjectObjectId>\n      <WBSObjectId>{}</WBSObjectId>\n      <Date>{}T00:00:00</Date>\n      <SpendingPlan>{}</SpendingPlan>\n    </ProjectSpendingPlan>\n",
                    object(w),
                    m.month,
                    money(m.amount)
                )),
            }
        }
    }
    // Each fund's share of the project's current budget, in percent, as P6 shows it.
    let current = project.budget.of(None).map_or(0, |b| b.original).saturating_add(
        project.budget.log.iter().filter(|c| c.wbs.is_none() && c.status == ChangeStatus::Approved).fold(0i64, |s, c| s.saturating_add(c.amount)),
    );
    for (k, f) in project.budget.funds.iter().enumerate() {
        let Some(source) = source_object(&f.source) else { continue };
        out.push_str(&format!(
            "    <ProjectFunding>\n      <ObjectId>{}</ObjectId>\n      <ProjectObjectId>1</ProjectObjectId>\n      <FundingSourceObjectId>{source}</FundingSourceObjectId>\n      <Amount>{}</Amount>\n",
            k + 1,
            money(f.amount)
        ));
        if current > 0 {
            let share = f.amount as f64 / current as f64 * 100.0;
            out.push_str(&format!("      <FundShare>{}</FundShare>\n", format!("{share:.2}").trim_end_matches('0').trim_end_matches('.')));
        }
        out.push_str("    </ProjectFunding>\n");
    }
    // Each activity's own amounts in each stored period.
    for (k, r) in project.earned_value.records.iter().enumerate() {
        let (Some(_), Some(period)) = (
            project.activities.iter().position(|a| a.id == r.activity).filter(|_| !summary(&r.activity)),
            project.earned_value.periods.iter().position(|q| q.name == r.period),
        ) else {
            continue;
        };
        out.push_str(&format!(
            "    <ActivityPeriodActual>\n      <ObjectId>{}</ObjectId>\n      <ActivityObjectId>{}</ActivityObjectId>\n      <FinancialPeriodObjectId>{}</FinancialPeriodObjectId>\n      <ProjectObjectId>1</ProjectObjectId>\n      <ActualLaborCost>{}</ActualLaborCost>\n      <ActualNonLaborCost>{}</ActualNonLaborCost>\n      <ActualMaterialCost>{}</ActualMaterialCost>\n      <ActualExpenseCost>{}</ActualExpenseCost>\n      <ActualLaborUnits>{}</ActualLaborUnits>\n      <ActualNonLaborUnits>{}</ActualNonLaborUnits>\n      <EarnedValueCost>{}</EarnedValueCost>\n      <PlannedValueCost>{}</PlannedValueCost>\n    </ActivityPeriodActual>\n",
            k + 1,
            object(&r.activity),
            period + 1,
            money(r.labor_cost),
            money(r.nonlabor_cost),
            money(r.material_cost),
            money(r.expense_cost),
            units_text(r.labor_units),
            units_text(r.nonlabor_units),
            money(r.earned),
            money(r.planned)
        ));
    }
    // Each activity's code values and field values; a summary is a WBS element and takes none.
    for a in project.activities.iter().filter(|a| !summary(&a.id)) {
        for (code, value) in &a.codes {
            let (Some(t), Some(v)) =
                (project.classification.codes.iter().position(|c| c.name == *code), value_object.get(&(code.as_str(), value.as_str())))
            else {
                continue;
            };
            out.push_str(&format!(
                "    <ActivityCodeAssignment>\n      <ActivityObjectId>{}</ActivityObjectId>\n      <ActivityCodeTypeObjectId>{}</ActivityCodeTypeObjectId>\n      <ActivityCodeObjectId>{v}</ActivityCodeObjectId>\n      <ProjectObjectId>1</ProjectObjectId>\n    </ActivityCodeAssignment>\n",
                object(&a.id),
                t + 1
            ));
        }
        for (field, value) in &a.fields {
            let Some(t) = project.classification.fields.iter().position(|f| f.name == *field) else { continue };
            let (tag, text) = match value {
                FieldValue::Text(x) => ("Text", esc(x)),
                FieldValue::Number(x) => ("Double", format!("{}", *x as f64 / 1e6)),
                FieldValue::Integer(x) => ("Integer", x.to_string()),
                FieldValue::Cost(x) => ("Cost", money(*x)),
                FieldValue::Date(d) => ("StartDate", format!("{d}T00:00:00")),
                FieldValue::Indicator(i) => ("Indicator", {
                    let c = i.code();
                    format!("{}{}", c.get(..1).unwrap_or_default().to_uppercase(), c.get(1..).unwrap_or_default())
                }),
            };
            out.push_str(&format!(
                "    <UDFValue>\n      <UDFTypeObjectId>{}</UDFTypeObjectId>\n      <ForeignObjectId>{}</ForeignObjectId>\n      <ProjectObjectId>1</ProjectObjectId>\n      <{tag}>{text}</{tag}>\n    </UDFValue>\n",
                t + 1,
                object(&a.id)
            ));
        }
    }
    let answered = project.activities.iter().filter(|a| !summary(&a.id) && a.responsible.is_some()).count();
    if answered > 0 {
        warnings.push(tr!("warn-p6-responsible", n = answered));
    }
    let on_summaries = project.activities.iter().filter(|a| summary(&a.id)).map(|a| a.codes.len() + a.fields.len()).sum::<usize>();
    if on_summaries > 0 {
        warnings.push(tr!("warn-p6-summary-codes", n = on_summaries));
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
