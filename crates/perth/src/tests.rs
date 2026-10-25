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
    assert_eq!((p.activities[0].notes.as_str(), p.activities[1].notes.as_str()), ("**ok**", ""));
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
    assert!(p.extra.contains_key("perth.people") && !p.extra.contains_key("perth.bands"));
    // The chart's notes, in the model.
    assert_eq!(p.bands, [Band { name: "Chuvas".into(), from: day(2026, 10, 1), to: day(2026, 10, 31), color: String::new() }]);
    assert_eq!((p.markers[0].name.as_str(), p.markers[0].date, p.markers[0].label_at), ("Visita", day(2026, 9, 15), 0));
    assert_eq!((p.month_marks[0].month, p.month_marks[0].name.as_str()), (day(2026, 9, 1), "Férias"));
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

#[test]
fn chart_notes_that_are_not_notes_are_errors() {
    let with = |field: &str, value: Value| {
        let mut v = full();
        v[field] = value;
        read(&v.to_string())
    };
    assert!(with("bands", json!("rain")).is_err());
    assert!(with("bands", json!([{"name": "x", "from": "2026-13-01", "to": "2026-10-31"}])).is_err());
    assert!(with("markers", json!([{"name": "x", "date": "2026-09-15", "label_at": "high"}])).is_err());
    assert!(with("month_marks", json!([7])).is_err());
    let many: Vec<Value> = (0..=MAX_ANNOTATIONS).map(|_| json!({"name": "x", "date": "2026-09-15"})).collect();
    assert!(with("markers", Value::Array(many)).is_err());
    // A label far down is held at the bottom; a project made here writes its notes.
    let p = with("markers", json!([{"name": "x", "date": "2026-09-15", "color": "", "label_at": 250}])).unwrap().project;
    assert_eq!(p.markers[0].label_at, 100);
    let made = Project { extra: Extra::new(), ..p };
    let (text, _) = write(&made);
    assert!(text.contains(r#""markers":[{"name":"x","date":"2026-09-15","color":"","label_at":100}]"#), "{text}");
}

#[test]
fn calendars_are_written_as_perth_names_them() {
    let mut p = read(&full().to_string()).unwrap().project;
    p.extra = Extra::new();
    let name = |p: &Project| {
        let (text, warnings) = write(p);
        let v: Value = serde_json::from_str(&text).unwrap();
        (v["calendar"].as_str().unwrap().to_string(), warnings.len())
    };
    assert_eq!(name(&p), ("Brazil".into(), 0));
    // From P6: a five-day calendar with no region is WeekendsOnly; seven days, calendar days.
    p.calendar = Calendar { name: "Standard".into(), ..Calendar::default() };
    assert_eq!(name(&p), ("WeekendsOnly".into(), 0));
    p.calendar = Calendar { name: "Calendar".into(), ..Calendar::every_day() };
    assert_eq!(name(&p), (String::new(), 0));
    // Its own holidays can't be said: the nearest, with a warning.
    p.calendar = Calendar { name: "Site".into(), ..Calendar::default() };
    p.calendar.holidays.insert(day(2026, 12, 24));
    assert_eq!(name(&p), ("WeekendsOnly".into(), 1));
    // WeekendsOnly reads back as Monday to Friday, without a warning.
    let mut v = full();
    v["calendar"] = json!("WeekendsOnly");
    let back = read(&v.to_string()).unwrap();
    assert!(back.warnings.is_empty(), "{:?}", back.warnings);
    assert_eq!(back.project.calendar.week, [true, true, true, true, true, false, false]);
}

/// Every cut, every byte flipped and every digit made huge: an error, or a project that then
/// schedules (or says why not), never a panic.
#[test]
fn mutated_files_never_panic() {
    let bytes = serde_json::to_string_pretty(&full()).unwrap().into_bytes();
    let try_one = |b: &[u8]| {
        if let Ok(r) = read(&String::from_utf8_lossy(b)) {
            let _ = mkperth_sched::schedule(&r.project);
        }
    };
    for cut in 0..bytes.len() {
        try_one(bytes.get(..cut).unwrap());
    }
    for i in 0..bytes.len() {
        let mut flipped = bytes.clone();
        flipped[i] ^= 0xff;
        try_one(&flipped);
        if bytes[i].is_ascii_digit() {
            let mut huge = bytes.clone();
            huge.splice(i..=i, *b"99999999999999999999999");
            try_one(&huge);
        }
    }
}

/// Task a's assignee, Ana, is a resource assigned for its effort (40 hours); written back as
/// she was named. A second resource on the task is written beside her, with a warning.
#[test]
fn an_assignee_is_a_resource_assigned() {
    let read = read(&full().to_string()).unwrap();
    let p = read.project;
    assert_eq!(p.resources.len(), 1);
    assert_eq!((p.resources[0].id.as_str(), p.resources[0].name.as_str()), ("Ana", "Ana"));
    assert_eq!(p.assignments.len(), 1);
    let a = &p.assignments[0];
    let task = p.activities.iter().find(|t| t.id == a.activity).unwrap();
    assert_eq!(a.resource.as_deref(), Some("Ana"));
    assert_eq!(a.planned, 40 * UNIT, "its effort");
    assert_eq!(task.id, "a");
    let mut two = p.clone();
    two.resources.push(Resource { id: "R2".into(), name: "Bia".into(), ..Resource::default() });
    two.assignments.push(Assignment { activity: a.activity.clone(), resource: Some("R2".into()), ..Assignment::default() });
    let (text, warnings) = write(&two);
    assert!(text.contains("\"assignee\":\"Ana, Bia\""), "{text}");
    assert_eq!(warnings.len(), 1);
}

/// Perth's task cost (`a`'s 1 200.50) is read as an expense and written back as it was; MK
/// Perth's own costs (a currency, accounts, prices, expenses) travel in a field of their own.
#[test]
fn costs_from_perth_and_our_own() {
    use mkperth_core::{Accrual, CostAccount, Expense, Price};
    let p = read(&full().to_string()).unwrap().project;
    assert_eq!(p.expenses, [Expense { activity: "a".into(), name: PERTH_COST.into(), planned: 120_050, ..Expense::default() }]);
    assert!(!write(&p).0.contains("mkperth_costs"), "Perth's own field says it");
    let mut q = p.clone();
    q.currency = "R$".into();
    q.cost_accounts = vec![CostAccount { id: "1".into(), name: "Site".into(), parent: None }];
    q.resources[0].prices = vec![Price { from: None, per_unit: 2550 }, Price { from: Some(day(2026, 10, 1)), per_unit: 3000 }];
    q.expenses.push(Expense {
        activity: "b".into(),
        name: "Permit".into(),
        account: Some("1".into()),
        planned: 45_050,
        actual: 10_000,
        remaining: Some(35_050),
        accrual: Accrual::Start,
    });
    let (text, _) = write(&q);
    let back = read(&text).unwrap().project;
    assert_eq!((&back.currency, &back.cost_accounts, &back.expenses), (&q.currency, &q.cost_accounts, &q.expenses));
    assert_eq!(back.resources[0].prices, q.resources[0].prices);
}

/// Perth's baseline (task `a` from 1 September, 5 days, on Brazil's calendar: the 1st to the
/// 8th, the 7th a holiday) is read as a baseline in use, and written back as it was; one taken
/// in MK Perth travels in a field of its own.
#[test]
fn baselines_from_perth_and_our_own() {
    let p = read(&full().to_string()).unwrap().project;
    assert_eq!(p.baseline_in_use.as_deref(), Some(PERTH_BASELINE));
    let b = p.baseline().unwrap();
    assert_eq!(b.activities.len(), 1);
    assert_eq!((b.activities[0].start, b.activities[0].finish), (day(2026, 9, 1), day(2026, 9, 8)));
    assert!(!write(&p).0.contains("mkperth_baselines"), "Perth's own fields say it");
    let mut q = p.clone();
    q.baselines.push(Baseline {
        name: "Contract".into(),
        activities: vec![BaselineActivity { id: "b".into(), start: day(2026, 9, 10), finish: day(2026, 9, 21), duration: 3840, cost: 1_234_500 }],
    });
    q.baseline_in_use = Some("Contract".into());
    let (text, _) = write(&q);
    let back = read(&text).unwrap().project;
    assert_eq!((back.baselines, back.baseline_in_use), (q.baselines, q.baseline_in_use));
}

/// PERT's estimates are Perth's own fields: read as hundredths of a day, written back as they
/// were; a task without them writes none, one with decimals keeps them.
#[test]
fn pert_estimates_are_perths_fields() {
    let read_back = read(&full().to_string()).unwrap().project;
    assert_eq!(read_back.activities[0].pert, Some(Pert { optimistic: 400, most_likely: 500, pessimistic: 900 }));
    assert_eq!(read_back.activities[1].pert, None, "zeros are none");
    let mut p = read_back.clone();
    p.activities[1].pert = Some(Pert { optimistic: 650, most_likely: 800, pessimistic: 1250 });
    let (text, _) = write(&p);
    assert!(text.contains("\"optimistic\":6.5") && text.contains("\"most_likely\":8,"), "{text}");
    assert_eq!(read(&text).unwrap().project.activities[1].pert, p.activities[1].pert);
    let mut v = full();
    v["tasks"][0]["pessimistic"] = json!("lots");
    assert!(read(&v.to_string()).is_err());
}

/// Levelling travels in fields of MK Perth's own: a task's priority and hold, the project's
/// options; a file without them is written back without them.
#[test]
fn levelling_is_written_and_read_back() {
    let mut p = read(&full().to_string()).unwrap().project;
    assert!(!write(&p).0.contains("mkperth_level"));
    p.activities[0].levelling_priority = LevellingPriority::High;
    p.activities[1].levelled = Some(Levelled { start: day(2026, 3, 9), resource: "Ana".into() });
    p.levelling = LevellingOptions { within_float: true, resources: vec!["Ana".into()], order: vec![LevelBy::LateStart, LevelBy::Priority] };
    let (text, warnings) = write(&p);
    assert!(warnings.is_empty() && text.contains("\"mkperth_levelling_priority\":\"high\""), "{text}");
    let back = read(&text).unwrap().project;
    assert_eq!((back.activities[0].levelling_priority, &back.activities[1].levelled), (LevellingPriority::High, &p.activities[1].levelled));
    assert_eq!(back.levelling, p.levelling);
    assert_eq!(write(&back).0, text);
    for bad in [r#"{"mkperth_levelled": 3}"#, r#"{"mkperth_levelling_priority": "urgent"}"#] {
        let mut v = full();
        let extra: Value = serde_json::from_str(bad).unwrap();
        for (k, x) in extra.as_object().unwrap() {
            v["tasks"][0][k] = x.clone();
        }
        assert!(read(&v.to_string()).is_err(), "{bad}");
    }
}

/// Comments travel in a field of MK Perth's own, a thread with its replies and resolution.
#[test]
fn comments_are_written_and_read_back() {
    let mut p = read(&full().to_string()).unwrap().project;
    p.comments.push(Comment {
        id: "C1".into(),
        activity: "a".into(),
        author: "Ana".into(),
        at: "2026-10-23 09:00".into(),
        text: "Booked?".into(),
        reply_to: None,
        resolved: true,
    });
    p.comments.push(Comment {
        id: "C2".into(),
        activity: "a".into(),
        author: "Bia".into(),
        at: String::new(),
        text: "Yes \"today\"".into(),
        reply_to: Some("C1".into()),
        resolved: false,
    });
    let (text, warnings) = write(&p);
    assert!(warnings.is_empty() && text.contains("\"mkperth_comments\""));
    assert_eq!(read(&text).unwrap().project.comments, p.comments);
}

/// MK Perth's budget travels in a field of its own, read back as it was written: budgets with
/// their plans, the log (a cut and a status), sources in a hierarchy, funds. None, no field;
/// a field Perth didn't write is refused.
#[test]
fn the_budget_in_its_own_field() {
    use mkperth_core::{BudgetChange, ChangeStatus, Fund, FundingSource, SpendingMonth, WbsBudget};
    let p = read(&full().to_string()).unwrap().project;
    assert!(!write(&p).0.contains("mkperth_budget"), "nothing to say");
    let mut q = p.clone();
    q.budget.budgets = vec![
        WbsBudget { wbs: None, original: 100_000_050, plan: vec![SpendingMonth { month: day(2026, 11, 1), amount: 40_000_000 }] },
        WbsBudget { wbs: Some("a".into()), original: 2_000, plan: Vec::new() },
    ];
    q.budget.log = vec![BudgetChange {
        number: 2,
        wbs: Some("a".into()),
        date: day(2026, 11, 10),
        amount: -30_050,
        status: ChangeStatus::NotApproved,
        responsible: "PM".into(),
        reason: "scope".into(),
    }];
    q.budget.sources = vec![
        FundingSource { id: "BANK".into(), name: "Bank".into(), parent: None },
        FundingSource { id: "B2".into(), name: "Loan".into(), parent: Some("BANK".into()) },
    ];
    q.budget.funds = vec![Fund { source: "B2".into(), amount: 5_000_000 }];
    let (text, _) = write(&q);
    assert!(text.contains("\"month\":\"2026-11\""), "{text}");
    assert_eq!(read(&text).unwrap().project.budget, q.budget);
    let mut bad: Value = serde_json::from_str(&text).unwrap();
    bad["mkperth_budget"]["log"][0]["date"] = Value::from("soon");
    assert!(read(&bad.to_string()).is_err());
    bad["mkperth_budget"] = Value::from(3);
    assert!(read(&bad.to_string()).is_err());
}

/// Perth's progress is MK Perth's physical percent complete, read and written back; earned
/// value's techniques, periods and closes travel in a field of their own.
#[test]
fn progress_and_earned_value() {
    use mkperth_core::{EarnedValue, EtcTechnique, EvTechnique, FinancialPeriod, PeriodRecord};
    let p = read(&full().to_string()).unwrap().project;
    assert_eq!(p.activities[0].percent_complete, Some(10_000), "a's 100");
    assert_eq!(p.activities[1].percent_complete, None, "b's 0");
    let mut q = p.clone();
    q.activities[1].percent_complete = Some(3_350);
    q.earned_value = EarnedValue {
        technique: EvTechnique::Custom(2_500),
        etc: EtcTechnique::Cpi,
        periods: vec![FinancialPeriod { name: "2026-09".into(), start: day(2026, 9, 1), finish: day(2026, 9, 30) }],
        records: vec![PeriodRecord { period: "2026-09".into(), activity: "a".into(), planned: 120_050, earned: 100_000, actual: 90_000 }],
    };
    let (text, _) = write(&q);
    assert!(text.contains("\"progress\":33.5") && text.contains("mkperth_earned_value"), "{text}");
    let back = read(&text).unwrap().project;
    assert_eq!(back.activities[1].percent_complete, Some(3_350));
    assert_eq!(back.earned_value, q.earned_value);
}

/// Perth.jl's S-curve as the oracle: two tasks on calendar days, x (1 to 10 October, 1 000,
/// 60 % done) then y (11 to 30 October, 3 000, 10 % done), their baselines the same. Perth.jl's
/// `_scurve` on 25 October 2026 gave planned 3 250.0 (1 000 + 15 days × 150), earned 900.0
/// (600 + 300) and a total of 4 000.0; MK Perth, judged physically, the same.
#[test]
fn earned_value_as_perth_jl_works_it() {
    let text = r#"{"name":"EV oracle","calendar":"","tasks":[
 {"id":"x","name":"Survey","start":"2026-10-01","duration":10,"progress":60,"dependencies":[],"cost":1000,"baseline_start":"2026-10-01","baseline_duration":10},
 {"id":"y","name":"Build","start":"2026-10-11","duration":20,"progress":10,"dependencies":["x"],"cost":3000,"baseline_start":"2026-10-11","baseline_duration":20}
]}"#;
    let p = read(text).unwrap().project;
    let s = mkperth_sched::schedule(&p).unwrap();
    let e = mkperth_sched::project_earned(&p, &s, day(2026, 10, 25));
    assert_eq!((e.planned, e.earned, e.budget), (325_000, 90_000, 400_000));
}

