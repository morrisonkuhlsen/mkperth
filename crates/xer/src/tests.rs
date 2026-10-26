#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use mkperth_core::{Accrual, Constraint, Date, FieldValue, RelationshipType, ResourceKind};

use super::*;

fn day(y: i32, m: u32, d: u32) -> Date {
    Date::from_ymd(y, m, d).unwrap()
}

/// An XER file from its tables: each its name, its columns and its rows, fields split by `|`
/// written as tabs; `@WEEK@` stands for the calendar below (whose own `|` stay).
fn xer(tables: &[(&str, &str, &[&str])]) -> String {
    let mut out = String::from("ERMHDR\t19.12\t2026-10-25\tProject\tadmin\tTest\tdb\tProject Management\tBRL\r\n");
    for (name, columns, rows) in tables {
        out.push_str(&format!("%T\t{name}\r\n%F\t{}\r\n", columns.replace(' ', "\t")));
        for r in *rows {
            out.push_str(&format!("%R\t{}\r\n", r.replace('|', "\t").replace("@WEEK@", WEEK)));
        }
    }
    out.push_str("%E\r\n");
    out
}

/// Monday to Friday 08:00–12:00 and 13:00–17:00; Christmas 2026 off (46381, days from
/// 30 December 1899) and Saturday 28 November 2026 worked 08:00–12:00 (46354).
const WEEK: &str = "(0||CalendarData()((0||DaysOfWeek()((0||1()())(0||2()((0||0(s|08:00|f|12:00)())(0||1(s|13:00|f|17:00)())))\
(0||3()((0||0(s|08:00|f|12:00)())(0||1(s|13:00|f|17:00)())))(0||4()((0||0(s|08:00|f|12:00)())(0||1(s|13:00|f|17:00)())))\
(0||5()((0||0(s|08:00|f|12:00)())(0||1(s|13:00|f|17:00)())))(0||6()((0||0(s|08:00|f|12:00)())(0||1(s|13:00|f|17:00)())))\
(0||7()())))(0||VIEW(ShowTotal|Y)())(0||Exceptions()((0||0(d|46381)())(0||1(d|46354)((0||0(s|08:00|f|12:00)())))))))";

