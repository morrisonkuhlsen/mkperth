//! Networks scheduled by hand, each date and float worked out in the comments.

use mkperth_core::{Activity, ActivityKind, Calendar, Constraint, Date, LagCalendar, Project, Relationship, RelationshipType};

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
        calendars: Vec::new(),
        lag_calendar: LagCalendar::default(),
        float_kind: Default::default(),
        critical_by: Default::default(),
        data_date: None,
        progress_mode: Default::default(),
        must_finish_by: None,
        bands: Vec::new(),
        markers: Vec::new(),
        month_marks: Vec::new(),
        resources: Vec::new(),
        roles: Vec::new(),
        assignments: Vec::new(),
        comments: Vec::new(),
        levelling: Default::default(),
        baselines: Vec::new(),
        baseline_in_use: None,
        expenses: Vec::new(),
        cost_accounts: Vec::new(),
        currency: String::new(),
        activities: activities
            .iter()
            .map(|(id, d)| Activity { id: id.to_string(), name: id.to_string(), duration: *d, ..Activity::default() })
            .collect(),
        extra: Default::default(),
        relationships: ties
            .iter()
            .map(|(p, s, kind, lag)| Relationship { predecessor: p.to_string(), successor: s.to_string(), kind: *kind, lag: *lag, lag_minutes: None })
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
    // The project's finish is its latest early finish: the milestone's, the 13th (a start
    // milestone shows its morning).
    assert_eq!(s.finish, day(2026, 10, 13));
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
    // A finish milestone with no work before it falls at the project's first working minute,
    // Monday the 5th at 08:00; it can wait until Friday the 9th's evening: 5 days.
    let kick = get(&s, "kick");
    assert_eq!(kick.early_finish, day(2026, 10, 5));
    assert_eq!((kick.total_float, kick.late_finish), (5, day(2026, 10, 9)));
    // The latest early finish is the start milestone's, Monday the 12th.
    assert_eq!(s.finish, day(2026, 10, 12));
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

/// A seven-day calendar named "seven", for concrete that cures over the weekend.
fn with_seven_days(p: &mut Project) {
    p.calendars.push(Calendar { name: "seven".into(), week: [true; 7], ..Calendar::default() });
}

#[test]
fn an_activity_on_its_own_calendar() {
    // A(5) on the five-day week, Monday the 5th to Friday the 9th; B(3) cures on the seven-day
    // calendar from Saturday the 10th to Monday the 12th; C(1) on the five-day week, Tuesday the
    // 13th. Every activity is critical; B's float counts its own days.
    let mut p = project(&[("A", 5), ("B", 3), ("C", 1)], &[("A", "B", FS, 0), ("B", "C", FS, 0)]);
    with_seven_days(&mut p);
    p.activities[1].calendar = Some("seven".into());
    let s = schedule(&p).unwrap();
    let b = get(&s, "B");
    assert_eq!((b.early_start, b.early_finish), (day(2026, 10, 10), day(2026, 10, 12)));
    assert_eq!(get(&s, "C").early_start, day(2026, 10, 13));
    assert!(s.activities.iter().all(|a| a.critical));
    // Without the cure calendar, B would wait for Monday: 12th to 14th, C on the 15th.
    p.activities[1].calendar = None;
    assert_eq!(get(&schedule(&p).unwrap(), "C").early_start, day(2026, 10, 15));
    // A calendar the project doesn't have.
    p.activities[1].calendar = Some("night shift".into());
    assert_eq!(schedule(&p), Err(ScheduleError::UnknownCalendar("B".into(), "night shift".into())));
}

