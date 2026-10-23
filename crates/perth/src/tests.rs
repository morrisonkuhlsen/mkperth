//! Perth files built here from the fields Perth.jl writes (its `types.jl`), never copied from a
//! machine.

use serde_json::json;

use super::*;

fn day(y: i32, m: u32, d: u32) -> Date {
    Date::from_ymd(y, m, d).unwrap()
}

/// A project with every field Perth writes, in its order.
fn full() -> Value {
    json!({
        "id": "p1", "name": "Estação de tratamento", "tasks": [
            {"id": "a", "name": "Levantamento", "start": "2026-09-01", "duration": 5, "progress": 100,
             "dependencies": [], "color": "#b58900", "assignee": "Ana", "notes": "**ok**", "milestone": false,
             "cost": 1200.5, "effort": 40.0, "parent": "", "order": 1, "baseline_start": "2026-09-01",
             "baseline_duration": 5, "deadline": null, "pinned": false, "status": "", "optimistic": 4,
             "most_likely": 5, "pessimistic": 9},
            {"id": "b", "name": "Projeto", "start": "2026-09-08", "duration": 8, "progress": 0,
             "dependencies": ["a+2", "SS:c", "FF:d-1"], "color": "", "assignee": "", "notes": "", "milestone": false,
             "cost": 0.0, "effort": 0.0, "parent": "g", "order": 0, "baseline_start": null,
             "baseline_duration": 0, "deadline": "2026-09-30", "pinned": true, "status": "hold", "optimistic": 0,
             "most_likely": 0, "pessimistic": 0},
            {"id": "c", "name": "Início", "start": "2026-09-01", "duration": 1, "progress": 0,
             "dependencies": [], "color": "", "assignee": "", "notes": "", "milestone": true,
             "cost": 0.0, "effort": 0.0, "parent": "", "order": 0, "baseline_start": null,
             "baseline_duration": 0, "deadline": null, "pinned": false, "status": "", "optimistic": 0,
             "most_likely": 0, "pessimistic": 0},
            {"id": "d", "name": "Licença", "start": "2026-09-02", "duration": 3, "progress": 0,
             "dependencies": ["c"], "color": "", "assignee": "", "notes": "", "milestone": false,
             "cost": 0.0, "effort": 0.0, "parent": "", "order": 0, "baseline_start": null,
             "baseline_duration": 0, "deadline": null, "pinned": false, "status": "", "optimistic": 0,
             "most_likely": 0, "pessimistic": 0}
        ],
        "calendar": "Brazil",
        "people": [{"name": "Ana", "role": "Engenheira", "team": "", "email": "", "notes": "", "capacity": 8.0}],
        "bands": [{"name": "Chuvas", "from": "2026-10-01", "to": "2026-10-31", "color": ""}],
        "markers": [{"name": "Visita", "date": "2026-09-15", "color": "", "label_at": 0}],
        "month_marks": [{"month": "2026-09-01", "name": "Férias", "color": ""}],
        "file_path": "", "baseline_at": "2026-09-01T10:00:00", "created_at": "2026-08-30T09:00:00",
        "updated_at": "2026-09-02T18:30:00"
    })
}

/// An older file: no `effort`, `order`, `status` or PERT fields, no bands.
fn older() -> Value {
    json!({
        "id": "q", "name": "Study", "tasks": [
            {"id": "x", "name": "Design", "start": "2026-08-13", "duration": 6, "progress": 0,
             "dependencies": [], "color": "", "assignee": "", "notes": "", "milestone": false, "cost": 0.0,
             "parent": "", "baseline_start": "2026-08-13", "baseline_duration": 6, "deadline": null, "pinned": false},
            {"id": "y", "name": "Collect", "start": "2026-08-21", "duration": 4, "progress": 0,
             "dependencies": ["x+1"], "color": "", "assignee": "", "notes": "", "milestone": false, "cost": 0.0,
             "parent": "", "baseline_start": null, "baseline_duration": 0, "deadline": null, "pinned": false}
        ],
        "calendar": "", "file_path": "", "baseline_at": null,
        "created_at": "2026-08-01T08:00:00", "updated_at": "2026-08-02T08:00:00"
    })
}

#[test]
fn a_file_read_and_written_is_the_same_json() {
    for file in [full(), older()] {
        let text = file.to_string();
        let read = read(&text).unwrap();
        assert!(read.warnings.is_empty(), "{:?}", read.warnings);
        let (written, warnings) = write(&read.project);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(written, text, "byte for byte, key order included");
    }
}

#[test]
fn what_maps_to_the_model() {
    let p = read(&full().to_string()).unwrap().project;
    assert_eq!(p.name, "Estação de tratamento");
    assert_eq!(p.calendar.name, "Brazil");
    assert_eq!(p.calendar.week, [true, true, true, true, true, false, false], "business days");
    assert_eq!(p.start, day(2026, 9, 1), "the earliest task start");
    let b = &p.activities[1];
    assert_eq!(b.constraint, Some(Constraint::StartOnOrAfter(day(2026, 9, 8))));
    assert_eq!(b.secondary_constraint, Some(Constraint::FinishOnOrBefore(day(2026, 9, 30))));
    assert_eq!(b.extra.get("perth.status").map(String::as_str), Some("\"hold\""));
    assert_eq!(b.extra.get("perth.pinned").map(String::as_str), Some("true"));
    assert_eq!(p.activities[2].kind, ActivityKind::StartMilestone);
    let ties: Vec<(&str, &str, RelationshipType, i64)> =
        p.relationships.iter().map(|r| (r.predecessor.as_str(), r.successor.as_str(), r.kind, r.lag)).collect();
    assert_eq!(
        ties,
        [
            ("a", "b", RelationshipType::FinishToStart, 2),
            ("c", "b", RelationshipType::StartToStart, 0),
            ("d", "b", RelationshipType::FinishToFinish, -1),
            ("c", "d", RelationshipType::FinishToStart, 0),
        ]
    );
    assert!(p.extra.contains_key("perth.people") && p.extra.contains_key("perth.bands"));
    let older = read(&older().to_string()).unwrap().project;
    assert_eq!(older.calendar.week, [true; 7], "an empty calendar: calendar days");
}

