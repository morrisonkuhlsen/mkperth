//! The sample project, built in code: a small building — design, permits, structure, finishes —
//! with a WBS, every kind of relationship, a lag and a lead, milestones, a constraint, and work on
//! Brazil's calendar, so the window has something real to show.

use mkperth_core::{Activity, ActivityKind, Calendar, Constraint, Date, Project, Region, Relationship, RelationshipType};

pub(crate) fn project() -> Project {
    use RelationshipType::*;
    let start = Date::from_ymd(2026, 11, 2).unwrap_or(Date::EPOCH);
    let task = |id: &str, name: &str, duration: i64, parent: &str| Activity {
        id: id.into(),
        name: name.into(),
        duration,
        parent: (!parent.is_empty()).then(|| parent.to_string()),
        ..Activity::default()
    };
    let milestone = |id: &str, name: &str, kind: ActivityKind, parent: &str| Activity { kind, ..task(id, name, 0, parent) };
    let mut activities = vec![
        task("W1", "Design", 0, ""),
        milestone("M0", "Project start", ActivityKind::StartMilestone, "W1"),
        task("A10", "Survey of the site", 5, "W1"),
        task("A20", "Architectural design", 15, "W1"),
        task("A30", "Structural design", 10, "W1"),
        task("A40", "Building permit", 20, "W1"),
        task("W2", "Structure", 0, ""),
        task("B10", "Earthworks", 6, "W2"),
        task("B20", "Foundations", 12, "W2"),
        task("B30", "Concrete frame", 25, "W2"),
        task("B40", "Frame cure", 7, "W2"),
        task("W3", "Finishes", 0, ""),
        task("C10", "Masonry", 15, "W3"),
        task("C20", "Electrical rough-in", 10, "W3"),
        task("C30", "Plumbing rough-in", 8, "W3"),
        task("C40", "Plaster", 12, "W3"),
        task("C50", "Painting", 10, "W3"),
        milestone("M9", "Handover", ActivityKind::FinishMilestone, ""),
    ];
    if let Some(a) = activities.iter_mut().find(|a| a.id == "B40") {
        a.calendar = Some("Seven days".into());
    }
    if let Some(a) = activities.iter_mut().find(|a| a.id == "C50") {
        a.constraint = Some(Constraint::FinishOnOrBefore(Date::from_ymd(2027, 6, 30).unwrap_or(start)));
    }
    let tie = |p: &str, s: &str, kind: RelationshipType, lag: i64| Relationship {
        predecessor: p.into(),
        successor: s.into(),
        kind,
        lag,
        lag_minutes: None,
    };
    Project {
        name: "Sample — a small building".into(),
        start,
        calendar: Calendar { name: "Brazil".into(), region: Some(Region::Brazil), ..Calendar::default() },
        calendars: vec![Calendar { name: "Seven days".into(), week: [true; 7], ..Calendar::default() }],
        lag_calendar: Default::default(),
        float_kind: Default::default(),
        critical_by: Default::default(),
        must_finish_by: None,
        activities,
        relationships: vec![
            tie("M0", "A10", FinishToStart, 0),
            tie("A10", "A20", FinishToStart, 0),
            tie("A20", "A30", StartToStart, 5),
            tie("A30", "A40", FinishToStart, 0),
            tie("A10", "B10", FinishToStart, 10),
            tie("A40", "B20", FinishToStart, 0),
            tie("B10", "B20", FinishToStart, 0),
            tie("B20", "B30", FinishToStart, -2),
            tie("B30", "B40", FinishToStart, 0),
            tie("B40", "C10", FinishToStart, 0),
            tie("C10", "C20", StartToStart, 3),
            tie("C10", "C30", StartToStart, 3),
            tie("C20", "C40", FinishToStart, 0),
            tie("C30", "C40", FinishToStart, 0),
            tie("C10", "C40", FinishToFinish, 5),
            tie("C40", "C50", FinishToStart, 0),
            tie("C50", "M9", FinishToStart, 0),
        ],
        extra: Default::default(),
    }
}