#[test]
fn the_calendar_a_lag_counts_on() {
    // A(5) on the five-day week finishes the evening of Friday the 9th; B(1) on the seven-day
    // calendar follows with a lag of 2 days.
    let mut p = project(&[("A", 5), ("B", 1)], &[("A", "B", FS, 2)]);
    with_seven_days(&mut p);
    p.activities[1].calendar = Some("seven".into());
    let b_start = |lag: LagCalendar, p: &mut Project| {
        p.lag_calendar = lag;
        get(&schedule(p).unwrap(), "B").early_start
    };
    // On A's five-day week: Monday the 12th and Tuesday the 13th, so B starts Wednesday the 14th.
    assert_eq!(b_start(LagCalendar::Predecessor, &mut p), day(2026, 10, 14));
    assert_eq!(b_start(LagCalendar::Project, &mut p), day(2026, 10, 14));
    // On B's seven days, or around the clock: Saturday and Sunday, so Monday the 12th.
    assert_eq!(b_start(LagCalendar::Successor, &mut p), day(2026, 10, 12));
    assert_eq!(b_start(LagCalendar::TwentyFourHour, &mut p), day(2026, 10, 12));
}

#[test]
fn a_start_and_a_deadline_together() {
    // B(2) after A(3) starts on or after Monday the 12th (5–7 on the clock) and must finish by
    // the evening of Tuesday the 13th: it just makes it, float 0. By Monday the 12th: −1.
    let mut p = a_then_b();
    p.activities[1].constraint = Some(Constraint::StartOnOrAfter(day(2026, 10, 12)));
    p.activities[1].secondary_constraint = Some(Constraint::FinishOnOrBefore(day(2026, 10, 13)));
    let s = schedule(&p).unwrap();
    assert_eq!((get(&s, "B").early_start, get(&s, "B").total_float), (day(2026, 10, 12), 0));
    p.activities[1].secondary_constraint = Some(Constraint::FinishOnOrBefore(day(2026, 10, 12)));
    let s = schedule(&p).unwrap();
    assert_eq!((get(&s, "B").total_float, get(&s, "A").total_float), (-1, 1), "A still has the days before the 12th");
}

#[test]
fn summaries_roll_up_their_children() {
    // H holds G and C; G holds A(3) → B(2); D(1) follows B. A tie from G to C reaches a summary
    // and is left out (a summary is its children, not work), so C(4) starts with the project.
    // Leaves: A 5th–7th, B 8th–9th, C 5th–8th, D Monday the 12th. C's late finish is the
    // project's, Monday the 12th's evening: 2 days of float (Friday and Monday).
    let mut p = project(&[("H", 0), ("G", 9), ("A", 3), ("B", 2), ("C", 4), ("D", 1)], &[("A", "B", FS, 0), ("G", "C", FS, 0), ("B", "D", FS, 0)]);
    for (id, parent) in [("G", "H"), ("A", "G"), ("B", "G"), ("C", "H")] {
        p.activities.iter_mut().find(|a| a.id == id).unwrap().parent = Some(parent.into());
    }
    let s = schedule(&p).unwrap();
    assert_eq!(s.ignored, [1], "the tie from the summary G");
    assert_eq!((get(&s, "C").early_start, get(&s, "C").total_float), (day(2026, 10, 5), 2));
    // G spans A and B: the 5th to the 9th, late the same, float 0 (its own duration of 9 counts
    // for nothing).
    let g = get(&s, "G");
    assert_eq!(
        (g.early_start, g.early_finish, g.late_finish, g.total_float, g.critical),
        (day(2026, 10, 5), day(2026, 10, 9), day(2026, 10, 9), 0, true)
    );
    // H spans G and C: early to Friday the 9th, late to C's Monday the 12th: one day.
    let h = get(&s, "H");
    assert_eq!(
        (h.early_start, h.early_finish, h.late_start, h.late_finish, h.total_float),
        (day(2026, 10, 5), day(2026, 10, 9), day(2026, 10, 5), day(2026, 10, 12), 1)
    );
    assert_eq!(s.finish, day(2026, 10, 12));
}

#[test]
fn a_broken_wbs_is_an_error() {
    let mut p = project(&[("A", 1), ("B", 1)], &[]);
    p.activities[0].parent = Some("Z".into());
    assert_eq!(schedule(&p), Err(ScheduleError::UnknownParent("A".into(), "Z".into())));
    p.activities[0].parent = Some("B".into());
    p.activities[1].parent = Some("A".into());
    let Err(ScheduleError::ParentLoop(ids)) = schedule(&p) else { panic!("a loop of parents") };
    assert_eq!(ids.first(), ids.last());
    p.activities[1].parent = None;
    p.activities[1].kind = ActivityKind::FinishMilestone;
    assert_eq!(schedule(&p), Err(ScheduleError::MilestoneParent("B".into())));
    // An empty parent is no parent.
    p.activities[0].parent = Some(String::new());
    assert!(schedule(&p).is_ok());
}

