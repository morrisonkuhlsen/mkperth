#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use mkperth_core::{Constraint, Date, DurationType, RelationshipType, ResourceKind, UNIT};

use super::*;

fn day(y: i32, m: u32, d: u32) -> Date {
    Date::from_ymd(y, m, d).unwrap()
}

/// A file built by hand from the published schema: 480 minutes a day; a Monday-to-Friday calendar
/// of 08:00–12:00 and 13:00–17:00 with two days off (24 and 25 December 2026) and a Saturday
/// worked in the morning (28 November); a summary over tasks whose durations are written in
/// each unit, links whose lags are written in each format, every kind of constraint, a work and
/// a material resource and their assignments.
fn file() -> String {
    let task = |uid: u32, wbs: &str, level: u8, extra: &str| {
        format!("<Task><UID>{uid}</UID><ID>{uid}</ID><Name>Task {uid}</Name><WBS>{wbs}</WBS><OutlineLevel>{level}</OutlineLevel>{extra}</Task>")
    };
    let link = |pred: u32, kind: u8, lag: i64, format: u8| {
        format!(
            "<PredecessorLink><PredecessorUID>{pred}</PredecessorUID><Type>{kind}</Type><LinkLag>{lag}</LinkLag><LagFormat>{format}</LagFormat></PredecessorLink>"
        )
    };
    let hours = "<WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>12:00:00</ToTime></WorkingTime><WorkingTime><FromTime>13:00:00</FromTime><ToTime>17:00:00</ToTime></WorkingTime></WorkingTimes>";
    let weekday = |t: u8, working: bool| {
        format!("<WeekDay><DayType>{t}</DayType><DayWorking>{}</DayWorking>{}</WeekDay>", u8::from(working), if working { hours } else { "" })
    };
    let week: String = (1..=7).map(|t| weekday(t, (2..=6).contains(&t))).collect();
    let tasks = [
        task(0, "", 0, "<Summary>1</Summary>"),
        task(1, "W", 1, "<Summary>1</Summary>"),
        // 40 hours shown in days: 5 days.
        task(2, "D", 2, "<Duration>PT40H0M0S</Duration><DurationFormat>7</DurationFormat><ConstraintType>4</ConstraintType><ConstraintDate>2026-11-03T08:00:00</ConstraintDate>"),
        // 12 hours shown in hours: a day and a half, kept in minutes.
        task(3, "H", 2, &format!("<Duration>PT12H0M0S</Duration><DurationFormat>5</DurationFormat>{}", link(2, 1, 900, 3))),
        // 80 hours shown in weeks: 10 days.
        task(4, "WK", 2, &format!("<Duration>PT80H0M0S</Duration><DurationFormat>9</DurationFormat>{}", link(2, 3, 4800, 5))),
        // 160 hours shown in months: 20 days.
        task(5, "MO", 2, &format!("<Duration>PT160H0M0S</Duration><DurationFormat>11</DurationFormat>{}", link(2, 0, 9600, 7))),
        // 90 minutes shown in minutes.
        task(6, "M", 1, &format!("<Duration>PT1H30M0S</Duration><DurationFormat>3</DurationFormat>{}{}", link(3, 1, 24000, 9), link(4, 2, 96000, 11))),
        // 48 elapsed hours: two days on the clock.
        task(7, "E", 1, &format!("<Duration>PT48H0M0S</Duration><DurationFormat>8</DurationFormat>{}{}", link(6, 1, 600, 8), link(6, 3, 500, 19))),
        task(8, "C1", 1, "<Milestone>1</Milestone><Duration>PT0H0M0S</Duration><ConstraintType>7</ConstraintType><ConstraintDate>2027-02-26T17:00:00</ConstraintDate><Deadline>2027-03-05T17:00:00</Deadline>"),
        task(9, "C2", 1, "<Duration>PT8H0M0S</Duration><ConstraintType>1</ConstraintType><Type>2</Type><PhysicalPercentComplete>40</PhysicalPercentComplete>"),
        task(10, "C3", 1, "<Duration>PT8H0M0S</Duration><ConstraintType>2</ConstraintType><ConstraintDate>2026-12-01T08:00:00</ConstraintDate><Type>0</Type>"),
    ]
    .concat();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Project xmlns="http://schemas.microsoft.com/project">
<Name>Hand &amp; built</Name><StartDate>2026-11-02T08:00:00</StartDate><MinutesPerDay>480</MinutesPerDay><CalendarUID>1</CalendarUID>
<StatusDate>2026-11-16T08:00:00</StatusDate><CurrencySymbol>R$</CurrencySymbol>
<Calendars><Calendar><UID>1</UID><Name>Standard</Name><IsBaseCalendar>1</IsBaseCalendar><BaseCalendarUID>-1</BaseCalendarUID>
<WeekDays>{week}</WeekDays>
<Exceptions>
<Exception><TimePeriod><FromDate>2026-12-24T00:00:00</FromDate><ToDate>2026-12-25T23:59:00</ToDate></TimePeriod><Type>1</Type><DayWorking>0</DayWorking></Exception>
<Exception><TimePeriod><FromDate>2026-11-28T00:00:00</FromDate><ToDate>2026-11-28T23:59:00</ToDate></TimePeriod><Type>1</Type><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>12:00:00</ToTime></WorkingTime></WorkingTimes></Exception>
<Exception><TimePeriod><FromDate>2026-01-01T00:00:00</FromDate><ToDate>2030-01-01T23:59:00</ToDate></TimePeriod><Type>2</Type><Month>0</Month><MonthDay>1</MonthDay><DayWorking>0</DayWorking></Exception>
</Exceptions></Calendar></Calendars>
<Tasks>{tasks}</Tasks>
<Resources><Resource><UID>0</UID><Name>Unassigned</Name></Resource>
<Resource><UID>1</UID><ID>1</ID><Name>Ana</Name><Type>1</Type><MaxUnits>0.5</MaxUnits><StandardRate>85.5</StandardRate></Resource>
<Resource><UID>2</UID><ID>2</ID><Name>Concrete</Name><Type>0</Type><MaterialLabel>m3</MaterialLabel></Resource>
<Resource><UID>3</UID><ID>3</ID><Name>Travel</Name><Type>2</Type></Resource></Resources>
<Assignments>
<Assignment><UID>1</UID><TaskUID>2</TaskUID><ResourceUID>1</ResourceUID><Units>0.5</Units><Work>PT20H0M0S</Work><ActualWork>PT4H0M0S</ActualWork><RemainingWork>PT16H0M0S</RemainingWork></Assignment>
<Assignment><UID>2</UID><TaskUID>4</TaskUID><ResourceUID>2</ResourceUID><Units>1</Units><Work>PT30H0M0S</Work><ActualWork>PT0H0M0S</ActualWork><RemainingWork>PT30H0M0S</RemainingWork></Assignment>
</Assignments>
</Project>"#
    )
}