/// A file built by hand from Oracle's published tables and codes: a project with a WBS element
/// under its own node, four activities (two tasks, a finish milestone, a task constrained to
/// start on or after 30 November), relationships of two kinds with lags, a labour and a material
/// resource with prices, an expense, an activity code, a user-defined field, a financial period
/// with its actuals; and a second project whose rows are left out.
fn file() -> String {
    xer(&[
        ("CURRTYPE", "curr_id curr_short_name curr_symbol base_exch_rate", &["1|BRL|R$|1"]),
        ("OBS", "obs_id obs_name parent_obs_id", &["1|Diretoria|", "2|Obras|1"]),
        ("UMEASURE", "unit_id unit_abbrev unit_name", &["1|m3|Metro cúbico"]),
        ("CALENDAR", "clndr_id clndr_name day_hr_cnt clndr_data proj_id", &["1|Standard|8|@WEEK@|", "2|Outro|8||9"]),
        (
            "PROJECT",
            "proj_id proj_short_name clndr_id plan_start_date last_recalc_date plan_end_date",
            &["1|OBRA-1|1|2026-11-02 08:00||", "9|OUTRO|1|2026-11-02 08:00||"],
        ),
        (
            "PROJWBS",
            "wbs_id proj_id proj_node_flag wbs_short_name wbs_name parent_wbs_id orig_cost obs_id",
            &["10|1|Y|OBRA-1|Obra & cia <1>||50000|1", "11|1|N|E|Estrutura|10|20000|2", "90|9|Y|OUTRO|Outro|||"],
        ),
        ("RSRC", "rsrc_id rsrc_short_name rsrc_name rsrc_type clndr_id unit_id", &["1|PED|Pedreiro|RT_Labor|1|", "2|CON|Concreto|RT_Mat||1"]),
        (
            "RSRCRATE",
            "rsrc_rate_id rsrc_id start_date cost_per_qty max_qty_per_hr",
            &["1|1|2026-01-01 00:00|50|2", "2|1|2027-01-01 00:00|55|2", "3|2|2026-01-01 00:00|400|"],
        ),
        ("ACCOUNT", "acct_id acct_short_name acct_name parent_acct_id", &["1|C1|Civil|"]),
        ("ACTVTYPE", "actv_code_type_id actv_code_type proj_id seq_num", &["1|Fase||0"]),
        ("ACTVCODE", "actv_code_id actv_code_type_id short_name actv_code_name parent_actv_code_id seq_num", &["1|1|F1|Fundações||0"]),
        (
            "TASK",
            "task_id proj_id wbs_id clndr_id task_code task_name task_type status_code duration_type priority_type complete_pct_type \
             phys_complete_pct target_drtn_hr_cnt remain_drtn_hr_cnt act_start_date act_end_date cstr_type cstr_date",
            &[
                "100|1|11|1|A1000|Escavação|TT_Task|TK_NotStart|DT_FixedDrtn|PT_High|CP_Phys|0|16|16||||",
                "101|1|11|1|A1010|Concretagem|TT_Task|TK_NotStart|DT_FixedDUR2|PT_Normal|CP_Phys|0|24|24||||",
                "102|1|10|1|M1|Fim da estrutura|TT_FinMile|TK_NotStart|DT_FixedDrtn|PT_Normal|CP_Phys|0|0|0||||",
                "103|1|10|1|A1020|Acabamento|TT_Task|TK_NotStart|DT_FixedDrtn|PT_Normal|CP_Phys|0|8|8|||CS_MSOA|2026-11-30 08:00",
                "900|9|90|1|X1|Of the other project|TT_Task|TK_NotStart|DT_FixedDrtn|PT_Normal|CP_Phys|0|8|8||||",
            ],
        ),
        (
            "TASKPRED",
            "task_pred_id task_id pred_task_id proj_id pred_proj_id pred_type lag_hr_cnt",
            &["1|101|100|1|1|PR_FS|8", "2|102|101|1|1|PR_FS|0", "3|103|100|1|1|PR_SS|4"],
        ),
        (
            "TASKRSRC",
            "taskrsrc_id task_id proj_id rsrc_id role_id target_qty target_qty_per_hr remain_qty act_reg_qty act_ot_qty",
            &["1|100|1|1||16|1|16|0|0", "2|101|1|2||12||12||"],
        ),
        (
            "PROJCOST",
            "cost_item_id task_id proj_id acct_id cost_name target_cost act_cost remain_cost cost_load_type",
            &["1|101|1|1|Bomba|1500|0|1500|Start of Activity"],
        ),
        ("TASKACTV", "task_id actv_code_type_id actv_code_id proj_id", &["100|1|1|1"]),
        (
            "UDFTYPE",
            "udf_type_id table_name udf_type_name udf_type_label logical_data_type",
            &["1|TASK|user_field_1|Responsável|Text", "2|PROJWBS|user_field_2|Peso|Number"],
        ),
        ("UDFVALUE", "udf_type_id fk_id proj_id udf_text udf_number udf_date", &["1|100|1|Ana||"]),
        ("FINDATES", "fin_dates_id fin_dates_name start_date end_date", &["1|2026-11|2026-11-01 00:00|2026-11-30 23:59"]),
        (
            "TASKFIN",
            "task_id fin_dates_id proj_id act_work_cost act_equip_cost act_mat_cost act_expense_cost act_work_qty act_equip_qty bcwp bcws",
            &["100|1|1|800|0|0|0|16|0|900|1000"],
        ),
        ("SCHEDOPTIONS", "proj_id sched_outer_depend_type", &["1|x"]),
    ])
}

#[test]
fn a_wbs_code_that_is_an_activity_id_gives_way() {
    // The WBS element E is coded as the activity A1000 is identified: both are read (#57).
    let r = read(&file().replace("N\tE\tEstrutura", "N\tA1000\tEstrutura")).unwrap();
    let ids: Vec<&str> = r.project.activities.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, ["A1000*", "A1000", "A1010", "M1", "A1020"]);
    let a = |id: &str| r.project.activities.iter().find(|a| a.id == id).unwrap();
    assert_eq!((a("A1000").name.as_str(), a("A1000").parent.as_deref()), ("Escavação", Some("A1000*")));
    assert!(r.warnings.iter().any(|w| w.contains("WBS")), "{:?}", r.warnings);
}