#[test]
fn dependency_strings() {
    use RelationshipType::*;
    for (text, expected) in [
        ("a1b2", ("a1b2", FinishToStart, 0)),
        ("a1b2+3", ("a1b2", FinishToStart, 3)),
        ("a1b2-2", ("a1b2", FinishToStart, -2)),
        ("SS:x+1", ("x", StartToStart, 1)),
        ("FF:x", ("x", FinishToFinish, 0)),
        ("FF:x-10", ("x", FinishToFinish, -10)),
        // A sign with nothing after it, or not all digits, is part of the id.
        ("x-", ("x-", FinishToStart, 0)),
        ("x-y", ("x-y", FinishToStart, 0)),
        ("+3", ("+3", FinishToStart, 0)),
        ("x+99999999999999999999", ("x+99999999999999999999", FinishToStart, 0)),
    ] {
        let (id, kind, lag) = parse_dependency(text);
        assert_eq!((id.as_str(), kind, lag), expected, "{text}");
    }
    for r in [
        Relationship { predecessor: "a".into(), successor: "b".into(), kind: StartToStart, lag: 2, lag_minutes: None },
        Relationship { predecessor: "a".into(), successor: "b".into(), kind: FinishToStart, lag: -1, lag_minutes: None },
        Relationship { predecessor: "a".into(), successor: "b".into(), kind: FinishToFinish, lag: 0, lag_minutes: None },
    ] {
        let text = write_dependency(&r).unwrap();
        assert_eq!(parse_dependency(&text), (r.predecessor.clone(), r.kind, r.lag), "{text}");
    }
}

#[test]
fn a_perth_network_schedules_on_calendar_days() {
    // x: 6 calendar days from Thursday 13 August, to Tuesday the 18th; y waits a day after it
    // ("x+1"): Thursday the 20th, but its own start is the 21st, a start no earlier than; 4 days
    // to the 24th.
    let p = read(&older().to_string()).unwrap().project;
    let s = mkperth_sched::schedule(&p).unwrap();
    assert_eq!(s.activities[0].early_finish, day(2026, 8, 18));
    assert_eq!((s.activities[1].early_start, s.activities[1].early_finish), (day(2026, 8, 21), day(2026, 8, 24)));
}

#[test]
fn hostile_files_are_errors_not_crashes() {
    let bad = |text: &str| read(text).err();
    assert!(matches!(bad("not json"), Some(PerthError::Json(_))));
    assert_eq!(bad("[1, 2]"), Some(PerthError::NotAProject));
    assert_eq!(bad("{\"tasks\": 3}"), Some(PerthError::NotAProject));
    assert!(matches!(bad("{\"tasks\": [3]}"), Some(PerthError::Field(..))));
    assert!(matches!(bad("{\"tasks\": [{\"name\": \"no id\"}]}"), Some(PerthError::Field(..))));
    assert!(matches!(bad("{\"tasks\": [{\"id\": \"a\", \"start\": \"2026-02-30\"}]}"), Some(PerthError::Field(..))));
    assert!(matches!(bad("{\"tasks\": [{\"id\": \"a\", \"duration\": \"5\"}]}"), Some(PerthError::Field(..))));
    assert!(matches!(bad("{\"tasks\": [{\"id\": \"a\", \"duration\": 1e300}]}"), Some(PerthError::Field(..))));
    assert!(matches!(bad("{\"tasks\": [{\"id\": \"a\", \"dependencies\": [1]}]}"), Some(PerthError::Field(..))));
    assert!(matches!(bad("{\"name\": 7, \"tasks\": []}"), Some(PerthError::Field(..))));
    // Nesting past serde_json's limit.
    let deep = format!("{{\"tasks\": [], \"x\": {}{}}}", "[".repeat(100_000), "]".repeat(100_000));
    assert!(matches!(bad(&deep), Some(PerthError::Json(_))));
    // A dependency on a task that isn't there is left out with a warning, as Perth does.
    let read = read("{\"tasks\": [{\"id\": \"a\", \"dependencies\": [\"gone+2\"]}]}").unwrap();
    assert!(read.project.relationships.is_empty());
    assert_eq!(read.warnings.len(), 1);
}

#[test]
fn what_perth_cannot_hold_is_reported_on_writing() {
    let mut p = read(&older().to_string()).unwrap().project;
    p.relationships.push(Relationship {
        predecessor: "x".into(),
        successor: "y".into(),
        kind: RelationshipType::StartToFinish,
        lag: 0,
        lag_minutes: None,
    });
    p.activities[0].constraint = Some(Constraint::MandatoryStart(day(2026, 8, 13)));
    let (text, warnings) = write(&p);
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(read(&text).is_ok(), "still a Perth file");
    // A project made here writes every field Perth needs.
    let made = Project { extra: Extra::new(), ..p };
    let (text, _) = write(&made);
    let back = read(&text).unwrap().project;
    assert_eq!(back.activities.len(), 2);
}