#[test]
fn every_date_with_its_calculation() {
    // The four-kinds network: G(2) finishes on or after E's finish + 1 (FF+1): its finish bound is
    // the evening of Monday the 12th, so it starts no earlier than Friday the 9th.
    let p = project(
        &[("E", 5), ("F", 3), ("G", 2), ("H", 4), ("M", 0)],
        &[("E", "F", SS, 2), ("E", "G", FF, 1), ("E", "H", SF, 1), ("F", "M", FS, 0), ("G", "M", FS, 0)],
    );
    let g = explain(&p, "G").unwrap();
    assert_eq!(g.early.len(), 2, "the project's start and the FF tie");
    assert_eq!(g.early[1].via, Some(Moment { date: day(2026, 10, 12), evening: true, time: None }));
    assert_eq!(g.early[1].moment, Moment { date: day(2026, 10, 9), evening: false, time: None });
    assert_eq!(g.earliest, g.early_start);
    assert_eq!(g.early_start.date, g.scheduled.early_start);
    let lines = g.lines(&|d: Date| d.to_string());
    assert!(lines.iter().any(|l| l.contains("Finish-to-finish with E, lag +1") && l.contains("the evening of 2026-10-12")), "{lines:#?}");
    // A start that falls on a weekend is placed on the next working morning, and says so.
    let mut q = a_then_b();
    q.activities[1].constraint = Some(Constraint::StartOnOrAfter(day(2026, 10, 10)));
    let b = explain(&q, "B").unwrap();
    assert_eq!((b.earliest.date, b.early_start.date), (day(2026, 10, 10), day(2026, 10, 12)));
    assert!(b.lines(&|d: Date| d.to_string()).iter().any(|l| l.contains("next working morning")));
    assert!(matches!(explain(&q, "nope"), Err(ScheduleError::UnknownActivity(_))));
}

/// For every activity of every network here, the explanation's dates are the schedule's, the
/// winning bound is the latest (forward) or the earliest (backward), and the free float is the
/// least slack it lists.
#[test]
fn the_explanation_is_the_calculation() {
    let mut nets = vec![
        project(&[("A", 3), ("B", 2), ("C", 4), ("D", 1)], &[("A", "B", FS, 0), ("A", "C", FS, 0), ("B", "D", FS, 0), ("C", "D", FS, 0)]),
        project(
            &[("E", 5), ("F", 3), ("G", 2), ("H", 4), ("M", 0)],
            &[("E", "G", FF, 1), ("E", "F", SS, 2), ("E", "H", SF, 1), ("F", "M", FS, 0), ("G", "M", FS, 0)],
        ),
        project(&[("P", 5), ("Q", 3)], &[("P", "Q", FS, -2)]),
    ];
    let mut c = a_then_b();
    c.activities[1].constraint = Some(Constraint::MandatoryStart(day(2026, 10, 7)));
    nets.push(c);
    for p in &nets {
        let s = schedule(p).unwrap();
        for a in &s.activities {
            let e = explain(p, &a.id).unwrap();
            assert_eq!(&e.scheduled, a);
            // In time order: a morning at the day's start of work, an evening at its end.
            let t = |m: Moment| (m.date, m.time.unwrap_or(if m.evening { u16::MAX } else { 0 }));
            let mandatory = e.early.iter().find(|b| b.overrides);
            let won = mandatory.map(|b| b.moment).or_else(|| e.early.iter().map(|b| b.moment).max_by_key(|m| t(*m)));
            assert_eq!(won, Some(e.earliest), "{}: forward", a.id);
            let mandatory = e.late.iter().find(|b| b.overrides);
            let won = mandatory.map(|b| b.moment).or_else(|| e.late.iter().map(|b| b.moment).min_by_key(|m| t(*m)));
            assert_eq!(won, Some(e.latest), "{}: backward", a.id);
            let least = e.free.iter().map(|s| s.minutes).min().unwrap_or(0).min(a.total_float_minutes).max(0);
            assert_eq!(least, a.free_float_minutes, "{}: free float", a.id);
            assert!(!e.lines(&|d: Date| d.to_string()).is_empty());
        }
    }
}