#[test]
fn reads_a_file() {
    let r = read(&file()).unwrap();
    let p = &r.project;
    assert_eq!((p.name.as_str(), p.currency.as_str(), p.start), ("Obra & cia <1>", "R$", day(2026, 11, 2)));
    assert_eq!(p.extra.get("p6.id").map(String::as_str), Some("OBRA-1"));
    assert!(r.warnings.iter().any(|w| w.contains('2') && w.contains("projects")), "{:?}", r.warnings);
    assert!(r.warnings.iter().any(|w| w.contains("SCHEDOPTIONS")), "{:?}", r.warnings);

    // The calendar: Monday to Friday on two periods, Christmas off, a Saturday morning worked.
    let c = &p.calendar;
    assert_eq!(c.week, [true, true, true, true, true, false, false]);
    assert_eq!(c.periods[0], vec![(480, 720), (780, 1020)]);
    assert!(c.holidays.contains(&day(2026, 12, 25)));
    assert_eq!(c.exceptions.get(&day(2026, 11, 28)), Some(&vec![(480, 720)]));
    assert!(p.calendars.is_empty(), "the other project's calendar is left out");

    // The WBS element and the activities, none of the other project's.
    let ids: Vec<&str> = p.activities.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, ["E", "A1000", "A1010", "M1", "A1020"]);
    let a = |id: &str| p.activities.iter().find(|a| a.id == id).unwrap();
    assert_eq!(a("E").parent, None, "under the project's own node");
    assert_eq!(a("A1000").parent.as_deref(), Some("E"));
    assert_eq!((a("A1000").duration, a("A1000").duration_minutes), (2, Some(960)));
    assert_eq!(a("A1000").levelling_priority, mkperth_core::LevellingPriority::High);
    assert_eq!(a("A1010").duration_type, mkperth_core::DurationType::FixedDurationAndUnits);
    assert!(a("M1").kind.is_milestone());
    assert_eq!(a("A1020").constraint, Some(Constraint::StartOnOrAfter(day(2026, 11, 30))));
    let kinds: Vec<(RelationshipType, Option<i64>)> = p.relationships.iter().map(|r| (r.kind, r.lag_minutes)).collect();
    assert_eq!(
        kinds,
        [(RelationshipType::FinishToStart, Some(480)), (RelationshipType::FinishToStart, Some(0)), (RelationshipType::StartToStart, Some(240))]
    );

    // Resources, their prices and most units an hour, the assignments and the expense.
    let ped = p.resources.iter().find(|r| r.id == "PED").unwrap();
    assert_eq!((ped.kind, ped.max_units, ped.prices.len()), (ResourceKind::Labor, 200, 2));
    assert_eq!((ped.prices[0].from, ped.prices[0].per_unit, ped.prices[1].from), (None, 5_000, Some(day(2027, 1, 1))));
    let con = p.resources.iter().find(|r| r.id == "CON").unwrap();
    assert_eq!((con.kind, con.unit.as_str()), (ResourceKind::Material, "m3"));
    let x = p.assignments.iter().find(|x| x.activity == "A1000").unwrap();
    assert_eq!((x.resource.as_deref(), x.planned, x.rate, x.actual), (Some("PED"), 1_600, Some(100), 0));
    let e = &p.expenses[0];
    assert_eq!(
        (e.activity.as_str(), e.name.as_str(), e.planned, e.account.as_deref(), e.accrual),
        ("A1010", "Bomba", 150_000, Some("C1"), Accrual::Start)
    );

    // The code, the field (the WBS's is left out), the OBS and the budget.
    assert_eq!(a("A1000").codes.get("Fase").map(String::as_str), Some("F1"));
    assert_eq!(p.classification.codes[0].values[0].description, "Fundações");
    assert_eq!(a("A1000").fields.get("Responsável"), Some(&FieldValue::Text("Ana".into())));
    assert_eq!(p.classification.fields.len(), 1);
    assert_eq!(p.organisation.responsible.as_deref(), Some("Diretoria"));
    assert_eq!(a("E").responsible.as_deref(), Some("Obras"));
    assert_eq!(p.budget.of(None).map(|b| b.original), Some(5_000_000));
    assert_eq!(p.budget.of(Some("E")).map(|b| b.original), Some(2_000_000));

    // The financial period and the activity's actuals in it.
    assert_eq!(p.earned_value.periods[0].finish, day(2026, 11, 30));
    let rec = &p.earned_value.records[0];
    assert_eq!((rec.activity.as_str(), rec.labor_cost, rec.labor_units, rec.earned, rec.planned), ("A1000", 80_000, 1_600, 90_000, 100_000));
}

