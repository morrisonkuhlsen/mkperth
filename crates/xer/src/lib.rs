//! Primavera P6's XER files, read into MK Perth's model and written from it.
//!
//! Oracle publishes no specification of XER. Its tables and columns are those of Oracle's
//! *XER Import/Export Data Map Guide (Project)*, the codes in them (`TT_Task`, `PR_FS`, `CS_MSO`,
//! `DT_FixedDrtn`, `TK_Active`, `PT_Normal`, `RT_Labor`, `CP_Phys`) those of its *Prime
//! Import/Export Data Map Guide*; the lines (`ERMHDR`, `%T`, `%F`, `%R`, `%E`) and the calendars'
//! `clndr_data` follow public third-party descriptions. XER's tables are P6's own; P6 XML's
//! objects hold the same things under other names. So a file is translated, table by table, to
//! the objects `mkperth-p6xml` reads, and written from the objects it writes: what one format
//! carries, the other does. A code Oracle doesn't publish is never guessed: what needs one is
//! read by its sense where it can be and left out of what is written, with a warning.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod calendar;
pub mod table;
#[cfg(test)]
mod tests;

use std::collections::{BTreeSet, HashMap};

use mkperth_core::Project;
use mkperth_i18n::tr;
use mkperth_p6xml::P6Error;
use mkperth_p6xml::tree::{self, Element};
use table::{File, Record, Table};
use thiserror::Error;

