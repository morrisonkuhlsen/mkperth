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
        a.levelling_priority = mkperth_core::LevellingPriority::Lowest;
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
    assert!(text.contains("<LevelingPriority>Lowest</LevelingPriority>"));
    let priorities = |p: &Project| p.activities.iter().map(|a| a.levelling_priority).collect::<Vec<_>>();
    assert_eq!(priorities(&back), priorities(&p), "levelling priorities, Normal unwritten");
}

/// Baselines are P6's baseline projects: written and read back with their activities' dates
/// and durations, and the one in use as the project's current baseline.
#[test]
fn baselines_read_back() {
    use mkperth_core::{Baseline, BaselineActivity};
    let mut p = read(&file()).unwrap().project;
    let id = p.activities[0].id.clone();
    p.baselines = vec![
        Baseline {
            name: "Contract".into(),
            activities: vec![BaselineActivity { id: id.clone(), start: day(2026, 11, 3), finish: day(2026, 11, 9), duration: 2400, cost: 150_050 }],
        },
        Baseline {
            name: "Revised & <later>".into(),
            activities: vec![BaselineActivity { id, start: day(2026, 11, 10), finish: day(2026, 11, 16), duration: 2430, cost: 0 }],
        },
    ];
    p.baseline_in_use = Some("Revised & <later>".into());
    let (text, _) = write(&p);
    assert!(text.contains("<BaselineProject>") && text.contains("<CurrentBaselineProjectObjectId>3</CurrentBaselineProjectObjectId>"), "{text}");
    let back = read(&text).unwrap().project;
    assert_eq!((back.baselines, back.baseline_in_use), (p.baselines, p.baseline_in_use));
}

/// Costs as P6 keeps them: resource rates over time, cost accounts in a hierarchy, and expenses
/// with their accrual, written and read back.
#[test]
fn costs_read_back() {
    use mkperth_core::{Accrual, CostAccount, Expense, Price, Resource};
    let mut p = read(&file()).unwrap().project;
    let id = p.activities.iter().find(|a| !p.activities.iter().any(|b| b.parent.as_deref() == Some(a.id.as_str()))).unwrap().id.clone();
    p.resources.push(Resource {
        id: "ANA".into(),
        name: "Ana".into(),
        prices: vec![Price { from: None, per_unit: 2500 }, Price { from: Some(day(2026, 12, 1)), per_unit: 3050 }],
        ..Resource::default()
    });
    p.cost_accounts = vec![
        CostAccount { id: "1".into(), name: "Site".into(), parent: None },
        CostAccount { id: "1.1".into(), name: "Permits & fees".into(), parent: Some("1".into()) },
    ];
    p.expenses = vec![
        Expense {
            activity: id.clone(),
            name: "Permit".into(),
            account: Some("1.1".into()),
            planned: 45_050,
            actual: 45_050,
            remaining: Some(0),
            accrual: Accrual::Start,
        },
        Expense { activity: id, name: "Crane".into(), account: None, planned: 120_000, actual: 0, remaining: None, accrual: Accrual::Uniform },
    ];
    let (text, _) = write(&p);
    assert!(text.contains("<AccrualType>Start of Activity</AccrualType>") && text.contains("<PricePerUnit>30.5</PricePerUnit>"), "{text}");
    let back = read(&text).unwrap().project;
    assert_eq!(back.resources.iter().find(|r| r.id == "ANA").map(|r| r.prices.clone()), Some(p.resources.last().unwrap().prices.clone()));
    assert_eq!(back.cost_accounts, p.cost_accounts);
    assert_eq!(back.expenses, p.expenses);
}

