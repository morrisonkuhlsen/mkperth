//! The critical path method (CPM) over a project network: a forward pass for the early dates,
//! a backward pass for the late ones, and the floats between them.
//!
//! Time runs in working days of the project's calendar, counted from the start of the project's
//! first working day: an activity of duration `d` that starts at `t` occupies working days
//! `t … t + d − 1` and finishes at the end of the last, `t + d`. A milestone (duration 0) starts
//! and finishes at `t`. Each relationship constrains the successor's early start:
//!
//! | kind | early start of the successor `s` after predecessor `p` |
//! | --- | --- |
//! | FS | `ES(s) ≥ EF(p) + lag` |
//! | SS | `ES(s) ≥ ES(p) + lag` |
//! | FF | `ES(s) ≥ EF(p) + lag − d(s)` |
//! | SF | `ES(s) ≥ ES(p) + lag − d(s)` |
//!
//! and, mirrored, the predecessor's late finish. No activity starts before the project does;
//! every activity finishes by the project's finish: the latest early finish, or the date it
//! must finish by. Constraints add their own bounds: an on-or-after date to the early dates, an
//! on-or-before date to the late ones, an "on" date to both, a mandatory date replaces both;
//! as late as possible moves an activity's early dates by its free float. Total float is
//! `LF − EF`; free float is the slack before the tightest successor; an activity is critical
//! when its total float is 0 or less. Sources: the precedence diagramming method as textbooks
//! state it (Moder, Phillips and Davis, *Project Management with CPM, PERT and Precedence
//! Diagramming*; PMI's *Practice Standard for Scheduling*).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::collections::{HashMap, VecDeque};

use mkperth_core::{ActivityKind, Calendar, CalendarError, Constraint, Date, Project, RelationshipType};
use mkperth_i18n::tr;
use thiserror::Error;

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
/// The longest a schedule may run, in working days (about four centuries).
pub const MAX_SPAN: i64 = 100_000;

/// One activity's dates and floats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheduled {
    pub id: String,
    pub early_start: Date,
    pub early_finish: Date,
    pub late_start: Date,
    pub late_finish: Date,
    /// In working days.
    pub total_float: i64,
    pub free_float: i64,
    pub critical: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    /// In the project's order.
    pub activities: Vec<Scheduled>,
    /// The latest early finish.
    pub finish: Date,
}

/// A relationship by activity indexes.
struct Tie {
    from: usize,
    to: usize,
    kind: RelationshipType,
    lag: i64,
}

/// Bounds a time so arithmetic on it stays far from overflow.
fn bounded(t: i64) -> Result<i64, ScheduleError> {
    if t.abs() > MAX_SPAN * 4 { Err(ScheduleError::TooLong) } else { Ok(t) }
}

