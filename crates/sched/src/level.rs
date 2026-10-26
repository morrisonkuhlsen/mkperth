//! Resource levelling: activities delayed so that no resource is asked, on any day, for more
//! units an hour than it has (its max units). The parallel method of the levelling literature
//! (Kelley, "The critical-path method: resources planning and scheduling", 1963; Davis and
//! Patterson, "A comparison of heuristic and optimum solutions in resource-constrained project
//! scheduling", 1975): day by day from the first over-allocation, the activities on an
//! over-allocated resource that day keep it in an order of priority until it is full; those
//! that would start that day and don't fit wait for the first of the others to finish. An
//! activity already going that day (begun, or started on an earlier day of the plan) isn't
//! interrupted, nor is one with a mandatory date moved. Each delay is a start no earlier than
//! a day, a bound the scheduler explains like any other; then the project is scheduled again
//! and the next over-allocated day looked for, until there is none.
//!
//! Within float, an activity waits only as far as its late start: what can't be resolved that
//! way stays over-allocated, and the project's finish stays where it was.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use mkperth_core::{Constraint, Date, LevelBy, Levelled, Project, ResourceKind};

use crate::usage::{calendar_named, working_days};
use crate::{Schedule, ScheduleError, Status, schedule};

/// The most times levelling schedules the project again before it stops where it got to: fewer
/// for a large project, so that it never takes more than about `MAX_WORK` activities scheduled.
pub const MAX_ROUNDS: usize = 10_000;
const MAX_WORK: usize = 20_000_000;

/// An activity on a resource, by its place in the project's list, and its units an hour.
type Use = (usize, i64);

/// A resource asked, on a day, for more units an hour than it has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overload {
    /// The resource's ID.
    pub resource: String,
    pub day: Date,
    /// What its activities ask for that day, and its limit: hundredths of a unit an hour.
    pub units: i64,
    pub limit: i64,
    /// The activities on it that day, by ID.
    pub activities: Vec<String>,
}

/// What levelling did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Levelling {
    /// Each activity held, by its place in the project's list, and where.
    pub held: Vec<(usize, Levelled)>,
    /// The over-allocations it left, day by day: within float, an activity alone over its
    /// resource's limit, or activities that can't be moved.
    pub left: Vec<Overload>,
    /// It stopped after `MAX_ROUNDS` with over-allocations still to resolve.
    pub stopped: bool,
}

/// The resources levelled: every one listed (all when the list is empty) but materials, which
/// are used up, not busy, and any with no limit.
fn levelled_resources(project: &Project, only: &[String]) -> Vec<usize> {
    project
        .resources
        .iter()
        .enumerate()
        .filter(|(_, r)| r.kind != ResourceKind::Material && r.max_units > 0 && (only.is_empty() || only.contains(&r.id)))
        .map(|(k, _)| k)
        .collect()
}

/// What each of `resources` is asked for, day by day: (day, resource) → each activity on it
/// that day and its units an hour. An activity asks from its early start (the data date, once
/// begun) to its early finish, on the working days of its calendar (its resource's, when it is
/// resource dependent); a milestone, a summary or a completed activity asks nothing.
fn loads(project: &Project, s: &Schedule, resources: &[usize]) -> BTreeMap<(Date, usize), Vec<(usize, i64)>> {
    let index: HashMap<&str, usize> = project.activities.iter().enumerate().map(|(i, a)| (a.id.as_str(), i)).collect();
    let summary: HashSet<&str> = project.activities.iter().filter_map(|a| a.parent.as_deref()).collect();
    let mut out: BTreeMap<(Date, usize), Vec<(usize, i64)>> = BTreeMap::new();
    for &r in resources {
        let Some(resource) = project.resources.get(r) else { continue };
        for x in project.assignments.iter().filter(|x| x.resource.as_deref() == Some(resource.id.as_str())) {
            let Some(&i) = index.get(x.activity.as_str()) else { continue };
            let (Some(a), Some(at)) = (project.activities.get(i), s.activities.get(i)) else { continue };
            if a.kind.is_milestone() || summary.contains(a.id.as_str()) || at.status == Status::Completed {
                continue;
            }
            let own = calendar_named(project, a.calendar.as_deref());
            let rate = x.rate_over(a.working_minutes(own.day_minutes()));
            if rate <= 0 {
                continue;
            }
            let from = match (at.status, project.data_date) {
                (Status::InProgress, Some(d)) => at.early_start.max(d),
                _ => at.early_start,
            };
            let calendar = if a.resource_dependent { calendar_named(project, resource.calendar.as_deref()) } else { own };
            for d in working_days(calendar, from, at.early_finish) {
                out.entry((d, r)).or_default().push((i, rate));
            }
        }
    }
    out
}

