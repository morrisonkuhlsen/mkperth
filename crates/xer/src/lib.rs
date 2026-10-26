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
/// the calendars, the user-defined fields, the baselines and the notes need more than that (see
/// `read` and `write`).
const MAPS: [Map; 20] = [
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
        table: "FUNDSRC",
        element: "FundingSource",
        in_project: false,
        columns: &[
            ("fund_id", "ObjectId", Text),
            ("fund_name", "Name", Text),
            ("fund_descr", "Description", Text),
            ("parent_fund_id", "ParentObjectId", Text),
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
    // A resource's most units an hour are on its prices (RSRCRATE), P6 XML's on the resource and
    // on its rates: the earliest rate's is the resource's.
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
            ("max_qty_per_hr", "MaxUnitsPerTime", Text),
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
            // A baseline's activities keep their dates here.
            ("target_start_date", "PlannedStartDate", Date),
            ("target_end_date", "PlannedFinishDate", Date),
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
    Map {
        table: "PROJFUND",
        element: "ProjectFunding",
        in_project: true,
        columns: &[
            ("proj_fund_id", "ObjectId", Text),
            ("proj_id", "ProjectObjectId", Text),
            ("fund_id", "FundingSourceObjectId", Text),
            ("fund_cost", "Amount", Text),
            ("fund_wt", "FundShare", Text),
        ],
    },
    // The project's own plan and log are on its node here, on no WBS there.
    Map {
        table: "WBSBUDG",
        element: "ProjectSpendingPlan",
        in_project: true,
        columns: &[
            ("wbs_budg_id", "ObjectId", Text),
            ("proj_id", "ProjectObjectId", Text),
            ("wbs_id", "WBSObjectId", Text),
            ("start_date", "Date", Date),
            ("spend_cost", "SpendingPlan", Text),
        ],
    },
    // The status's codes aren't published: read by their sense, not written.
    Map {
        table: "BUDGCHNG",
        element: "ProjectBudgetChangeLog",
        in_project: true,
        columns: &[
            ("budg_chng_id", "ObjectId", Text),
            ("proj_id", "ProjectObjectId", Text),
            ("wbs_id", "WBSObjectId", Text),
            ("chng_short_name", "ChangeNumber", Text),
            ("chng_date", "Date", Date),
            ("chng_cost", "Amount", Text),
            ("chng_by_name", "Responsible", Text),
            ("chng_descr", "Reason", Text),
        ],
    },
];

/// The tables read besides those in `MAPS`.
const READ_TOO: [&str; 9] = ["PROJECT", "PROJWBS", "CALENDAR", "UDFTYPE", "UDFVALUE", "CURRTYPE", "MEMOTYPE", "TASKMEMO", "WBSMEMO"];

/// The notebook topic MK Perth's notes are written under.
const TOPIC: &str = "Notes";

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

/// P6 XML's status of a budget change, by the sense of XER's code (whose codes aren't published).
fn change_status(code: &str) -> &'static str {
    let c = code.to_ascii_lowercase();
    if c.contains("not") || c.contains("reject") {
        "Not Approved"
    } else if c.contains("approv") {
        "Approved"
    } else {
        "Pending"
    }
}

/// A notebook's text as plain text: notebooks may hold HTML (a text wrapped in tags, closed),
/// whose line breaks and paragraphs become lines and whose other tags are dropped.
fn plain(text: &str) -> String {
    let t = text.trim();
    if !(t.starts_with('<') && t.ends_with('>') && t.contains("</")) {
        return t.to_string();
    }
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '<' {
            out.push(c);
            continue;
        }
        let tag: String = chars.by_ref().take_while(|c| *c != '>').collect::<String>().trim().to_ascii_lowercase();
        if ["br", "/p", "/div", "/li", "/h"].iter().any(|t| tag.starts_with(t)) {
            out.push('\n');
        }
    }
    let out =
        out.replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&");
    let mut lines: Vec<&str> = Vec::new();
    for line in out.lines().map(str::trim_end) {
        if !(line.trim().is_empty() && lines.last().is_none_or(|l| l.trim().is_empty())) {
            lines.push(line);
        }
    }
    lines.join("\n").trim().to_string()
}

