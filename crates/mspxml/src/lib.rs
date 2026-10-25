//! Microsoft Project's XML interchange format (MSPDI), read into MK Perth's model and written
//! from it, from the schema Microsoft publishes (the Project XML Data Interchange Schema
//! reference on learn.microsoft.com): tasks with their outline levels as the WBS, links with
//! their lags in tenths of a minute and the format they are shown in, constraints, calendars
//! with their weekdays, work periods and exceptions, resources and assignments. What has no
//! place in one model or the other is said in a warning.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(test)]
mod tests;
mod tree;

use std::collections::{BTreeSet, HashMap};

use mkperth_core::{
    Accrual, Activity, ActivityKind, Assignment, Calendar, Constraint, Date, DurationType, Expense, LagKind, Period, Project, Relationship,
    RelationshipType, Resource, ResourceKind, UNIT,
};
use mkperth_i18n::tr;
use thiserror::Error;

use tree::Element;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MspError {
    #[error("{}", tr!("err-msp-xml", detail = .0.as_str()))]
    Xml(String),
    #[error("{}", tr!("err-msp-not-msp"))]
    NotMsp,
    #[error("{}", tr!("err-msp-too-big"))]
    TooBig,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Read {
    pub project: Project,
    pub warnings: Vec<String>,
}

/// The schema's namespace.
const NAMESPACE: &str = "http://schemas.microsoft.com/project";
/// The most days one calendar exception may cover, and tasks a file may hold.
const MAX_EXCEPTION_DAYS: i32 = 3660;
const MAX_TASKS: usize = 100_000;
/// The name given to the calendar of tasks whose duration is elapsed time.
pub const ELAPSED: &str = "Elapsed (24 hours)";

/// The format codes for durations and lags: minutes, hours, days, weeks, months; their elapsed
/// forms; percent. The estimated forms (32 more) are the same units.
fn format_unit(code: i64) -> Option<(Unit, bool)> {
    let code = if (35..=52).contains(&code) { code - 32 } else { code };
    Some(match code {
        3 => (Unit::Minute, false),
        4 => (Unit::Minute, true),
        5 => (Unit::Hour, false),
        6 => (Unit::Hour, true),
        7 => (Unit::Day, false),
        8 => (Unit::Day, true),
        9 => (Unit::Week, false),
        10 => (Unit::Week, true),
        11 => (Unit::Month, false),
        12 => (Unit::Month, true),
        19 => (Unit::Percent, false),
        20 => (Unit::Percent, true),
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Percent,
}

/// A schema duration (`PT8H30M0S`, `P2DT4H0M0S`) as whole minutes; days in it are 24 hours.
fn duration_of(text: &str) -> Option<i64> {
    let t = text.trim();
    let rest = t.strip_prefix('P')?;
    let (days, time) = match rest.split_once('T') {
        Some((d, t)) => (d, t),
        None => (rest, ""),
    };
    let number = |s: &str, unit: char| -> Option<f64> {
        if s.is_empty() {
            return Some(0.0);
        }
        s.strip_suffix(unit)?.parse::<f64>().ok().filter(|x| x.is_finite() && *x >= 0.0)
    };
    let d = number(days, 'D')?;
    let (mut h, mut m, mut s) = (0.0, 0.0, 0.0);
    let mut part = String::new();
    for c in time.chars() {
        part.push(c);
        match c {
            'H' => h = number(&std::mem::take(&mut part), 'H')?,
            'M' => m = number(&std::mem::take(&mut part), 'M')?,
            'S' => s = number(&std::mem::take(&mut part), 'S')?,
            _ => {}
        }
    }
    if !part.is_empty() {
        return None;
    }
    let minutes = d * 1440.0 + h * 60.0 + m + s / 60.0;
    (minutes <= 1e9).then(|| minutes.round() as i64)
}

/// Minutes as a schema duration: `PT8H30M0S`.
fn duration(minutes: i64) -> String {
    format!("PT{}H{}M0S", minutes / 60, minutes % 60)
}

fn date_of(text: &str) -> Option<Date> {
    Date::parse(text.get(..10).unwrap_or(text))
}

/// `08:00:00` as minutes after midnight; `24:00:00` and `00:00:00` as an end are the day's end.
fn time_of(text: &str) -> Option<u16> {
    let mut parts = text.trim().split(':');
    let h: u16 = parts.next()?.parse().ok()?;
    let m: u16 = parts.next().unwrap_or("0").parse().ok()?;
    (h <= 24 && m < 60).then_some(h * 60 + m).filter(|t| *t <= 1440)
}

fn time(minutes: u16) -> String {
    format!("{:02}:{:02}:00", minutes / 60, minutes % 60)
}

/// The work periods in a `WorkingTimes` element, in order, each starting before it ends.
fn periods_of(e: &Element) -> Vec<Period> {
    let mut out: Vec<Period> = e
        .child("WorkingTimes")
        .map(|w| {
            w.all("WorkingTime")
                .filter_map(|t| {
                    let from = time_of(t.text_of("FromTime")?)?;
                    let to = time_of(t.text_of("ToTime")?)?;
                    // Midnight as an end is the day's end.
                    let to = if to == 0 { 1440 } else { to };
                    (from < to).then_some((from, to))
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_unstable();
    out.dedup();
    out
}

fn working_times(periods: &[Period], indent: &str) -> String {
    let mut out = format!("{indent}<WorkingTimes>\n");
    for (a, b) in periods {
        out.push_str(&format!(
            "{indent}  <WorkingTime>\n{indent}    <FromTime>{}</FromTime>\n{indent}    <ToTime>{}</ToTime>\n{indent}  </WorkingTime>\n",
            time(*a),
            time(if *b == 1440 { 0 } else { *b })
        ));
    }
    out.push_str(&format!("{indent}</WorkingTimes>\n"));
    out
}

/// The days of an exception's (or an old-style exception weekday's) `TimePeriod`, capped.
fn days_of(e: &Element) -> Vec<Date> {
    let Some(p) = e.child("TimePeriod") else { return Vec::new() };
    let (Some(from), Some(to)) = (p.text_of("FromDate").and_then(date_of), p.text_of("ToDate").and_then(date_of)) else { return Vec::new() };
    (0..=(to.days() - from.days()).clamp(0, MAX_EXCEPTION_DAYS)).filter_map(|k| Date::from_days(from.days() + k)).collect()
}

/// The `n`th (from 0; 4 is the last) day of month `m` of year `y` that `item` matches: any day
/// (0), a weekday (1), a weekend day (2), or Sunday (3) to Saturday (9).
fn nth_in_month(y: i32, m: u32, item: i64, position: i64) -> Option<Date> {
    let first = Date::from_ymd(y, m, 1)?;
    let next = if m == 12 { Date::from_ymd(y + 1, 1, 1)? } else { Date::from_ymd(y, m + 1, 1)? };
    // Sunday 0 to Saturday 6.
    let sunday_first = |d: Date| (d.weekday().index() + 1) % 7;
    let fits = |d: Date| match item {
        0 => true,
        1 => (1..=5).contains(&sunday_first(d)),
        2 => matches!(sunday_first(d), 0 | 6),
        3..=9 => sunday_first(d) as i64 == item - 3,
        _ => false,
    };
    let days: Vec<Date> = (first.days()..next.days()).filter_map(Date::from_days).filter(|d| fits(*d)).collect();
    match position {
        4 => days.last().copied(),
        0..=3 => days.get(position as usize).copied(),
        _ => None,
    }
}

/// The days a calendar exception covers, by its recurrence: daily every `Period` days (1),
/// yearly on a day of a month (2) or at a position in it (3), monthly on a day (4) or at a
/// position (5) every `Period` months, weekly on `DaysOfWeek` every `Period` weeks (6); from its
/// `TimePeriod`'s first day, to its last or for its `Occurrences`. `None` for a kind not read.
fn recurrence_days(x: &Element) -> Option<Vec<Date>> {
    let int = |name: &str| x.text_of(name).and_then(|t| t.parse::<i64>().ok());
    let p = x.child("TimePeriod")?;
    let from = p.text_of("FromDate").and_then(date_of)?;
    let to = p.text_of("ToDate").and_then(date_of).unwrap_or(from);
    let by_count = x.text_of("EnteredByOccurrences") == Some("1");
    let limit = if by_count { int("Occurrences").unwrap_or(1).clamp(0, 5_000) as usize } else { 5_000 };
    // Counted occurrences may run past the period's end: a century at most.
    let last = if by_count { Date::from_days(from.days() + 36_600).unwrap_or(to) } else { to };
    let period = int("Period").unwrap_or(1).clamp(1, 1_000);
    let (month, day, item, position) =
        (int("Month").unwrap_or(0), int("MonthDay").unwrap_or(1), int("MonthItem").unwrap_or(0), int("MonthPosition").unwrap_or(0));
    let mut out: Vec<Date> = Vec::new();
    let mut keep = |d: Date| {
        if d >= from && d <= last && out.len() < limit && !out.contains(&d) {
            out.push(d);
        }
    };
    let (y0, m0, _) = from.ymd();
    let (y1, _, _) = last.ymd();
    match int("Type").unwrap_or(1) {
        1 => {
            let mut k = from.days();
            while k <= last.days() && (k - from.days()) / (period as i32) < 5_000 {
                if let Some(d) = Date::from_days(k) {
                    keep(d);
                }
                k += period as i32;
            }
        }
        2 | 3 => {
            let m = u32::try_from(month + 1).ok().filter(|m| (1..=12).contains(m))?;
            for y in y0..=y1.min(y0 + 200) {
                let d = if int("Type") == Some(2) {
                    u32::try_from(day).ok().and_then(|d| Date::from_ymd(y, m, d))
                } else {
                    nth_in_month(y, m, item, position)
                };
                if let Some(d) = d {
                    keep(d);
                }
            }
        }
        4 | 5 => {
            let (mut y, mut m) = (y0, m0);
            let mut steps = 0;
            while (y, m) <= (y1, 12) && steps < 2_400 {
                let d = if int("Type") == Some(4) {
                    u32::try_from(day).ok().and_then(|d| Date::from_ymd(y, m, d))
                } else {
                    nth_in_month(y, m, item, position)
                };
                if let Some(d) = d {
                    keep(d);
                }
                for _ in 0..period {
                    (y, m) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
                }
                steps += 1;
            }
        }
        6 => {
            let mask = int("DaysOfWeek").unwrap_or(0);
            // Weeks counted from the Sunday on or before the first day.
            let week0 = from.days() - ((from.weekday().index() as i32 + 1) % 7);
            for k in from.days()..=last.days().min(from.days() + 36_600) {
                let Some(d) = Date::from_days(k) else { continue };
                let bit = 1i64 << ((d.weekday().index() + 1) % 7);
                if mask & bit != 0 && ((k - week0) / 7) % (period as i32) == 0 {
                    keep(d);
                }
            }
        }
        _ => return None,
    }
    Some(out)
}

/// A calendar from its element; the counts of recurring exceptions left out are added to `skipped`.
fn calendar_of(e: &Element, base: Option<&Calendar>, skipped: &mut usize) -> Calendar {
    let mut c = base.cloned().unwrap_or_default();
    c.name = e.text_of("Name").unwrap_or("Calendar").to_string();
    // Weekdays: DayType 1 is Sunday; MK Perth's week starts on Monday.
    let mut exceptions: Vec<(Vec<Date>, bool, Vec<Period>)> = Vec::new();
    for w in e.child("WeekDays").map(|w| w.all("WeekDay").collect::<Vec<_>>()).unwrap_or_default() {
        let Some(day) = w.text_of("DayType").and_then(|d| d.parse::<usize>().ok()) else { continue };
        let working = w.text_of("DayWorking") == Some("1");
        if day == 0 {
            // An exception as the old schema wrote them.
            exceptions.push((days_of(w), working, periods_of(w)));
            continue;
        }
        let Some(k) = (1..=7).contains(&day).then(|| (day + 5) % 7) else { continue };
        if let Some(slot) = c.week.get_mut(k) {
            *slot = working;
        }
        if let Some(slot) = c.periods.get_mut(k) {
            *slot = if working { periods_of(w) } else { Vec::new() };
        }
    }
    for x in e.child("Exceptions").map(|x| x.all("Exception").collect::<Vec<_>>()).unwrap_or_default() {
        match recurrence_days(x) {
            Some(days) => exceptions.push((days, x.text_of("DayWorking") == Some("1"), periods_of(x))),
            None => *skipped += 1,
        }
    }
    for (days, working, periods) in exceptions {
        for d in days {
            let week = c.week.get(d.weekday().index()).copied().unwrap_or(false);
            match (working, periods.is_empty()) {
                (false, _) if week => {
                    c.holidays.insert(d);
                }
                (false, _) => {}
                (true, true) if !week => {
                    c.extra_days.insert(d);
                }
                (true, true) => {}
                (true, false) => {
                    c.exceptions.insert(d, periods.clone());
                }
            }
        }
    }
    c
}

fn constraint_of(kind: i64, date: Option<Date>) -> Option<Constraint> {
    Some(match (kind, date) {
        (1, _) => Constraint::AsLateAsPossible,
        (2, Some(d)) => Constraint::StartOn(d),
        (3, Some(d)) => Constraint::FinishOn(d),
        (4, Some(d)) => Constraint::StartOnOrAfter(d),
        (5, Some(d)) => Constraint::StartOnOrBefore(d),
        (6, Some(d)) => Constraint::FinishOnOrAfter(d),
        (7, Some(d)) => Constraint::FinishOnOrBefore(d),
        _ => return None,
    })
}

/// A constraint's code and date (a start in the morning, a finish in the evening).
fn constraint_code(c: Constraint) -> (u8, Option<String>) {
    let morning = |d: Date| Some(format!("{d}T08:00:00"));
    let evening = |d: Date| Some(format!("{d}T17:00:00"));
    match c {
        Constraint::AsLateAsPossible => (1, None),
        Constraint::StartOn(d) | Constraint::MandatoryStart(d) => (2, morning(d)),
        Constraint::FinishOn(d) | Constraint::MandatoryFinish(d) => (3, evening(d)),
        Constraint::StartOnOrAfter(d) => (4, morning(d)),
        Constraint::StartOnOrBefore(d) => (5, morning(d)),
        Constraint::FinishOnOrAfter(d) => (6, evening(d)),
        Constraint::FinishOnOrBefore(d) => (7, evening(d)),
    }
}

fn link_of(code: i64) -> RelationshipType {
    match code {
        0 => RelationshipType::FinishToFinish,
        2 => RelationshipType::StartToFinish,
        3 => RelationshipType::StartToStart,
        _ => RelationshipType::FinishToStart,
    }
}

fn link_code(kind: RelationshipType) -> u8 {
    match kind {
        RelationshipType::FinishToFinish => 0,
        RelationshipType::FinishToStart => 1,
        RelationshipType::StartToFinish => 2,
        RelationshipType::StartToStart => 3,
    }
}

/// Reads an MSPDI file.
pub fn read(text: &str) -> Result<Read, MspError> {
    let root = tree::parse(text)?;
    if root.name != "Project" {
        return Err(MspError::NotMsp);
    }
    let mut warnings = Vec::new();
    let number = |e: &Element, name: &str| e.text_of(name).and_then(|x| x.parse::<f64>().ok()).filter(|x| x.is_finite());
    let int = |e: &Element, name: &str| e.text_of(name).and_then(|x| x.parse::<i64>().ok());
    let per_day = number(&root, "MinutesPerDay").filter(|m| *m >= 1.0 && *m <= 1440.0).map_or(480, |m| m.round() as i64);

    // Calendars by their UID: the base ones first, then those built on them.
    let mut skipped = 0usize;
    let elements: Vec<&Element> = root.child("Calendars").map(|c| c.all("Calendar").collect()).unwrap_or_default();
    let mut calendars: Vec<(String, Calendar)> = Vec::new();
    for pass in [true, false] {
        for e in &elements {
            let based = e.text_of("BaseCalendarUID").filter(|u| *u != "-1");
            if based.is_none() != pass {
                continue;
            }
            let base = based.and_then(|u| calendars.iter().find(|(id, _)| id == u)).map(|(_, c)| c.clone());
            let mut c = calendar_of(e, base.as_ref(), &mut skipped);
            while calendars.iter().any(|(_, x)| x.name == c.name) {
                c.name.push('*');
            }
            calendars.push((e.text_of("UID").unwrap_or_default().to_string(), c));
        }
    }
    if skipped > 0 {
        warnings.push(tr!("warn-msp-recurring", n = skipped));
    }
    let project_cal = root.text_of("CalendarUID");
    let main = project_cal.and_then(|u| calendars.iter().position(|(id, _)| id == u)).or(if calendars.is_empty() { None } else { Some(0) });
    let calendar = main.and_then(|k| calendars.get(k)).map(|(_, c)| c.clone()).unwrap_or_default();
    let name_of = |uid: &str| calendars.iter().find(|(id, _)| id == uid).map(|(_, c)| c.name.clone());

    // Tasks in outline order: each under the nearest task above it a level up.
    let mut activities: Vec<Activity> = Vec::new();
    let mut by_uid: HashMap<String, String> = HashMap::new();
    let mut stack: Vec<(i64, String)> = Vec::new();
    let mut elapsed = false;
    let mut links: Vec<(String, &Element)> = Vec::new();
    let tasks: Vec<&Element> = root.child("Tasks").map(|t| t.all("Task").take(MAX_TASKS).collect()).unwrap_or_default();
    for t in &tasks {
        let Some(uid) = t.text_of("UID") else { continue };
        let level = int(t, "OutlineLevel").unwrap_or(1);
        // The project's own summary task (UID 0, level 0) is the project itself.
        if uid == "0" || level < 1 {
            continue;
        }
        // Its WBS code as its ID (MK Perth writes its IDs there), else its row number.
        let mut id = t.text_of("WBS").or_else(|| t.text_of("ID")).map_or_else(|| uid.to_string(), str::to_string);
        while activities.iter().any(|a| a.id == id) {
            id.push('*');
        }
        while stack.last().is_some_and(|(l, _)| *l >= level) {
            stack.pop();
        }
        let parent = stack.last().map(|(_, p)| p.clone());
        stack.push((level, id.clone()));
        let format = int(t, "DurationFormat").and_then(format_unit);
        let minutes = t.text_of("Duration").and_then(duration_of).unwrap_or(0);
        let is_elapsed = format.is_some_and(|(_, e)| e);
        elapsed |= is_elapsed;
        let kind = if t.text_of("Milestone") == Some("1") { ActivityKind::StartMilestone } else { ActivityKind::Task };
        let day = if is_elapsed { 1440 } else { per_day };
        let (days, part) = (minutes / day, minutes % day != 0);
        let mut a = Activity {
            id: id.clone(),
            name: t.text_of("Name").unwrap_or_default().to_string(),
            kind,
            duration: days + i64::from(part),
            duration_minutes: part.then_some(minutes),
            parent,
            notes: t.text_of("Notes").unwrap_or_default().to_string(),
            calendar: if is_elapsed { Some(ELAPSED.into()) } else { t.text_of("CalendarUID").filter(|u| *u != "-1").and_then(name_of) },
            constraint: int(t, "ConstraintType").and_then(|k| constraint_of(k, t.text_of("ConstraintDate").and_then(date_of))),
            secondary_constraint: t.text_of("Deadline").and_then(date_of).map(Constraint::FinishOnOrBefore),
            actual_start: t.text_of("ActualStart").and_then(date_of),
            actual_finish: t.text_of("ActualFinish").and_then(date_of),
            percent_complete: number(t, "PhysicalPercentComplete").filter(|p| *p > 0.0 && *p <= 100.0).map(|p| (p * 100.0).round() as i64),
            duration_type: match int(t, "Type") {
                Some(0) => DurationType::FixedRate,
                Some(2) => DurationType::FixedUnits,
                _ => DurationType::FixedDurationAndRate,
            },
            ..Activity::default()
        };
        if a.calendar.as_deref() == Some(calendar.name.as_str()) {
            a.calendar = None;
        }
        if a.actual_start.is_some() && a.actual_finish.is_none() {
            a.remaining_minutes = t.text_of("RemainingDuration").and_then(duration_of);
        }
        a.extra.insert("msp.uid".into(), uid.to_string());
        by_uid.insert(uid.to_string(), id.clone());
        for l in t.all("PredecessorLink") {
            links.push((id.clone(), l));
        }
        activities.push(a);
    }
    // Links: the lag in tenths of a minute of working time, shown in the format given.
    let (mut dangling, mut across) = (0usize, 0usize);
    let mut relationships = Vec::new();
    for (successor, l) in links {
        if l.text_of("CrossProject") == Some("1") {
            across += 1;
            continue;
        }
        let Some(predecessor) = l.text_of("PredecessorUID").and_then(|u| by_uid.get(u)).cloned() else {
            dangling += 1;
            continue;
        };
        let tenths = int(l, "LinkLag").unwrap_or(0).clamp(-100_000_000_000, 100_000_000_000);
        let format = int(l, "LagFormat").and_then(format_unit);
        let minutes = tenths / 10;
        let whole = minutes % per_day == 0;
        let (lag, lag_minutes, lag_kind) = match format {
            // A share of the predecessor's duration, in tenths of a percent like the minutes.
            Some((Unit::Percent, _)) => (0, None, LagKind::Percent(tenths.saturating_mul(10).clamp(-10_000_000, 10_000_000))),
            Some((_, true)) => (0, Some(minutes), LagKind::Elapsed),
            _ => (if whole { minutes / per_day } else { 0 }, (!whole).then_some(minutes), LagKind::Working),
        };
        relationships.push(Relationship { predecessor, successor, kind: link_of(int(l, "Type").unwrap_or(1)), lag, lag_minutes, lag_kind });
    }
    // A milestone after a predecessor sits at its finish, as Microsoft Project draws it: a
    // finish milestone; one with none, at the start.
    for a in activities.iter_mut().filter(|a| a.kind.is_milestone()) {
        if relationships.iter().any(|r| r.successor == a.id) {
            a.kind = ActivityKind::FinishMilestone;
        }
    }
    if dangling > 0 {
        warnings.push(tr!("warn-msp-dangling", n = dangling));
    }
    if across > 0 {
        warnings.push(tr!("warn-msp-cross", n = across));
    }

    // Resources and their assignments: work in hours, units 1 for 100 %.
    let mut resources: Vec<Resource> = Vec::new();
    let mut resource_by_uid: HashMap<String, String> = HashMap::new();
    let mut cost_resources: HashMap<String, (String, Accrual)> = HashMap::new();
    let mut expenses: Vec<Expense> = Vec::new();
    for r in root.child("Resources").map(|r| r.all("Resource").collect::<Vec<_>>()).unwrap_or_default() {
        let (Some(uid), Some(name)) = (r.text_of("UID"), r.text_of("Name")) else { continue };
        if uid == "0" {
            continue;
        }
        let kind = match int(r, "Type") {
            Some(0) => ResourceKind::Material,
            Some(2) => {
                // A cost resource: its assignments are expenses, accrued as it says.
                let accrual = match int(r, "AccrueAt") {
                    Some(1) => Accrual::Start,
                    Some(2) => Accrual::End,
                    _ => Accrual::Uniform,
                };
                cost_resources.insert(uid.to_string(), (name.to_string(), accrual));
                continue;
            }
            _ => ResourceKind::Labor,
        };
        let mut id = r.text_of("ID").map_or_else(|| uid.to_string(), str::to_string);
        while resources.iter().any(|x| x.id == id) {
            id.push('*');
        }
        let rate = number(r, "StandardRate").filter(|x| *x > 0.0 && *x < 1e13);
        resources.push(Resource {
            id: id.clone(),
            name: name.to_string(),
            kind,
            unit: r.text_of("MaterialLabel").unwrap_or_default().to_string(),
            max_units: number(r, "MaxUnits").filter(|x| *x >= 0.0 && *x <= 1e6).map_or(UNIT, |x| (x * UNIT as f64).round() as i64),
            calendar: r.text_of("CalendarUID").and_then(name_of).filter(|n| *n != calendar.name),
            prices: rate.map(|x| vec![mkperth_core::Price { from: None, per_unit: (x * 100.0).round() as i64 }]).unwrap_or_default(),
            ..Resource::default()
        });
        resource_by_uid.insert(uid.to_string(), id);
    }
    let mut assignments = Vec::new();
    let hours = |e: &Element, name: &str| e.text_of(name).and_then(duration_of).map(|m| m * UNIT / 60);
    let money = |e: &Element, name: &str| number(e, name).filter(|x| x.abs() < 1e13).map(|x| (x * 100.0).round() as i64);
    for x in root.child("Assignments").map(|a| a.all("Assignment").collect::<Vec<_>>()).unwrap_or_default() {
        // A cost resource's: an expense on the task, named after the resource.
        if let (Some(activity), Some((name, accrual))) =
            (x.text_of("TaskUID").and_then(|u| by_uid.get(u)).cloned(), x.text_of("ResourceUID").and_then(|u| cost_resources.get(u)))
        {
            let planned = money(x, "Cost").unwrap_or(0);
            let actual = money(x, "ActualCost").unwrap_or(0);
            let remaining = money(x, "RemainingCost").filter(|r| *r != planned.saturating_sub(actual).max(0));
            if !expenses.iter().any(|e| e.activity == activity && e.name == *name) {
                expenses.push(Expense { activity, name: name.clone(), planned, actual, remaining, accrual: *accrual, ..Expense::default() });
            }
            continue;
        }
        let (Some(activity), Some(resource)) =
            (x.text_of("TaskUID").and_then(|u| by_uid.get(u)).cloned(), x.text_of("ResourceUID").and_then(|u| resource_by_uid.get(u)).cloned())
        else {
            continue;
        };
        let planned = hours(x, "Work").unwrap_or(0);
        let actual = hours(x, "ActualWork").unwrap_or(0);
        let remaining = hours(x, "RemainingWork").filter(|r| *r != planned.saturating_sub(actual).max(0));
        let rate = number(x, "Units").filter(|u| *u >= 0.0 && *u <= 1e6).map(|u| (u * UNIT as f64).round() as i64).filter(|r| *r != UNIT);
        assignments.push(Assignment { activity, resource: Some(resource), planned, actual, remaining, rate, ..Assignment::default() });
    }

    let mut others: Vec<Calendar> = calendars.into_iter().map(|(_, c)| c).filter(|c| c.name != calendar.name).collect();
    if elapsed {
        let mut c = Calendar::around_the_clock();
        c.name = ELAPSED.into();
        others.push(c);
    }
    let start = root.text_of("StartDate").and_then(date_of).unwrap_or(Date::EPOCH);
    Ok(Read {
        project: Project {
            name: root.text_of("Title").or_else(|| root.text_of("Name")).unwrap_or_default().to_string(),
            start,
            calendar,
            calendars: others,
            lag_calendar: Default::default(),
            float_kind: Default::default(),
            critical_by: Default::default(),
            data_date: root.text_of("StatusDate").and_then(date_of),
            progress_mode: Default::default(),
            must_finish_by: None,
            activities,
            relationships,
            bands: Vec::new(),
            markers: Vec::new(),
            month_marks: Vec::new(),
            resources,
            roles: Vec::new(),
            assignments,
            comments: Vec::new(),
            levelling: Default::default(),
            baselines: Vec::new(),
            baseline_in_use: None,
            expenses,
            cost_accounts: Vec::new(),
            currency: root.text_of("CurrencySymbol").unwrap_or_default().to_string(),
            budget: Default::default(),
            earned_value: Default::default(),
            classification: Default::default(),
            external: Vec::new(),
            organisation: Default::default(),
            extra: Default::default(),
        },
        warnings,
    })
}

/// Text for XML: `&`, `<`, `>`, quotes escaped.
fn esc(text: &str) -> String {
    quick_xml::escape::escape(text).into_owned()
}

/// The activities in outline order, depth first, each with its outline level from 1; without
/// recursion, and never visiting one twice.
fn outline(project: &Project) -> Vec<(usize, usize)> {
    let acts = &project.activities;
    let index: HashMap<&str, usize> = acts.iter().enumerate().map(|(i, a)| (a.id.as_str(), i)).collect();
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); acts.len()];
    let mut top = Vec::new();
    for (i, a) in acts.iter().enumerate() {
        match a.parent.as_deref().and_then(|p| index.get(p)) {
            Some(&p) if p != i => {
                if let Some(c) = children.get_mut(p) {
                    c.push(i);
                }
            }
            _ => top.push(i),
        }
    }
    let mut out = Vec::with_capacity(acts.len());
    let mut seen = vec![false; acts.len()];
    let mut stack: Vec<(usize, usize)> = top.into_iter().rev().map(|i| (i, 1)).collect();
    while let Some((i, level)) = stack.pop() {
        if seen.get(i).copied().unwrap_or(true) {
            continue;
        }
        if let Some(s) = seen.get_mut(i) {
            *s = true;
        }
        out.push((i, level));
        for &c in children.get(i).map_or(&[][..], Vec::as_slice).iter().rev() {
            stack.push((c, level + 1));
        }
    }
    out
}

/// A calendar as the schema writes it: its weekdays (Sunday first), and its days off, days
/// worked besides the week and days of their own hours as exceptions. A region's holidays are
/// written as days off from `years`' first to its last.
fn calendar_xml(uid: usize, c: &Calendar, years: (i32, i32)) -> String {
    let mut out = format!(
        "    <Calendar>\n      <UID>{uid}</UID>\n      <Name>{}</Name>\n      <IsBaseCalendar>1</IsBaseCalendar>\n      <BaseCalendarUID>-1</BaseCalendarUID>\n      <WeekDays>\n",
        esc(&c.name)
    );
    for day_type in 1..=7usize {
        let k = (day_type + 5) % 7;
        let working = c.week.get(k).copied().unwrap_or(false);
        out.push_str(&format!(
            "        <WeekDay>\n          <DayType>{day_type}</DayType>\n          <DayWorking>{}</DayWorking>\n",
            u8::from(working)
        ));
        if working {
            out.push_str(&working_times(c.weekday_periods(k), "          "));
        }
        out.push_str("        </WeekDay>\n");
    }
    out.push_str("      </WeekDays>\n");
    let mut off: BTreeSet<Date> = c.holidays.clone();
    if let Some(region) = c.region {
        for y in years.0..=years.1 {
            let (Some(first), Some(last)) = (Date::from_ymd(y, 1, 1), Date::from_ymd(y, 12, 31)) else { continue };
            for d in (first.days()..=last.days()).filter_map(Date::from_days) {
                if region.is_holiday(d) && c.week.get(d.weekday().index()).copied().unwrap_or(false) {
                    off.insert(d);
                }
            }
        }
    }
    let mut exceptions: Vec<(Date, bool, Vec<Period>)> = off.into_iter().map(|d| (d, false, Vec::new())).collect();
    exceptions.extend(c.extra_days.iter().map(|d| (*d, true, c.weekday_periods(d.weekday().index()).to_vec())));
    exceptions.extend(c.exceptions.iter().map(|(d, p)| (*d, true, p.clone())));
    exceptions.sort_by_key(|(d, _, _)| *d);
    if !exceptions.is_empty() {
        out.push_str("      <Exceptions>\n");
        for (d, working, periods) in exceptions {
            out.push_str(&format!(
                "        <Exception>\n          <EnteredByOccurrences>0</EnteredByOccurrences>\n          <TimePeriod>\n            <FromDate>{d}T00:00:00</FromDate>\n            <ToDate>{d}T23:59:00</ToDate>\n          </TimePeriod>\n          <Occurrences>1</Occurrences>\n          <Type>1</Type>\n          <DayWorking>{}</DayWorking>\n",
                u8::from(working)
            ));
            if working {
                out.push_str(&working_times(&periods, "          "));
            }
            out.push_str("        </Exception>\n");
        }
        out.push_str("      </Exceptions>\n");
    }
    out.push_str("    </Calendar>\n");
    out
}

/// Writes `project` as MSPDI. Returns the text and what the format couldn't hold.
pub fn write(project: &Project) -> (String, Vec<String>) {
    let mut warnings = Vec::new();
    // What MK Perth holds that this file doesn't carry.
    let left_out: Vec<String> = [
        (!project.cost_accounts.is_empty(), "msp-left-costs"),
        (!project.baselines.is_empty(), "msp-left-baselines"),
        (!project.comments.is_empty(), "msp-left-comments"),
        (!(project.bands.is_empty() && project.markers.is_empty() && project.month_marks.is_empty()), "msp-left-notes"),
        (!project.classification.is_empty(), "msp-left-codes"),
        (!project.budget.is_empty() || !project.earned_value.is_empty(), "msp-left-budget"),
        (!project.external.is_empty(), "msp-left-external"),
        (!project.organisation.is_empty(), "msp-left-organisation"),
        (!project.roles.is_empty(), "msp-left-roles"),
    ]
    .into_iter()
    .filter(|(on, _)| *on)
    .map(|(_, id)| tr!(id))
    .collect();
    if !left_out.is_empty() {
        warnings.push(tr!("warn-msp-left-out", what = left_out.join(", ")));
    }
    let per_day = project.calendar.day_minutes().max(1);
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    out.push_str(&format!("<Project xmlns=\"{NAMESPACE}\">\n"));
    out.push_str(&format!("  <Name>{}</Name>\n  <Title>{}</Title>\n", esc(&project.name), esc(&project.name)));
    out.push_str(&format!("  <ScheduleFromStart>1</ScheduleFromStart>\n  <StartDate>{}T08:00:00</StartDate>\n", project.start));
    if let Some(d) = project.data_date {
        out.push_str(&format!("  <StatusDate>{d}T08:00:00</StatusDate>\n"));
    }
    out.push_str(&format!("  <MinutesPerDay>{per_day}</MinutesPerDay>\n  <MinutesPerWeek>{}</MinutesPerWeek>\n", per_day * 5));
    if !project.currency.is_empty() {
        out.push_str(&format!("  <CurrencySymbol>{}</CurrencySymbol>\n", esc(&project.currency)));
    }
    out.push_str("  <CalendarUID>1</CalendarUID>\n");
    // Calendars: the project's (1), then the others; the elapsed one is the tasks' duration format.
    let calendars: Vec<&Calendar> = std::iter::once(&project.calendar).chain(project.calendars.iter().filter(|c| c.name != ELAPSED)).collect();
    let calendar_uid = |name: &str| calendars.iter().position(|c| c.name == name).map(|k| k + 1);
    let years = (project.start.ymd().0, project.start.ymd().0 + 5);
    if calendars.iter().any(|c| c.region.is_some()) {
        warnings.push(tr!("warn-msp-region", from = years.0, to = years.1));
    }
    out.push_str("  <Calendars>\n");
    for (k, c) in calendars.iter().enumerate() {
        out.push_str(&calendar_xml(k + 1, c, years));
    }
    out.push_str("  </Calendars>\n");

    // Tasks in outline order; their UIDs from 1 in that order.
    let order = outline(project);
    let uid: HashMap<&str, usize> =
        order.iter().enumerate().filter_map(|(n, (i, _))| project.activities.get(*i).map(|a| (a.id.as_str(), n + 1))).collect();
    let summary = |id: &str| project.activities.iter().any(|a| a.parent.as_deref() == Some(id));
    let mut mandatory = 0usize;
    let mut secondary = 0usize;
    out.push_str("  <Tasks>\n");
    for (n, &(i, level)) in order.iter().enumerate() {
        let Some(a) = project.activities.get(i) else { continue };
        let elapsed = a.calendar.as_deref() == Some(ELAPSED);
        let day = if elapsed {
            1440
        } else {
            a.calendar.as_deref().and_then(|c| project.calendars.iter().find(|x| x.name == c)).map_or(per_day, |c| c.day_minutes().max(1))
        };
        let minutes = a.working_minutes(day);
        let format = match (elapsed, minutes % day == 0) {
            (true, _) => 8,
            (false, true) => 7,
            (false, false) => 5,
        };
        out.push_str(&format!(
            "    <Task>\n      <UID>{}</UID>\n      <ID>{}</ID>\n      <Name>{}</Name>\n      <WBS>{}</WBS>\n      <OutlineLevel>{level}</OutlineLevel>\n      <Summary>{}</Summary>\n      <Milestone>{}</Milestone>\n      <Duration>{}</Duration>\n      <DurationFormat>{format}</DurationFormat>\n",
            n + 1,
            n + 1,
            esc(&a.name),
            esc(&a.id),
            u8::from(summary(&a.id)),
            u8::from(a.kind.is_milestone()),
            duration(minutes)
        ));
        let task_type = match a.duration_type {
            DurationType::FixedRate => 0,
            DurationType::FixedUnits => 2,
            DurationType::FixedDurationAndRate | DurationType::FixedDurationAndUnits => 1,
        };
        out.push_str(&format!("      <Type>{task_type}</Type>\n"));
        if let Some(c) = a.constraint {
            mandatory += usize::from(matches!(c, Constraint::MandatoryStart(_) | Constraint::MandatoryFinish(_)));
            let (code, date) = constraint_code(c);
            out.push_str(&format!("      <ConstraintType>{code}</ConstraintType>\n"));
            if let Some(d) = date {
                out.push_str(&format!("      <ConstraintDate>{d}</ConstraintDate>\n"));
            }
        }
        match a.secondary_constraint {
            Some(Constraint::FinishOnOrBefore(d)) => out.push_str(&format!("      <Deadline>{d}T17:00:00</Deadline>\n")),
            Some(_) => secondary += 1,
            None => {}
        }
        if let Some(k) = a.calendar.as_deref().filter(|_| !elapsed).and_then(calendar_uid) {
            out.push_str(&format!("      <CalendarUID>{k}</CalendarUID>\n"));
        }
        if let Some(d) = a.actual_start {
            out.push_str(&format!("      <ActualStart>{d}T08:00:00</ActualStart>\n"));
        }
        if let Some(d) = a.actual_finish {
            out.push_str(&format!("      <ActualFinish>{d}T17:00:00</ActualFinish>\n"));
        }
        if let (Some(_), None, Some(m)) = (a.actual_start, a.actual_finish, a.remaining_minutes) {
            out.push_str(&format!("      <RemainingDuration>{}</RemainingDuration>\n", duration(m)));
        }
        if let Some(p) = a.percent_complete {
            out.push_str(&format!("      <PhysicalPercentComplete>{}</PhysicalPercentComplete>\n", p / 100));
        }
        if !a.notes.is_empty() {
            out.push_str(&format!("      <Notes>{}</Notes>\n", esc(&a.notes)));
        }
        for r in project.relationships.iter().filter(|r| r.successor == a.id) {
            let Some(p) = uid.get(r.predecessor.as_str()) else { continue };
            // The lag in tenths of a minute (of a percent, for a share), and its format.
            let (tenths, format) = match r.lag_kind {
                LagKind::Percent(p) => (p / 10, 19),
                LagKind::Elapsed => {
                    let m = r.lag_minutes.unwrap_or_else(|| r.lag.saturating_mul(1440));
                    (
                        m.saturating_mul(10),
                        if m % 1440 == 0 {
                            8
                        } else if m % 60 == 0 {
                            6
                        } else {
                            4
                        },
                    )
                }
                LagKind::Working => {
                    let m = r.lag_minutes.unwrap_or_else(|| r.lag.saturating_mul(per_day));
                    (
                        m.saturating_mul(10),
                        if r.lag_minutes.is_none() {
                            7
                        } else if m % 60 == 0 {
                            5
                        } else {
                            3
                        },
                    )
                }
            };
            out.push_str(&format!(
                "      <PredecessorLink>\n        <PredecessorUID>{p}</PredecessorUID>\n        <Type>{}</Type>\n        <CrossProject>0</CrossProject>\n        <LinkLag>{}</LinkLag>\n        <LagFormat>{format}</LagFormat>\n      </PredecessorLink>\n",
                link_code(r.kind),
                tenths
            ));
        }
        out.push_str("    </Task>\n");
    }
    out.push_str("  </Tasks>\n");
    if mandatory > 0 {
        warnings.push(tr!("warn-msp-mandatory", n = mandatory));
    }
    let start_after = project
        .activities
        .iter()
        .filter(|a| a.kind == ActivityKind::StartMilestone && project.relationships.iter().any(|r| r.successor == a.id))
        .count();
    if start_after > 0 {
        warnings.push(tr!("warn-msp-start-milestones", n = start_after));
    }
    if secondary > 0 {
        warnings.push(tr!("warn-msp-secondary", n = secondary));
    }

    // Resources (work or material), their price an hour or a unit, and their assignments.
    let resource_uid = |id: &str| project.resources.iter().position(|r| r.id == id).map(|k| k + 1);
    out.push_str("  <Resources>\n");
    for (k, r) in project.resources.iter().enumerate() {
        let kind = u8::from(r.kind != ResourceKind::Material);
        out.push_str(&format!(
            "    <Resource>\n      <UID>{}</UID>\n      <ID>{}</ID>\n      <Name>{}</Name>\n      <Type>{kind}</Type>\n      <MaxUnits>{}</MaxUnits>\n",
            k + 1,
            k + 1,
            esc(&r.name),
            r.max_units as f64 / UNIT as f64
        ));
        if r.kind == ResourceKind::Material && !r.unit.is_empty() {
            out.push_str(&format!("      <MaterialLabel>{}</MaterialLabel>\n", esc(&r.unit)));
        }
        if let Some(p) = r.prices.first() {
            out.push_str(&format!("      <StandardRate>{}</StandardRate>\n", p.per_unit as f64 / 100.0));
        }
        if let Some(c) = r.calendar.as_deref().and_then(calendar_uid) {
            out.push_str(&format!("      <CalendarUID>{c}</CalendarUID>\n"));
        }
        out.push_str("    </Resource>\n");
    }
    // Expenses as cost resources, one for each name, after the others.
    let mut cost_names: Vec<(&str, Accrual)> = Vec::new();
    for e in project.expenses.iter().filter(|e| !summary(&e.activity)) {
        if !cost_names.iter().any(|(n, _)| *n == e.name) {
            cost_names.push((e.name.as_str(), e.accrual));
        }
    }
    let first_cost = project.resources.len() + 1;
    for (k, (name, accrual)) in cost_names.iter().enumerate() {
        let accrue = match accrual {
            Accrual::Start => 1,
            Accrual::End => 2,
            Accrual::Uniform => 3,
        };
        out.push_str(&format!(
            "    <Resource>\n      <UID>{}</UID>\n      <ID>{}</ID>\n      <Name>{}</Name>\n      <Type>2</Type>\n      <AccrueAt>{accrue}</AccrueAt>\n    </Resource>\n",
            first_cost + k,
            first_cost + k,
            esc(name)
        ));
    }
    let on_summaries = project.expenses.iter().filter(|e| summary(&e.activity)).count();
    if on_summaries > 0 {
        warnings.push(tr!("warn-msp-summary-expenses", n = on_summaries));
    }
    out.push_str("  </Resources>\n");
    let changing = project.resources.iter().filter(|r| r.prices.len() > 1).count();
    if changing > 0 {
        warnings.push(tr!("warn-msp-prices", n = changing));
    }
    let hours = |units: i64| duration(units.saturating_mul(60) / UNIT);
    let mut unstaffed = 0usize;
    out.push_str("  <Assignments>\n");
    for (k, x) in project.assignments.iter().enumerate() {
        let (Some(t), Some(r)) = (uid.get(x.activity.as_str()), x.resource.as_deref().and_then(resource_uid)) else {
            unstaffed += 1;
            continue;
        };
        out.push_str(&format!(
            "    <Assignment>\n      <UID>{}</UID>\n      <TaskUID>{t}</TaskUID>\n      <ResourceUID>{r}</ResourceUID>\n      <Units>{}</Units>\n      <Work>{}</Work>\n      <ActualWork>{}</ActualWork>\n      <RemainingWork>{}</RemainingWork>\n    </Assignment>\n",
            k + 1,
            x.rate.unwrap_or(UNIT) as f64 / UNIT as f64,
            hours(x.planned),
            hours(x.actual),
            hours(x.remaining_units())
        ));
    }
    for (k, e) in project.expenses.iter().enumerate() {
        let (Some(t), Some(r)) = (uid.get(e.activity.as_str()), cost_names.iter().position(|(n, _)| *n == e.name).map(|n| first_cost + n)) else {
            continue;
        };
        out.push_str(&format!(
            "    <Assignment>\n      <UID>{}</UID>\n      <TaskUID>{t}</TaskUID>\n      <ResourceUID>{r}</ResourceUID>\n      <Cost>{}</Cost>\n      <ActualCost>{}</ActualCost>\n      <RemainingCost>{}</RemainingCost>\n    </Assignment>\n",
            project.assignments.len() + k + 1,
            e.planned as f64 / 100.0,
            e.actual as f64 / 100.0,
            e.remaining_cost() as f64 / 100.0
        ));
    }
    out.push_str("  </Assignments>\n</Project>\n");
    if unstaffed > 0 {
        warnings.push(tr!("warn-msp-roles", n = unstaffed));
    }
    (out, warnings)
}