/// Every day a resource (not a material) is asked for more than its limit, day by day and, on a
/// day, in the project's order of resources.
pub fn overloads(project: &Project, s: &Schedule) -> Vec<Overload> {
    overloads_of(project, s, &levelled_resources(project, &[]))
}

fn overloads_of(project: &Project, s: &Schedule, resources: &[usize]) -> Vec<Overload> {
    let mut out = Vec::new();
    for ((day, r), users) in loads(project, s, resources) {
        let Some(resource) = project.resources.get(r) else { continue };
        let units: i64 = users.iter().map(|(_, u)| u).sum();
        if units > resource.max_units {
            let activities = users.iter().filter_map(|(i, _)| project.activities.get(*i)).map(|a| a.id.clone()).collect();
            out.push(Overload { resource: resource.id.clone(), day, units, limit: resource.max_units, activities });
        }
    }
    out
}

/// Levels the project's resources by its levelling options: where each activity it delays must
/// start no earlier, from the schedule without any earlier levelling. The same project levels
/// the same way every time.
pub fn level(project: &Project) -> Result<Levelling, ScheduleError> {
    let options = &project.levelling;
    let mut work = project.clone();
    for a in &mut work.activities {
        a.levelled = None;
    }
    let base = schedule(&work)?;
    let resources = levelled_resources(project, &options.resources);
    let n = work.activities.len();
    // An activity that may not wait past where it is that day (within float), by that day.
    let mut pinned: Vec<Option<Date>> = vec![None; n];
    // Over-allocated days given up on: nothing on them can move.
    let mut given_up: BTreeSet<(Date, usize)> = BTreeSet::new();
    let mut stopped = true;
    for _ in 0..MAX_ROUNDS.min(MAX_WORK / n.max(1)).max(1) {
        let s = schedule(&work)?;
        let loads = loads(&work, &s, &resources);
        let limit = |r: usize| work.resources.get(r).map_or(0, |x| x.max_units);
        let over = |given_up: &BTreeSet<(Date, usize)>, key: &(Date, usize), users: &[(usize, i64)]| {
            !given_up.contains(key) && users.iter().map(|(_, u)| u).sum::<i64>() > limit(key.1)
        };
        // The first day some resource is over its limit.
        let Some(day) = loads.iter().find(|(key, users)| over(&given_up, key, users)).map(|((d, _), _)| *d) else {
            stopped = false;
            break;
        };
        // Activities delayed this round no longer ask for anything that day.
        let mut delayed: HashSet<usize> = HashSet::new();
        for &r in &resources {
            let Some(users) = loads.get(&(day, r)) else { continue };
            let users: Vec<(usize, i64)> = users.iter().filter(|(i, _)| !delayed.contains(i)).copied().collect();
            if !over(&given_up, &(day, r), &users) {
                continue;
            }
            let mandatory = |i: usize| {
                work.activities.get(i).is_some_and(|a| {
                    [a.constraint, a.secondary_constraint]
                        .into_iter()
                        .flatten()
                        .any(|c| matches!(c, Constraint::MandatoryStart(_) | Constraint::MandatoryFinish(_)))
                })
            };
            // Only what would start that day can wait.
            let can_wait = |i: usize| {
                s.activities.get(i).is_some_and(|x| x.status == Status::NotStarted && x.early_start == day)
                    && pinned.get(i).copied().flatten() != Some(day)
                    && !mandatory(i)
            };
            let (mut keep, mut wait): (Vec<Use>, Vec<Use>) = users.into_iter().partition(|(i, _)| !can_wait(*i));
            wait.sort_by(|a, b| first(&work, &s, &options.order, a.0, b.0));
            let mut used: i64 = keep.iter().map(|(_, u)| u).sum();
            let mut rejected = Vec::new();
            for (i, u) in wait {
                // Alone on it, an activity keeps it even over the limit: waiting wouldn't help.
                if used == 0 || used + u <= limit(r) {
                    used += u;
                    keep.push((i, u));
                } else {
                    rejected.push(i);
                }
            }
            if rejected.is_empty() {
                given_up.insert((day, r));
                continue;
            }
            // They wait for the first of those keeping it to finish.
            let release = keep
                .iter()
                .filter_map(|(i, _)| s.activities.get(*i).map(|x| x.early_finish))
                .filter(|f| *f >= day)
                .min()
                .and_then(Date::succ)
                .or_else(|| day.succ())
                .unwrap_or(day);
            let id = work.resources.get(r).map_or_else(String::new, |x| x.id.clone());
            for i in rejected {
                if options.within_float && base.activities.get(i).is_none_or(|x| x.late_start < release) {
                    if let Some(p) = pinned.get_mut(i) {
                        *p = Some(day);
                    }
                    continue;
                }
                if let Some(a) = work.activities.get_mut(i) {
                    let start = a.levelled.as_ref().map_or(release, |l| l.start.max(release));
                    a.levelled = Some(Levelled { start, resource: id.clone() });
                    delayed.insert(i);
                }
            }
        }
    }
    let s = schedule(&work)?;
    let held = work.activities.iter().enumerate().filter_map(|(i, a)| a.levelled.clone().map(|l| (i, l))).collect();
    Ok(Levelling { held, left: overloads_of(&work, &s, &resources), stopped })
}