#[test]
fn critical_by_a_float_threshold() {
    // A(3) → B(2) finish on Friday the 9th; C(1) on its own has 4 days of float.
    let mut p = project(&[("A", 3), ("B", 2), ("C", 1)], &[("A", "B", FS, 0)]);
    assert!(!get(&schedule(&p).unwrap(), "C").critical);
    p.critical_by = mkperth_core::CriticalBy::TotalFloat(4);
    let s = schedule(&p).unwrap();
    assert_eq!(get(&s, "C").total_float, 4);
    assert!(get(&s, "C").critical, "4 is at the threshold");
}

#[test]
fn the_longest_path() {
    // A(3) → B(2) make the project's finish, Friday the 9th. C(2) must finish by Tuesday the
    // 6th's evening: its float is 0, so it is critical by float, but it doesn't drive the finish.
    let mut p = project(&[("A", 3), ("B", 2), ("C", 2)], &[("A", "B", FS, 0)]);
    p.activities[2].constraint = Some(Constraint::FinishOnOrBefore(day(2026, 10, 6)));
    let s = schedule(&p).unwrap();
    assert_eq!((get(&s, "C").total_float, get(&s, "C").critical, get(&s, "C").longest_path), (0, true, false));
    assert!(get(&s, "A").longest_path && get(&s, "B").longest_path);
    p.critical_by = mkperth_core::CriticalBy::LongestPath;
    let s = schedule(&p).unwrap();
    assert!(!get(&s, "C").critical && get(&s, "A").critical && get(&s, "B").critical);
    // A start on or after Monday the 12th takes the drive from A: the path is B alone.
    p.activities[1].constraint = Some(Constraint::StartOnOrAfter(day(2026, 10, 12)));
    let s = schedule(&p).unwrap();
    assert!(get(&s, "B").longest_path && !get(&s, "A").longest_path);
    assert!(!get(&s, "A").critical, "A has float now");
}

#[test]
fn start_float_and_the_smallest() {
    // Without progress a task's start and finish float count the same days; the options choose.
    let mut p = project(&[("A", 3), ("B", 2), ("C", 1)], &[("A", "B", FS, 0)]);
    let finish = get(&schedule(&p).unwrap(), "C").total_float;
    for kind in [mkperth_core::FloatKind::Start, mkperth_core::FloatKind::Smallest] {
        p.float_kind = kind;
        assert_eq!(get(&schedule(&p).unwrap(), "C").total_float, finish, "{kind:?}");
    }
}

#[test]
fn half_days_and_lags_in_hours() {
    // A takes half a day (four hours): Monday the 5th, 08:00 to 12:00. B(1) follows: it starts
    // after lunch, Monday 13:00, and finishes Tuesday 12:00. C follows B two hours later: Tuesday
    // 15:00 (13:00 plus two hours of work), one day to Wednesday 15:00.
    let mut p = project(&[("A", 1), ("B", 1), ("C", 1)], &[("A", "B", FS, 0), ("B", "C", FS, 0)]);
    p.activities[0].duration_minutes = Some(240);
    p.relationships[1].lag_minutes = Some(120);
    let e = |id: &str| explain(&p, id).unwrap();
    let s = schedule(&p).unwrap();
    assert_eq!((get(&s, "A").early_start, get(&s, "A").early_finish), (day(2026, 10, 5), day(2026, 10, 5)));
    assert_eq!((get(&s, "B").early_start, get(&s, "B").early_finish), (day(2026, 10, 5), day(2026, 10, 6)));
    assert_eq!(e("B").early_start.time, Some(13 * 60), "after lunch");
    assert_eq!(e("C").early_start.time, Some(15 * 60));
    assert_eq!(get(&s, "C").early_finish, day(2026, 10, 7));
    // A half-day activity on its own beside a five-day one: four and a half days of float.
    let mut q = project(&[("A", 5), ("H", 1)], &[]);
    q.activities[1].duration_minutes = Some(240);
    let h = get(&schedule(&q).unwrap(), "H").clone();
    assert_eq!((h.total_float_minutes, h.total_float), (4 * 480 + 240, 4), "whole days towards zero");
}