/// Reads an XER file: its first project, with the calendars, resources, codes and fields it
/// names.
pub fn read(text: &str) -> Result<Read, XerError> {
    let file = table::parse(text)?;
    let projects: Vec<Record> = file.records("PROJECT").collect();
    // The project is the first that is no other's baseline; its baselines are projects of their
    // own, each naming it as their original.
    let Some(project) = projects.iter().find(|r| r.get("orig_proj_id").is_none()).or(projects.first()) else { return Err(XerError::NoProject) };
    let proj_id = project.get("proj_id").unwrap_or_default();
    let baselines: Vec<&Record> = projects.iter().filter(|r| r.get("orig_proj_id") == Some(proj_id) && r.get("proj_id") != Some(proj_id)).collect();
    let mut warnings = Vec::new();
    let others = projects.len().saturating_sub(baselines.len());
    if others > 1 {
        warnings.push(tr!("warn-xer-projects", n = others));
    }
    // A row of another project's is left out; one of none (a global row) is kept.
    let mine = |r: &Record| r.get("proj_id").is_none_or(|p| p == proj_id);
    let node = file.records("PROJWBS").filter(mine).find(|r| r.get("proj_node_flag") == Some("Y"));
    let node_id = node.and_then(|n| n.get("wbs_id"));

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
                "BUDGCHNG" => c.push(("Status".into(), change_status(r.get("status_code").unwrap_or_default()).into())),
                _ => {}
            }
            if matches!(m.table, "WBSBUDG" | "BUDGCHNG") && r.get("wbs_id") == node_id {
                c.retain(|(name, _)| name != "WBSObjectId");
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

    // Each baseline's activities at their planned dates; their cost is what their assignments
    // and expenses plan.
    let mut planned: HashMap<&str, f64> = HashMap::new();
    for (table, column) in [("PROJCOST", "target_cost"), ("TASKRSRC", "target_cost")] {
        for r in file.records(table) {
            let (Some(task), Some(cost)) = (r.get("task_id"), r.get(column).and_then(|c| c.parse::<f64>().ok()).filter(|c| c.is_finite())) else {
                continue;
            };
            *planned.entry(task).or_insert(0.0) += cost;
        }
    }
    for b in &baselines {
        let Some(bid) = b.get("proj_id") else { continue };
        let name = file
            .records("PROJWBS")
            .find(|r| r.get("proj_id") == Some(bid) && r.get("proj_node_flag") == Some("Y"))
            .and_then(|r| r.get("wbs_name"))
            .or_else(|| b.get("proj_short_name"))
            .unwrap_or_default();
        let mut c = vec![("ObjectId".to_string(), bid.to_string()), ("Name".into(), name.into())];
        if let Some(id) = b.get("proj_short_name") {
            c.push(("Id".into(), id.into()));
        }
        let mut activities = String::new();
        for r in file.records("TASK").filter(|r| r.get("proj_id") == Some(bid)) {
            let mut a = Vec::new();
            for (value, element) in [
                (r.get("task_code").map(str::to_string), "Id"),
                (r.get("target_start_date").and_then(xml_date), "PlannedStartDate"),
                (r.get("target_end_date").and_then(xml_date), "PlannedFinishDate"),
                (r.get("target_drtn_hr_cnt").map(str::to_string), "PlannedDuration"),
                (r.get("task_id").and_then(|t| planned.get(t)).map(|c| format!("{c}")), "PlannedTotalCost"),
            ] {
                if let Some(v) = value {
                    a.push((element.to_string(), v));
                }
            }
            activities.push_str(&element("Activity", &a, ""));
        }
        root.push_str(&element("BaselineProject", &c, &activities));
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
        (project.get("sum_base_proj_id").filter(|b| baselines.iter().any(|r| r.get("proj_id") == Some(*b))), "CurrentBaselineProjectObjectId"),
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
    // The notebooks of activities and WBS elements: one topic's text as it is, several each under
    // its topic's name.
    let topics: HashMap<&str, &str> = file.records("MEMOTYPE").filter_map(|r| Some((r.get("memo_type_id")?, r.get("memo_type")?))).collect();
    // P6 XML's reader keeps each activity's and WBS element's object.
    let by_object = |key: &str| -> HashMap<String, String> {
        read.project.activities.iter().filter_map(|a| Some((a.extra.get(key)?.clone(), a.id.clone()))).collect()
    };
    let (tasks, wbs) = (by_object("p6.object"), by_object("p6.wbs"));
    let mut notes: HashMap<String, Vec<(Option<&str>, String)>> = HashMap::new();
    let memos = file
        .records("TASKMEMO")
        .filter(mine)
        .filter_map(|r| Some((tasks.get(r.get("task_id")?)?.clone(), r, r.get("task_memo")?)))
        .chain(file.records("WBSMEMO").filter(mine).filter_map(|r| Some((wbs.get(r.get("wbs_id")?)?.clone(), r, r.get("wbs_memo")?))));
    for (id, r, text) in memos {
        let text = plain(text);
        if !text.is_empty() {
            notes.entry(id).or_default().push((r.get("memo_type_id").and_then(|t| topics.get(t)).copied(), text));
        }
    }
    for a in &mut read.project.activities {
        match notes.remove(&a.id).as_deref() {
            Some([(_, text)]) => a.notes = text.clone(),
            Some(list) if !list.is_empty() => {
                a.notes = list.iter().map(|(topic, text)| format!("{}\n{text}", topic.unwrap_or(TOPIC))).collect::<Vec<_>>().join("\n\n");
            }
            _ => {}
        }
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
    let project_columns =
        ["proj_id", "proj_short_name", "clndr_id", "plan_start_date", "last_recalc_date", "plan_end_date", "orig_proj_id", "sum_base_proj_id"];
    let t = table_mut(&mut tables, "PROJECT", &project_columns);
    let date_of = |name: &str| p.text_of(name).map(xer_date).unwrap_or_default();
    let calendar = p.text_of("ActivityDefaultCalendarObjectId").unwrap_or_default().to_string();
    t.rows.push(vec![
        proj_id.clone(),
        p.text_of("Id").unwrap_or_default().to_string(),
        calendar.clone(),
        date_of("PlannedStartDate"),
        date_of("DataDate"),
        date_of("MustFinishByDate"),
        String::new(),
        p.text_of("CurrentBaselineProjectObjectId").unwrap_or_default().to_string(),
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

    let (mut odd_accruals, mut odd_statuses) = (0usize, 0usize);
    for m in &MAPS {
        let mut columns: Vec<&str> = m.columns.iter().map(|(c, _, _)| *c).collect();
        if m.table == "TASKRSRC" {
            columns.extend(["act_reg_qty", "act_ot_qty"]);
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
                // The project's own plan and log are on its node.
                "WBSBUDG" | "BUDGCHNG" => {
                    if let Some(v) = at("wbs_id").and_then(|k| r.get_mut(k)).filter(|v| v.is_empty()) {
                        *v = node.clone();
                    }
                    odd_statuses += usize::from(m.table == "BUDGCHNG" && e.text_of("Status").is_some_and(|s| s != "Pending"));
                }
                // A rate written without a limit has the resource's.
                "RSRCRATE" => {
                    let resource = e.text_of("ResourceObjectId");
                    let max = root.all("Resource").find(|x| x.text_of("ObjectId") == resource).and_then(|x| x.text_of("MaxUnitsPerTime"));
                    if let Some(v) = at("max_qty_per_hr").and_then(|k| r.get_mut(k)).filter(|v| v.is_empty()) {
                        *v = max.unwrap_or("1").to_string();
                    }
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
    if odd_statuses > 0 {
        warnings.push(tr!("warn-xer-budget-status", n = odd_statuses));
    }
    write_baselines(&root, &proj_id, &calendar, &node, &mut tables);
    write_notes(project, p, &proj_id, &mut tables, &mut warnings);
    if !project.classification.fields.is_empty() {
        warnings.push(tr!("warn-xer-fields", n = project.classification.fields.len()));
    }
    // What P6 XML holds and XER isn't given here, each kind named once.
    let written: BTreeSet<&str> =
        MAPS.iter().map(|m| m.element).chain(["Calendar", "Project", "WBS", "UDFType", "UDFValue", "BaselineProject"]).collect();
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
    const ORDER: [&str; 27] = [
        "OBS", "UMEASURE", "ROLES", "ACCOUNT", "FUNDSRC", "MEMOTYPE", "FINDATES", "ACTVTYPE", "CALENDAR", "PROJECT", "RSRC", "RSRCRATE", "RSRCROLE",
        "PROJWBS", "PROJFUND", "WBSBUDG", "BUDGCHNG", "WBSMEMO", "ACTVCODE", "TASK", "TASKPRED", "TASKRSRC", "PROJCOST", "TASKACTV", "TASKFIN",
        "TASKMEMO", "UDFVALUE",
    ];
    let mut tables: Vec<Table> = tables.into_values().collect();
    tables.sort_by_key(|t| ORDER.iter().position(|n| *n == t.name).unwrap_or(ORDER.len()));
    file.tables = tables;
    (table::write(&file), warnings)
}

/// Each baseline as P6 keeps one: a project of its own naming the project as its original, under
/// its own node, its activities at their planned dates; their cost an expense of each.
fn write_baselines(root: &Element, proj_id: &str, calendar: &str, node: &str, tables: &mut HashMap<&'static str, Table>) {
    let task = MAPS.iter().find(|m| m.table == "TASK").map_or(&[][..], |m| m.columns);
    let cost_columns = ["cost_item_id", "task_id", "proj_id", "cost_name", "target_cost", "act_cost", "remain_cost"];
    let mut next_node = node.parse::<u64>().unwrap_or(0);
    let mut next_cost = tables.get("PROJCOST").map(|t| t.rows.iter().filter_map(|r| r.first()?.parse::<u64>().ok()).max().unwrap_or(0)).unwrap_or(0);
    for b in root.all("BaselineProject") {
        let Some(bid) = b.text_of("ObjectId") else { continue };
        let name = b.text_of("Name").unwrap_or_default();
        let start = b.all("Activity").filter_map(|a| a.text_of("PlannedStartDate")).min().map(xer_date).unwrap_or_default();
        if let Some(t) = tables.get_mut("PROJECT") {
            t.rows.push(vec![
                bid.to_string(),
                b.text_of("Id").unwrap_or(name).to_string(),
                calendar.to_string(),
                start,
                String::new(),
                String::new(),
                proj_id.to_string(),
                String::new(),
            ]);
        }
        next_node = next_node.saturating_add(1);
        if let Some(t) = tables.get_mut("PROJWBS") {
            let mut row = vec![String::new(); t.columns.len()];
            for (column, value) in
                [("wbs_id", next_node.to_string()), ("proj_id", bid.into()), ("proj_node_flag", "Y".into()), ("wbs_name", name.into())]
            {
                if let Some(v) = t.columns.iter().position(|c| c == column).and_then(|k| row.get_mut(k)) {
                    *v = value;
                }
            }
            t.rows.push(row);
        }
        let columns: Vec<&str> = task.iter().map(|(c, _, _)| *c).collect();
        for a in b.all("Activity") {
            let mut r = row(a, task);
            let at = |column: &str| task.iter().position(|(c, _, _)| *c == column);
            for (column, value) in [
                ("wbs_id", next_node.to_string()),
                ("task_type", "TT_Task".into()),
                ("status_code", "TK_NotStart".into()),
                ("remain_drtn_hr_cnt", a.text_of("PlannedDuration").unwrap_or("0").into()),
            ] {
                if let Some(v) = at(column).and_then(|k| r.get_mut(k)) {
                    *v = value;
                }
            }
            table_mut(tables, "TASK", &columns).rows.push(r);
            let cost = a.text_of("PlannedTotalCost").filter(|c| c.parse::<f64>().is_ok_and(|c| c != 0.0));
            if let (Some(cost), Some(id)) = (cost, a.text_of("ObjectId")) {
                next_cost = next_cost.saturating_add(1);
                let row: [String; 7] = [next_cost.to_string(), id.into(), bid.into(), name.into(), cost.into(), "0".into(), cost.into()];
                let t = table_mut(tables, "PROJCOST", &cost_columns);
                let row: Vec<String> = t
                    .columns
                    .iter()
                    .map(|c| cost_columns.iter().position(|x| x == c).and_then(|k| row.get(k)).cloned().unwrap_or_default())
                    .collect();
                t.rows.push(row);
            }
        }
    }
}

/// The activities' notes in their notebooks, under one topic: a summary's in its WBS element's.
fn write_notes(project: &Project, p: &Element, proj_id: &str, tables: &mut HashMap<&'static str, Table>, warnings: &mut Vec<String>) {
    let noted: Vec<&mkperth_core::Activity> = project.activities.iter().filter(|a| !a.notes.trim().is_empty()).collect();
    // P6 XML's writer leaves them out; here they are written.
    let p6_warning = tr!("warn-p6-notes", n = noted.len());
    warnings.retain(|w| *w != p6_warning);
    if noted.is_empty() {
        return;
    }
    let topic = vec!["1".to_string(), TOPIC.into(), "N".into(), "N".into(), "Y".into(), "Y".into(), "0".into()];
    table_mut(tables, "MEMOTYPE", &["memo_type_id", "memo_type", "eps_flag", "proj_flag", "wbs_flag", "task_flag", "seq_num"]).rows.push(topic);
    let mut lines = 0usize;
    for (k, a) in noted.iter().enumerate() {
        let (table, columns, object) = match p.all("Activity").find(|e| e.text_of("Id") == Some(a.id.as_str())) {
            Some(e) => ("TASKMEMO", ["memo_id", "memo_type_id", "proj_id", "task_id", "task_memo"], e.text_of("ObjectId")),
            None => (
                "WBSMEMO",
                ["wbs_memo_id", "memo_type_id", "proj_id", "wbs_id", "wbs_memo"],
                p.all("WBS").find(|e| e.text_of("Code") == Some(a.id.as_str())).and_then(|e| e.text_of("ObjectId")),
            ),
        };
        let Some(object) = object else { continue };
        lines += usize::from(a.notes.trim().contains('\n'));
        table_mut(tables, table, &columns).rows.push(vec![(k + 1).to_string(), "1".into(), proj_id.into(), object.into(), a.notes.trim().into()]);
    }
    if lines > 0 {
        warnings.push(tr!("warn-xer-note-lines", n = lines));
    }
}
