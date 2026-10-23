//! P6 XML files built here from the schema's element names, never exported from P6.

use super::*;

fn day(y: i32, m: u32, d: u32) -> Date {
    Date::from_ymd(y, m, d).unwrap()
}

/// A project: a five-day calendar with a holiday and a worked Saturday, a seven-day calendar, a
/// WBS of two levels, four activities and a milestone, the four kinds of relationship with lags
/// in hours, two constraints.
fn file() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<APIBusinessObjects xmlns="http://xmlns.oracle.com/Primavera/P6/V8.3/API/BusinessObjects">
  <Calendar>
    <ObjectId>10</ObjectId><Name>Standard &amp; Holidays</Name><HoursPerDay>8</HoursPerDay>
    <StandardWorkWeek>
      <StandardWorkHours><DayOfWeek>Sunday</DayOfWeek></StandardWorkHours>
      <StandardWorkHours><DayOfWeek>Monday</DayOfWeek><WorkTime><Start>08:00:00</Start><Finish>17:00:00</Finish></WorkTime></StandardWorkHours>
      <StandardWorkHours><DayOfWeek>Tuesday</DayOfWeek><WorkTime><Start>08:00:00</Start><Finish>17:00:00</Finish></WorkTime></StandardWorkHours>
      <StandardWorkHours><DayOfWeek>Wednesday</DayOfWeek><WorkTime><Start>08:00:00</Start><Finish>17:00:00</Finish></WorkTime></StandardWorkHours>
      <StandardWorkHours><DayOfWeek>Thursday</DayOfWeek><WorkTime><Start>08:00:00</Start><Finish>17:00:00</Finish></WorkTime></StandardWorkHours>
      <StandardWorkHours><DayOfWeek>Friday</DayOfWeek><WorkTime><Start>08:00:00</Start><Finish>17:00:00</Finish></WorkTime></StandardWorkHours>
      <StandardWorkHours><DayOfWeek>Saturday</DayOfWeek></StandardWorkHours>
    </StandardWorkWeek>
    <HolidayOrExceptions>
      <HolidayOrException><Date>2026-11-02T00:00:00</Date></HolidayOrException>
      <HolidayOrException><Date>2026-11-07T00:00:00</Date><WorkTime><Start>08:00:00</Start><Finish>12:00:00</Finish></WorkTime></HolidayOrException>
    </HolidayOrExceptions>
  </Calendar>
  <Calendar><ObjectId>11</ObjectId><Name>Seven days</Name><HoursPerDay>10</HoursPerDay>
    <StandardWorkWeek>
      <StandardWorkHours><DayOfWeek>Monday</DayOfWeek><WorkTime/></StandardWorkHours>
      <StandardWorkHours><DayOfWeek>Tuesday</DayOfWeek><WorkTime/></StandardWorkHours>
      <StandardWorkHours><DayOfWeek>Wednesday</DayOfWeek><WorkTime/></StandardWorkHours>
      <StandardWorkHours><DayOfWeek>Thursday</DayOfWeek><WorkTime/></StandardWorkHours>
      <StandardWorkHours><DayOfWeek>Friday</DayOfWeek><WorkTime/></StandardWorkHours>
      <StandardWorkHours><DayOfWeek>Saturday</DayOfWeek><WorkTime/></StandardWorkHours>
      <StandardWorkHours><DayOfWeek>Sunday</DayOfWeek><WorkTime/></StandardWorkHours>
    </StandardWorkWeek>
  </Calendar>
  <Project>
    <ObjectId>1</ObjectId><Id>PLANT</Id><Name>Water plant</Name>
    <PlannedStartDate>2026-11-02T08:00:00</PlannedStartDate>
    <MustFinishByDate>2026-12-18T17:00:00</MustFinishByDate>
    <DataDate>2026-11-12T08:00:00</DataDate>
    <ActivityDefaultCalendarObjectId>10</ActivityDefaultCalendarObjectId>
    <WBS><ObjectId>100</ObjectId><Code>PLANT.1</Code><Name>Design</Name></WBS>
    <WBS><ObjectId>101</ObjectId><Code>PLANT.1.1</Code><Name>Civil</Name><ParentObjectId>100</ParentObjectId></WBS>
    <Activity><ObjectId>1000</ObjectId><Id>A100</Id><Name>Survey</Name><Type>Task Dependent</Type><PlannedDuration>40</PlannedDuration><CalendarObjectId>10</CalendarObjectId><WBSObjectId>101</WBSObjectId>
      <Status>Completed</Status><ActualStartDate>2026-11-03T08:00:00</ActualStartDate><ActualFinishDate>2026-11-09T17:00:00</ActualFinishDate><RemainingDuration>0</RemainingDuration></Activity>
    <Activity><ObjectId>1001</ObjectId><Id>A110</Id><Name>Design</Name><Type>Resource Dependent</Type><PlannedDuration>60</PlannedDuration><WBSObjectId>101</WBSObjectId>
      <ActualStartDate>2026-11-10T08:00:00</ActualStartDate><RemainingDuration>28</RemainingDuration>
      <PrimaryConstraintType>Start On or After</PrimaryConstraintType><PrimaryConstraintDate>2026-11-10T08:00:00</PrimaryConstraintDate>
      <SecondaryConstraintType>Finish On or Before</SecondaryConstraintType><SecondaryConstraintDate>2026-12-04T17:00:00</SecondaryConstraintDate></Activity>
    <Activity><ObjectId>1002</ObjectId><Id>A120</Id><Name>Cure</Name><PlannedDuration>30</PlannedDuration><CalendarObjectId>11</CalendarObjectId><WBSObjectId>100</WBSObjectId></Activity>
    <Activity><ObjectId>1003</ObjectId><Id>A130</Id><Name>Review</Name><PlannedDuration>20</PlannedDuration><ExpectedFinishDate>2026-11-20T17:00:00</ExpectedFinishDate></Activity>
    <Activity><ObjectId>1004</ObjectId><Id>M900</Id><Name>Handover</Name><Type>Finish Milestone</Type><PlannedDuration>0</PlannedDuration></Activity>
    <Relationship><ObjectId>1</ObjectId><PredecessorActivityObjectId>1000</PredecessorActivityObjectId><SuccessorActivityObjectId>1001</SuccessorActivityObjectId><Type>Finish to Start</Type><Lag>8</Lag></Relationship>
    <Relationship><ObjectId>2</ObjectId><PredecessorActivityObjectId>1001</PredecessorActivityObjectId><SuccessorActivityObjectId>1002</SuccessorActivityObjectId><Type>Start to Start</Type><Lag>16</Lag></Relationship>
    <Relationship><ObjectId>3</ObjectId><PredecessorActivityObjectId>1002</PredecessorActivityObjectId><SuccessorActivityObjectId>1003</SuccessorActivityObjectId><Type>Finish to Finish</Type><Lag>0</Lag></Relationship>
    <Relationship><ObjectId>4</ObjectId><PredecessorActivityObjectId>1003</PredecessorActivityObjectId><SuccessorActivityObjectId>1004</SuccessorActivityObjectId><Type>Finish to Start</Type><Lag>0</Lag></Relationship>
    <Relationship><ObjectId>5</ObjectId><PredecessorActivityObjectId>1003</PredecessorActivityObjectId><SuccessorActivityObjectId>9999</SuccessorActivityObjectId><Type>Finish to Start</Type></Relationship>
    <Relationship><ObjectId>6</ObjectId><PredecessorActivityObjectId>1000</PredecessorActivityObjectId><SuccessorActivityObjectId>1003</SuccessorActivityObjectId><Type>Start to Finish</Type><Lag>-4</Lag></Relationship>
  </Project>