/// Each duration and lag unit converts to working time: durations from their hours whatever the
/// format they're shown in (days, hours, weeks, months, minutes; elapsed on a 24-hour calendar),
/// lags from their tenths of a minute (90 minutes; 8 hours, a whole day; 2 days; 5 days in
/// weeks; 20 days in months). An elapsed lag is read as working time and a percent one as none,
/// each with a warning; the recurring exception and the cost resource are left out, said.
#[test]
fn every_unit_converts() {
    let r = read(&file()).unwrap();
    let p = &r.project;
    let a = |id: &str| p.activities.iter().find(|a| a.id == id).unwrap();
    assert_eq!(p.name, "Hand & built");
    assert_eq!((p.start, p.data_date, p.currency.as_str()), (day(2026, 11, 2), Some(day(2026, 11, 16)), "R$"));
    assert_eq!(p.activities.len(), 10, "the project's own summary task isn't an activity");
    assert_eq!(a("D").parent.as_deref(), Some("W"));
    assert_eq!(a("M").parent, None);
    assert_eq!((a("D").duration, a("D").duration_minutes), (5, None));
    assert_eq!((a("H").duration, a("H").duration_minutes), (2, Some(720)));
    assert_eq!(a("WK").duration, 10);
    assert_eq!(a("MO").duration, 20);
    assert_eq!(a("M").duration_minutes, Some(90));
    assert_eq!((a("E").calendar.as_deref(), a("E").duration), (Some(ELAPSED), 2));
    let lag = |pred: &str, succ: &str| {
        let r = p.relationships.iter().find(|r| r.predecessor == pred && r.successor == succ).unwrap();
        (r.kind, r.lag, r.lag_minutes)
    };
    assert_eq!(lag("D", "H"), (RelationshipType::FinishToStart, 0, Some(90)));
    assert_eq!(lag("D", "WK"), (RelationshipType::StartToStart, 1, None));
    assert_eq!(lag("D", "MO"), (RelationshipType::FinishToFinish, 2, None));
    assert_eq!(lag("H", "M"), (RelationshipType::FinishToStart, 5, None));
    assert_eq!(lag("WK", "M"), (RelationshipType::StartToFinish, 20, None));
    assert_eq!(lag("M", "E").2, Some(60), "elapsed, read as working time");
    assert_eq!(p.relationships.iter().filter(|r| r.successor == "E").count(), 2);
    // Constraints, the deadline, the task types, the percent complete.
    assert_eq!(a("D").constraint, Some(Constraint::StartOnOrAfter(day(2026, 11, 3))));
    assert_eq!(a("C1").constraint, Some(Constraint::FinishOnOrBefore(day(2027, 2, 26))));
    assert_eq!(a("C1").secondary_constraint, Some(Constraint::FinishOnOrBefore(day(2027, 3, 5))));
    assert_eq!(
        (a("C2").constraint, a("C2").duration_type, a("C2").percent_complete),
        (Some(Constraint::AsLateAsPossible), DurationType::FixedUnits, Some(4_000))
    );
    assert_eq!((a("C3").constraint, a("C3").duration_type), (Some(Constraint::StartOn(day(2026, 12, 1))), DurationType::FixedRate));
    assert!(a("C1").kind.is_milestone());
    // The calendar: its days off, its Saturday morning.
    assert!(p.calendar.holidays.contains(&day(2026, 12, 24)) && p.calendar.holidays.contains(&day(2026, 12, 25)));
    assert_eq!(p.calendar.exceptions.get(&day(2026, 11, 28)), Some(&vec![(480, 720)]));
    assert_eq!(p.calendar.periods[0], vec![(480, 720), (780, 1020)]);
    // Resources and assignments.
    let ana = p.resources.iter().find(|r| r.name == "Ana").unwrap();
    assert_eq!((ana.max_units, ana.prices[0].per_unit), (50, 8_550));
    assert_eq!(p.resources.iter().find(|r| r.name == "Concrete").map(|r| (r.kind, r.unit.as_str())), Some((ResourceKind::Material, "m3")));
    assert_eq!(p.resources.len(), 2);
    let x = &p.assignments[0];
    assert_eq!((x.planned, x.actual, x.remaining, x.rate), (20 * UNIT, 4 * UNIT, None, Some(50)));
    // Warnings: the elapsed and percent lags, the recurring exception, the cost resource.
    assert_eq!(r.warnings.len(), 3, "{:?}", r.warnings);
}