/// The time of a date on the schedule's clock: the morning of `date` (the next working day's if
/// it isn't one) for a start, its evening (the working day before's if it isn't one) for a
/// finish. Before the project's first working day the count goes negative.
fn moment(calendar: &Calendar, first: Date, date: Date, finish: bool) -> Result<i64, ScheduleError> {
    let day = if finish { calendar.previous_working(date)? } else { calendar.next_working(date)? };
    // Far beyond any schedule: refused before the walk below.
    if (i64::from(day.days()) - i64::from(first.days())).abs() > MAX_SPAN * 3 {
        return Err(ScheduleError::TooLong);
    }
    let (from, to, sign) = if day >= first { (first, day, 1) } else { (day, first, -1) };
    let mut count = 0i64;
    let mut d = from;
    while d < to {
        if calendar.is_working(d) {
            count += 1;
        }
        d = d.succ().ok_or(ScheduleError::TooLong)?;
    }
    Ok(sign * count + i64::from(finish))
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
    let mut index = HashMap::with_capacity(acts.len());
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
    }
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
        ties.push(Tie { from, to, kind: r.kind, lag: r.lag });
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
    let first = project.calendar.next_working(project.start)?;
    // Each constraint's date on the clock.
    let mut held: Vec<Option<(Constraint, i64)>> = Vec::with_capacity(n);
    for a in acts {
        held.push(match a.constraint {
            None => None,
            Some(c @ Constraint::AsLateAsPossible) => Some((c, 0)),
            Some(c @ (Constraint::StartOnOrAfter(d) | Constraint::StartOnOrBefore(d) | Constraint::StartOn(d) | Constraint::MandatoryStart(d))) => {
                Some((c, moment(&project.calendar, first, d, false)?))
            }
            Some(
                c @ (Constraint::FinishOnOrAfter(d) | Constraint::FinishOnOrBefore(d) | Constraint::FinishOn(d) | Constraint::MandatoryFinish(d)),
            ) => Some((c, moment(&project.calendar, first, d, true)?)),
        });
    }
    let constraint = |i: usize| held.get(i).copied().flatten();

    // Forward pass.
    let mut es = vec![0i64; n];
    for &i in &order {
        let mut start = 0i64;
        for &k in incoming.get(i).map_or(&[][..], Vec::as_slice) {
            let Some(t) = ties.get(k) else { continue };
            let p_es = es.get(t.from).copied().unwrap_or(0);
            let p_ef = p_es + dur(t.from);
            let bound = match t.kind {
                RelationshipType::FinishToStart => p_ef + t.lag,
                RelationshipType::StartToStart => p_es + t.lag,
                RelationshipType::FinishToFinish => p_ef + t.lag - dur(i),
                RelationshipType::StartToFinish => p_es + t.lag - dur(i),
            };
            start = start.max(bounded(bound)?);
        }
        // Early-date constraints: no earlier than their date; a mandatory one, on it.
        start = match constraint(i) {
            Some((Constraint::StartOnOrAfter(_) | Constraint::StartOn(_), t)) => start.max(t),
            Some((Constraint::FinishOnOrAfter(_) | Constraint::FinishOn(_), t)) => start.max(t - dur(i)),
            Some((Constraint::MandatoryStart(_), t)) => t.max(0),
            Some((Constraint::MandatoryFinish(_), t)) => (t - dur(i)).max(0),
            _ => start,
        };
        if let Some(slot) = es.get_mut(i) {
            *slot = start;
        }
    }
    let mut ef: Vec<i64> = (0..n).map(|i| es.get(i).copied().unwrap_or(0) + dur(i)).collect();
    let finish = ef.iter().copied().max().unwrap_or(0);
    if finish > MAX_SPAN {
        return Err(ScheduleError::TooLong);
    }

    // Backward pass, from the date the project must finish by, or from its early finish.
    let end = match project.must_finish_by {
        Some(date) => moment(&project.calendar, first, date, true)?,
        None => finish,
    };
    let mut lf = vec![end; n];
    for &i in order.iter().rev() {
        let mut latest = end;
        for &k in outgoing.get(i).map_or(&[][..], Vec::as_slice) {
            let Some(t) = ties.get(k) else { continue };
            let s_lf = lf.get(t.to).copied().unwrap_or(finish);
            let s_ls = s_lf - dur(t.to);
            let bound = match t.kind {
                RelationshipType::FinishToStart => s_ls - t.lag,
                RelationshipType::StartToStart => s_ls - t.lag + dur(i),
                RelationshipType::FinishToFinish => s_lf - t.lag,
                RelationshipType::StartToFinish => s_lf - t.lag + dur(i),
            };
            latest = latest.min(bounded(bound)?);
        }
        // Late-date constraints: no later than their date; a mandatory one, on it.
        latest = match constraint(i) {
            Some((Constraint::StartOnOrBefore(_) | Constraint::StartOn(_), t)) => latest.min(t + dur(i)),
            Some((Constraint::FinishOnOrBefore(_) | Constraint::FinishOn(_), t)) => latest.min(t),
            Some((Constraint::MandatoryStart(_), t)) => t + dur(i),
            Some((Constraint::MandatoryFinish(_), t)) => t,
            _ => latest,
        };
        if let Some(slot) = lf.get_mut(i) {
            *slot = latest;
        }
    }

    // Free float, successors first, so an activity held as late as possible moves by its own
    // before its predecessors measure theirs against it.
    let mut free = vec![0i64; n];
    for &i in order.iter().rev() {
        let (e_s, e_f) = (es.get(i).copied().unwrap_or(0), ef.get(i).copied().unwrap_or(0));
        let mut slack = finish - e_f;
        for &k in outgoing.get(i).map_or(&[][..], Vec::as_slice) {
            let Some(t) = ties.get(k) else { continue };
            let s_es = es.get(t.to).copied().unwrap_or(0);
            let s_ef = ef.get(t.to).copied().unwrap_or(0);
            slack = slack.min(match t.kind {
                RelationshipType::FinishToStart => s_es - t.lag - e_f,
                RelationshipType::StartToStart => s_es - t.lag - e_s,
                RelationshipType::FinishToFinish => s_ef - t.lag - e_f,
                RelationshipType::StartToFinish => s_ef - t.lag - e_s,
            });
        }
        let total = lf.get(i).copied().unwrap_or(end) - e_f;
        let mut own = slack.min(total).max(0);
        if matches!(constraint(i), Some((Constraint::AsLateAsPossible, _))) && own > 0 {
            if let (Some(s), Some(f)) = (es.get_mut(i), ef.get_mut(i)) {
                *s += own;
                *f += own;
            }
            own = 0;
        }
        if let Some(slot) = free.get_mut(i) {
            *slot = own;
        }
    }

    // Dates: the project's working days, once.
    let mut days = Vec::with_capacity(usize::try_from(finish.max(1)).unwrap_or(0).saturating_add(1));
    let mut d = first;
    days.push(d);
    let highest = finish.max(lf.iter().copied().max().unwrap_or(0));
    for _ in 0..highest {
        d = project.calendar.add_working_days(d, 1)?;
        days.push(d);
    }
    // A time before the first working day (a late start pulled back by a lead): count back.
    let date_at = |t: i64| -> Result<Date, ScheduleError> {
        if t >= 0 {
            usize::try_from(t).ok().and_then(|t| days.get(t).copied()).ok_or(ScheduleError::TooLong)
        } else {
            Ok(project.calendar.add_working_days(first, t)?)
        }
    };
    // A start is the morning of working day `t`, a finish the evening of day `t − 1`. A start
    // milestone (or a task of no duration) is the morning of its day; a finish milestone the
    // evening before, except at the project's very start, where there is no day before.
    let start_date = |t: i64, kind: ActivityKind| match kind {
        ActivityKind::FinishMilestone if t > 0 => date_at(t - 1),
        _ => date_at(t),
    };
    let finish_date = |t: i64, kind: ActivityKind, d: i64| match kind {
        ActivityKind::FinishMilestone if t > 0 => date_at(t - 1),
        ActivityKind::Task if d > 0 => date_at(t - 1),
        _ => date_at(t),
    };

    let mut out = Vec::with_capacity(n);
    for (i, a) in acts.iter().enumerate() {
        let (e_s, e_f, l_f) = (es.get(i).copied().unwrap_or(0), ef.get(i).copied().unwrap_or(0), lf.get(i).copied().unwrap_or(end));
        let d = a.working_days();
        let l_s = l_f - d;
        let total = l_f - e_f;
        out.push(Scheduled {
            id: a.id.clone(),
            early_start: start_date(e_s, a.kind)?,
            early_finish: finish_date(e_f, a.kind, d)?,
            late_start: start_date(l_s, a.kind)?,
            late_finish: finish_date(l_f, a.kind, d)?,
            total_float: total,
            free_float: free.get(i).copied().unwrap_or(0),
            critical: total <= 0,
        });
    }
    Ok(Schedule { activities: out, finish: date_at(finish - i64::from(finish > 0))? })
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
