//! Networks scheduled by hand, each date and float worked out in the comments.

use mkperth_core::{Activity, ActivityKind, Calendar, Constraint, Date, Project, Relationship, RelationshipType};

use super::*;

fn day(y: i32, m: u32, d: u32) -> Date {
    Date::from_ymd(y, m, d).unwrap()
}

fn project(activities: &[(&str, i64)], ties: &[(&str, &str, RelationshipType, i64)]) -> Project {
    Project {
        name: "test".into(),
        // Monday 5 October 2026.
        start: day(2026, 10, 5),
        calendar: Calendar::default(),
        must_finish_by: None,
        activities: activities
            .iter()
            .map(|(id, d)| Activity { id: id.to_string(), name: id.to_string(), duration: *d, ..Activity::default() })
            .collect(),
        relationships: ties
            .iter()
            .map(|(p, s, kind, lag)| Relationship { predecessor: p.to_string(), successor: s.to_string(), kind: *kind, lag: *lag })
            .collect(),
    }
}

fn get<'a>(s: &'a Schedule, id: &str) -> &'a Scheduled {
    s.activities.iter().find(|a| a.id == id).unwrap()
}

use RelationshipType::{FinishToFinish as FF, FinishToStart as FS, StartToFinish as SF, StartToStart as SS};

/// Working days from Monday 5 October 2026: t = 0 is the 5th, 4 Friday the 9th, 5 Monday the
/// 12th, 7 the 14th.
#[test]
fn finish_to_start_network_by_hand() {
    // A(3) → B(2) → D(1); A → C(4) → D.
    let p = project(&[("A", 3), ("B", 2), ("C", 4), ("D", 1)], &[("A", "B", FS, 0), ("A", "C", FS, 0), ("B", "D", FS, 0), ("C", "D", FS, 0)]);
    let s = schedule(&p).unwrap();
    // Forward: A 0–3, B 3–5, C 3–7, D 7–8. Backward from 8: D 7–8, C 3–7, B 5–7, A 0–3.
    let a = get(&s, "A");
    assert_eq!((a.early_start, a.early_finish), (day(2026, 10, 5), day(2026, 10, 7)));
    let b = get(&s, "B");
    assert_eq!(
        (b.early_start, b.early_finish, b.late_start, b.late_finish),
        (day(2026, 10, 8), day(2026, 10, 9), day(2026, 10, 12), day(2026, 10, 13))
    );
    assert_eq!((b.total_float, b.free_float, b.critical), (2, 2, false));
    let c = get(&s, "C");
    assert_eq!((c.early_start, c.early_finish), (day(2026, 10, 8), day(2026, 10, 13)), "over the weekend");
    assert!(c.critical && a.critical && get(&s, "D").critical);
    assert_eq!(get(&s, "D").early_start, day(2026, 10, 14));
    assert_eq!(s.finish, day(2026, 10, 14));
}

#[test]
fn the_four_kinds_of_relationship_with_lags() {
    // E(5); F(3) SS+2 from E; G(2) FF+1 from E; H(4) SF+1 from E; milestone M after F and G.
    let p = project(
        &[("E", 5), ("F", 3), ("G", 2), ("H", 4), ("M", 0)],
        &[("E", "F", SS, 2), ("E", "G", FF, 1), ("E", "H", SF, 1), ("F", "M", FS, 0), ("G", "M", FS, 0)],
    );
    let s = schedule(&p).unwrap();
    let t = |id: &str| {
        let a = get(&s, id);
        (a.total_float, a.free_float, a.critical)
    };
    // Forward: E 0–5; F from 0 + 2 = 2, to 5; G from 5 + 1 − 2 = 4, to 6; H from 0 + 1 − 4 = −3,
    // held at the project's start, 0–4; M at max(5, 6) = 6. The project finishes at 6.
    assert_eq!(get(&s, "F").early_start, day(2026, 10, 7));
    assert_eq!(get(&s, "G").early_start, day(2026, 10, 9));
    assert_eq!(get(&s, "H").early_start, day(2026, 10, 5));
    let m = get(&s, "M");
    assert_eq!((m.early_start, m.early_finish), (day(2026, 10, 13), day(2026, 10, 13)), "a milestone at t = 6");
    // Backward: M 6; F by 6 (float 1); G by 6 (float 0); H open, by 6 (float 2); E by
    // min(SS: 3 − 2 + 5, FF: 6 − 1, SF: 6 − 1 + 5, 6) = 5, float 0.
    assert_eq!(t("F"), (1, 1, false));
    assert_eq!(t("G"), (0, 0, true));
    assert_eq!(t("H"), (2, 2, false));
    assert_eq!(t("E"), (0, 0, true));
    assert_eq!(t("M"), (0, 0, true));
    // The project finishes the evening of working day 5, the 12th: the same moment as the
    // morning of the 13th, where the milestone (a start milestone, for now) is shown.
    assert_eq!(s.finish, day(2026, 10, 12));
}

