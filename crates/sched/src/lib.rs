//! The critical path method (CPM) over a project network: a forward pass for the early dates,
//! a backward pass for the late ones, and the floats between them.
//!
//! Time runs in minutes (see `clock`). Each activity works on its own calendar: it starts at a
//! working minute, takes its duration in working minutes (days of duration at the calendar's
//! hours a day), and finishes at the end of its last. Each relationship bounds the successor `s` by its predecessor `p`, the lag counted in
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
mod explain;

use std::collections::{HashMap, VecDeque};

use mkperth_core::{
    ActivityKind, Calendar, CalendarError, Constraint, CriticalBy, Date, FloatKind, LagCalendar, MINUTES_PER_DAY, ProgressMode, Project,
    RelationshipType,
};
use mkperth_i18n::tr;
use thiserror::Error;

use clock::Clock;
pub use explain::{Bound, Explanation, Moment, Slack, Source, constraint_name};

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
    /// The activity, and the parent it names that the project doesn't have.
    #[error("{}", tr!("err-unknown-parent", id = .0.as_str(), parent = .1.as_str()))]
    UnknownParent(String, String),
    /// The activities of a loop of parents, each under the next.
    #[error("{}", tr!("err-parent-loop", ids = .0.join(" → ")))]
    ParentLoop(Vec<String>),
    #[error("{}", tr!("err-milestone-parent", id = .0.as_str()))]
    MilestoneParent(String),
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

/// Where an activity stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    NotStarted,
    InProgress,
    Completed,
}

/// One activity's dates and floats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheduled {
    pub id: String,
    pub early_start: Date,
    pub early_finish: Date,
    pub late_start: Date,
    pub late_finish: Date,
    /// In whole working days of the activity's calendar (towards zero), and in working minutes.
    pub total_float: i64,
    pub free_float: i64,
    pub total_float_minutes: i64,
    pub free_float_minutes: i64,
    /// By the project's rule: total float at or below its threshold, or on the longest path.
    pub critical: bool,
    /// On the longest path, whatever the rule: a chain of driving relationships back from the
    /// activities that finish the project.
    pub longest_path: bool,
    pub status: Status,
    /// Working minutes left (none when completed).
    pub remaining_minutes: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    /// In the project's order.
    pub activities: Vec<Scheduled>,
    /// The latest early finish among the activities (the project's start when it has none).
    pub finish: Date,
    /// The relationships left out because they reach a summary (by index in the project's
    /// list): a summary is the sum of its children, not work of its own.
    pub ignored: Vec<usize>,
}

/// A relationship by activity indexes, with the calendar its lag counts on.
struct Tie {
    from: usize,
    to: usize,
    kind: RelationshipType,
    /// In days, as the relationship says it (for explanations), and in working minutes.
    lag: i64,
    lag_minutes: i64,
    lag_calendar: usize,
}

/// The constraint's date as a time on the activity's calendar: the start of its first work
/// period for a start, the end of the last working time on or before it for a finish.
fn constraint_time(c: Constraint, clock: &Clock) -> Result<Option<i64>, ScheduleError> {
    let midnight = |d: Date| i64::from(d.days()) * MINUTES_PER_DAY;
    Ok(match c {
        Constraint::StartOnOrAfter(d) | Constraint::StartOnOrBefore(d) | Constraint::StartOn(d) | Constraint::MandatoryStart(d) => {
            let first = clock.periods(i64::from(d.days()))?.first().map_or(0, |(a, _)| i64::from(*a));
            Some(midnight(d) + first)
        }
        Constraint::FinishOnOrAfter(d) | Constraint::FinishOnOrBefore(d) | Constraint::FinishOn(d) | Constraint::MandatoryFinish(d) => {
            Some(clock.previous_finish(midnight(d) + MINUTES_PER_DAY)?)
        }
        Constraint::AsLateAsPossible => None,
    })
}

/// Schedules `project` by the critical path method.
pub fn schedule(project: &Project) -> Result<Schedule, ScheduleError> {
    Ok(run(project, None)?.0)
}

/// Schedules `project` and explains the dates of the activity `id`: every bound on it in each
/// pass, which one won, where its calendar placed it, and its floats.
pub fn explain(project: &Project, id: &str) -> Result<Explanation, ScheduleError> {
    run(project, Some(id))?.1.ok_or_else(|| ScheduleError::UnknownActivity(id.to_string()))
}