</APIBusinessObjects>"#
        .to_string()
}

#[test]
fn reads_a_project() {
    let read = read(&file()).unwrap();
    let p = &read.project;
    assert_eq!((p.name.as_str(), p.start, p.must_finish_by), ("Water plant", day(2026, 11, 2), Some(day(2026, 12, 18))));
    // The default calendar: five days, the 2nd off, Saturday the 7th worked; its name unescaped.
    assert_eq!(p.calendar.name, "Standard & Holidays");
    assert_eq!(p.calendar.week, [true, true, true, true, true, false, false]);
    assert!(p.calendar.holidays.contains(&day(2026, 11, 2)));
    assert_eq!(p.calendar.exceptions.get(&day(2026, 11, 7)), Some(&vec![(480, 720)]), "Saturday the 7th, the morning");
    assert_eq!(p.calendars.iter().map(|c| (c.name.as_str(), c.week)).collect::<Vec<_>>(), [("Seven days", [true; 7])]);
    let a = |id: &str| p.activities.iter().find(|a| a.id == id).unwrap();
    // The WBS as summaries; activities under them; hours to days at each calendar's hours.
    assert_eq!(a("PLANT.1.1").parent.as_deref(), Some("PLANT.1"));
    assert_eq!((a("A100").duration, a("A100").parent.as_deref(), a("A100").calendar.as_deref()), (5, Some("PLANT.1.1"), None));
    assert_eq!(a("A110").constraint, Some(Constraint::StartOnOrAfter(day(2026, 11, 10))));
    assert_eq!(a("A110").secondary_constraint, Some(Constraint::FinishOnOrBefore(day(2026, 12, 4))));
    assert_eq!((a("A120").duration, a("A120").calendar.as_deref()), (3, Some("Seven days")), "30 hours at 10 a day");
    assert_eq!(a("M900").kind, ActivityKind::FinishMilestone);
    assert_eq!(a("A100").duration_minutes, Some(2400), "the hours themselves, scheduled on the periods");
    let ties: Vec<(&str, &str, RelationshipType, i64)> =
        p.relationships.iter().map(|r| (r.predecessor.as_str(), r.successor.as_str(), r.kind, r.lag)).collect();
    assert_eq!(
        ties,
        [
            ("A100", "A110", RelationshipType::FinishToStart, 1),
            ("A110", "A120", RelationshipType::StartToStart, 2),
            ("A120", "A130", RelationshipType::FinishToFinish, 0),
            ("A130", "M900", RelationshipType::FinishToStart, 0),
            // A lead of half a day: no whole day, four hours kept in minutes.
            ("A100", "A130", RelationshipType::StartToFinish, 0),
        ]
    );
    assert_eq!(p.relationships[4].lag_minutes, Some(-240));
    // Progress: the data date, a completed activity, one under way with three and a half days left.
    assert_eq!(p.data_date, Some(day(2026, 11, 12)));
    assert_eq!(a("A130").expected_finish, Some(day(2026, 11, 20)));
    assert_eq!(
        (a("A100").actual_start, a("A100").actual_finish, a("A100").remaining_minutes),
        (Some(day(2026, 11, 3)), Some(day(2026, 11, 9)), None)
    );
    assert_eq!((a("A110").actual_start, a("A110").actual_finish, a("A110").remaining_minutes), (Some(day(2026, 11, 10)), None, Some(1680)));
    // Seven days of ten hours from WorkTime without times; the default calendar's 08:00–17:00.
    assert_eq!(p.calendars[0].periods[0], [(480, 1080)]);
    assert_eq!(p.calendar.periods[0], [(480, 1020)]);
    // A relationship to an activity that isn't there.
    assert_eq!(read.warnings.len(), 1, "{:?}", read.warnings);
    // What isn't read is named once.
    let more = file()
        .replace("<Project>", "<Currency><ObjectId>5</ObjectId></Currency><Currency/><Project><ActivityNote><ObjectId>9</ObjectId></ActivityNote>");
    let warned = super::read(&more).unwrap().warnings;
    assert!(warned.iter().any(|w| w.contains("ActivityNote, Currency")), "{warned:?}");
}

