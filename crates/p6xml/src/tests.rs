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
    <ActivityDefaultCalendarObjectId>10</ActivityDefaultCalendarObjectId>
    <WBS><ObjectId>100</ObjectId><Code>PLANT.1</Code><Name>Design</Name></WBS>
    <WBS><ObjectId>101</ObjectId><Code>PLANT.1.1</Code><Name>Civil</Name><ParentObjectId>100</ParentObjectId></WBS>
    <Activity><ObjectId>1000</ObjectId><Id>A100</Id><Name>Survey</Name><Type>Task Dependent</Type><PlannedDuration>40</PlannedDuration><CalendarObjectId>10</CalendarObjectId><WBSObjectId>101</WBSObjectId></Activity>
    <Activity><ObjectId>1001</ObjectId><Id>A110</Id><Name>Design</Name><Type>Resource Dependent</Type><PlannedDuration>60</PlannedDuration><WBSObjectId>101</WBSObjectId>
      <PrimaryConstraintType>Start On or After</PrimaryConstraintType><PrimaryConstraintDate>2026-11-10T08:00:00</PrimaryConstraintDate>
      <SecondaryConstraintType>Finish On or Before</SecondaryConstraintType><SecondaryConstraintDate>2026-12-04T17:00:00</SecondaryConstraintDate></Activity>
    <Activity><ObjectId>1002</ObjectId><Id>A120</Id><Name>Cure</Name><PlannedDuration>30</PlannedDuration><CalendarObjectId>11</CalendarObjectId><WBSObjectId>100</WBSObjectId></Activity>
    <Activity><ObjectId>1003</ObjectId><Id>A130</Id><Name>Review</Name><PlannedDuration>20</PlannedDuration></Activity>
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
    assert!(p.calendar.holidays.contains(&day(2026, 11, 2)) && p.calendar.extra_days.contains(&day(2026, 11, 7)));
    assert_eq!(p.calendars.iter().map(|c| (c.name.as_str(), c.week)).collect::<Vec<_>>(), [("Seven days", [true; 7])]);
    let a = |id: &str| p.activities.iter().find(|a| a.id == id).unwrap();
    // The WBS as summaries; activities under them; hours to days at each calendar's hours.
    assert_eq!(a("PLANT.1.1").parent.as_deref(), Some("PLANT.1"));
    assert_eq!((a("A100").duration, a("A100").parent.as_deref(), a("A100").calendar.as_deref()), (5, Some("PLANT.1.1"), None));
    assert_eq!(a("A110").constraint, Some(Constraint::StartOnOrAfter(day(2026, 11, 10))));
    assert_eq!(a("A110").secondary_constraint, Some(Constraint::FinishOnOrBefore(day(2026, 12, 4))));
    assert_eq!((a("A120").duration, a("A120").calendar.as_deref()), (3, Some("Seven days")), "30 hours at 10 a day");
    assert_eq!(a("M900").kind, ActivityKind::FinishMilestone);
    let ties: Vec<(&str, &str, RelationshipType, i64)> =
        p.relationships.iter().map(|r| (r.predecessor.as_str(), r.successor.as_str(), r.kind, r.lag)).collect();
    assert_eq!(
        ties,
        [
            ("A100", "A110", RelationshipType::FinishToStart, 1),
            ("A110", "A120", RelationshipType::StartToStart, 2),
            ("A120", "A130", RelationshipType::FinishToFinish, 0),
            ("A130", "M900", RelationshipType::FinishToStart, 0),
            // A lead of half a day rounds up, the cautious way: none.
            ("A100", "A130", RelationshipType::StartToFinish, 0),
        ]
    );
    // A relationship to an activity that isn't there, and a lead of half a day.
    assert_eq!(read.warnings.len(), 2, "{:?}", read.warnings);
    // What isn't read is named once.
    let more = file().replace(
        "<Project>",
        "<Resource><ObjectId>5</ObjectId></Resource><Resource/><Project><ResourceAssignment><ObjectId>9</ObjectId></ResourceAssignment>",
    );
    let warned = super::read(&more).unwrap().warnings;
    assert!(warned.iter().any(|w| w.contains("Resource, ResourceAssignment")), "{warned:?}");
}

#[test]
fn what_is_written_reads_back_the_same() {
    let p = read(&file()).unwrap().project;
    let (text, _) = write(&p);
    let back = read(&text).unwrap();
    assert!(back.warnings.is_empty(), "{:?}", back.warnings);
    let b = back.project;
    assert_eq!((b.name.as_str(), b.start, b.must_finish_by), (p.name.as_str(), p.start, p.must_finish_by));
    assert_eq!(b.calendar.week, p.calendar.week);
    assert_eq!(b.calendar.holidays, p.calendar.holidays);
    assert_eq!(b.relationships, p.relationships);
    for a in &p.activities {
        let x = b.activities.iter().find(|x| x.id == a.id).unwrap();
        assert_eq!(
            (x.name.as_str(), x.kind, x.working_days(), &x.parent, x.constraint, x.secondary_constraint),
            (a.name.as_str(), a.kind, a.working_days(), &a.parent, a.constraint, a.secondary_constraint),
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