/// The network scheduled by hand, on 8 hours a day (08:00–12:00, 13:00–17:00):
/// - A1000, 16 h from Monday 2 November 08:00: Monday and Tuesday, to 3 November 17:00.
/// - A1010 after it with 8 h of lag: the lag fills Wednesday 4; 24 h from Thursday 5 run Thursday,
///   Friday and Monday 9 November.
/// - M1 finishes with A1010, Monday 9 November.
/// - A1020 starts 4 h after A1000 starts (Monday 2 at 13:00), but on or after Monday 30 November
///   08:00: 8 h that Monday.
#[test]
fn schedules_as_worked_by_hand() {
    let p = read(&file()).unwrap().project;
    let s = mkperth_sched::schedule(&p).unwrap();
    let at = |id: &str| {
        let k = p.activities.iter().position(|a| a.id == id).unwrap();
        (s.activities[k].early_start, s.activities[k].early_finish)
    };
    assert_eq!(at("A1000"), (day(2026, 11, 2), day(2026, 11, 3)));
    assert_eq!(at("A1010"), (day(2026, 11, 5), day(2026, 11, 9)));
    assert_eq!(at("M1").1, day(2026, 11, 9));
    assert_eq!(at("A1020"), (day(2026, 11, 30), day(2026, 11, 30)));
}

/// Written and read back, a project comes back as it was, save what XER isn't given (the
/// user-defined field, the expense's accrual), each said in a warning.
#[test]
fn written_and_read_back() {
    let p = read(&file()).unwrap().project;
    let (text, warnings) = write(&p);
    assert!(text.starts_with("ERMHDR\t19.12\t2026-11-02\t"), "{text}");
    assert!(text.contains("%T\tTASK\r\n") && text.contains("TT_FinMile") && text.contains("CS_MSOA") && text.contains("PR_SS"), "{text}");
    assert!(text.contains("(0||7()())") && text.contains("(d|46381)") && text.contains("(d|46354)((0||0(s|08:00|f|12:00)()))"), "{text}");
    assert!(warnings.iter().any(|w| w.contains("user-defined")), "{warnings:?}");
    assert!(warnings.iter().any(|w| w.contains("accrue")), "{warnings:?}");

    let q = read(&text).unwrap().project;
    assert_eq!(q.name, p.name);
    assert_eq!(q.calendar, p.calendar);
    assert_eq!(q.relationships, p.relationships);
    assert_eq!(q.resources, p.resources);
    assert_eq!(q.assignments, p.assignments);
    assert_eq!(q.cost_accounts, p.cost_accounts);
    assert_eq!(q.organisation, p.organisation);
    assert_eq!(q.budget, p.budget);
    assert_eq!(q.earned_value, p.earned_value);
    assert_eq!(q.classification.codes, p.classification.codes);
    assert_eq!(q.expenses[0].accrual, Accrual::Uniform, "P6's default");
    let strip = |p: &Project| -> Vec<mkperth_core::Activity> {
        p.activities
            .iter()
            .map(|a| {
                let mut a = a.clone();
                a.extra.clear();
                a.fields.clear();
                a
            })
            .collect()
    };
    assert_eq!(strip(&q), strip(&p));
    let (s1, s2) = (mkperth_sched::schedule(&p).unwrap(), mkperth_sched::schedule(&q).unwrap());
    assert_eq!(s1.activities, s2.activities);
}