#[test]
fn start_and_finish_milestones() {
    // A(5) from Monday the 5th to Friday the 9th; then a finish milestone "done" and a start
    // milestone "go", both at t = 5. "done" is shown the evening of Friday the 9th, "go" the
    // morning of Monday the 12th. A milestone takes no time, whatever its duration says.
    let mut p = project(&[("A", 5), ("done", 3), ("go", 0), ("kick", 7)], &[("A", "done", FS, 0), ("A", "go", FS, 0)]);
    for (id, kind) in [("done", ActivityKind::FinishMilestone), ("go", ActivityKind::StartMilestone), ("kick", ActivityKind::FinishMilestone)] {
        p.activities.iter_mut().find(|a| a.id == id).unwrap().kind = kind;
    }
    let s = schedule(&p).unwrap();
    let done = get(&s, "done");
    assert_eq!((done.early_start, done.early_finish), (day(2026, 10, 9), day(2026, 10, 9)));
    let go = get(&s, "go");
    assert_eq!((go.early_start, go.early_finish), (day(2026, 10, 12), day(2026, 10, 12)));
    assert!(done.critical && go.critical);
    // A finish milestone at the project's very start has no evening before: its day.
    let kick = get(&s, "kick");
    assert_eq!(kick.early_finish, day(2026, 10, 5));
    assert_eq!((kick.total_float, kick.late_finish), (5, day(2026, 10, 9)));
    assert_eq!(s.finish, day(2026, 10, 9));
}

#[test]
fn a_lead_overlaps_the_predecessor() {
    // P(5) → Q(3) FS−2: Q starts at 5 − 2 = 3, finishes at 6.
    let s = schedule(&project(&[("P", 5), ("Q", 3)], &[("P", "Q", FS, -2)])).unwrap();
    assert_eq!(get(&s, "Q").early_start, day(2026, 10, 8));
    assert_eq!(get(&s, "Q").early_finish, day(2026, 10, 12));
    assert!(get(&s, "P").critical);
}

#[test]
fn holidays_move_the_dates() {
    let mut p = project(&[("A", 3), ("B", 2)], &[("A", "B", FS, 0)]);
    p.calendar.holidays.insert(day(2026, 10, 12));
    let s = schedule(&p).unwrap();
    // A 5–7 October, B the 8th and 9th: the holiday on the 12th is after them.
    assert_eq!(get(&s, "B").early_finish, day(2026, 10, 9));
    p.calendar.holidays.insert(day(2026, 10, 8));
    let s = schedule(&p).unwrap();
    // The 8th off: B the 9th and, past the weekend and the 12th, the 13th.
    assert_eq!((get(&s, "B").early_start, get(&s, "B").early_finish), (day(2026, 10, 9), day(2026, 10, 13)));
    // A project starting on a Saturday starts on Monday.
    p.start = day(2026, 10, 3);
    assert_eq!(get(&schedule(&p).unwrap(), "A").early_start, day(2026, 10, 5));
}

#[test]
fn a_loop_is_named_not_a_hang() {
    let p = project(&[("A", 1), ("B", 1), ("C", 1), ("X", 1)], &[("X", "A", FS, 0), ("A", "B", FS, 0), ("B", "C", FS, 0), ("C", "A", FS, 0)]);
    let Err(ScheduleError::Loop(ids)) = schedule(&p) else { panic!("a loop") };
    assert_eq!(ids.first(), ids.last());
    assert_eq!(ids.len(), 4, "{ids:?}");
    for id in ["A", "B", "C"] {
        assert!(ids.iter().any(|i| i == id), "{ids:?}");
    }
}