/// Codes and fields travel in fields of their own: the project's in `mkperth_classification`,
/// each task's values beside its own fields, read back as they were; none, none written.
#[test]
fn codes_and_fields_in_their_own_fields() {
    use mkperth_core::{ActivityCode, Classification, CodeValue, FieldKind, FieldValue, Indicator, UserField};
    let p = read(&full().to_string()).unwrap().project;
    assert!(!write(&p).0.contains("mkperth_classification"));
    let mut q = p.clone();
    q.classification = Classification {
        codes: vec![ActivityCode {
            name: "Phase".into(),
            values: vec![
                CodeValue { code: "DES".into(), description: "Design".into(), parent: None },
                CodeValue { code: "DES.A".into(), description: String::new(), parent: Some("DES".into()) },
            ],
        }],
        fields: FieldKind::ALL.iter().map(|k| UserField { name: k.code().into(), kind: *k }).collect(),
    };
    q.activities[0].codes.insert("Phase".into(), "DES.A".into());
    for v in [
        FieldValue::Text("note".into()),
        FieldValue::Number(1_500_000),
        FieldValue::Integer(3),
        FieldValue::Cost(99_950),
        FieldValue::Date(day(2026, 9, 3)),
        FieldValue::Indicator(Indicator::Red),
    ] {
        q.activities[0].fields.insert(v.kind().code().into(), v);
    }
    let (text, _) = write(&q);
    assert!(text.contains(r#""mkperth_fields":{"cost":{"cost":999.5}"#), "{text}");
    let back = read(&text).unwrap().project;
    assert_eq!(back.classification, q.classification);
    assert_eq!((&back.activities[0].codes, &back.activities[0].fields), (&q.activities[0].codes, &q.activities[0].fields));
    assert_eq!(write(&back).0, text, "and written again the same");
}

/// A project whose tasks have no start of their own keeps its start through the file: before,
/// it came back as 1970 and its activities were scheduled then.
#[test]
fn the_start_kept_when_the_tasks_dont_give_it() {
    let mut p = read(&full().to_string()).unwrap().project;
    for a in &mut p.activities {
        a.constraint = None;
    }
    p.start = day(2026, 11, 2);
    let (text, _) = write(&p);
    assert!(text.contains(r#""mkperth_start":"2026-11-02""#), "{text}");
    assert_eq!(read(&text).unwrap().project.start, day(2026, 11, 2));
    // Given by the tasks, nothing more is written.
    assert!(!write(&read(&full().to_string()).unwrap().project).0.contains("mkperth_start"));
}

/// The organisation and each task's responsible travel in fields of their own.
#[test]
fn organisation_in_its_own_field() {
    use mkperth_core::{EpsNode, ObsNode, Organisation};
    let mut p = read(&full().to_string()).unwrap().project;
    assert!(!write(&p).0.contains("mkperth_organisation"));
    p.organisation = Organisation {
        eps: vec![EpsNode { id: "MK".into(), name: "Morrison Kühlsen".into() }],
        obs: vec![ObsNode { name: "PM".into(), description: "Project office".into(), parent: None }],
        responsible: Some("PM".into()),
    };
    p.activities[1].responsible = Some("PM".into());
    let (text, _) = write(&p);
    let back = read(&text).unwrap().project;
    assert_eq!((back.organisation, back.activities[1].responsible.clone()), (p.organisation, Some("PM".into())));
}