/// What P6 keeps beside the network (#36): two baselines, one in use; notes on an activity and on a
/// WBS element; the budget's log, spending plans and funding. Written and read back unchanged,
/// save the approved change's status, whose code isn't published: left for P6, with a warning.
#[test]
fn baselines_notes_and_the_budget_read_back() {
    use mkperth_core::{Baseline, BaselineActivity, BudgetChange, ChangeStatus, Fund, FundingSource, SpendingMonth, WbsBudget};
    let mut p = read(&file()).unwrap().project;
    let at = |id: &str| BaselineActivity { id: id.into(), start: day(2026, 11, 2), finish: day(2026, 11, 3), duration: 960, cost: 0 };
    p.baselines = vec![
        Baseline { name: "Contrato".into(), activities: vec![BaselineActivity { cost: 150_050, ..at("A1000") }, at("M1")] },
        Baseline { name: "Revisão & <2>".into(), activities: vec![BaselineActivity { finish: day(2026, 11, 4), duration: 1440, ..at("A1000") }] },
    ];
    p.baseline_in_use = Some("Revisão & <2>".into());
    let note = |p: &mut Project, id: &str, text: &str| p.activities.iter_mut().find(|a| a.id == id).unwrap().notes = text.into();
    note(&mut p, "A1010", "Bomba alugada & <confirmada>.");
    note(&mut p, "E", "Estrutura de concreto.");
    let month = |m, amount| SpendingMonth { month: day(2026, m, 1), amount };
    p.budget.budgets = vec![
        WbsBudget { wbs: None, original: 5_000_000, plan: vec![month(11, 3_000_000), month(12, 2_000_050)] },
        WbsBudget { wbs: Some("E".into()), original: 2_000_000, plan: vec![month(11, 2_000_000)] },
    ];
    let change = |number, wbs: Option<&str>, amount, status| BudgetChange {
        number,
        wbs: wbs.map(str::to_string),
        date: day(2026, 11, 10),
        amount,
        status,
        responsible: "Ana & cia".into(),
        reason: "<escopo>".into(),
    };
    p.budget.log = vec![change(1, None, 500_000, ChangeStatus::Pending), change(2, Some("E"), -200_000, ChangeStatus::Pending)];
    p.budget.sources = vec![
        FundingSource { id: "BANCO".into(), name: "Empréstimo".into(), parent: None },
        FundingSource { id: "BNDES".into(), name: "Banco de fomento".into(), parent: Some("BANCO".into()) },
    ];
    p.budget.funds = vec![Fund { source: "BNDES".into(), amount: 3_000_000 }];

    let (text, warnings) = write(&p);
    for table in ["MEMOTYPE", "TASKMEMO", "WBSMEMO", "BUDGCHNG", "WBSBUDG", "FUNDSRC", "PROJFUND"] {
        assert!(text.contains(&format!("%T\t{table}\r\n")), "{table}: {text}");
    }
    assert!(!warnings.iter().any(|w| w.contains("notes") || w.contains("Not written")), "{warnings:?}");
    let back = read(&text).unwrap();
    assert!(!back.warnings.iter().any(|w| w.contains("projects") || w.contains("left out") || w.contains("Left out")), "{:?}", back.warnings);
    let q = back.project;
    assert_eq!((&q.baselines, &q.baseline_in_use), (&p.baselines, &p.baseline_in_use));
    assert_eq!(q.budget, p.budget);
    let notes = |p: &Project| p.activities.iter().map(|a| (a.id.clone(), a.notes.clone())).collect::<Vec<_>>();
    assert_eq!(notes(&q), notes(&p));

    // An approved change's status and notes on several lines: told, not written as they are.
    p.budget.log[0].status = ChangeStatus::Approved;
    note(&mut p, "A1010", "Linha 1\nLinha 2");
    let (text, warnings) = write(&p);
    assert!(warnings.iter().any(|w| w.starts_with("1 budget changes")), "{warnings:?}");
    assert!(warnings.iter().any(|w| w.contains("several lines")), "{warnings:?}");
    let q = read(&text).unwrap().project;
    assert_eq!(q.budget.log[0].status, ChangeStatus::Pending);
    assert_eq!(q.activities.iter().find(|a| a.id == "A1010").unwrap().notes, "Linha 1 Linha 2");
}