pub use table::{decode, encode};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum XerError {
    #[error("{}", tr!("err-xer-not-xer"))]
    NotXer,
    #[error("{}", tr!("err-xer-too-big"))]
    TooBig,
    /// The line, from 1, of a row or columns before any table.
    #[error("{}", tr!("err-xer-line", line = *.0))]
    Line(usize),
    #[error("{}", tr!("err-xer-no-project"))]
    NoProject,
    #[error(transparent)]
    P6(#[from] P6Error),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Read {
    pub project: Project,
    pub warnings: Vec<String>,
}

/// How a column's value becomes an element's text and back.
#[derive(Clone, Copy)]
enum Kind {
    Text,
    /// `2026-11-03 08:00` here, `2026-11-03T08:00:00` there.
    Date,
    /// XER's code and P6 XML's name for each value.
    Codes(&'static [(&'static str, &'static str)]),
}
use Kind::{Codes, Date, Text};

const TASK_TYPES: &[(&str, &str)] = &[
    ("TT_Task", "Task Dependent"),
    ("TT_Rsrc", "Resource Dependent"),
    ("TT_LOE", "Level of Effort"),
    ("TT_Mile", "Start Milestone"),
    ("TT_FinMile", "Finish Milestone"),
    ("TT_WBS", "WBS Summary"),
];
const DURATION_TYPES: &[(&str, &str)] = &[
    ("DT_FixedRate", "Fixed Units/Time"),
    ("DT_FixedDrtn", "Fixed Duration and Units/Time"),
    ("DT_FixedQty", "Fixed Units"),
    ("DT_FixedDUR2", "Fixed Duration and Units"),
];
const STATUSES: &[(&str, &str)] = &[("TK_NotStart", "Not Started"), ("TK_Active", "In Progress"), ("TK_Complete", "Completed")];
const CONSTRAINTS: &[(&str, &str)] = &[
    ("CS_MSO", "Start On"),
    ("CS_MSOB", "Start On or Before"),
    ("CS_MSOA", "Start On or After"),
    ("CS_MEO", "Finish On"),
    ("CS_MEOB", "Finish On or Before"),
    ("CS_MEOA", "Finish On or After"),
    ("CS_ALAP", "As Late As Possible"),
    ("CS_MANDSTART", "Mandatory Start"),
    ("CS_MANDFIN", "Mandatory Finish"),
];
const PRIORITIES: &[(&str, &str)] = &[("PT_Top", "Top"), ("PT_High", "High"), ("PT_Normal", "Normal"), ("PT_Low", "Low"), ("PT_Lowest", "Lowest")];
const PERCENT_TYPES: &[(&str, &str)] = &[("CP_Phys", "Physical"), ("CP_Drtn", "Duration"), ("CP_Units", "Units")];
const RELATIONSHIPS: &[(&str, &str)] =
    &[("PR_FS", "Finish to Start"), ("PR_SS", "Start to Start"), ("PR_FF", "Finish to Finish"), ("PR_SF", "Start to Finish")];
const RESOURCE_TYPES: &[(&str, &str)] = &[("RT_Labor", "Labor"), ("RT_Equip", "Nonlabor"), ("RT_Mat", "Material")];

/// A table and the P6 XML object its rows are: each column and the element it is.
struct Map {
    table: &'static str,
    element: &'static str,
    /// Inside the project's element, or beside it.
    in_project: bool,
    columns: &'static [(&'static str, &'static str, Kind)],
}

/// The tables translated column by column, in the order they are written. The project, its WBS,
/// the calendars and the user-defined fields need more than that (see `read` and `write`).
const MAPS: [Map; 16] = [
    Map {
        table: "OBS",
        element: "OBS",
        in_project: false,
        columns: &[
            ("obs_id", "ObjectId", Text),
            ("obs_name", "Name", Text),
            ("obs_descr", "Description", Text),
            ("parent_obs_id", "ParentObjectId", Text),
        ],
    },
    Map {
        table: "UMEASURE",
        element: "UnitOfMeasure",
        in_project: false,
        columns: &[("unit_id", "ObjectId", Text), ("unit_abbrev", "Abbreviation", Text), ("unit_name", "Name", Text)],
    },
    Map {
        table: "ROLES",
        element: "Role",
        in_project: false,
        columns: &[("role_id", "ObjectId", Text), ("role_short_name", "Id", Text), ("role_name", "Name", Text)],
    },
    Map {
        table: "ACCOUNT",
        element: "CostAccount",
        in_project: false,
        columns: &[
            ("acct_id", "ObjectId", Text),
            ("acct_short_name", "Id", Text),
            ("acct_name", "Name", Text),
            ("parent_acct_id", "ParentObjectId", Text),
        ],
    },
    Map {
        table: "FINDATES",
        element: "FinancialPeriod",
        in_project: false,
        columns: &[
            ("fin_dates_id", "ObjectId", Text),
            ("fin_dates_name", "Name", Text),
            ("start_date", "StartDate", Date),
            ("end_date", "EndDate", Date),
        ],
    },
    Map {
        table: "ACTVTYPE",
        element: "ActivityCodeType",
        in_project: false,
        columns: &[
            ("actv_code_type_id", "ObjectId", Text),
            ("actv_code_type", "Name", Text),
            ("proj_id", "ProjectObjectId", Text),
            ("seq_num", "SequenceNumber", Text),
        ],
    },
    Map {
        table: "ACTVCODE",
        element: "ActivityCode",
        in_project: false,
        columns: &[
            ("actv_code_id", "ObjectId", Text),
            ("actv_code_type_id", "CodeTypeObjectId", Text),
            ("short_name", "CodeValue", Text),
            ("actv_code_name", "Description", Text),
            ("parent_actv_code_id", "ParentObjectId", Text),
            ("seq_num", "SequenceNumber", Text),
        ],
    },
    // A resource's most units an hour are on its prices (RSRCRATE), P6 XML's on the resource.
    Map {
        table: "RSRC",
        element: "Resource",
        in_project: false,
        columns: &[
            ("rsrc_id", "ObjectId", Text),
            ("rsrc_short_name", "Id", Text),
            ("rsrc_name", "Name", Text),
            ("rsrc_type", "ResourceType", Codes(RESOURCE_TYPES)),
            ("clndr_id", "CalendarObjectId", Text),
            ("unit_id", "UnitOfMeasureObjectId", Text),
        ],
    },
    Map {
        table: "RSRCRATE",
        element: "ResourceRate",
        in_project: false,
        columns: &[
            ("rsrc_rate_id", "ObjectId", Text),
            ("rsrc_id", "ResourceObjectId", Text),
            ("start_date", "EffectiveDate", Date),
            ("cost_per_qty", "PricePerUnit", Text),
        ],
    },
    Map {
        table: "RSRCROLE",
        element: "ResourceRole",
        in_project: false,
        columns: &[("rsrc_id", "ResourceObjectId", Text), ("role_id", "RoleObjectId", Text)],
    },
    Map {
        table: "TASK",
        element: "Activity",
        in_project: true,
        columns: &[
            ("task_id", "ObjectId", Text),
            ("proj_id", "ProjectObjectId", Text),
            ("wbs_id", "WBSObjectId", Text),
            ("clndr_id", "CalendarObjectId", Text),
            ("task_code", "Id", Text),
            ("task_name", "Name", Text),
            ("task_type", "Type", Codes(TASK_TYPES)),
            ("status_code", "Status", Codes(STATUSES)),
            ("duration_type", "DurationType", Codes(DURATION_TYPES)),
            ("priority_type", "LevelingPriority", Codes(PRIORITIES)),
            ("complete_pct_type", "PercentCompleteType", Codes(PERCENT_TYPES)),
            ("phys_complete_pct", "PhysicalPercentComplete", Text),
            ("target_drtn_hr_cnt", "PlannedDuration", Text),
            ("remain_drtn_hr_cnt", "RemainingDuration", Text),
            ("act_start_date", "ActualStartDate", Date),
            ("act_end_date", "ActualFinishDate", Date),
            ("expect_end_date", "ExpectedFinishDate", Date),
            ("cstr_type", "PrimaryConstraintType", Codes(CONSTRAINTS)),
            ("cstr_date", "PrimaryConstraintDate", Date),
            ("cstr_type2", "SecondaryConstraintType", Codes(CONSTRAINTS)),
            ("cstr_date2", "SecondaryConstraintDate", Date),
        ],
    },
    Map {
        table: "TASKPRED",
        element: "Relationship",
        in_project: true,
        columns: &[
            ("task_pred_id", "ObjectId", Text),
            ("task_id", "SuccessorActivityObjectId", Text),
            ("pred_task_id", "PredecessorActivityObjectId", Text),
            ("proj_id", "SuccessorProjectObjectId", Text),
            ("pred_proj_id", "PredecessorProjectObjectId", Text),
            ("pred_type", "Type", Codes(RELATIONSHIPS)),
            ("lag_hr_cnt", "Lag", Text),
        ],
    },
    // The actual units are regular and overtime here, one figure there.
    Map {
        table: "TASKRSRC",
        element: "ResourceAssignment",
        in_project: true,
        columns: &[
            ("taskrsrc_id", "ObjectId", Text),
            ("task_id", "ActivityObjectId", Text),
            ("proj_id", "ProjectObjectId", Text),
            ("rsrc_id", "ResourceObjectId", Text),
            ("role_id", "RoleObjectId", Text),
            ("target_qty", "PlannedUnits", Text),
            ("target_qty_per_hr", "PlannedUnitsPerTime", Text),
            ("remain_qty", "RemainingUnits", Text),
        ],
    },
    // The accrual's codes aren't published: read by their sense, not written.
    Map {
        table: "PROJCOST",
        element: "ActivityExpense",
        in_project: true,
        columns: &[
            ("cost_item_id", "ObjectId", Text),
            ("task_id", "ActivityObjectId", Text),
            ("proj_id", "ProjectObjectId", Text),
            ("acct_id", "CostAccountObjectId", Text),
            ("cost_name", "ExpenseItem", Text),
            ("target_cost", "PlannedCost", Text),
            ("act_cost", "ActualCost", Text),
            ("remain_cost", "RemainingCost", Text),
        ],
    },
    Map {
        table: "TASKACTV",
        element: "ActivityCodeAssignment",
        in_project: true,
        columns: &[
            ("task_id", "ActivityObjectId", Text),
            ("actv_code_type_id", "ActivityCodeTypeObjectId", Text),
            ("actv_code_id", "ActivityCodeObjectId", Text),
            ("proj_id", "ProjectObjectId", Text),
        ],
    },
    Map {
        table: "TASKFIN",
        element: "ActivityPeriodActual",
        in_project: true,
        columns: &[
            ("task_id", "ActivityObjectId", Text),
            ("fin_dates_id", "FinancialPeriodObjectId", Text),
            ("proj_id", "ProjectObjectId", Text),
            ("act_work_cost", "ActualLaborCost", Text),
            ("act_equip_cost", "ActualNonLaborCost", Text),
            ("act_mat_cost", "ActualMaterialCost", Text),
            ("act_expense_cost", "ActualExpenseCost", Text),
            ("act_work_qty", "ActualLaborUnits", Text),
            ("act_equip_qty", "ActualNonLaborUnits", Text),
            ("bcwp", "EarnedValueCost", Text),
            ("bcws", "PlannedValueCost", Text),
        ],
    },
];

/// The tables read besides those in `MAPS`.
const READ_TOO: [&str; 6] = ["PROJECT", "PROJWBS", "CALENDAR", "UDFTYPE", "UDFVALUE", "CURRTYPE"];

fn esc(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// `2026-11-03 08:00` (or a bare date) as P6 XML writes it; `None` unless it is a date.
fn xml_date(text: &str) -> Option<String> {
    let day = mkperth_core::Date::parse(text.get(..10).unwrap_or(text))?;
    let time = text.get(11..16).filter(|t| t.len() == 5 && t.as_bytes().get(2) == Some(&b':')).unwrap_or("00:00");
    Some(format!("{day}T{time}:00"))
}

/// `2026-11-03T08:00:00` as XER writes it, `2026-11-03 08:00`.
fn xer_date(text: &str) -> String {
    match (text.get(..10), text.get(11..16)) {
        (Some(day), Some(time)) => format!("{day} {time}"),
        _ => format!("{} 00:00", text.get(..10).unwrap_or(text)),
    }
}

/// A column's value as its element's text.
fn to_element(kind: Kind, value: &str) -> Option<String> {
    match kind {
        Text => Some(value.to_string()),
        Date => xml_date(value),
        Codes(codes) => codes.iter().find(|(c, _)| c.eq_ignore_ascii_case(value)).map(|(_, n)| n.to_string()),
    }
}

/// An element's text as its column's value.
fn to_column(kind: Kind, text: &str) -> String {
    match kind {
        Text => text.to_string(),
        Date => xer_date(text),
        Codes(codes) => codes.iter().find(|(_, n)| *n == text).map_or_else(String::new, |(c, _)| c.to_string()),
    }
}

/// An element with its children, each a name and its text (escaped here).
fn element(name: &str, children: &[(String, String)], inner: &str) -> String {
    let mut out = format!("<{name}>");
    for (n, t) in children {
        out.push_str(&format!("<{n}>{}</{n}>", esc(t)));
    }
    out.push_str(inner);
    out.push_str(&format!("</{name}>"));
    out
}

/// A row's columns as an object's children, by its table's map.
fn children(m: &Map, r: &Record) -> Vec<(String, String)> {
    m.columns.iter().filter_map(|(column, name, kind)| Some((name.to_string(), to_element(*kind, r.get(column)?)?))).collect()
}

/// A number's text, summed with another's; `None` when neither is a number.
fn sum(a: Option<&str>, b: Option<&str>) -> Option<String> {
    let n = |x: Option<&str>| x.and_then(|t| t.trim().parse::<f64>().ok()).filter(|v| v.is_finite());
    match (n(a), n(b)) {
        (None, None) => None,
        (x, y) => Some(format!("{}", x.unwrap_or(0.0) + y.unwrap_or(0.0))),
    }
}

/// P6 XML's data type for a user-defined field, by the sense of XER's (whose codes aren't
/// published): text, an indicator, an integer, a cost, a finish or start date, else a number.
fn data_type(logical: &str) -> &'static str {
    let t = logical.to_ascii_uppercase();
    if t.contains("TEXT") || t.contains("STRING") {
        "Text"
    } else if t.contains("IND") {
        "Indicator"
    } else if t.contains("INT") {
        "Integer"
    } else if t.contains("COST") || t.contains("MONEY") {
        "Cost"
    } else if t.contains("FINISH") || t.contains("END") {
        "Finish Date"
    } else if t.contains("DATE") || t.contains("START") {
        "Start Date"
    } else {
        "Double"
    }
}

/// P6 XML's accrual, by the sense of XER's code (whose codes aren't published).
fn accrual(code: &str) -> &'static str {
    let c = code.to_ascii_lowercase();
    if c.contains("start") {
        "Start of Activity"
    } else if c.contains("end") || c.contains("finish") {
        "End of Activity"
    } else {
        "Uniform over Activity"
    }
}

/// Reads an XER file: its first project, with the calendars, resources, codes and fields it
/// names.
pub fn read(text: &str) -> Result<Read, XerError> {
    let file = table::parse(text)?;
    let projects: Vec<Record> = file.records("PROJECT").collect();
    let Some(project) = projects.first() else { return Err(XerError::NoProject) };
    let mut warnings = Vec::new();
    if projects.len() > 1 {
        warnings.push(tr!("warn-xer-projects", n = projects.len()));
    }
    let proj_id = project.get("proj_id").unwrap_or_default();
    // A row of another project's is left out; one of none (a global row) is kept.
    let mine = |r: &Record| r.get("proj_id").is_none_or(|p| p == proj_id);
    let node = file.records("PROJWBS").filter(mine).find(|r| r.get("proj_node_flag") == Some("Y"));

    let (mut root, mut inner) = (String::new(), String::new());
    for r in file.records("CALENDAR").filter(mine) {
        let mut c = vec![("Type".to_string(), "Global".to_string())];
        for (column, name) in [("clndr_id", "ObjectId"), ("clndr_name", "Name"), ("day_hr_cnt", "HoursPerDay")] {
            if let Some(v) = r.get(column) {
                c.push((name.to_string(), v.to_string()));
            }
        }
        root.push_str(&element("Calendar", &c, &calendar::to_xml(r.get("clndr_data").unwrap_or_default())));
    }
    // Each resource's most units an hour: on its earliest price.
    let mut max_units: HashMap<&str, (&str, &str)> = HashMap::new();
    for r in file.records("RSRCRATE") {
        let (Some(resource), Some(units)) = (r.get("rsrc_id"), r.get("max_qty_per_hr")) else { continue };
        let from = r.get("start_date").unwrap_or_default();
        if max_units.get(resource).is_none_or(|(earliest, _)| from < *earliest) {
            max_units.insert(resource, (from, units));
        }
    }
    for m in &MAPS {
        for r in file.records(m.table).filter(mine) {
            let mut c = children(m, &r);
            match m.table {
                "RSRC" => {
                    if let Some((_, units)) = r.get("rsrc_id").and_then(|id| max_units.get(id)) {
                        c.push(("MaxUnitsPerTime".into(), units.to_string()));
                    }
                }
                "TASKRSRC" => {
                    if let Some(units) = sum(r.get("act_reg_qty"), r.get("act_ot_qty")) {
                        c.push(("ActualUnits".into(), units));
                    }
                }
                "PROJCOST" => c.push(("AccrualType".into(), accrual(r.get("cost_load_type").unwrap_or_default()).into())),
                _ => {}
            }
            let e = element(m.element, &c, "");
            if m.in_project { inner.push_str(&e) } else { root.push_str(&e) }
        }
    }
    // The WBS under the project's own node; the node's budget and manager are the project's.
    for r in file.records("PROJWBS").filter(mine).filter(|r| r.get("proj_node_flag") != Some("Y")) {
        let mut c = Vec::new();
        for (column, name) in [
            ("wbs_id", "ObjectId"),
            ("wbs_short_name", "Code"),
            ("wbs_name", "Name"),
            ("parent_wbs_id", "ParentObjectId"),
            ("orig_cost", "OriginalBudget"),
            ("obs_id", "OBSObjectId"),
            ("seq_num", "SequenceNumber"),
        ] {
            if let Some(v) = r.get(column) {
                c.push((name.to_string(), v.to_string()));
            }
        }
        inner.push_str(&element("WBS", &c, ""));
    }
    // The fields activities have (P6's table for them is TASK), and their values.
    for r in file.records("UDFTYPE") {
        let mut c = vec![
            ("DataType".to_string(), data_type(r.get("logical_data_type").unwrap_or_default()).to_string()),
            (
                "SubjectArea".to_string(),
                if r.get("table_name") == Some("TASK") { "Activity".into() } else { r.get("table_name").unwrap_or("?").to_string() },
            ),
        ];
        if let Some(id) = r.get("udf_type_id") {
            c.push(("ObjectId".into(), id.into()));
        }
        if let Some(title) = r.get("udf_type_label").or_else(|| r.get("udf_type_name")) {
            c.push(("Title".into(), title.into()));
        }
        root.push_str(&element("UDFType", &c, ""));
    }
    for r in file.records("UDFVALUE").filter(mine) {
        let mut c = Vec::new();
        for (column, name) in [("udf_type_id", "UDFTypeObjectId"), ("fk_id", "ForeignObjectId"), ("udf_text", "Text"), ("udf_text", "Indicator")] {
            if let Some(v) = r.get(column) {
                c.push((name.to_string(), v.to_string()));
            }
        }
        if let Some(n) = r.get("udf_number") {
            c.push(("Double".into(), n.into()));
            c.push(("Cost".into(), n.into()));
            if let Some(i) = n.trim().parse::<f64>().ok().filter(|i| i.is_finite() && i.fract() == 0.0 && i.abs() < 1e15) {
                c.push(("Integer".into(), format!("{}", i as i64)));
            }
        }
        if let Some(d) = r.get("udf_date").and_then(xml_date) {
            c.push(("StartDate".into(), d));
        }
        inner.push_str(&element("UDFValue", &c, ""));
    }

    let mut p = vec![("ObjectId".to_string(), proj_id.to_string())];
    let name = node.and_then(|n| n.get("wbs_name")).or_else(|| project.get("proj_short_name")).unwrap_or_default();
    p.push(("Name".into(), name.into()));
    for (value, element) in [
        (project.get("proj_short_name"), "Id"),
        (project.get("plan_start_date").and_then(xml_date).as_deref(), "PlannedStartDate"),
        (project.get("last_recalc_date").and_then(xml_date).as_deref(), "DataDate"),
        (project.get("plan_end_date").and_then(xml_date).as_deref(), "MustFinishByDate"),
        (project.get("clndr_id"), "ActivityDefaultCalendarObjectId"),
        (node.and_then(|n| n.get("orig_cost")), "OriginalBudget"),
        (node.and_then(|n| n.get("obs_id")), "OBSObjectId"),
    ] {
        if let Some(v) = value {
            p.push((element.to_string(), v.to_string()));
        }
    }
    let xml = format!("<APIBusinessObjects>{root}{}</APIBusinessObjects>", element("Project", &p, &inner));
    let mut read = mkperth_p6xml::read(&xml)?;

    // The currency the costs are in: the one at a rate of one, else the first.
    let currencies: Vec<Record> = file.records("CURRTYPE").collect();
    let base = currencies.iter().find(|c| c.get("base_exch_rate").and_then(|r| r.parse::<f64>().ok()) == Some(1.0)).or(currencies.first());
    if let Some(symbol) = base.and_then(|c| c.get("curr_symbol")) {
        read.project.currency = symbol.to_string();
    }
    let known: BTreeSet<&str> = MAPS.iter().map(|m| m.table).chain(READ_TOO).collect();
    let skipped: BTreeSet<&str> = file.tables.iter().filter(|t| !t.rows.is_empty()).map(|t| t.name.as_str()).filter(|n| !known.contains(n)).collect();
    if !skipped.is_empty() {
        warnings.push(tr!("warn-xer-skipped", tables = skipped.into_iter().collect::<Vec<_>>().join(", ")));
    }
    warnings.append(&mut read.warnings);
    Ok(Read { project: read.project, warnings })
}

/// The table `name` among `tables`, made with `columns` if it isn't there yet.
fn table_mut<'a>(tables: &'a mut HashMap<&'static str, Table>, name: &'static str, columns: &[&str]) -> &'a mut Table {
    tables.entry(name).or_insert_with(|| Table::new(name, columns))
}

