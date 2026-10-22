//! The critical path method (CPM) over a project network: a forward pass for the early dates,
//! a backward pass for the late ones, and the floats between them.
//!
//! Time runs in calendar days (see `clock`): a time `t` is the morning of day `t`, the evening
//! of day `t − 1`. Each activity works on its own calendar: it starts on the morning of one of
//! its working days, takes its duration in its working days, and finishes on the evening of the
//! last. Each relationship bounds the successor `s` by its predecessor `p`, the lag counted in
//! working days of the lag calendar (the predecessor's by default) and `⊕` moving by them:
//!
//! | kind | bound |
//! | --- | --- |
//! | FS | `ES(s) ≥ EF(p) ⊕ lag` |
//! | SS | `ES(s) ≥ ES(p) ⊕ lag` |
//! | FF | `EF(s) ≥ EF(p) ⊕ lag` |
//! | SF | `EF(s) ≥ ES(p) ⊕ lag` |
//!
//! and, mirrored, the predecessor's late finish. No activity starts before the project does;
//! every activity finishes by the project's finish: the latest early finish, or the date it
//! must finish by. Constraints add their own bounds: an on-or-after date to the early dates, an
//! on-or-before date to the late ones, an "on" date to both, a mandatory date replaces both;
//! as late as possible moves an activity's early dates by its free float. Total float is the
//! working days from the early finish to the late finish, free float those before the
//! tightest successor, both on the activity's calendar; an activity is critical when its total
//! float is 0 or less. Sources: the precedence diagramming method as textbooks state it (Moder,
//! Phillips and Davis, *Project Management with CPM, PERT and Precedence Diagramming*; PMI's
//! *Practice Standard for Scheduling*).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod clock;

use std::collections::{HashMap, VecDeque};

use mkperth_core::{ActivityKind, Calendar, CalendarError, Constraint, Date, LagCalendar, Project, RelationshipType};
use mkperth_i18n::tr;
use thiserror::Error;

use clock::Clock;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ScheduleError {
    #[error("{}", tr!("err-duplicate-activity", id = .0.as_str()))]
    DuplicateActivity(String),
    #[error("{}", tr!("err-unknown-activity", id = .0.as_str()))]
    UnknownActivity(String),
    #[error("{}", tr!("err-self-relationship", id = .0.as_str()))]
    SelfRelationship(String),
    #[error("{}", tr!("err-negative-duration", id = .0.as_str()))]
    NegativeDuration(String),
    /// The activity, and the calendar it names that the project doesn't have.
    #[error("{}", tr!("err-unknown-calendar", id = .0.as_str(), name = .1.as_str()))]
    UnknownCalendar(String, String),
    /// The activities of a loop of relationships, in order.
    #[error("{}", tr!("err-loop", ids = .0.join(" → ")))]
    Loop(Vec<String>),
    #[error("{}", tr!("err-too-many", what = .0.as_str(), max = *.1))]
    TooMany(String, usize),
    #[error("{}", tr!("err-too-long", max = MAX_SPAN))]
    TooLong,
    #[error(transparent)]
    Calendar(#[from] CalendarError),
}

/// The most activities and relationships a project may have.
pub const MAX_ACTIVITIES: usize = 1_000_000;
pub const MAX_RELATIONSHIPS: usize = 4_000_000;
/// The longest a duration or a lag may be, in working days, and (times two) the calendar days a
/// schedule may span either side of its start: about five centuries.
pub const MAX_SPAN: i64 = 100_000;

/// One activity's dates and floats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheduled {
    pub id: String,
    pub early_start: Date,
    pub early_finish: Date,
    pub late_start: Date,
    pub late_finish: Date,
    /// In working days of the activity's calendar.
    pub total_float: i64,
    pub free_float: i64,
    pub critical: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    /// In the project's order.
    pub activities: Vec<Scheduled>,
    /// The latest early finish among the activities (the project's start when it has none).
    pub finish: Date,
}

/// A relationship by activity indexes, with the calendar its lag counts on.
struct Tie {
    from: usize,
    to: usize,
    kind: RelationshipType,
    lag: i64,
    lag_calendar: usize,
}