/// Notebooks as P6 may write them: HTML, several topics on one activity.
#[test]
fn notebooks() {
    let text = xer(&[
        ("PROJECT", "proj_id proj_short_name", &["1|P"]),
        ("TASK", "task_id proj_id task_code task_name target_drtn_hr_cnt", &["1|1|A|A|8"]),
        ("MEMOTYPE", "memo_type_id memo_type", &["1|Escopo", "2|Riscos"]),
        (
            "TASKMEMO",
            "memo_id memo_type_id proj_id task_id task_memo",
            &["1|1|1|1|<html><body><p>Fazer &amp; medir</p><p>Depois</p></body></html>", "2|2|1|1|Chuva"],
        ),
        ("BUDGCHNG", "budg_chng_id proj_id chng_short_name chng_date chng_cost status_code", &["1|1|7|2026-11-10 00:00|10|Approved"]),
    ]);
    let p = read(&text).unwrap().project;
    assert_eq!(p.activities[0].notes, "Escopo\nFazer & medir\nDepois\n\nRiscos\nChuva");
    assert_eq!((p.budget.log[0].number, p.budget.log[0].status), (7, mkperth_core::ChangeStatus::Approved));
}

/// A resource without a price keeps its most units an hour on a price of none.
#[test]
fn units_without_a_price() {
    let mut p = read(&file()).unwrap().project;
    p.resources.iter_mut().for_each(|r| {
        r.prices.clear();
        r.max_units = 300;
    });
    let q = read(&write(&p).0).unwrap().project;
    assert!(q.resources.iter().all(|r| r.max_units == 300), "{:?}", q.resources);
}

#[test]
fn hostile_files() {
    assert_eq!(read("%T\tTASK\n"), Err(XerError::NotXer));
    assert_eq!(read(""), Err(XerError::NotXer));
    assert_eq!(read("ERMHDR\t19.12\n%R\t1\n"), Err(XerError::Line(2)));
    assert_eq!(read("ERMHDR\t19.12\n%T\tTASK\n%F\ttask_id\n%R\t1\n%E\n"), Err(XerError::NoProject));
    // Rows shorter or longer than their columns, a calendar that isn't well formed, nested too
    // deep, or with dates out of range: read, never a crash.
    let deep = format!("{}{}", "(0||x()(".repeat(500), "))".repeat(500));
    for data in ["(0||CalendarData()((0||DaysOfWeek()(", "((((", ")", &deep, "(0||CalendarData()((0||Exceptions()((0||0(d|99999999999)())))))"] {
        let text = xer(&[
            ("CALENDAR", "clndr_id clndr_name day_hr_cnt clndr_data", &["1|C|8|@DATA@"]),
            ("PROJECT", "proj_id proj_short_name clndr_id plan_start_date", &["1|P|1|2026-11-02 08:00|extra|fields"]),
            ("TASK", "task_id proj_id task_code task_name task_type target_drtn_hr_cnt", &["1|1|A"]),
        ])
        .replace("@DATA@", data);
        let r = read(&text).unwrap();
        assert_eq!(r.project.calendar.week, [true, true, true, true, true, false, false], "{data}");
    }
    // A duration that isn't a number is P6 XML's error, named.
    let text = xer(&[
        ("PROJECT", "proj_id proj_short_name", &["1|P"]),
        ("TASK", "task_id proj_id task_code task_name target_drtn_hr_cnt", &["1|1|A|A|lots"]),
    ]);
    assert!(matches!(read(&text), Err(XerError::P6(_))));
}

/// The code page XER files are usually in: read when the bytes aren't UTF-8, and written.
#[test]
fn windows_1252() {
    let text = "Fundações – “a” € ½ 中";
    let bytes = encode(text);
    assert_eq!(bytes.len(), text.chars().count());
    assert!(std::str::from_utf8(&bytes).is_err());
    assert_eq!(decode(&bytes), "Fundações – “a” € ½ ?");
    assert_eq!(decode("já UTF-8".as_bytes()), "já UTF-8");
    let file = file();
    assert_eq!(read(&decode(&encode(&file))).unwrap().project.classification.codes[0].values[0].description, "Fundações");
}