#[test]
fn bad_networks_are_errors() {
    assert_eq!(schedule(&project(&[("A", 1), ("A", 2)], &[])), Err(ScheduleError::DuplicateActivity("A".into())));
    assert_eq!(schedule(&project(&[("A", 1)], &[("A", "Z", FS, 0)])), Err(ScheduleError::UnknownActivity("Z".into())));
    assert_eq!(schedule(&project(&[("A", 1)], &[("A", "A", FS, 0)])), Err(ScheduleError::SelfRelationship("A".into())));
    assert_eq!(schedule(&project(&[("A", -1)], &[])), Err(ScheduleError::NegativeDuration("A".into())));
    assert_eq!(schedule(&project(&[("A", MAX_SPAN + 1)], &[])), Err(ScheduleError::TooLong));
    assert_eq!(schedule(&project(&[("A", 1), ("B", 1)], &[("A", "B", FS, i64::MAX)])), Err(ScheduleError::TooLong));
    // A chain that adds up past the limit.
    let acts: Vec<(String, i64)> = (0..20).map(|i| (format!("a{i}"), MAX_SPAN / 10)).collect();
    let refs: Vec<(&str, i64)> = acts.iter().map(|(id, d)| (id.as_str(), *d)).collect();
    let ties: Vec<(String, String)> = (1..20).map(|i| (format!("a{}", i - 1), format!("a{i}"))).collect();
    let tie_refs: Vec<(&str, &str, RelationshipType, i64)> = ties.iter().map(|(a, b)| (a.as_str(), b.as_str(), FS, 0)).collect();
    assert_eq!(schedule(&project(&refs, &tie_refs)), Err(ScheduleError::TooLong));
    // Nothing to schedule: it finishes where it starts.
    let empty = schedule(&project(&[], &[])).unwrap();
    assert_eq!((empty.activities.len(), empty.finish), (0, day(2026, 10, 5)));
}

#[test]
fn a_long_chain_schedules_without_recursion() {
    let n = 50_000;
    let acts: Vec<(String, i64)> = (0..n).map(|i| (format!("a{i}"), 1)).collect();
    let refs: Vec<(&str, i64)> = acts.iter().map(|(id, d)| (id.as_str(), *d)).collect();
    let ties: Vec<(String, String)> = (1..n).map(|i| (format!("a{}", i - 1), format!("a{i}"))).collect();
    let tie_refs: Vec<(&str, &str, RelationshipType, i64)> = ties.iter().map(|(a, b)| (a.as_str(), b.as_str(), FS, 0)).collect();
    let s = schedule(&project(&refs, &tie_refs)).unwrap();
    assert!(s.activities.iter().all(|a| a.critical));
}

fn constrained(id: &str, constraint: Constraint, p: &mut Project) {
    p.activities.iter_mut().find(|a| a.id == id).unwrap().constraint = Some(constraint);
}

/// A(3) → B(2) from Monday 5 October 2026: A 0–3 (the 5th to the 7th), B 3–5 (the 8th and 9th).
fn a_then_b() -> Project {
    project(&[("A", 3), ("B", 2)], &[("A", "B", FS, 0)])
}

#[test]
fn early_date_constraints() {
    // B starts on or after Monday the 12th (t = 5): 5–7, the 12th and 13th; A gets 2 days.
    let mut p = a_then_b();
    constrained("B", Constraint::StartOnOrAfter(day(2026, 10, 12)), &mut p);
    let s = schedule(&p).unwrap();
    assert_eq!((get(&s, "B").early_start, get(&s, "B").early_finish), (day(2026, 10, 12), day(2026, 10, 13)));
    assert_eq!(get(&s, "A").total_float, 2);
    // On or after a Saturday: the next working morning, Monday the 12th, the same.
    constrained("B", Constraint::StartOnOrAfter(day(2026, 10, 10)), &mut p);
    assert_eq!(get(&schedule(&p).unwrap(), "B").early_start, day(2026, 10, 12));
    // Finishes on or after the evening of Wednesday the 14th (t = 8): starts at 8 − 2 = 6.
    constrained("B", Constraint::FinishOnOrAfter(day(2026, 10, 14)), &mut p);
    let s = schedule(&p).unwrap();
    assert_eq!((get(&s, "B").early_start, get(&s, "B").early_finish), (day(2026, 10, 13), day(2026, 10, 14)));
    // A date before the project's start holds nothing back.
    constrained("B", Constraint::StartOnOrAfter(day(2026, 9, 1)), &mut p);
    assert_eq!(get(&schedule(&p).unwrap(), "B").early_start, day(2026, 10, 8));
}