/// The budget as P6 keeps it: the project's and its WBS's original budgets, the change log, the
/// project's and a WBS's spending plans, funding sources in a hierarchy and the project's funds;
/// written and read back as they were. A budget on an activity that isn't a summary is left out
/// with a warning; nothing MK Perth writes comes back as "left out, not read".
#[test]
fn the_budget_read_back() {
    use mkperth_core::{BudgetChange, ChangeStatus, Fund, FundingSource, SpendingMonth, WbsBudget};
    let mut p = read(&file()).unwrap().project;
    let parents: Vec<String> = p.activities.iter().filter_map(|a| a.parent.clone()).collect();
    let wbs = parents.first().unwrap().clone();
    let leaf = p.activities.iter().find(|a| !parents.contains(&a.id)).unwrap().id.clone();
    let month = |y, m, amount| SpendingMonth { month: day(y, m, 1), amount };
    p.budget.budgets = vec![
        WbsBudget { wbs: None, original: 100_000_000, plan: vec![month(2026, 11, 40_000_000), month(2026, 12, 60_000_050)] },
        WbsBudget { wbs: Some(wbs.clone()), original: 30_000_000, plan: vec![month(2026, 11, 30_000_000)] },
    ];
    let change = |number, wbs: Option<&str>, amount, status| BudgetChange {
        number,
        wbs: wbs.map(str::to_string),
        date: day(2026, 11, 10),
        amount,
        status,
        responsible: "PM & co".into(),
        reason: "<scope>".into(),
    };
    p.budget.log = vec![
        change(1, None, 5_000_000, ChangeStatus::Approved),
        change(3, Some(&wbs), -2_000_000, ChangeStatus::Pending),
        change(4, None, 100, ChangeStatus::NotApproved),
    ];
    p.budget.sources = vec![
        FundingSource { id: "BANK".into(), name: "Bank loan".into(), parent: None },
        FundingSource { id: "BNDES".into(), name: "Development bank".into(), parent: Some("BANK".into()) },
    ];
    p.budget.funds = vec![Fund { source: "BNDES".into(), amount: 70_000_000 }];
    let (text, warnings) = write(&p);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(text.contains("<Status>Not Approved</Status>") && text.contains("<FundShare>66.67</FundShare>"), "{text}");
    let read_back = read(&text).unwrap();
    assert!(!read_back.warnings.iter().any(|w| w.starts_with("Left out")), "{:?}", read_back.warnings);
    assert_eq!(read_back.project.budget, p.budget);
    // On a leaf it can't be written.
    p.budget.budgets.push(WbsBudget { wbs: Some(leaf.clone()), original: 1, plan: Vec::new() });
    p.budget.log.push(change(5, Some(&leaf), 1, ChangeStatus::Approved));
    let (text, warnings) = write(&p);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].starts_with("2 budgets"));
    assert_eq!(read(&text).unwrap().project.budget.budgets.len(), 2);
}

/// Earned value as P6 keeps it: each technique (by percent complete type, 0/100, 50/50, a custom
/// share) and each estimate to complete written and read back, the physical percent complete,
/// the financial periods; the stored closes left out with a warning.
#[test]
fn earned_value_read_back() {
    use mkperth_core::{EtcTechnique, EvTechnique, FinancialPeriod, PeriodRecord};
    let mut p = read(&file()).unwrap().project;
    let leaf = p.activities.iter().position(|a| !p.activities.iter().any(|b| b.parent.as_deref() == Some(a.id.as_str()))).unwrap();
    p.activities[leaf].percent_complete = Some(3_350);
    p.earned_value.periods = vec![
        FinancialPeriod { name: "2026-11".into(), start: day(2026, 11, 1), finish: day(2026, 11, 30) },
        FinancialPeriod { name: "2026-12".into(), start: day(2026, 12, 1), finish: day(2026, 12, 31) },
    ];
    let techniques = [
        EvTechnique::Physical,
        EvTechnique::Duration,
        EvTechnique::Units,
        EvTechnique::ZeroHundred,
        EvTechnique::FiftyFifty,
        EvTechnique::Custom(2_550),
    ];
    for (technique, etc) in techniques.into_iter().zip(EtcTechnique::ALL.into_iter().cycle()) {
        p.earned_value.technique = technique;
        p.earned_value.etc = etc;
        let (text, warnings) = write(&p);
        assert!(warnings.is_empty(), "{warnings:?}");
        let back = read(&text).unwrap();
        assert!(!back.warnings.iter().any(|w| w.starts_with("Left out")), "{:?}", back.warnings);
        assert_eq!(back.project.earned_value, p.earned_value, "{technique:?}");
        assert_eq!(back.project.activities[leaf].percent_complete, Some(3_350));
    }
    // Each period's own amounts by kind travel as ActivityPeriodActual (#34).
    let id = p.activities[leaf].id.clone();
    for (period, labor) in [("2026-11", 40_000), ("2026-12", 25_050)] {
        p.earned_value.records.push(PeriodRecord {
            period: period.into(),
            activity: id.clone(),
            planned: 50_000,
            earned: 45_000,
            labor_cost: labor,
            nonlabor_cost: 12_000,
            material_cost: 8_000,
            expense_cost: 3_000,
            labor_units: 3_250,
            nonlabor_units: 800,
        });
    }
    let (text, warnings) = write(&p);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(text.contains("<ActualLaborCost>250.5</ActualLaborCost>") && text.contains("<ActualLaborUnits>32.5</ActualLaborUnits>"), "{text}");
    let back = read(&text).unwrap();
    assert!(!back.warnings.iter().any(|w| w.starts_with("Left out")), "{:?}", back.warnings);
    assert_eq!(back.project.earned_value.records, p.earned_value.records);
}