/// Written and read back, the project is the same: IDs (kept as WBS codes), the outline,
/// durations, lags, constraints, the calendar, resources and assignments; and it schedules to the
/// same dates.
#[test]
fn round_trip() {
    let p = read(&file()).unwrap().project;
    let (text, warnings) = write(&p);
    assert!(warnings.is_empty(), "{warnings:?}");
    let back = read(&text).unwrap();
    assert!(back.warnings.is_empty(), "{:?}", back.warnings);
    let q = back.project;
    let ids = |p: &Project| p.activities.iter().map(|a| a.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&q), ids(&p));
    for (a, b) in p.activities.iter().zip(&q.activities) {
        assert_eq!(
            (&a.parent, a.duration, a.duration_minutes, a.constraint, a.secondary_constraint),
            (&b.parent, b.duration, b.duration_minutes, b.constraint, b.secondary_constraint),
            "{}",
            a.id
        );
        assert_eq!(
            (&a.calendar, a.duration_type, a.percent_complete, a.kind),
            (&b.calendar, b.duration_type, b.percent_complete, b.kind),
            "{}",
            a.id
        );
    }
    let mut r1 = p.relationships.clone();
    let mut r2 = q.relationships.clone();
    let key = |r: &Relationship| (r.predecessor.clone(), r.successor.clone());
    r1.sort_by_key(key);
    r2.sort_by_key(key);
    assert_eq!(r1, r2);
    assert_eq!((&q.calendar.holidays, &q.calendar.exceptions, &q.calendar.week), (&p.calendar.holidays, &p.calendar.exceptions, &p.calendar.week));
    assert_eq!(q.resources, p.resources);
    assert_eq!(q.assignments, p.assignments);
    let (s1, s2) = (mkperth_sched::schedule(&p).unwrap(), mkperth_sched::schedule(&q).unwrap());
    assert_eq!(s1.activities, s2.activities);
}

/// A duration in the schema's own form: hours, minutes and seconds, and days of 24 hours.
#[test]
fn schema_durations() {
    assert_eq!(duration_of("PT8H0M0S"), Some(480));
    assert_eq!(duration_of("PT1H30M0S"), Some(90));
    assert_eq!(duration_of("PT0H0M30S"), Some(1));
    assert_eq!(duration_of("P1DT2H0M0S"), Some(1560));
    assert_eq!(duration_of("PT7.5H"), Some(450));
    for bad in ["8H", "PT8X", "PTH", "P-1D", ""] {
        assert_eq!(duration_of(bad), None, "{bad}");
    }
    assert_eq!(duration(450), "PT7H30M0S");
}

/// Hostile input: not a project, not XML, nested too deep; never a panic.
#[test]
fn hostile_files() {
    assert_eq!(read("<APIBusinessObjects/>"), Err(MspError::NotMsp));
    assert!(matches!(read("<Project><Tasks>"), Err(MspError::Xml(_))));
    let deep = "<a>".repeat(100) + &"</a>".repeat(100);
    assert!(read(&deep).is_err());
    // A link to a task that isn't there, an exception over a century: read, capped, said.
    let text = file()
        .replace("<PredecessorUID>2</PredecessorUID><Type>3</Type>", "<PredecessorUID>99</PredecessorUID><Type>3</Type>")
        .replace("2026-12-25T23:59:00", "2126-12-25T23:59:00");
    let r = read(&text).unwrap();
    assert!(r.project.calendar.holidays.len() <= MAX_EXCEPTION_DAYS as usize + 1);
    assert!(r.warnings.iter().any(|w| w.contains("isn't in the file")), "{:?}", r.warnings);
}