#[test]
fn a_short_friday() {
    // Fridays work 08:00 to 12:00 only: five days (40 hours) from Monday the 5th take Monday to
    // Thursday (32 hours), Friday morning (4) and Monday the 12th until 12:00 (4).
    let mut p = project(&[("A", 5), ("B", 1)], &[("A", "B", FS, 0)]);
    p.calendar.periods[4] = vec![(8 * 60, 12 * 60)];
    let s = schedule(&p).unwrap();
    assert_eq!(get(&s, "A").early_finish, day(2026, 10, 12));
    assert_eq!(explain(&p, "A").unwrap().early_finish.time, Some(12 * 60));
    // B starts after lunch on Monday the 12th.
    assert_eq!(explain(&p, "B").unwrap().early_start.time, Some(13 * 60));
    // A day of duration is the calendar's first working day: eight hours.
    assert_eq!(p.calendar.day_minutes(), 480);
}

/// Progress, data date Wednesday 7 October 2026: A(3) → B(2) from Monday the 5th, C(1) alone.
fn progressed(mode: ProgressMode) -> Project {
    let mut p = project(&[("A", 3), ("B", 2), ("C", 1)], &[("A", "B", FS, 0)]);
    p.data_date = Some(day(2026, 10, 7));
    p.progress_mode = mode;
    p
}

fn dates(s: &Schedule, id: &str) -> (Date, Date) {
    let a = get(s, id);
    (a.early_start, a.early_finish)
}

#[test]
fn a_completed_activity_keeps_its_actual_dates() {
    // A ran Monday to Tuesday, a day early. B can't start before the data date: Wed 7–Thu 8.
    // C, not started, neither.
    let mut p = progressed(ProgressMode::RetainedLogic);
    p.activities[0].actual_start = Some(day(2026, 10, 5));
    p.activities[0].actual_finish = Some(day(2026, 10, 6));
    let s = schedule(&p).unwrap();
    let a = get(&s, "A");
    assert_eq!(
        (a.early_start, a.early_finish, a.late_start, a.late_finish),
        (day(2026, 10, 5), day(2026, 10, 6), day(2026, 10, 5), day(2026, 10, 6))
    );
    assert_eq!((a.status, a.total_float, a.critical, a.remaining_minutes), (Status::Completed, 0, false, 0));
    assert_eq!(dates(&s, "B"), (day(2026, 10, 7), day(2026, 10, 8)));
    assert!(get(&s, "B").critical);
    assert_eq!(dates(&s, "C"), (day(2026, 10, 7), day(2026, 10, 7)));
    assert_eq!(s.finish, day(2026, 10, 8));
}

#[test]
fn work_left_goes_on_from_the_data_date() {
    // A started Monday; by the data date two of its three days are worked, so one is left by
    // default: Wednesday. Told two are left: Wednesday and Thursday, and B Friday to Monday.
    let mut p = progressed(ProgressMode::RetainedLogic);
    p.activities[0].actual_start = Some(day(2026, 10, 5));
    let s = schedule(&p).unwrap();
    let a = get(&s, "A");
    assert_eq!((a.status, a.remaining_minutes), (Status::InProgress, 480));
    assert_eq!((a.early_start, a.early_finish, a.late_start), (day(2026, 10, 5), day(2026, 10, 7), day(2026, 10, 5)));
    assert_eq!(dates(&s, "B"), (day(2026, 10, 8), day(2026, 10, 9)));
    p.activities[0].remaining_minutes = Some(960);
    let s = schedule(&p).unwrap();
    assert_eq!(dates(&s, "A"), (day(2026, 10, 5), day(2026, 10, 8)));
    assert_eq!(dates(&s, "B"), (day(2026, 10, 9), day(2026, 10, 12)));
    assert!(get(&s, "A").critical && get(&s, "B").critical);
}