/// The names checked against Oracle's public P6 reference (#31): the estimate to complete's
/// performance factors spelt out, a WBS's spending plan as a project spending plan naming it, a
/// resource's role with no object number, a calendar's type. Files MK Perth wrote before still
/// read: `PF = …` and `WBSSpendingPlan`.
#[test]
fn names_as_oracle_documents_them() {
    use mkperth_core::{EtcTechnique, Resource, Role, SpendingMonth, WbsBudget};
    let mut p = read(&file()).unwrap().project;
    let wbs = p.activities.iter().find_map(|a| a.parent.clone()).unwrap();
    p.earned_value.etc = EtcTechnique::CpiSpi;
    p.budget.budgets = vec![WbsBudget { wbs: Some(wbs.clone()), original: 100, plan: vec![SpendingMonth { month: day(2026, 11, 1), amount: 100 }] }];
    p.roles = vec![Role { id: "ENG".into(), name: "Engineer".into(), ..Role::default() }];
    p.resources.push(Resource { id: "ANA".into(), roles: vec!["ENG".into()], ..Resource::default() });
    let (text, _) = write(&p);
    assert!(text.contains(
        "<EarnedValueETCComputeType>Performance Factor = 1 / (Cost Performance Index * Schedule Performance Index)</EarnedValueETCComputeType>"
    ));
    assert!(text.contains("<ProjectSpendingPlan>") && !text.contains("WBSSpendingPlan"));
    assert!(text.contains("<ResourceRole>\n    <ResourceObjectId>"), "no ObjectId before the pair");
    assert!(text.contains("<Type>Global</Type>"));
    let back = read(&text).unwrap().project;
    assert_eq!((back.budget, back.earned_value.etc), (p.budget.clone(), EtcTechnique::CpiSpi));
    // Before #31.
    let before = text
        .replace(
            "Performance Factor = 1 / (Cost Performance Index * Schedule Performance Index)",
            "PF = 1 / (Cost Performance Index * Schedule Performance Index)",
        )
        .replace("ProjectSpendingPlan>", "WBSSpendingPlan>");
    let back = read(&before).unwrap().project;
    assert_eq!((back.budget, back.earned_value.etc), (p.budget, EtcTechnique::CpiSpi));
}