/// The constraint's date as a time: its morning for a start, its evening for a finish.
fn constraint_time(c: Constraint) -> Option<i64> {
    match c {
        Constraint::StartOnOrAfter(d) | Constraint::StartOnOrBefore(d) | Constraint::StartOn(d) | Constraint::MandatoryStart(d) => {
            Some(i64::from(d.days()))
        }
        Constraint::FinishOnOrAfter(d) | Constraint::FinishOnOrBefore(d) | Constraint::FinishOn(d) | Constraint::MandatoryFinish(d) => {
            Some(i64::from(d.days()) + 1)
        }
        Constraint::AsLateAsPossible => None,
    }
}

/// Schedules `project` by the critical path method.
pub fn schedule(project: &Project) -> Result<Schedule, ScheduleError> {
    let acts = &project.activities;
    if acts.len() > MAX_ACTIVITIES {
        return Err(ScheduleError::TooMany(tr!("what-activities"), MAX_ACTIVITIES));
    }
    if project.relationships.len() > MAX_RELATIONSHIPS {
        return Err(ScheduleError::TooMany(tr!("what-relationships"), MAX_RELATIONSHIPS));
    }
    // The calendars: the project's (0), the others it lists, and 24 hours for lags (last).
    let every_day = Calendar::every_day();
    let mut calendars: Vec<&Calendar> = vec![&project.calendar];
    calendars.extend(project.calendars.iter());
    calendars.push(&every_day);
    for c in &calendars {
        if !c.works() {
            return Err(CalendarError::NoWorkingDays(c.name.clone()).into());
        }
    }
    let twenty_four = calendars.len() - 1;
    let named = |name: &str| project.calendars.iter().position(|c| c.name == name).map(|k| k + 1);
    let mut index = HashMap::with_capacity(acts.len());
    let mut on = Vec::with_capacity(acts.len());
    for (i, a) in acts.iter().enumerate() {
        if index.insert(a.id.as_str(), i).is_some() {
            return Err(ScheduleError::DuplicateActivity(a.id.clone()));
        }
        if a.duration < 0 {
            return Err(ScheduleError::NegativeDuration(a.id.clone()));
        }
        if a.working_days() > MAX_SPAN {
            return Err(ScheduleError::TooLong);
        }
        on.push(match &a.calendar {
            None => 0,
            Some(name) if *name == project.calendar.name => 0,
            Some(name) => named(name).ok_or_else(|| ScheduleError::UnknownCalendar(a.id.clone(), name.clone()))?,
        });
    }
    let cal_of = |i: usize| on.get(i).copied().unwrap_or(0);
    let clock = |k: usize| Clock { calendar: calendars.get(k).copied().unwrap_or(&project.calendar) };
    let find = |id: &str| index.get(id).copied().ok_or_else(|| ScheduleError::UnknownActivity(id.to_string()));
    let mut ties = Vec::with_capacity(project.relationships.len());
    for r in &project.relationships {
        let (from, to) = (find(&r.predecessor)?, find(&r.successor)?);
        if from == to {
            return Err(ScheduleError::SelfRelationship(r.predecessor.clone()));
        }
        if r.lag.abs() > MAX_SPAN {
            return Err(ScheduleError::TooLong);
        }
        let lag_calendar = match project.lag_calendar {
            LagCalendar::Predecessor => cal_of(from),
            LagCalendar::Successor => cal_of(to),
            LagCalendar::TwentyFourHour => twenty_four,
            LagCalendar::Project => 0,
        };
        ties.push(Tie { from, to, kind: r.kind, lag: r.lag, lag_calendar });
    }
    let n = acts.len();
    let mut outgoing: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut incoming: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (k, tie) in ties.iter().enumerate() {
        if let (Some(o), Some(i)) = (outgoing.get_mut(tie.from), incoming.get_mut(tie.to)) {
            o.push(k);
            i.push(k);
        }
    }
    let order = topological(n, &ties, &outgoing, &incoming, acts.iter().map(|a| a.id.as_str()).collect())?;
    let dur = |i: usize| acts.get(i).map_or(0, |a| a.working_days());
    let kind = |i: usize| acts.get(i).map_or(ActivityKind::Task, |a| a.kind);
    // Both constraints of an activity hold; the primary first.
    let constraints = |i: usize| acts.get(i).map_or([None, None], |a| [a.constraint, a.secondary_constraint]).into_iter().flatten();
    let origin = i64::from(project.start.days());
    // Every time stays within the span of the project's start: far from overflow and from the
    // ends of the calendar.
    let bounded = |t: i64| if (t - origin).abs() > MAX_SPAN * 2 { Err(ScheduleError::TooLong) } else { Ok(t) };

    // Where an activity's working time can begin and end, on its calendar.
    let place_start = |i: usize, t: i64| -> Result<i64, ScheduleError> {
        let c = clock(cal_of(i));
        match kind(i) {
            ActivityKind::FinishMilestone => c.next_finish(t),
            _ => c.next_start(t),
        }
    };
    let finish_of = |i: usize, es: i64| -> Result<i64, ScheduleError> { clock(cal_of(i)).advance(es, dur(i)) };
    // The earliest start that finishes no earlier than `t`.
    let start_for_finish = |i: usize, t: i64| -> Result<i64, ScheduleError> {
        let c = clock(cal_of(i));
        if dur(i) == 0 { Ok(t) } else { c.retreat(c.next_finish(t)?, dur(i)) }
    };
    let place_finish = |i: usize, t: i64| -> Result<i64, ScheduleError> {
        let c = clock(cal_of(i));
        match kind(i) {
            ActivityKind::Task if dur(i) > 0 => c.previous_finish(t),
            ActivityKind::FinishMilestone => c.previous_finish(t),
            _ => c.previous_start(t),
        }
    };
    let start_of = |i: usize, lf: i64| -> Result<i64, ScheduleError> { clock(cal_of(i)).retreat(lf, dur(i)) };
    // The latest finish that starts no later than `t`.
    let finish_for_start = |i: usize, t: i64| -> Result<i64, ScheduleError> {
        let c = clock(cal_of(i));
        if dur(i) == 0 { Ok(t) } else { c.advance(c.previous_start(t)?, dur(i)) }
    };
    // The bound a tie puts on its successor, from the predecessor's early dates: a start for
    // FS and SS, a finish for FF and SF.
    let forward_bound = |t: &Tie, p_es: i64, p_ef: i64| -> Result<i64, ScheduleError> {
        let lag = clock(t.lag_calendar);
        bounded(match t.kind {
            RelationshipType::FinishToStart | RelationshipType::FinishToFinish => lag.shift(p_ef, t.lag)?,
            RelationshipType::StartToStart | RelationshipType::StartToFinish => lag.shift(p_es, t.lag)?,
        })
    };

    // Forward pass.
    let mut es = vec![origin; n];
    let mut ef = vec![origin; n];
    for &i in &order {
        let mut start = origin;
        for &k in incoming.get(i).map_or(&[][..], Vec::as_slice) {
            let Some(t) = ties.get(k) else { continue };
            let bound = forward_bound(t, es.get(t.from).copied().unwrap_or(origin), ef.get(t.from).copied().unwrap_or(origin))?;
            let s = match t.kind {
                RelationshipType::FinishToStart | RelationshipType::StartToStart => bound,
                RelationshipType::FinishToFinish | RelationshipType::StartToFinish => start_for_finish(i, bound)?,
            };
            start = start.max(s);
        }
        // Early-date constraints: no earlier than their date; a mandatory one, on it.
        for c in constraints(i) {
            let at = constraint_time(c).map(bounded).transpose()?;
            start = match (c, at) {
                (Constraint::StartOnOrAfter(_) | Constraint::StartOn(_), Some(t)) => start.max(t),
                (Constraint::FinishOnOrAfter(_) | Constraint::FinishOn(_), Some(t)) => start.max(start_for_finish(i, t)?),
                (Constraint::MandatoryStart(_), Some(t)) => t.max(origin),
                (Constraint::MandatoryFinish(_), Some(t)) => start_for_finish(i, t)?.max(origin),
                _ => start,
            };
        }
        let s = bounded(place_start(i, start)?)?;
        let f = bounded(finish_of(i, s)?)?;
        if let (Some(a), Some(b)) = (es.get_mut(i), ef.get_mut(i)) {
            *a = s;
            *b = f;
        }
    }
    let finish = ef.iter().copied().max().unwrap_or(origin);

    // Backward pass, from the evening the project must finish by, or from its early finish.
    let end = match project.must_finish_by {
        Some(date) => bounded(i64::from(date.days()) + 1)?,
        None => finish,
    };
    let mut ls = vec![end; n];
    let mut lf = vec![end; n];
    for &i in order.iter().rev() {
        let mut latest = end;
        for &k in outgoing.get(i).map_or(&[][..], Vec::as_slice) {
            let Some(t) = ties.get(k) else { continue };
            let lag = clock(t.lag_calendar);
            let (s_ls, s_lf) = (ls.get(t.to).copied().unwrap_or(end), lf.get(t.to).copied().unwrap_or(end));
            let f = match t.kind {
                RelationshipType::FinishToStart => lag.shift(s_ls, -t.lag)?,
                RelationshipType::StartToStart => finish_for_start(i, lag.shift(s_ls, -t.lag)?)?,
                RelationshipType::FinishToFinish => lag.shift(s_lf, -t.lag)?,
                RelationshipType::StartToFinish => finish_for_start(i, lag.shift(s_lf, -t.lag)?)?,
            };
            latest = latest.min(bounded(f)?);
        }
        // Late-date constraints: no later than their date; a mandatory one, on it.
        for c in constraints(i) {
            let at = constraint_time(c).map(bounded).transpose()?;
            latest = match (c, at) {
                (Constraint::StartOnOrBefore(_) | Constraint::StartOn(_), Some(t)) => latest.min(finish_for_start(i, t)?),
                (Constraint::FinishOnOrBefore(_) | Constraint::FinishOn(_), Some(t)) => latest.min(t),
                (Constraint::MandatoryStart(_), Some(t)) => finish_for_start(i, t)?,
                (Constraint::MandatoryFinish(_), Some(t)) => t,
                _ => latest,
            };
        }
        let f = bounded(place_finish(i, latest)?)?;
        let s = bounded(start_of(i, f)?)?;
        if let (Some(a), Some(b)) = (ls.get_mut(i), lf.get_mut(i)) {
            *a = s;
            *b = f;
        }
    }

    // Working days between two times, by each calendar's running count over the schedule's
    // span: one pass per calendar, not one per activity.
    let lo = es.iter().chain(&ls).copied().min().unwrap_or(origin).min(origin) - 1;
    let hi = ef.iter().chain(&lf).copied().max().unwrap_or(origin).max(end) + 1;
    let span = usize::try_from(hi - lo).map_err(|_| ScheduleError::TooLong)?;
    let mut counts: Vec<Vec<i64>> = Vec::with_capacity(calendars.len());
    for k in 0..calendars.len() {
        let c = clock(k);
        let mut running = Vec::with_capacity(span + 1);
        let mut total = 0i64;
        running.push(0);
        for day in lo..hi {
            total += i64::from(c.working(day)?);
            running.push(total);
        }
        counts.push(running);
    }
    let between = |k: usize, a: i64, b: i64| -> Result<i64, ScheduleError> {
        let at = |t: i64| usize::try_from(t - lo).ok().and_then(|x| counts.get(k).and_then(|r| r.get(x)).copied());
        match (at(a), at(b)) {
            (Some(x), Some(y)) => Ok(y - x),
            _ => clock(k).between(a, b),
        }
    };

    // Free float, successors first, so an activity held as late as possible moves by its own
    // before its predecessors measure theirs against it.
    let mut free = vec![0i64; n];
    for &i in order.iter().rev() {
        let c = clock(cal_of(i));
        let (e_s, e_f) = (es.get(i).copied().unwrap_or(origin), ef.get(i).copied().unwrap_or(origin));
        let mut slack = between(cal_of(i), e_f, finish)?;
        for &k in outgoing.get(i).map_or(&[][..], Vec::as_slice) {
            let Some(t) = ties.get(k) else { continue };
            let bound = forward_bound(t, e_s, e_f)?;
            let actual = match t.kind {
                RelationshipType::FinishToStart | RelationshipType::StartToStart => es.get(t.to).copied().unwrap_or(origin),
                RelationshipType::FinishToFinish | RelationshipType::StartToFinish => ef.get(t.to).copied().unwrap_or(origin),
            };
            slack = slack.min(between(cal_of(i), bound, actual)?);
        }
        let total = between(cal_of(i), e_f, lf.get(i).copied().unwrap_or(end))?;
        let mut own = slack.min(total).max(0);
        if constraints(i).any(|c| c == Constraint::AsLateAsPossible) && own > 0 {
            let moved = place_start(i, c.advance(e_s, own)?)?;
            let s = if dur(i) == 0 { moved } else { c.retreat(c.advance(e_f, own)?, dur(i))? };
            let f = finish_of(i, s)?;
            if let (Some(a), Some(b)) = (es.get_mut(i), ef.get_mut(i)) {
                *a = s;
                *b = f;
            }
            own = 0;
        }
        if let Some(slot) = free.get_mut(i) {
            *slot = own;
        }
    }

    // A start is the morning of its day; a finish the evening of the day before its time. A
    // start milestone (or a task of no duration) is shown on its morning's day, a finish
    // milestone on its evening's.
    let date = |t: i64| i32::try_from(t).ok().and_then(Date::from_days).ok_or(ScheduleError::TooLong);
    let start_date = |i: usize, t: i64| if kind(i) == ActivityKind::FinishMilestone { date(t - 1) } else { date(t) };
    let finish_date = |i: usize, t: i64| match kind(i) {
        ActivityKind::FinishMilestone => date(t - 1),
        ActivityKind::Task if dur(i) > 0 => date(t - 1),
        _ => date(t),
    };
    let mut out = Vec::with_capacity(n);
    let mut last = project.start;
    for (i, a) in acts.iter().enumerate() {
        let (e_s, e_f) = (es.get(i).copied().unwrap_or(origin), ef.get(i).copied().unwrap_or(origin));
        let (l_s, l_f) = (ls.get(i).copied().unwrap_or(end), lf.get(i).copied().unwrap_or(end));
        let total = between(cal_of(i), e_f, l_f)?;
        let early_finish = finish_date(i, e_f)?;
        last = last.max(early_finish);
        out.push(Scheduled {
            id: a.id.clone(),
            early_start: start_date(i, e_s)?,
            early_finish,
            late_start: start_date(i, l_s)?,
            late_finish: finish_date(i, l_f)?,
            total_float: total,
            free_float: free.get(i).copied().unwrap_or(0),
            critical: total <= 0,
        });
    }
    Ok(Schedule { activities: out, finish: last })
}