/// The scheduler, recording the bounds on `trace` as it goes.
fn run(project: &Project, trace: Option<&str>) -> Result<(Schedule, Option<Explanation>), ScheduleError> {
    let acts = &project.activities;
    if acts.len() > MAX_ACTIVITIES {
        return Err(ScheduleError::TooMany(tr!("what-activities"), MAX_ACTIVITIES));
    }
    if project.relationships.len() > MAX_RELATIONSHIPS {
        return Err(ScheduleError::TooMany(tr!("what-relationships"), MAX_RELATIONSHIPS));
    }
    // The calendars: the project's (0), the others it lists, and 24 hours for lags (last).
    let every_day = Calendar::around_the_clock();
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
        if a.working_days() > MAX_SPAN || a.duration_minutes.is_some_and(|m| !(0..=MAX_SPAN * MINUTES_PER_DAY).contains(&m)) {
            return Err(ScheduleError::TooLong);
        }
        on.push(match &a.calendar {
            None => 0,
            Some(name) if *name == project.calendar.name => 0,
            Some(name) => named(name).ok_or_else(|| ScheduleError::UnknownCalendar(a.id.clone(), name.clone()))?,
        });
    }
    // The WBS: each activity's parent, which activities are summaries, and no loops.
    let mut parent: Vec<Option<usize>> = Vec::with_capacity(acts.len());
    for a in acts {
        parent.push(match a.parent.as_deref() {
            None | Some("") => None,
            Some(p) => Some(index.get(p).copied().ok_or_else(|| ScheduleError::UnknownParent(a.id.clone(), p.to_string()))?),
        });
    }
    let mut summary = vec![false; acts.len()];
    for p in parent.iter().flatten() {
        if let Some(s) = summary.get_mut(*p) {
            *s = true;
        }
    }
    for (i, a) in acts.iter().enumerate() {
        if summary.get(i) == Some(&true) && a.kind.is_milestone() {
            return Err(ScheduleError::MilestoneParent(a.id.clone()));
        }
        // Climbing from any activity reaches the top within as many steps as there are
        // activities, or the parents go round.
        let mut at = i;
        for _ in 0..=acts.len() {
            match parent.get(at).copied().flatten() {
                None => break,
                Some(p) if p == i => {
                    let mut ids = vec![a.id.clone()];
                    let mut k = parent.get(i).copied().flatten();
                    while let Some(j) = k.filter(|j| *j != i) {
                        ids.push(acts.get(j).map_or_else(String::new, |x| x.id.clone()));
                        k = parent.get(j).copied().flatten();
                        if ids.len() > acts.len() {
                            break;
                        }
                    }
                    ids.push(a.id.clone());
                    return Err(ScheduleError::ParentLoop(ids));
                }
                Some(p) => at = p,
            }
        }
    }
    let is_summary = |i: usize| summary.get(i) == Some(&true);
    let cal_of = |i: usize| on.get(i).copied().unwrap_or(0);
    let clock = |k: usize| Clock { calendar: calendars.get(k).copied().unwrap_or(&project.calendar) };
    let find = |id: &str| index.get(id).copied().ok_or_else(|| ScheduleError::UnknownActivity(id.to_string()));
    let target = trace.map(find).transpose()?;
    let traced = |i: usize| target == Some(i);
    let id_of = |i: usize| acts.get(i).map_or_else(String::new, |a| a.id.clone());
    // Moments on the traced activity's calendar: a time of day named only when it isn't the
    // start of the day's work (a morning) or its end (an evening).
    let traced_calendar = target.and_then(|i| on.get(i).copied()).and_then(|k| calendars.get(k).copied()).unwrap_or(&project.calendar);
    // A finish at the start of a work period is the end of the last one, the same moment: shown
    // as that evening. Starts stay as they were asked for, so the calendar's placing shows.
    let at = |t: i64, evening: bool| -> Result<Moment, ScheduleError> {
        let t = if evening { Clock { calendar: traced_calendar }.previous_finish(t)? } else { t };
        explain::moment(t, evening, traced_calendar).ok_or(ScheduleError::TooLong)
    };
    let (mut early_bounds, mut late_bounds, mut slacks) = (Vec::new(), Vec::new(), Vec::new());
    let (mut earliest, mut latest_seen, mut held_late) = (None, None, 0);
    let mut ties = Vec::with_capacity(project.relationships.len());
    let mut ignored = Vec::new();
    for (k, r) in project.relationships.iter().enumerate() {
        let (from, to) = (find(&r.predecessor)?, find(&r.successor)?);
        if from == to {
            return Err(ScheduleError::SelfRelationship(r.predecessor.clone()));
        }
        if is_summary(from) || is_summary(to) {
            ignored.push(k);
            continue;
        }
        if r.lag.abs() > MAX_SPAN || r.lag_minutes.is_some_and(|m| m.abs() > MAX_SPAN * MINUTES_PER_DAY) {
            return Err(ScheduleError::TooLong);
        }
        let lag_calendar = match project.lag_calendar {
            LagCalendar::Predecessor => cal_of(from),
            LagCalendar::Successor => cal_of(to),
            LagCalendar::TwentyFourHour => twenty_four,
            LagCalendar::Project => 0,
        };
        let day = calendars.get(lag_calendar).map_or(480, |c| c.day_minutes());
        ties.push(Tie { from, to, kind: r.kind, lag: r.lag, lag_minutes: r.lag_minutes.unwrap_or(r.lag.saturating_mul(day)), lag_calendar });
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
    // A summary takes no time and holds no constraint of its own: its dates are its children's.
    let day_minutes = |i: usize| calendars.get(cal_of(i)).map_or(480, |c| c.day_minutes());
    let full = |i: usize| if is_summary(i) { 0 } else { acts.get(i).map_or(0, |a| a.working_minutes(day_minutes(i))) };
    let kind = |i: usize| acts.get(i).map_or(ActivityKind::Task, |a| a.kind);
    // Both constraints of an activity hold; the primary first.
    let constraints =
        |i: usize| acts.get(i).filter(|_| !is_summary(i)).map_or([None, None], |a| [a.constraint, a.secondary_constraint]).into_iter().flatten();
    let origin = i64::from(project.start.days()) * MINUTES_PER_DAY;
    // Nothing not yet started, and no remaining work, before the data date.
    let data = project.data_date.map(|d| i64::from(d.days()) * MINUTES_PER_DAY);
    let floor = data.map_or(origin, |d| d.max(origin));
    let status = |i: usize| match acts.get(i).filter(|_| !is_summary(i)) {
        Some(a) if a.actual_finish.is_some() => Status::Completed,
        Some(a) if a.actual_start.is_some() => Status::InProgress,
        _ => Status::NotStarted,
    };
    // Every time stays within the span of the project's start: far from overflow and from the
    // ends of the calendar.
    let bounded = |t: i64| if (t - origin).abs() > MAX_SPAN * 2 * MINUTES_PER_DAY { Err(ScheduleError::TooLong) } else { Ok(t) };
    // An activity's actual start and finish as times: the first working minute of its start day,
    // the end of the work of its finish day (its start day when only a finish was given).
    let actual = |i: usize| -> Result<Option<(i64, Option<i64>)>, ScheduleError> {
        let Some(a) = acts.get(i).filter(|_| !is_summary(i)) else { return Ok(None) };
        let c = clock(cal_of(i));
        let Some(start_day) = a.actual_start.or(a.actual_finish) else { return Ok(None) };
        let s = c.next_start(i64::from(start_day.days()) * MINUTES_PER_DAY)?;
        let f = match a.actual_finish {
            Some(d) => Some(c.previous_finish((i64::from(d.days()) + 1) * MINUTES_PER_DAY)?.max(s)),
            None => None,
        };
        Ok(Some((s, f)))
    };
    // The work left: none once completed; for an activity under way, what it says, else its
    // duration less the work from its actual start to the data date.
    let mut remaining = vec![0i64; acts.len()];
    for i in 0..acts.len() {
        let left = match (status(i), actual(i)?) {
            (Status::Completed, _) => 0,
            (Status::InProgress, Some((s, _))) => match acts.get(i).and_then(|a| a.remaining_minutes) {
                Some(m) => m.clamp(0, MAX_SPAN * MINUTES_PER_DAY),
                None => (full(i) - data.map_or(Ok(0), |d| clock(cal_of(i)).between(s, d.max(s)))?).max(0),
            },
            _ => full(i),
        };
        if let Some(slot) = remaining.get_mut(i) {
            *slot = left;
        }
    }
    // A started activity's relationships from its start count from its actual start.
    let mut begun = vec![None; acts.len()];
    for (i, slot) in begun.iter_mut().enumerate() {
        if status(i) != Status::NotStarted {
            *slot = actual(i)?.map(|(s, _)| s);
        }
    }
    let begun = |i: usize, es: i64| begun.get(i).copied().flatten().unwrap_or(es);
    // What the passes schedule: the work left.
    let dur = |i: usize| remaining.get(i).copied().unwrap_or(0);
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
    // The latest finish at or before `t`; a start milestone (or a task of no duration) at the next
    // working minute when no work lies between, which is the same moment.
    let place_finish = |i: usize, t: i64| -> Result<i64, ScheduleError> {
        let c = clock(cal_of(i));
        let f = c.previous_finish(t)?;
        if kind(i) == ActivityKind::StartMilestone || (kind(i) == ActivityKind::Task && dur(i) == 0) {
            let s = c.next_start(f)?;
            return Ok(if s <= t { s } else { f });
        }
        Ok(f)
    };
    let start_of = |i: usize, lf: i64| -> Result<i64, ScheduleError> { clock(cal_of(i)).retreat(lf, dur(i)) };
    // The latest finish that starts no later than `t`: a relationship's bound counts moments
    // with no work between as one; a constraint's date is strict.
    let finish_for_start = |i: usize, t: i64| -> Result<i64, ScheduleError> {
        let c = clock(cal_of(i));
        if dur(i) == 0 { Ok(t) } else { c.advance(c.previous_start(t)?, dur(i)) }
    };
    let finish_for_start_by = |i: usize, t: i64| -> Result<i64, ScheduleError> {
        let c = clock(cal_of(i));
        if dur(i) == 0 { Ok(t) } else { c.advance(c.start_by(t)?, dur(i)) }
    };
    // The bound a tie puts on its successor, from the predecessor's early dates: a start for
    // FS and SS, a finish for FF and SF.
    let forward_bound = |t: &Tie, p_es: i64, p_ef: i64| -> Result<i64, ScheduleError> {
        let lag = clock(t.lag_calendar);
        bounded(match t.kind {
            RelationshipType::FinishToStart | RelationshipType::FinishToFinish => lag.shift(p_ef, t.lag_minutes)?,
            RelationshipType::StartToStart | RelationshipType::StartToFinish => lag.shift(p_es, t.lag_minutes)?,
        })
    };

    // Forward pass.
    let mut es = vec![origin; n];
    let mut ef = vec![origin; n];
    for &i in &order {
        // A completed activity stays where it happened.
        if status(i) == Status::Completed
            && let Some((s, Some(f))) = actual(i)?
        {
            if let (Some(a), Some(b)) = (es.get_mut(i), ef.get_mut(i)) {
                *a = s;
                *b = f;
            }
            continue;
        }
        let mut start = floor;
        if traced(i) {
            let source = if data.is_some_and(|d| d > origin) { Source::DataDate } else { Source::ProjectStart };
            early_bounds.push(Bound { source, moment: at(floor, false)?, via: None, overrides: false });
        }
        for &k in incoming.get(i).map_or(&[][..], Vec::as_slice) {
            let Some(t) = ties.get(k) else { continue };
            let bound = forward_bound(t, begun(t.from, es.get(t.from).copied().unwrap_or(origin)), ef.get(t.from).copied().unwrap_or(origin))?;
            let (s, via) = match t.kind {
                RelationshipType::FinishToStart | RelationshipType::StartToStart => (bound, None),
                RelationshipType::FinishToFinish | RelationshipType::StartToFinish => (start_for_finish(i, bound)?, Some(bound)),
            };
            if traced(i) {
                let source = Source::Relationship { other: id_of(t.from), kind: t.kind, lag: t.lag };
                early_bounds.push(Bound { source, moment: at(s, false)?, via: via.map(|v| at(v, true)).transpose()?, overrides: false });
            }
            start = start.max(s);
        }
        // Early-date constraints: no earlier than their date; a mandatory one, on it. Once
        // started, an activity's start is what happened.
        for c in constraints(i).filter(|_| status(i) == Status::NotStarted) {
            let when = constraint_time(c, &clock(cal_of(i)))?.map(bounded).transpose()?;
            let (candidate, via, overrides) = match (c, when) {
                (Constraint::StartOnOrAfter(_) | Constraint::StartOn(_), Some(t)) => (Some(t), None, false),
                (Constraint::FinishOnOrAfter(_) | Constraint::FinishOn(_), Some(t)) => (Some(start_for_finish(i, t)?), Some(t), false),
                (Constraint::MandatoryStart(_), Some(t)) => (Some(t.max(floor)), None, true),
                (Constraint::MandatoryFinish(_), Some(t)) => (Some(start_for_finish(i, t)?.max(floor)), Some(t), true),
                _ => (None, None, false),
            };
            if let Some(x) = candidate {
                start = if overrides { x } else { start.max(x) };
                if traced(i) {
                    early_bounds.push(Bound {
                        source: Source::Constraint(c),
                        moment: at(x, false)?,
                        via: via.map(|v| at(v, true)).transpose()?,
                        overrides,
                    });
                }
            }
        }
        // Under way: the work left goes on from the data date, after its predecessors with
        // retained logic, at once with progress override; without a data date, from its start.
        if status(i) == Status::InProgress {
            start = match (data, project.progress_mode) {
                (None, _) => actual(i)?.map_or(start, |(s, _)| s),
                (Some(_), ProgressMode::ProgressOverride) => floor,
                (Some(_), ProgressMode::RetainedLogic) => start,
            };
        }
        if traced(i) {
            earliest = Some(start);
        }
        let s = bounded(place_start(i, start)?)?;
        let f = bounded(finish_of(i, s)?)?;
        if let (Some(a), Some(b)) = (es.get_mut(i), ef.get_mut(i)) {
            *a = s;
            *b = f;
        }
    }
    let finish = (0..n).filter(|&i| !is_summary(i)).filter_map(|i| ef.get(i).copied()).max().unwrap_or(origin);

    // Backward pass, from the evening the project must finish by, or from its early finish.
    let end = match project.must_finish_by {
        Some(date) => bounded((i64::from(date.days()) + 1) * MINUTES_PER_DAY)?,
        None => finish,
    };
    let mut ls = vec![end; n];
    let mut lf = vec![end; n];
    for &i in order.iter().rev() {
        if status(i) == Status::Completed {
            if let (Some(a), Some(b), Some(s), Some(f)) = (ls.get_mut(i), lf.get_mut(i), es.get(i), ef.get(i)) {
                *a = *s;
                *b = *f;
            }
            continue;
        }
        let mut latest = end;
        if traced(i) {
            let source = if project.must_finish_by.is_some() { Source::MustFinishBy } else { Source::ProjectFinish };
            late_bounds.push(Bound { source, moment: at(end, true)?, via: None, overrides: false });
        }
        for &k in outgoing.get(i).map_or(&[][..], Vec::as_slice) {
            let Some(t) = ties.get(k) else { continue };
            let lag = clock(t.lag_calendar);
            let (s_ls, s_lf) = (ls.get(t.to).copied().unwrap_or(end), lf.get(t.to).copied().unwrap_or(end));
            let (f, via) = match t.kind {
                RelationshipType::FinishToStart => (lag.shift(s_ls, -t.lag_minutes)?, None),
                RelationshipType::StartToStart => {
                    let v = lag.shift(s_ls, -t.lag_minutes)?;
                    (finish_for_start(i, v)?, Some(v))
                }
                RelationshipType::FinishToFinish => (lag.shift(s_lf, -t.lag_minutes)?, None),
                RelationshipType::StartToFinish => {
                    let v = lag.shift(s_lf, -t.lag_minutes)?;
                    (finish_for_start(i, v)?, Some(v))
                }
            };
            if traced(i) {
                let source = Source::Relationship { other: id_of(t.to), kind: t.kind, lag: t.lag };
                late_bounds.push(Bound { source, moment: at(f, true)?, via: via.map(|v| at(v, false)).transpose()?, overrides: false });
            }
            latest = latest.min(bounded(f)?);
        }
        // Late-date constraints: no later than their date; a mandatory one, on it.
        for c in constraints(i) {
            let when = constraint_time(c, &clock(cal_of(i)))?.map(bounded).transpose()?;
            let (candidate, via, overrides) = match (c, when) {
                (Constraint::StartOnOrBefore(_) | Constraint::StartOn(_), Some(t)) => (Some(finish_for_start_by(i, t)?), Some(t), false),
                (Constraint::FinishOnOrBefore(_) | Constraint::FinishOn(_), Some(t)) => (Some(t), None, false),
                (Constraint::MandatoryStart(_), Some(t)) => (Some(finish_for_start_by(i, t)?), Some(t), true),
                (Constraint::MandatoryFinish(_), Some(t)) => (Some(t), None, true),
                _ => (None, None, false),
            };
            if let Some(x) = candidate {
                latest = if overrides { x } else { latest.min(x) };
                if traced(i) {
                    late_bounds.push(Bound {
                        source: Source::Constraint(c),
                        moment: at(x, true)?,
                        via: via.map(|v| at(v, false)).transpose()?,
                        overrides,
                    });
                }
            }
        }
        if traced(i) {
            latest_seen = Some(latest);
        }
        let f = bounded(place_finish(i, latest)?)?;
        let s = bounded(start_of(i, f)?)?;
        if let (Some(a), Some(b)) = (ls.get_mut(i), lf.get_mut(i)) {
            *a = s;
            *b = f;
        }
    }

    // Working minutes between two times, by each calendar's running count of whole days over the
    // schedule's span plus the part of the day: one pass per calendar, not one per activity.
    let lo = (es.iter().chain(&ls).copied().min().unwrap_or(origin).min(origin)).div_euclid(MINUTES_PER_DAY) - 1;
    let hi = (ef.iter().chain(&lf).copied().max().unwrap_or(origin).max(end)).div_euclid(MINUTES_PER_DAY) + 2;
    let span = usize::try_from(hi - lo).map_err(|_| ScheduleError::TooLong)?;
    let mut counts: Vec<Vec<i64>> = Vec::with_capacity(calendars.len());
    for k in 0..calendars.len() {
        let c = clock(k);
        let mut running = Vec::with_capacity(span + 1);
        let mut total = 0i64;
        running.push(0);
        for day in lo..hi {
            total += c.day_work(day)?;
            running.push(total);
        }
        counts.push(running);
    }
    let between = |k: usize, a: i64, b: i64| -> Result<i64, ScheduleError> {
        let c = clock(k);
        let at = |t: i64| -> Result<Option<i64>, ScheduleError> {
            let day = t.div_euclid(MINUTES_PER_DAY);
            Ok(match usize::try_from(day - lo).ok().and_then(|x| counts.get(k).and_then(|r| r.get(x)).copied()) {
                Some(whole) => Some(whole + c.worked_before(t)?),
                None => None,
            })
        };
        match (at(a)?, at(b)?) {
            (Some(x), Some(y)) => Ok(y - x),
            _ => c.between(a, b),
        }
    };
    // Working minutes as whole working days of an activity's calendar, towards zero.
    let days = |i: usize, minutes: i64| minutes / day_minutes(i).max(1);

    // Free float, successors first, so an activity held as late as possible moves by its own
    // before its predecessors measure theirs against it.
    let mut free = vec![0i64; n];
    for &i in order.iter().rev() {
        let c = clock(cal_of(i));
        let (e_s, e_f) = (es.get(i).copied().unwrap_or(origin), ef.get(i).copied().unwrap_or(origin));
        let mut slack = between(cal_of(i), e_f, finish)?;
        if traced(i) {
            slacks.push(Slack { source: Source::ProjectFinish, minutes: slack });
        }
        for &k in outgoing.get(i).map_or(&[][..], Vec::as_slice) {
            let Some(t) = ties.get(k) else { continue };
            let bound = forward_bound(t, begun(i, e_s), e_f)?;
            let actual = match t.kind {
                RelationshipType::FinishToStart | RelationshipType::StartToStart => es.get(t.to).copied().unwrap_or(origin),
                RelationshipType::FinishToFinish | RelationshipType::StartToFinish => ef.get(t.to).copied().unwrap_or(origin),
            };
            let gap = between(cal_of(i), bound, actual)?;
            if traced(i) {
                slacks.push(Slack { source: Source::Relationship { other: id_of(t.to), kind: t.kind, lag: t.lag }, minutes: gap });
            }
            slack = slack.min(gap);
        }
        let total = between(cal_of(i), e_f, lf.get(i).copied().unwrap_or(end))?;
        let mut own = if status(i) == Status::Completed { 0 } else { slack.min(total).max(0) };
        if status(i) == Status::NotStarted && constraints(i).any(|c| c == Constraint::AsLateAsPossible) && own > 0 {
            let moved = place_start(i, c.advance(e_s, own)?)?;
            let s = if dur(i) == 0 { moved } else { c.retreat(c.advance(e_f, own)?, dur(i))? };
            let f = finish_of(i, s)?;
            if let (Some(a), Some(b)) = (es.get_mut(i), ef.get_mut(i)) {
                *a = s;
                *b = f;
            }
            if traced(i) {
                held_late = own;
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
    let date = |t: i64| i32::try_from(t.div_euclid(MINUTES_PER_DAY)).ok().and_then(Date::from_days).ok_or(ScheduleError::TooLong);
    let start_date = |i: usize, t: i64| if kind(i) == ActivityKind::FinishMilestone { date(t - 1) } else { date(t) };
    let finish_date = |i: usize, t: i64| match kind(i) {
        ActivityKind::FinishMilestone => date(t - 1),
        ActivityKind::Task if dur(i) > 0 => date(t - 1),
        _ => date(t),
    };
    // The longest path: from the activities that finish the project, back along driving
    // relationships (the ones that set their successor's early start), successors first.
    let mut longest = vec![false; n];
    for i in 0..n {
        if !is_summary(i)
            && status(i) != Status::Completed
            && ef.get(i) == Some(&finish)
            && let Some(slot) = longest.get_mut(i)
        {
            *slot = true;
        }
    }
    for &i in order.iter().rev() {
        if longest.get(i) != Some(&true) {
            continue;
        }
        let at = es.get(i).copied().unwrap_or(origin);
        for &k in incoming.get(i).map_or(&[][..], Vec::as_slice) {
            let Some(t) = ties.get(k) else { continue };
            let bound = forward_bound(t, begun(t.from, es.get(t.from).copied().unwrap_or(origin)), ef.get(t.from).copied().unwrap_or(origin))?;
            let candidate = match t.kind {
                RelationshipType::FinishToStart | RelationshipType::StartToStart => bound,
                RelationshipType::FinishToFinish | RelationshipType::StartToFinish => start_for_finish(i, bound)?,
            };
            if place_start(i, candidate)? == at
                && status(t.from) != Status::Completed
                && let Some(slot) = longest.get_mut(t.from)
            {
                *slot = true;
            }
        }
    }
    let critical = |i: usize, total: i64, longest: bool| match project.critical_by {
        CriticalBy::TotalFloat(threshold) => total <= threshold.saturating_mul(day_minutes(i)),
        CriticalBy::LongestPath => longest,
    };

    let mut out = Vec::with_capacity(n);
    for (i, a) in acts.iter().enumerate() {
        let (e_s, e_f) = (es.get(i).copied().unwrap_or(origin), ef.get(i).copied().unwrap_or(origin));
        let (l_s, l_f) = (ls.get(i).copied().unwrap_or(end), lf.get(i).copied().unwrap_or(end));
        let finish_float = between(cal_of(i), e_f, l_f)?;
        let start_float = between(cal_of(i), e_s, l_s)?;
        let total = match project.float_kind {
            FloatKind::Finish => finish_float,
            FloatKind::Start => start_float,
            FloatKind::Smallest => finish_float.min(start_float),
        };
        let on_longest = longest.get(i).copied().unwrap_or(false);
        let state = status(i);
        // A completed activity has no float and isn't critical; a started one shows its actual
        // start as both its starts.
        let total = if state == Status::Completed { 0 } else { total };
        let actual_start = actual(i)?.map(|(s, _)| s);
        let (shown_es, shown_ls) = match (state, actual_start) {
            (Status::NotStarted, _) | (_, None) => (e_s, l_s),
            (_, Some(s)) => (s, s),
        };
        let early_finish = finish_date(i, e_f)?;
        out.push(Scheduled {
            id: a.id.clone(),
            early_start: start_date(i, shown_es)?,
            early_finish,
            late_start: start_date(i, shown_ls)?,
            late_finish: finish_date(i, l_f)?,
            total_float: days(i, total),
            free_float: days(i, free.get(i).copied().unwrap_or(0)),
            total_float_minutes: total,
            free_float_minutes: free.get(i).copied().unwrap_or(0),
            critical: state != Status::Completed && critical(i, total, on_longest),
            longest_path: on_longest,
            status: state,
            remaining_minutes: dur(i),
        });
    }
    // Summaries: from their children's dates, deepest first so a summary under a summary is
    // ready when its parent reads it.
    let depth = |i: usize| {
        let (mut d, mut at) = (0usize, i);
        while let Some(p) = parent.get(at).copied().flatten() {
            d += 1;
            at = p;
            if d > n {
                break;
            }
        }
        d
    };
    let mut summaries: Vec<usize> = (0..n).filter(|&i| is_summary(i)).collect();
    summaries.sort_by_key(|&i| std::cmp::Reverse(depth(i)));
    let mut bounds: Vec<Option<(i64, i64)>> =
        (0..n).map(|i| (!is_summary(i)).then(|| (ef.get(i).copied().unwrap_or(origin), lf.get(i).copied().unwrap_or(end)))).collect();
    for &s in &summaries {
        let children: Vec<usize> = (0..n).filter(|&c| parent.get(c).copied().flatten() == Some(s)).collect();
        let pick = |f: fn(&Scheduled) -> Date, max: bool| {
            let dates = children.iter().filter_map(|&c| out.get(c)).map(f);
            if max { dates.max() } else { dates.min() }
        };
        let (Some(es_d), Some(ef_d), Some(ls_d), Some(lf_d)) =
            (pick(|a| a.early_start, false), pick(|a| a.early_finish, true), pick(|a| a.late_start, false), pick(|a| a.late_finish, true))
        else {
            continue;
        };
        let e = children.iter().filter_map(|&c| bounds.get(c).copied().flatten()).map(|b| b.0).max().unwrap_or(origin);
        let l = children.iter().filter_map(|&c| bounds.get(c).copied().flatten()).map(|b| b.1).max().unwrap_or(end);
        let total = between(cal_of(s), e, l)?;
        let on_longest = children.iter().any(|&c| out.get(c).is_some_and(|x| x.longest_path));
        if let Some(slot) = bounds.get_mut(s) {
            *slot = Some((e, l));
        }
        if let Some(a) = out.get_mut(s) {
            a.early_start = es_d;
            a.early_finish = ef_d;
            a.late_start = ls_d;
            a.late_finish = lf_d;
            a.total_float = days(s, total);
            a.free_float = 0;
            a.total_float_minutes = total;
            a.free_float_minutes = 0;
            a.longest_path = on_longest;
            a.critical = critical(s, total, on_longest);
        }
    }
    let last = out.iter().map(|a| a.early_finish).max().unwrap_or(project.start).max(project.start);
    let explanation = match target {
        None => None,
        Some(i) => {
            let kind = kind(i);
            let start_evening = kind == ActivityKind::FinishMilestone;
            let finish_evening = kind == ActivityKind::FinishMilestone || (kind == ActivityKind::Task && dur(i) > 0) || is_summary(i);
            let get = |v: &[i64], d: i64| v.get(i).copied().unwrap_or(d);
            Some(Explanation {
                id: id_of(i),
                calendar: calendars.get(cal_of(i)).map_or_else(String::new, |c| c.name.clone()),
                duration: dur(i),
                day_minutes: day_minutes(i),
                children: (0..n).filter(|&c| parent.get(c).copied().flatten() == Some(i)).map(id_of).collect(),
                early: early_bounds,
                earliest: at(earliest.unwrap_or(origin), false)?,
                early_start: at(get(&es, origin), start_evening)?,
                early_finish: at(get(&ef, origin), finish_evening)?,
                late: late_bounds,
                latest: at(latest_seen.unwrap_or(end), true)?,
                late_start: at(get(&ls, end), start_evening)?,
                late_finish: at(get(&lf, end), finish_evening)?,
                free: slacks,
                held_late,
                scheduled: out.get(i).cloned().ok_or(ScheduleError::UnknownActivity(id_of(i)))?,
            })
        }
    };
    Ok((Schedule { activities: out, finish: last, ignored }, explanation))
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