#[test]
fn out_of_sequence_progress_by_retained_logic_and_by_override() {
    // B started Tuesday before A finished (A has two days left, to Thursday); B has a day left.
    // Retained logic keeps it after A: Friday. Progress override lets it go on at the data date.
    for (mode, finish) in [(ProgressMode::RetainedLogic, day(2026, 10, 9)), (ProgressMode::ProgressOverride, day(2026, 10, 7))] {
        let mut p = progressed(mode);
        p.activities[0].actual_start = Some(day(2026, 10, 5));
        p.activities[0].remaining_minutes = Some(960);
        p.activities[1].actual_start = Some(day(2026, 10, 6));
        p.activities[1].remaining_minutes = Some(480);
        let s = schedule(&p).unwrap();
        assert_eq!(dates(&s, "B"), (day(2026, 10, 6), finish), "{mode:?}");
    }
}

#[test]
fn without_a_data_date_a_started_activity_runs_from_its_actual_start() {
    let mut p = progressed(ProgressMode::RetainedLogic);
    p.data_date = None;
    p.activities[2].actual_start = Some(day(2026, 10, 9));
    let s = schedule(&p).unwrap();
    assert_eq!(dates(&s, "C"), (day(2026, 10, 9), day(2026, 10, 9)));
    assert_eq!(dates(&s, "A"), (day(2026, 10, 5), day(2026, 10, 7)));
}

#[test]
fn a_started_activity_ignores_its_start_constraint() {
    let mut p = progressed(ProgressMode::RetainedLogic);
    p.activities[2].constraint = Some(Constraint::StartOnOrAfter(day(2026, 10, 14)));
    p.activities[2].actual_start = Some(day(2026, 10, 6));
    p.activities[2].remaining_minutes = Some(480);
    let s = schedule(&p).unwrap();
    assert_eq!(dates(&s, "C"), (day(2026, 10, 6), day(2026, 10, 7)));
}

#[test]
fn a_start_to_start_successor_counts_from_the_actual_start() {
    // A started Monday, two days left from the data date (to Thursday); C SS+3 from A: Monday + 3
    // working days is Thursday, not the data date + 3.
    let mut p = progressed(ProgressMode::RetainedLogic);
    p.relationships.push(Relationship { predecessor: "A".into(), successor: "C".into(), kind: SS, lag: 3, lag_minutes: None });
    p.activities[0].actual_start = Some(day(2026, 10, 5));
    p.activities[0].remaining_minutes = Some(960);
    let s = schedule(&p).unwrap();
    assert_eq!(dates(&s, "C"), (day(2026, 10, 8), day(2026, 10, 8)));
}

#[test]
fn an_expected_finish_sets_the_work() {
    // B(2) after A(3), expected to finish Tuesday 13 October: from Thursday 8 to Tuesday 13 is four
    // working days, which push the project's finish.
    let mut p = project(&[("A", 3), ("B", 2)], &[("A", "B", FS, 0)]);
    p.activities[1].expected_finish = Some(day(2026, 10, 13));
    let s = schedule(&p).unwrap();
    assert_eq!(dates(&s, "B"), (day(2026, 10, 8), day(2026, 10, 13)));
    assert_eq!(get(&s, "B").remaining_minutes, 4 * 480);
    assert!(explain(&p, "B").unwrap().lines(&|d: Date| d.to_string()).iter().any(|l| l.contains("2026-10-13")));
    // A done, B under way with a data date: the work left runs from the data date (Monday 12) to Tuesday.
    p.data_date = Some(day(2026, 10, 12));
    p.activities[0].actual_start = Some(day(2026, 10, 5));
    p.activities[0].actual_finish = Some(day(2026, 10, 7));
    p.activities[1].actual_start = Some(day(2026, 10, 8));
    let s = schedule(&p).unwrap();
    assert_eq!(get(&s, "B").remaining_minutes, 2 * 480);
    assert_eq!(dates(&s, "B"), (day(2026, 10, 8), day(2026, 10, 13)));
    // An expected day already past leaves no work: it finishes as it starts.
    p.activities[1].expected_finish = Some(day(2026, 10, 9));
    assert_eq!(get(&schedule(&p).unwrap(), "B").remaining_minutes, 0);
}

