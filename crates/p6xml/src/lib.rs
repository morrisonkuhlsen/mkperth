//! Primavera P6's XML interchange format: a project's WBS, activities, relationships and
//! calendars, read into MK Perth's model and written from it.
//!
//! What maps: the first project's name, planned start and must-finish-by date; its WBS as
//! summaries; each activity's ID, name, type (start and finish milestones; every other type as a
//! task), planned duration, calendar, WBS and primary and secondary constraints; relationships of
//! the four types with their lags; calendars' work weeks, holidays and worked exceptions. P6 keeps
//! durations and lags in hours: they become working days by the calendar's hours per day, rounded
//! up, with a warning when that isn't whole. Element names follow Oracle's published P6 XML schema
//! (`APIBusinessObjects`); the format's other elements are left out, each kind named once in the
//! warnings.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod tree;

use std::collections::{BTreeSet, HashMap};

use mkperth_core::{Activity, ActivityKind, Calendar, Constraint, Date, Project, Relationship, RelationshipType, Weekday};
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

/// A calendar and its hours per day.
fn calendar_of(e: &Element) -> (Calendar, f64) {
    let name = e.text_of("Name").unwrap_or("Calendar").to_string();
    let hours = e.text_of("HoursPerDay").and_then(|h| h.parse::<f64>().ok()).filter(|h| *h > 0.0 && *h <= 24.0).unwrap_or(HOURS_PER_DAY);
    let mut week = [false; 7];
    let mut any_day = false;
    if let Some(standard) = e.child("StandardWorkWeek") {
        for day in standard.all("StandardWorkHours") {
            any_day = true;
            if let Some(w) = day.text_of("DayOfWeek").and_then(weekday_of)
                && let Some(slot) = week.get_mut(w.index())
            {
                *slot = day.all("WorkTime").next().is_some();
            }
        }
    }
    if !any_day {
        week = [true, true, true, true, true, false, false];
    }
    let (mut holidays, mut extra_days) = (BTreeSet::new(), BTreeSet::new());
    if let Some(exceptions) = e.child("HolidayOrExceptions") {
        for x in exceptions.all("HolidayOrException") {
            let Some(d) = x.text_of("Date").and_then(date_of) else { continue };
            if x.all("WorkTime").next().is_some() {
                extra_days.insert(d);
            } else {
                holidays.insert(d);
            }
        }
    }
    (Calendar { name, week, holidays, extra_days, region: None }, hours)
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
    let to_days = |hours: f64, per_day: f64, warned: &mut bool| -> i64 {
        let days = hours / per_day;
        if (days - days.round()).abs() > 1e-9 {
            *warned = true;
        }
        days.ceil().clamp(-1e9, 1e9) as i64
    };
    let mut fractional = false;

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
            duration: to_days(hours, per_day, &mut fractional),
            constraint: constraint("PrimaryConstraintType", "PrimaryConstraintDate"),
            secondary_constraint: constraint("SecondaryConstraintType", "SecondaryConstraintDate"),
            calendar,
            parent: e.text_of("WBSObjectId").and_then(|w| wbs_id.get(w)).cloned(),
            ..Activity::default()
        };
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
        let lag = match e.text_of("Lag") {
            Some(h) => to_days(
                h.parse::<f64>().ok().filter(|h| h.is_finite()).ok_or_else(|| P6Error::Field("Relationship".into(), "Lag".into()))?,
                per_day,
                &mut fractional,
            ),
            None => 0,
        };
        relationships.push(Relationship { predecessor, successor, kind, lag });
    }
    if dangling > 0 {
        warnings.push(tr!("warn-p6-dangling", n = dangling));
    }
    if fractional {
        warnings.push(tr!("warn-p6-hours"));
    }
    // What else the project and the file hold, each kind named once.
    const READ: [&str; 6] = ["Project", "Calendar", "WBS", "Activity", "Relationship", "ProjectCodeType"];
    const PROJECT_FIELDS_READ: [&str; 11] = [
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
    Ok(Read {
        project: Project {
            name: p.text_of("Name").or_else(|| p.text_of("Id")).unwrap_or_default().to_string(),
            start,
            calendar: project_calendar.clone(),
            calendars: calendars.into_iter().map(|(_, c, _)| c).filter(|c| c.name != project_calendar.name).collect(),
            lag_calendar: Default::default(),
            float_kind: Default::default(),
            critical_by: Default::default(),
            must_finish_by: p.text_of("MustFinishByDate").and_then(date_of),
            activities,
            relationships,
            extra,
        },
        warnings,
    })
}