/// Which of activities `a` and `b` keeps a resource first: by each of `order` in turn, then by ID.
fn first(project: &Project, s: &Schedule, order: &[LevelBy], a: usize, b: usize) -> Ordering {
    let (x, y) = (project.activities.get(a), project.activities.get(b));
    let (sx, sy) = (s.activities.get(a), s.activities.get(b));
    // A value's place in its code's list; none, after every value.
    let rank = |v: Option<&mkperth_core::Activity>, name: &str| {
        let value = v.and_then(|v| v.codes.get(name));
        let values = project.classification.code(name).map_or(&[][..], |c| c.values.as_slice());
        value.and_then(|value| values.iter().position(|x| x.code == *value)).unwrap_or(usize::MAX)
    };
    let by = |key: &LevelBy| match key {
        LevelBy::Priority => x.map(|v| v.levelling_priority).cmp(&y.map(|v| v.levelling_priority)),
        LevelBy::TotalFloat => sx.map(|v| v.total_float_minutes).cmp(&sy.map(|v| v.total_float_minutes)),
        LevelBy::EarlyStart => sx.map(|v| v.early_start).cmp(&sy.map(|v| v.early_start)),
        LevelBy::LateStart => sx.map(|v| v.late_start).cmp(&sy.map(|v| v.late_start)),
        LevelBy::Duration => sx.map(|v| v.remaining_minutes).cmp(&sy.map(|v| v.remaining_minutes)),
        LevelBy::Code(name) => rank(x, name).cmp(&rank(y, name)),
    };
    order.iter().fold(Ordering::Equal, |o, k| o.then_with(|| by(k))).then_with(|| x.map(|v| &v.id).cmp(&y.map(|v| &v.id)))
}

#[cfg(test)]
mod tests {
    use mkperth_core::{Activity, Assignment, Calendar, LevellingPriority, Relationship, RelationshipType, Resource, UNIT};