/// Activity codes and user-defined fields as P6 keeps them: a code with a value under another,
/// fields of every kind, each activity's values, written and read back; a field for resources
/// left out with a warning.
#[test]
fn codes_and_fields_read_back() {
    use mkperth_core::{ActivityCode, Classification, CodeValue, FieldKind, FieldValue, Indicator, UserField};
    let mut p = read(&file()).unwrap().project;
    let leaves: Vec<usize> =
        (0..p.activities.len()).filter(|&i| !p.activities.iter().any(|b| b.parent.as_deref() == Some(p.activities[i].id.as_str()))).collect();
    p.classification = Classification {
        codes: vec![ActivityCode {
            name: "Phase & stage".into(),
            values: vec![
                CodeValue { code: "DES".into(), description: "Design".into(), parent: None },
                CodeValue { code: "DES.A".into(), description: "Architecture".into(), parent: Some("DES".into()) },
            ],
        }],
        fields: FieldKind::ALL.iter().map(|k| UserField { name: format!("F {}", k.code()), kind: *k }).collect(),
    };
    let a = &mut p.activities[leaves[0]];
    a.codes.insert("Phase & stage".into(), "DES.A".into());
    for v in [
        FieldValue::Text("<risky>".into()),
        FieldValue::Number(2_500_000),
        FieldValue::Integer(-7),
        FieldValue::Cost(125_050),
        FieldValue::Date(day(2026, 11, 20)),
        FieldValue::Indicator(Indicator::Yellow),
    ] {
        a.fields.insert(format!("F {}", v.kind().code()), v);
    }
    let (text, warnings) = write(&p);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(text.contains("<DataType>Double</DataType>") && text.contains("<Indicator>Yellow</Indicator>"), "{text}");
    let back = read(&text).unwrap();
    assert!(!back.warnings.iter().any(|w| w.starts_with("Left out")), "{:?}", back.warnings);
    assert_eq!(back.project.classification, p.classification);
    assert_eq!(back.project.activities[leaves[0]].codes, p.activities[leaves[0]].codes);
    assert_eq!(back.project.activities[leaves[0]].fields, p.activities[leaves[0]].fields);
    // A field P6 keeps for resources isn't an activity's.
    let other = text
        .replace("<SubjectArea>Activity</SubjectArea>\n  </UDFType>\n  <UDFType>", "<SubjectArea>Resource</SubjectArea>\n  </UDFType>\n  <UDFType>");
    assert_eq!(read(&other).unwrap().warnings.len(), 1);
}

/// The organisation as P6 keeps it: the EPS from the top down to the project's node, the OBS in
/// its hierarchy, who answers for the project and a WBS; written and read back. One on an
/// activity that isn't a summary is left out, with a warning.
#[test]
fn organisation_read_back() {
    use mkperth_core::{EpsNode, ObsNode, Organisation};
    let mut p = read(&file()).unwrap().project;
    let wbs = p.activities.iter().find_map(|a| a.parent.clone()).unwrap();
    p.organisation = Organisation {
        eps: vec![EpsNode { id: "MK".into(), name: "Morrison Kühlsen".into() }, EpsNode { id: "MK.B".into(), name: "Buildings & works".into() }],
        obs: vec![
            ObsNode { name: "Engineering".into(), description: "All engineering".into(), parent: None },
            ObsNode { name: "Structures".into(), description: String::new(), parent: Some("Engineering".into()) },
        ],
        responsible: Some("Engineering".into()),
    };
    p.activities.iter_mut().find(|a| a.id == wbs).unwrap().responsible = Some("Structures".into());
    let (text, warnings) = write(&p);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(text.contains("<ParentEPSObjectId>2</ParentEPSObjectId>") && text.contains("<ObsObjectId>1</ObsObjectId>"), "{text}");
    let back = read(&text).unwrap();
    assert!(!back.warnings.iter().any(|w| w.starts_with("Left out")), "{:?}", back.warnings);
    assert_eq!(back.project.organisation, p.organisation);
    assert_eq!(back.project.activities.iter().find(|a| a.id == wbs).unwrap().responsible.as_deref(), Some("Structures"));
    let leaf = p.activities.iter().position(|a| !p.activities.iter().any(|b| b.parent.as_deref() == Some(a.id.as_str()))).unwrap();
    p.activities[leaf].responsible = Some("Engineering".into());
    assert_eq!(write(&p).1.len(), 1);
}