#[test]
fn late_date_constraints_and_negative_float() {
    // B finishes on or before the evening of Friday the 9th (t = 5): exactly its early finish.
    let mut p = a_then_b();
    constrained("B", Constraint::FinishOnOrBefore(day(2026, 10, 9)), &mut p);
    assert_eq!(get(&schedule(&p).unwrap(), "B").total_float, 0);
    // By Thursday the 8th's evening (t = 4): late finish 4, a day short. B and A have −1.
    constrained("B", Constraint::FinishOnOrBefore(day(2026, 10, 8)), &mut p);
    let s = schedule(&p).unwrap();
    assert_eq!((get(&s, "B").total_float, get(&s, "A").total_float), (-1, -1));
    assert_eq!(get(&s, "A").late_start, day(2026, 10, 2), "a late start before the project's: Friday the 2nd");
    assert!(get(&s, "A").critical && get(&s, "B").critical);
    assert_eq!(get(&s, "B").early_finish, day(2026, 10, 9), "the early dates don't move");
    // Starts on or before the morning of Wednesday the 7th (t = 2): late finish 2 + 2 = 4, −1.
    constrained("B", Constraint::StartOnOrBefore(day(2026, 10, 7)), &mut p);
    assert_eq!(get(&schedule(&p).unwrap(), "B").total_float, -1);
}

#[test]
fn on_and_mandatory_constraints() {
    // Finishes on the evening of Tuesday the 13th (t = 7): early start ≥ 5, late finish ≤ 7.
    let mut p = a_then_b();
    constrained("B", Constraint::FinishOn(day(2026, 10, 13)), &mut p);
    let s = schedule(&p).unwrap();
    assert_eq!((get(&s, "B").early_start, get(&s, "B").total_float, get(&s, "A").total_float), (day(2026, 10, 12), 0, 2));
    // Starts on Monday the 12th: the same.
    constrained("B", Constraint::StartOn(day(2026, 10, 12)), &mut p);
    assert_eq!(get(&schedule(&p).unwrap(), "B").early_start, day(2026, 10, 12));
    // A mandatory start on Wednesday the 7th (t = 2), before A finishes: B 2–4 regardless,
    // its late start 2 as well; A must finish by 2, a day short: −1.
    constrained("B", Constraint::MandatoryStart(day(2026, 10, 7)), &mut p);
    let s = schedule(&p).unwrap();
    assert_eq!((get(&s, "B").early_start, get(&s, "B").late_start, get(&s, "B").total_float), (day(2026, 10, 7), day(2026, 10, 7), 0));
    assert_eq!(get(&s, "A").total_float, -1);
    // A mandatory finish on the evening of Friday the 16th (t = 10): B 8–10 early and late.
    constrained("B", Constraint::MandatoryFinish(day(2026, 10, 16)), &mut p);
    let s = schedule(&p).unwrap();
    assert_eq!((get(&s, "B").early_start, get(&s, "B").late_finish), (day(2026, 10, 15), day(2026, 10, 16)));
    assert_eq!(get(&s, "A").total_float, 5);
}

#[test]
fn as_late_as_possible_uses_the_free_float() {
    // A(3) → B(5) → D(1); A → C(1) → D. C has 4 days of free float (3–4 against D at 8) and,
    // held as late as possible, takes 7–8: the 14th. A's free float stays 0, set by B.
    let mut p = project(&[("A", 3), ("B", 5), ("C", 1), ("D", 1)], &[("A", "B", FS, 0), ("A", "C", FS, 0), ("B", "D", FS, 0), ("C", "D", FS, 0)]);
    assert_eq!(get(&schedule(&p).unwrap(), "C").free_float, 4);
    constrained("C", Constraint::AsLateAsPossible, &mut p);
    let s = schedule(&p).unwrap();
    let c = get(&s, "C");
    assert_eq!((c.early_start, c.free_float, c.total_float), (day(2026, 10, 14), 0, 0));
    assert_eq!(get(&s, "D").early_start, day(2026, 10, 15), "the successor doesn't move");
    assert_eq!(get(&s, "A").free_float, 0);
}

#[test]
fn the_project_must_finish_by() {
    // A then B finish at t = 5. Must finish by Monday the 12th's evening (t = 6): a day of
    // float each; by Thursday the 8th's (t = 4): −1 each.
    let mut p = a_then_b();
    p.must_finish_by = Some(day(2026, 10, 12));
    let s = schedule(&p).unwrap();
    assert_eq!((get(&s, "A").total_float, get(&s, "B").total_float, get(&s, "B").late_finish), (1, 1, day(2026, 10, 12)));
    assert!(!get(&s, "B").critical);
    p.must_finish_by = Some(day(2026, 10, 8));
    let s = schedule(&p).unwrap();
    assert_eq!((get(&s, "A").total_float, get(&s, "B").total_float), (-1, -1));
    assert_eq!(s.finish, day(2026, 10, 9), "the finish is still the early one");
    // A date far beyond any schedule is an error, not a walk across millennia.
    p.must_finish_by = Some(day(9000, 1, 1));
    assert_eq!(schedule(&p), Err(ScheduleError::TooLong));
}