    use super::*;

    fn day(y: i32, m: u32, d: u32) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    /// From Monday 5 October 2026, Monday to Friday: A takes 3 days, B 2, both on R (one person,
    /// one unit an hour), each at a unit an hour (A's 24 hours over its 24, B's 16 over 16); C
    /// takes a day after B.
    fn two_on_one() -> Project {
        let activity = |id: &str, d: i64| Activity { id: id.into(), name: id.into(), duration: d, ..Activity::default() };
        let on =
            |id: &str, hours: i64| Assignment { activity: id.into(), resource: Some("R".into()), planned: hours * UNIT, ..Assignment::default() };
        Project {
            name: "levelling".into(),
            start: day(2026, 10, 5),
            calendar: Calendar::default(),
            calendars: Vec::new(),
            lag_calendar: Default::default(),
            float_kind: Default::default(),
            critical_by: Default::default(),
            data_date: None,
            progress_mode: Default::default(),
            must_finish_by: None,
            activities: vec![activity("A", 3), activity("B", 2), activity("C", 1)],
            relationships: vec![Relationship {
                predecessor: "B".into(),
                successor: "C".into(),
                kind: RelationshipType::FinishToStart,
                lag: 0,
                lag_minutes: None,
                lag_kind: Default::default(),
            }],
            bands: Vec::new(),
            markers: Vec::new(),
            month_marks: Vec::new(),
            resources: vec![Resource { id: "R".into(), name: "R".into(), ..Resource::default() }],
            roles: Vec::new(),
            assignments: vec![on("A", 24), on("B", 16)],
            comments: Vec::new(),
            levelling: Default::default(),
            baselines: Vec::new(),
            baseline_in_use: None,
            expenses: Vec::new(),
            cost_accounts: Vec::new(),
            currency: String::new(),
            budget: Default::default(),
            earned_value: Default::default(),
            classification: Default::default(),
            external: Vec::new(),
            organisation: Default::default(),
            extra: Default::default(),
        }
    }

    /// The project with what levelling held applied, scheduled.
    fn levelled(p: &Project) -> (Levelling, Project, Schedule) {
        let result = level(p).unwrap();
        let mut q = p.clone();
        for (i, l) in &result.held {
            q.activities[*i].levelled = Some(l.clone());
        }
        let s = schedule(&q).unwrap();
        (result, q, s)
    }

    /// Unlevelled, A is the 5th to the 7th (no float: it finishes the project), B the 5th and
    /// 6th and C the 7th (a day of float each). On the 5th R is asked for 2 an hour, 1 over. By
    /// float A keeps R; B waits for A to finish, the evening of the 7th: B is Thursday the 8th
    /// and Friday the 9th, C Monday the 12th, which is the project's finish now.
    #[test]
    fn the_one_with_less_float_keeps_the_resource() {
        let p = two_on_one();
        let before = schedule(&p).unwrap();
        assert_eq!(
            overloads(&p, &before).iter().map(|o| (o.day, o.units, o.limit)).collect::<Vec<_>>(),
            [(day(2026, 10, 5), 200, 100), (day(2026, 10, 6), 200, 100)]
        );
        let (result, q, s) = levelled(&p);
        assert_eq!(result.held, [(1, Levelled { start: day(2026, 10, 8), resource: "R".into() })]);
        assert!(result.left.is_empty() && !result.stopped);
        assert_eq!((s.activities[1].early_start, s.activities[1].early_finish), (day(2026, 10, 8), day(2026, 10, 9)));
        assert_eq!(s.activities[2].early_start, day(2026, 10, 12));
        assert_eq!(s.finish, day(2026, 10, 12));
        assert!(overloads(&q, &s).is_empty(), "no resource over its limit");
        // The calculation says why B starts on the 8th.
        let lines = crate::explain(&q, "B").unwrap().lines(&|d| d.to_string());
        assert!(lines.iter().any(|l| l.contains("Levelling, so R stays within its limit") && l.contains("2026-10-08")), "{lines:?}");
        // Deterministic: the same again, and again from the levelled project.
        assert_eq!(level(&p).unwrap(), result);
        assert_eq!(level(&q).unwrap(), result);
    }