#[test]
fn what_is_written_reads_back_the_same() {
    let p = read(&file()).unwrap().project;
    let (text, _) = write(&p);
    let back = read(&text).unwrap();
    assert!(back.warnings.is_empty(), "{:?}", back.warnings);
    let b = back.project;
    assert_eq!((b.name.as_str(), b.start, b.must_finish_by, b.data_date), (p.name.as_str(), p.start, p.must_finish_by, p.data_date));
    assert_eq!(b.calendar.week, p.calendar.week);
    assert_eq!(b.calendar.holidays, p.calendar.holidays);
    assert_eq!(b.calendar.exceptions, p.calendar.exceptions);
    assert_eq!(b.relationships, p.relationships);
    for a in &p.activities {
        let x = b.activities.iter().find(|x| x.id == a.id).unwrap();
        assert_eq!(
            (
                x.name.as_str(),
                x.kind,
                x.working_days(),
                &x.parent,
                x.constraint,
                x.secondary_constraint,
                x.actual_start,
                x.actual_finish,
                x.remaining_minutes,
                x.expected_finish
            ),
            (
                a.name.as_str(),
                a.kind,
                a.working_days(),
                &a.parent,
                a.constraint,
                a.secondary_constraint,
                a.actual_start,
                a.actual_finish,
                a.remaining_minutes,
                a.expected_finish
            ),
            "{}",
            a.id
        );
    }
}

#[test]
fn a_region_calendar_is_written_as_its_holidays() {
    let mut p = read(&file()).unwrap().project;
    p.calendar.region = Some(mkperth_core::Region::Brazil);
    let back = read(&write(&p).0).unwrap().project;
    // Christmas 2026 and Carnival Monday 2027, on weekdays, come back as dates.
    assert!(back.calendar.holidays.contains(&day(2026, 12, 25)) && back.calendar.holidays.contains(&day(2027, 2, 8)));
}