/// Kahn's algorithm: the activities in an order where every predecessor comes first, or the
/// activities of a loop.
fn topological(n: usize, ties: &[Tie], outgoing: &[Vec<usize>], incoming: &[Vec<usize>], ids: Vec<&str>) -> Result<Vec<usize>, ScheduleError> {
    let mut waiting: Vec<usize> = incoming.iter().map(Vec::len).collect();
    let mut ready: VecDeque<usize> = (0..n).filter(|i| waiting.get(*i) == Some(&0)).collect();
    let mut order = Vec::with_capacity(n);
    while let Some(i) = ready.pop_front() {
        order.push(i);
        for &k in outgoing.get(i).map_or(&[][..], Vec::as_slice) {
            let Some(to) = ties.get(k).map(|t| t.to) else { continue };
            if let Some(w) = waiting.get_mut(to) {
                *w -= 1;
                if *w == 0 {
                    ready.push_back(to);
                }
            }
        }
    }
    if order.len() == n {
        return Ok(order);
    }
    // A loop: walk back from any activity still waiting along predecessors still waiting.
    let stuck = |i: usize| waiting.get(i).is_some_and(|w| *w > 0);
    let Some(mut at) = (0..n).find(|&i| stuck(i)) else { return Ok(order) };
    let mut seen: Vec<Option<usize>> = vec![None; n];
    let mut path = Vec::new();
    while seen.get(at).copied().flatten().is_none() {
        if let Some(slot) = seen.get_mut(at) {
            *slot = Some(path.len());
        }
        path.push(at);
        let Some(prev) = incoming.get(at).and_then(|ks| ks.iter().filter_map(|k| ties.get(*k)).map(|t| t.from).find(|&p| stuck(p))) else {
            break;
        };
        at = prev;
    }
    let from = seen.get(at).copied().flatten().unwrap_or(0);
    let mut cycle: Vec<String> = path.get(from..).unwrap_or_default().iter().rev().map(|&i| ids.get(i).copied().unwrap_or("").to_string()).collect();
    if let Some(first) = cycle.first().cloned() {
        cycle.push(first);
    }
    Err(ScheduleError::Loop(cycle))
}

#[cfg(test)]
mod tests;