/// Text for XML: `&`, `<`, `>`, quotes escaped.
fn esc(text: &str) -> String {
    quick_xml::escape::escape(text).into_owned()
}

fn timestamp(d: Date, evening: bool) -> String {
    format!("{d}T{}", if evening { "17:00:00" } else { "08:00:00" })
}

/// Writes `project` as P6 XML (one project, its calendars, WBS, activities and relationships),
/// durations and lags in hours at eight a day. Returns the text and what P6's format couldn't
/// hold.
pub fn write(project: &Project) -> (String, Vec<String>) {
    let warnings = Vec::new();
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!("<APIBusinessObjects xmlns=\"{NAMESPACE}\">\n"));
    let calendars: Vec<&Calendar> = std::iter::once(&project.calendar).chain(project.calendars.iter()).collect();
    let calendar_object = |name: &str| calendars.iter().position(|c| c.name == name).map_or(1, |k| k + 1);
    for (k, c) in calendars.iter().enumerate() {
        out.push_str(&format!(
            "  <Calendar>\n    <ObjectId>{}</ObjectId>\n    <Name>{}</Name>\n    <HoursPerDay>8</HoursPerDay>\n    <StandardWorkWeek>\n",
            k + 1,
            esc(&c.name)
        ));
        for w in Weekday::ALL {
            out.push_str(&format!("      <StandardWorkHours>\n        <DayOfWeek>{w:?}</DayOfWeek>\n"));
            if c.week.get(w.index()).copied().unwrap_or(false) {
                out.push_str("        <WorkTime>\n          <Start>08:00:00</Start>\n          <Finish>17:00:00</Finish>\n        </WorkTime>\n");
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
        for d in &holidays {
            out.push_str(&format!("      <HolidayOrException>\n        <Date>{d}T00:00:00</Date>\n      </HolidayOrException>\n"));
        }
        for d in &c.extra_days {
            out.push_str(&format!(
                "      <HolidayOrException>\n        <Date>{d}T00:00:00</Date>\n        <WorkTime>\n          <Start>08:00:00</Start>\n          <Finish>17:00:00</Finish>\n        </WorkTime>\n      </HolidayOrException>\n"
            ));
        }
        out.push_str("    </HolidayOrExceptions>\n  </Calendar>\n");
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
            ActivityKind::Task => "Task Dependent",
            ActivityKind::StartMilestone => "Start Milestone",
            ActivityKind::FinishMilestone => "Finish Milestone",
        };
        out.push_str(&format!(
            "    <Activity>\n      <ObjectId>{}</ObjectId>\n      <Id>{}</Id>\n      <Name>{}</Name>\n      <Type>{kind}</Type>\n      <PlannedDuration>{}</PlannedDuration>\n      <CalendarObjectId>{}</CalendarObjectId>\n      <ProjectObjectId>1</ProjectObjectId>\n",
            object(&a.id),
            esc(&a.id),
            esc(&a.name),
            a.working_days().saturating_mul(8),
            a.calendar.as_deref().map_or(1, calendar_object)
        ));
        if let Some(p) = &a.parent {
            out.push_str(&format!("      <WBSObjectId>{}</WBSObjectId>\n", object(p)));
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
            r.lag.saturating_mul(8)
        ));
    }
    out.push_str("  </Project>\n</APIBusinessObjects>\n");
    (out, warnings)
}

#[cfg(test)]
mod tests;