#[test]
fn hostile_files_are_errors_not_crashes() {
    let bad = |text: &str| read(text).err();
    assert!(matches!(bad("not xml <"), Some(P6Error::Xml(_)) | Some(P6Error::NotP6)));
    assert_eq!(bad("<Other/>"), Some(P6Error::NotP6));
    assert_eq!(bad("<APIBusinessObjects></APIBusinessObjects>"), Some(P6Error::NotP6));
    assert!(matches!(bad("<APIBusinessObjects><Project>"), Some(P6Error::Xml(_))));
    assert!(matches!(
        bad("<APIBusinessObjects><Project><Activity><Name>x</Name></Activity></Project></APIBusinessObjects>"),
        Some(P6Error::Field(..))
    ));
    assert!(matches!(
        bad("<APIBusinessObjects><Project><Activity><Id>A</Id><PlannedDuration>1e400</PlannedDuration></Activity></Project></APIBusinessObjects>"),
        Some(P6Error::Field(..))
    ));
    let deep = format!("{}{}", "<a>".repeat(10_000), "</a>".repeat(10_000));
    assert_eq!(bad(&deep), Some(P6Error::TooBig));
    // A huge duration is capped, not overflowed; the scheduler refuses it in its own words.
    let huge = "<APIBusinessObjects><Project><Activity><Id>A</Id><PlannedDuration>1e300</PlannedDuration></Activity></Project></APIBusinessObjects>";
    assert!(read(huge).is_ok());
}

/// Every cut, every byte flipped and every digit made huge: an error or a project, never a panic.
#[test]
fn mutated_files_never_panic() {
    let bytes = file().into_bytes();
    for cut in 0..bytes.len() {
        let _ = read(&String::from_utf8_lossy(bytes.get(..cut).unwrap()));
    }
    for i in 0..bytes.len() {
        let mut flipped = bytes.clone();
        flipped[i] ^= 0xff;
        let _ = read(&String::from_utf8_lossy(&flipped));
        if bytes[i].is_ascii_digit() {
            let mut huge = bytes.clone();
            huge.splice(i..=i, *b"99999999999999999999999");
            let _ = read(&String::from_utf8_lossy(&huge));
        }
    }
}

/// Resources, roles, the roles a resource fills and assignments: written, read back the same.
#[test]
fn resources_and_assignments_read_back() {
    let mut p = read(&file()).unwrap().project;
    let task = p.activities.iter().find(|a| !p.activities.iter().any(|c| c.parent.as_deref() == Some(a.id.as_str()))).unwrap().id.clone();
    p.roles.push(Role { id: "MASON".into(), name: "Mason & co".into(), ..Role::default() });
    p.resources.push(Resource { id: "R1".into(), name: "Ana".into(), roles: vec!["MASON".into()], ..Resource::default() });
    p.resources.push(Resource { id: "CR".into(), name: "Crane".into(), kind: ResourceKind::Nonlabor, max_units: 250, ..Resource::default() });
    p.resources
        .push(Resource { id: "CONC".into(), name: "Concrete".into(), kind: ResourceKind::Material, unit: "m³".into(), ..Resource::default() });
    p.assignments.push(Assignment {
        activity: task.clone(),
        resource: Some("R1".into()),
        role: Some("MASON".into()),
        planned: 4000,
        actual: 1250,
        remaining: Some(3000),
        ..Assignment::default()
    });
    p.assignments.push(Assignment { activity: task.clone(), resource: Some("CONC".into()), planned: 1275, ..Assignment::default() });
    p.assignments.push(Assignment { activity: task.clone(), role: Some("MASON".into()), planned: 800, rate: Some(25), ..Assignment::default() });
    if let Some(a) = p.activities.iter_mut().find(|a| a.id == task) {
        a.duration_type = DurationType::FixedUnits;
        a.resource_dependent = true;
    }
    let (text, _) = write(&p);
    assert!(text.contains("<MaxUnitsPerTime>2.5</MaxUnitsPerTime>") && text.contains("<PlannedUnits>12.75</PlannedUnits>"), "{text}");
    let back = read(&text).unwrap().project;
    assert_eq!(back.roles, p.roles);
    assert_eq!(back.resources, p.resources);
    assert_eq!(back.assignments, p.assignments);
    let types = |p: &Project| p.activities.iter().map(|a| a.duration_type).collect::<Vec<_>>();
    assert_eq!(types(&back), types(&p), "the duration types too");
    assert!(text.contains("<Type>Resource Dependent</Type>"));
    let dependent = |p: &Project| p.activities.iter().map(|a| a.resource_dependent).collect::<Vec<_>>();
    assert_eq!(dependent(&back), dependent(&p), "resource dependent, as P6's type");
}