/// An object's children as a row of `columns`, each found by the element it is.
fn row(e: &Element, columns: &[(&str, &str, Kind)]) -> Vec<String> {
    columns.iter().map(|(_, name, kind)| e.text_of(name).map_or_else(String::new, |t| to_column(*kind, t))).collect()
}

/// Writes the project as an XER file, and what it couldn't hold.
pub fn write(project: &Project) -> (String, Vec<String>) {
    let (xml, mut warnings) = mkperth_p6xml::write(project);
    let date = project.data_date.unwrap_or(project.start);
    // The header: the version of P6 the file's columns are those of, the day written, what it
    // holds, the program that wrote it, and the currency.
    let header = ["19.12", &date.to_string(), "Project", "", "MK Perth", "", "Project Management", project.currency.as_str()];
    let mut file = File { header: header.iter().map(|h| h.to_string()).collect(), tables: Vec::new() };
    let root = match tree::parse(&xml) {
        Ok(root) => root,
        Err(e) => {
            warnings.push(e.to_string());
            return (table::write(&file), warnings);
        }
    };
    let Some(p) = root.child("Project") else { return (table::write(&file), warnings) };
    let proj_id = p.text_of("ObjectId").unwrap_or("1").to_string();
    let objects = |name: &'static str| root.all(name).chain(p.all(name));
    let mut tables: HashMap<&'static str, Table> = HashMap::new();

    for c in root.all("Calendar") {
        let t = table_mut(&mut tables, "CALENDAR", &["clndr_id", "clndr_name", "day_hr_cnt", "clndr_data"]);
        t.rows.push(vec![
            c.text_of("ObjectId").unwrap_or_default().to_string(),
            c.text_of("Name").unwrap_or_default().to_string(),
            c.text_of("HoursPerDay").unwrap_or_default().to_string(),
            calendar::from_xml(c),
        ]);
    }
    let t = table_mut(&mut tables, "PROJECT", &["proj_id", "proj_short_name", "clndr_id", "plan_start_date", "last_recalc_date", "plan_end_date"]);
    let date_of = |name: &str| p.text_of(name).map(xer_date).unwrap_or_default();
    t.rows.push(vec![
        proj_id.clone(),
        p.text_of("Id").unwrap_or_default().to_string(),
        p.text_of("ActivityDefaultCalendarObjectId").unwrap_or_default().to_string(),
        date_of("PlannedStartDate"),
        date_of("DataDate"),
        date_of("MustFinishByDate"),
    ]);
    // The project's own node heads its WBS: a number no WBS element has.
    let node = p.all("WBS").filter_map(|w| w.text_of("ObjectId")?.parse::<u64>().ok()).max().unwrap_or(0).saturating_add(1).to_string();
    let wbs_columns = ["wbs_id", "proj_id", "proj_node_flag", "wbs_short_name", "wbs_name", "parent_wbs_id", "orig_cost", "obs_id", "seq_num"];
    let t = table_mut(&mut tables, "PROJWBS", &wbs_columns);
    t.rows.push(vec![
        node.clone(),
        proj_id.clone(),
        "Y".into(),
        p.text_of("Id").unwrap_or_default().into(),
        p.text_of("Name").unwrap_or_default().into(),
        String::new(),
        p.text_of("OriginalBudget").unwrap_or_default().into(),
        p.text_of("OBSObjectId").unwrap_or_default().into(),
        "0".into(),
    ]);
    for (k, w) in p.all("WBS").enumerate() {
        t.rows.push(vec![
            w.text_of("ObjectId").unwrap_or_default().into(),
            proj_id.clone(),
            "N".into(),
            w.text_of("Code").unwrap_or_default().into(),
            w.text_of("Name").unwrap_or_default().into(),
            w.text_of("ParentObjectId").unwrap_or(&node).into(),
            w.text_of("OriginalBudget").unwrap_or_default().into(),
            w.text_of("OBSObjectId").unwrap_or_default().into(),
            (k + 1).to_string(),
        ]);
    }

    let mut odd_accruals = 0usize;
    for m in &MAPS {
        let mut columns: Vec<&str> = m.columns.iter().map(|(c, _, _)| *c).collect();
        match m.table {
            "RSRCRATE" => columns.push("max_qty_per_hr"),
            "TASKRSRC" => columns.extend(["act_reg_qty", "act_ot_qty"]),
            _ => {}
        }
        for e in objects(m.element) {
            let mut r = row(e, m.columns);
            let at = |column: &str| m.columns.iter().position(|(c, _, _)| *c == column);
            if let Some(k) = at("proj_id").filter(|_| m.in_project)
                && let Some(v) = r.get_mut(k)
            {
                *v = proj_id.clone();
            }
            match m.table {
                "RSRCRATE" => {
                    let resource = e.text_of("ResourceObjectId");
                    let max = root.all("Resource").find(|x| x.text_of("ObjectId") == resource).and_then(|x| x.text_of("MaxUnitsPerTime"));
                    r.push(max.unwrap_or("1").to_string());
                }
                "TASKRSRC" => r.extend([e.text_of("ActualUnits").unwrap_or("0").to_string(), "0".to_string()]),
                // An activity in no WBS element is under the project's node; its remaining
                // duration is all of it until it starts, none once it finishes.
                "TASK" => {
                    if let Some(v) = at("wbs_id").and_then(|k| r.get_mut(k)).filter(|v| v.is_empty()) {
                        *v = node.clone();
                    }
                    let remaining = match e.text_of("Status") {
                        Some("Completed") => "0".to_string(),
                        _ => e.text_of("PlannedDuration").unwrap_or("0").to_string(),
                    };
                    if let Some(v) = at("remain_drtn_hr_cnt").and_then(|k| r.get_mut(k)).filter(|v| v.is_empty()) {
                        *v = remaining;
                    }
                }
                "TASKPRED" => {
                    for column in ["proj_id", "pred_proj_id"] {
                        if let Some(v) = at(column).and_then(|k| r.get_mut(k)) {
                            *v = proj_id.clone();
                        }
                    }
                }
                "PROJCOST" => odd_accruals += usize::from(e.text_of("AccrualType").is_some_and(|a| a != "Uniform over Activity")),
                _ => {}
            }
            table_mut(&mut tables, m.table, &columns).rows.push(r);
        }
        // A resource without a price still has its most units an hour, on a price of none.
        if m.table == "RSRCRATE" {
            let mut next = objects("ResourceRate").filter_map(|x| x.text_of("ObjectId")?.parse::<u64>().ok()).max().unwrap_or(0);
            for res in root.all("Resource") {
                let id = res.text_of("ObjectId");
                if id.is_none() || root.all("ResourceRate").any(|x| x.text_of("ResourceObjectId") == id) {
                    continue;
                }
                next = next.saturating_add(1);
                table_mut(&mut tables, m.table, &columns).rows.push(vec![
                    next.to_string(),
                    id.unwrap_or_default().to_string(),
                    date_of("PlannedStartDate"),
                    "0".into(),
                    res.text_of("MaxUnitsPerTime").unwrap_or("1").to_string(),
                ]);
            }
        }
    }
    if odd_accruals > 0 {
        warnings.push(tr!("warn-xer-accrual", n = odd_accruals));
    }
    if !project.classification.fields.is_empty() {
        warnings.push(tr!("warn-xer-fields", n = project.classification.fields.len()));
    }
    // What P6 XML holds and XER isn't given here, each kind named once.
    let written: BTreeSet<&str> = MAPS.iter().map(|m| m.element).chain(["Calendar", "Project", "WBS", "UDFType", "UDFValue"]).collect();
    let left: BTreeSet<&str> = root
        .children
        .iter()
        .chain(p.children.iter().filter(|c| !c.children.is_empty()))
        .map(|c| c.name.as_str())
        .filter(|n| !written.contains(n))
        .collect();
    if !left.is_empty() {
        warnings.push(tr!("warn-xer-left-out", kinds = left.into_iter().collect::<Vec<_>>().join(", ")));
    }
    // The tables in the order P6's own files list them: what others name comes first.
    const ORDER: [&str; 20] = [
        "OBS", "UMEASURE", "ROLES", "ACCOUNT", "FINDATES", "ACTVTYPE", "CALENDAR", "PROJECT", "RSRC", "RSRCRATE", "RSRCROLE", "PROJWBS", "ACTVCODE",
        "TASK", "TASKPRED", "TASKRSRC", "PROJCOST", "TASKACTV", "TASKFIN", "UDFVALUE",
    ];
    let mut tables: Vec<Table> = tables.into_values().collect();
    tables.sort_by_key(|t| ORDER.iter().position(|n| *n == t.name).unwrap_or(ORDER.len()));
    file.tables = tables;
    (table::write(&file), warnings)
}