    /// B at top priority keeps R first: A waits for B to finish, the evening of the 6th, and is
    /// the 7th to the 9th; C stays on the 7th.
    #[test]
    fn priority_comes_before_float() {
        let mut p = two_on_one();
        p.activities[1].levelling_priority = LevellingPriority::Top;
        let (result, _, s) = levelled(&p);
        assert_eq!(result.held, [(0, Levelled { start: day(2026, 10, 7), resource: "R".into() })]);
        assert_eq!((s.activities[0].early_start, s.activities[0].early_finish), (day(2026, 10, 7), day(2026, 10, 9)));
        assert_eq!(s.activities[2].early_start, day(2026, 10, 7));
        // By float alone the order is A's again.
        p.levelling.order = vec![LevelBy::TotalFloat];
        assert_eq!(level(&p).unwrap().held[0].0, 1);
    }

    /// By an activity code, Phase, whose values are listed "Early" then "Late": B in Early keeps
    /// R before A in Late, so A waits for B to finish (the evening of the 6th) and is the 7th to
    /// the 9th, as at top priority. An activity without a value comes after every value: with
    /// A's value gone, B still keeps R. A code no activity has decides nothing, and float does.
    #[test]
    fn an_activity_code_as_priority() {
        use mkperth_core::{ActivityCode, CodeValue};
        let mut p = two_on_one();
        let value = |code: &str| CodeValue { code: code.into(), ..CodeValue::default() };
        p.classification.codes = vec![
            ActivityCode { name: "Phase".into(), values: vec![value("Early"), value("Late")] },
            ActivityCode { name: "Unused".into(), values: vec![value("X")] },
        ];
        p.activities[0].codes.insert("Phase".into(), "Late".into());
        p.activities[1].codes.insert("Phase".into(), "Early".into());
        p.levelling.order = vec![LevelBy::Code("Phase".into()), LevelBy::TotalFloat];
        let (result, _, s) = levelled(&p);
        assert_eq!(result.held, [(0, Levelled { start: day(2026, 10, 7), resource: "R".into() })]);
        assert_eq!((s.activities[0].early_start, s.activities[0].early_finish), (day(2026, 10, 7), day(2026, 10, 9)));
        p.activities[0].codes.clear();
        assert_eq!(level(&p).unwrap().held[0].0, 0, "no value comes last");
        p.levelling.order = vec![LevelBy::Code("Unused".into()), LevelBy::TotalFloat];
        assert_eq!(level(&p).unwrap().held[0].0, 1, "a tie, then float: A keeps R");
        assert_eq!(LevelBy::from_code("code:Phase"), Some(LevelBy::Code("Phase".into())));
        assert_eq!(LevelBy::Code("Phase".into()).code(), "code:Phase");
        assert_eq!(LevelBy::from_code("code: "), None);
    }

    /// Within float B could wait only to its late start, the 6th: Thursday is past it, so nothing
    /// moves, the finish stays on the 7th, and the 5th and 6th stay over-allocated.
    #[test]
    fn within_float_the_finish_stays() {
        let mut p = two_on_one();
        p.levelling.within_float = true;
        let (result, _, s) = levelled(&p);
        assert!(result.held.is_empty());
        assert_eq!(s.finish, day(2026, 10, 7));
        assert_eq!(result.left.iter().map(|o| o.day).collect::<Vec<_>>(), [day(2026, 10, 5), day(2026, 10, 6)]);
        assert_eq!(result.left[0].activities, ["A", "B"]);
        // D, 6 days on no resource, takes the finish to Monday the 12th: C may be as late as the
        // 12th, B as late as the 8th and 9th. Waiting for A (to the 8th) is within that.
        let mut q = two_on_one();
        q.levelling.within_float = true;
        q.activities.push(Activity { id: "D".into(), duration: 6, ..Activity::default() });
        let (result, _, s) = levelled(&q);
        assert_eq!(result.held, [(1, Levelled { start: day(2026, 10, 8), resource: "R".into() })], "B waits for A, to its late start");
        assert!(result.left.is_empty());
        assert_eq!((s.activities[2].early_start, s.finish), (day(2026, 10, 12), day(2026, 10, 12)), "the finish stays");
    }