#[test]
fn latex_is_plain_ascii_with_accents_as_commands() {
    assert_eq!(latex::escape("Concluída: ação & 50% — A → B ≤ 0"), "Conclu\\'{i}da: a\\c{c}\\~{a}o \\& 50\\% --- A $\\to$ B $\\le$ 0");
    assert_eq!(latex::escape("日本 x_1 {y}"), "?? x\\_1 \\{y\\}");
}

/// Every activity of the hand networks, in both languages, as a document: ASCII, and it compiles
/// where pdflatex is installed.
#[test]
fn the_calculation_as_latex_compiles() {
    let mut p = progressed(ProgressMode::RetainedLogic);
    p.activities[0].actual_start = Some(day(2026, 10, 5));
    p.activities[1].name = "Fundação & 100% — açúcar".into();
    p.activities[2].expected_finish = Some(day(2026, 10, 9));
    let dir = std::env::temp_dir().join(format!("mkperth-tex-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pdflatex = std::process::Command::new("pdflatex").arg("--version").output().is_ok();
    for lang in mkperth_i18n::Lang::ALL {
        mkperth_i18n::set(lang);
        for a in &p.activities {
            let tex = explain(&p, &a.id).unwrap().to_latex("test", &a.name, &|d: Date| d.to_string());
            assert!(tex.is_ascii(), "{tex}");
            assert!(tex.contains("\\begin{document}") && tex.contains("\\subsection*"));
            if pdflatex {
                let file = dir.join(format!("{}-{}.tex", a.id, lang.code()));
                std::fs::write(&file, &tex).unwrap();
                let run = std::process::Command::new("pdflatex")
                    .args(["-interaction=nonstopmode", "-halt-on-error", "-output-directory"])
                    .arg(&dir)
                    .arg(&file)
                    .output()
                    .unwrap();
                assert!(run.status.success(), "{}\n{tex}", String::from_utf8_lossy(&run.stdout));
            }
        }
    }
    mkperth_i18n::set(mkperth_i18n::Lang::En);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_exception_day_works_its_own_periods() {
    // A(2) from Monday 5 October on 08–12, 13–17; Monday works only 08–12 as an exception. Two
    // days are 960 minutes: Monday's 240, Tuesday's 480, then 240 more to Wednesday at 12:00.
    let mut p = project(&[("A", 2), ("B", 1)], &[("A", "B", FS, 0)]);
    p.calendar.exceptions.insert(day(2026, 10, 5), vec![(480, 720)]);
    let s = schedule(&p).unwrap();
    assert_eq!(dates(&s, "A"), (day(2026, 10, 5), day(2026, 10, 7)));
    assert_eq!(explain(&p, "A").unwrap().early_finish.time, Some(12 * 60));
    // B, a day from Wednesday 13:00, ends Thursday at 12:00.
    assert_eq!(dates(&s, "B"), (day(2026, 10, 7), day(2026, 10, 8)));
    // A Saturday worked on its own morning; an exception with no periods is a day off.
    let mut q = project(&[("C", 5)], &[]);
    q.calendar.exceptions.insert(day(2026, 10, 10), vec![(480, 720)]);
    q.calendar.exceptions.insert(day(2026, 10, 7), vec![]);
    let s = schedule(&q).unwrap();
    // Monday 480, Tuesday 960, Wednesday off, Thursday 1440, Friday 1920, Saturday's morning
    // 2160, and the last 240 on Monday the 12th.
    assert_eq!(dates(&s, "C"), (day(2026, 10, 5), day(2026, 10, 12)));
    assert!(q.calendar.is_working(day(2026, 10, 10)) && !q.calendar.is_working(day(2026, 10, 7)));
}

/// A resource-dependent activity works on its resources' calendars and finishes with the last.
/// A takes 40 hours from Monday 5 October 2026, B a day after it. Ana works Monday to Thursday:
/// her 40 hours are the 5th to the 8th and Monday the 12th, so A finishes on the 12th and B is on
/// the 13th (as a task, A would finish Friday the 9th and B be on the 12th). Bia, Monday to
/// Friday, would finish on the 9th: Ana sets the pace. Backwards from B on the 13th, Ana must
/// start by the morning of the 5th (Bia by the 6th): A starts at the latest on the 5th, no float.
#[test]
fn a_resource_dependent_activity_waits_for_its_last_resource() {
    use mkperth_core::{Assignment, Resource, UNIT};
    let mut p = project(&[("A", 5), ("B", 1)], &[("A", "B", FS, 0)]);
    p.calendars.push(Calendar { name: "Four days".into(), week: [true, true, true, true, false, false, false], ..Calendar::default() });
    let as_task = schedule(&p).unwrap();
    assert_eq!((as_task.activities[0].early_finish, as_task.activities[1].early_start), (day(2026, 10, 9), day(2026, 10, 12)));
    p.activities[0].resource_dependent = true;
    p.resources = vec![
        Resource { id: "ANA".into(), name: "Ana".into(), calendar: Some("Four days".into()), ..Resource::default() },
        Resource { id: "BIA".into(), name: "Bia".into(), ..Resource::default() },
    ];
    p.assignments = vec![
        Assignment { activity: "A".into(), resource: Some("ANA".into()), planned: 40 * UNIT, rate: Some(UNIT), ..Assignment::default() },
        Assignment { activity: "A".into(), resource: Some("BIA".into()), planned: 40 * UNIT, rate: Some(UNIT), ..Assignment::default() },
    ];
    let s = schedule(&p).unwrap();
    let (a, b) = (&s.activities[0], &s.activities[1]);
    assert_eq!((a.early_start, a.early_finish), (day(2026, 10, 5), day(2026, 10, 12)));
    assert_eq!((b.early_start, b.early_finish), (day(2026, 10, 13), day(2026, 10, 13)));
    assert_eq!((a.late_start, a.late_finish, a.total_float), (day(2026, 10, 5), day(2026, 10, 12), 0));
    // The calculation names who sets the pace, on which calendar.
    let lines = explain(&p, "A").unwrap().lines(&|d| d.to_string());
    assert!(lines.iter().any(|l| l.contains("Ana, 5 working days on Four days") && l.contains("2026-10-12")), "{lines:?}");
    // Without resources, or begun, it works its own duration again.
    p.assignments.clear();
    assert_eq!(schedule(&p).unwrap().activities[0].early_finish, day(2026, 10, 9));
}

/// A resource on a seven-day week works the Saturday its activity's calendar doesn't: A's 40
/// hours from Thursday 8 October 2026 are the 8th to Monday the 12th for Ana (Saturday and
/// Sunday too), and B the next day starts on Tuesday the 13th. Backwards, A may finish as late as
/// Monday evening, the latest Ana can before B: its late dates are its early ones, not a Friday
/// before its early finish.
#[test]
fn a_resource_on_a_longer_week_keeps_its_dates_backwards() {
    use mkperth_core::{Assignment, Constraint, Resource, UNIT};
    let mut p = project(&[("A", 5), ("B", 1)], &[("A", "B", FS, 0)]);
    p.calendars.push(Calendar { name: "Seven days".into(), week: [true; 7], ..Calendar::default() });
    p.activities[0].constraint = Some(Constraint::StartOnOrAfter(day(2026, 10, 8)));
    p.activities[0].resource_dependent = true;
    p.resources = vec![Resource { id: "ANA".into(), name: "Ana".into(), calendar: Some("Seven days".into()), ..Resource::default() }];
    p.assignments =
        vec![Assignment { activity: "A".into(), resource: Some("ANA".into()), planned: 40 * UNIT, rate: Some(UNIT), ..Assignment::default() }];
    let s = schedule(&p).unwrap();
    let a = &s.activities[0];
    assert_eq!((a.early_start, a.early_finish), (day(2026, 10, 8), day(2026, 10, 12)));
    assert_eq!((a.late_start, a.late_finish), (day(2026, 10, 8), day(2026, 10, 12)));
    assert_eq!(s.activities[1].early_start, day(2026, 10, 13));
    let lines = explain(&p, "A").unwrap().lines(&|d| d.to_string());
    assert!(lines.iter().any(|l| l.contains("Ana, 5 working days on Seven days") && l.contains("2026-10-12")), "{lines:?}");
}