    /// An activity begun isn't interrupted, whatever its priority: A started on the 5th, the data
    /// date the 6th, so A goes on the 6th and 7th; B (top priority) waits to the 8th.
    #[test]
    fn what_has_begun_goes_on() {
        let mut p = two_on_one();
        p.data_date = Some(day(2026, 10, 6));
        p.activities[0].actual_start = Some(day(2026, 10, 5));
        p.activities[1].levelling_priority = LevellingPriority::Top;
        let (result, _, s) = levelled(&p);
        assert_eq!(result.held, [(1, Levelled { start: day(2026, 10, 8), resource: "R".into() })]);
        assert_eq!(s.activities[1].early_start, day(2026, 10, 8));
    }

    /// An activity asking for more than the resource has, alone, can't be helped by waiting: it
    /// keeps the resource and is left over-allocated. A material isn't levelled.
    #[test]
    fn what_no_wait_can_resolve_is_left() {
        let mut p = two_on_one();
        p.assignments = vec![Assignment { activity: "A".into(), resource: Some("R".into()), planned: 48 * UNIT, ..Assignment::default() }];
        let (result, _, _) = levelled(&p);
        assert!(result.held.is_empty());
        assert_eq!(result.left.iter().map(|o| (o.units, o.limit)).collect::<Vec<_>>(), [(200, 100); 3]);
        let mut m = two_on_one();
        m.resources[0].kind = ResourceKind::Material;
        assert!(level(&m).unwrap().held.is_empty() && overloads(&m, &schedule(&m).unwrap()).is_empty());
        // Nor a resource left out of the options.
        let mut o = two_on_one();
        o.resources.push(Resource { id: "S".into(), ..Resource::default() });
        o.levelling.resources = vec!["S".into()];
        assert!(level(&o).unwrap().held.is_empty());
    }

    /// 120 one-day activities at once on one person go one after another, in the order of their
    /// IDs (all else equal): the last on the 120th working day.
    #[test]
    fn many_on_one_resource_go_one_after_another() {
        let mut p = two_on_one();
        p.relationships.clear();
        p.activities = (0..120).map(|k| Activity { id: format!("T{k:03}"), duration: 1, ..Activity::default() }).collect();
        p.assignments = (0..120)
            .map(|k| Assignment { activity: format!("T{k:03}"), resource: Some("R".into()), planned: 8 * UNIT, ..Assignment::default() })
            .collect();
        let (result, q, s) = levelled(&p);
        assert!(!result.stopped && result.left.is_empty() && result.held.len() == 119);
        assert!(overloads(&q, &s).is_empty());
        // 119 working days after Monday 5 October 2026: 23 weeks and 4 days, Friday 19 March 2027.
        assert_eq!(s.activities[119].early_start, day(2027, 3, 19));
        assert!(s.activities.windows(2).all(|w| w[0].early_start < w[1].early_start));
    }

    /// Two people on R: both fit, nothing moves. A mandatory start isn't moved: B keeps R, A waits.
    #[test]
    fn what_fits_or_is_mandatory_stays() {
        let mut p = two_on_one();
        p.resources[0].max_units = 2 * UNIT;
        assert!(level(&p).unwrap().held.is_empty());
        let mut q = two_on_one();
        q.activities[1].constraint = Some(Constraint::MandatoryStart(day(2026, 10, 5)));
        let (result, _, _) = levelled(&q);
        assert_eq!(result.held, [(0, Levelled { start: day(2026, 10, 7), resource: "R".into() })]);
    }
}
