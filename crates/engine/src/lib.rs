//! MK Perth's session: the open project and its last schedule. Every frontend — the window,
//! later the command line and MCP — acts through `Session::run`, a command by id with JSON
//! parameters, so they all behave the same.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod comments;
mod edit;
mod notes;
mod report;
mod resources;
mod sample;
mod write;

use std::path::{Path, PathBuf};

use mkperth_core::Project;
use mkperth_i18n::tr;
use mkperth_perth::PerthError;
use mkperth_sched::{Schedule, ScheduleError};
use serde_json::Value;
use thiserror::Error;

pub use mkperth_core as core;
pub use mkperth_i18n as i18n;
pub use mkperth_sched as sched;
pub use write::write_atomic;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EngineError {
    #[error("{}", tr!("err-unknown-command", id = .0.as_str()))]
    UnknownCommand(String),
    /// The command and the parameter it lacks or can't read.
    #[error("{}", tr!("err-command-param", command = .0.as_str(), param = .1.as_str()))]
    Param(String, String),
    /// The path and what the system said.
    #[error("{}", tr!("err-file", path = .0.as_str(), detail = .1.as_str()))]
    File(String, String),
    /// The command and the parameter it can't read.
    #[error("{}", tr!("err-command-value", command = .0.as_str(), param = .1.as_str()))]
    Value(String, String),
    #[error("{}", tr!("err-no-activity", id = .0.as_str()))]
    UnknownActivity(String),
    #[error("{}", tr!("err-id-taken", id = .0.as_str()))]
    DuplicateId(String),
    #[error("{}", tr!("err-relationship-exists", predecessor = .0.as_str(), successor = .1.as_str()))]
    DuplicateRelationship(String, String),
    #[error("{}", tr!("err-no-relationship", predecessor = .0.as_str(), successor = .1.as_str()))]
    UnknownRelationship(String, String),
    /// The activity whose secondary constraint can't stand beside its primary.
    #[error("{}", tr!("err-constraint-pair", id = .0.as_str()))]
    ConstraintPair(String),
    #[error("{}", tr!("err-nothing-to-undo"))]
    NothingToUndo,
    #[error("{}", tr!("err-nothing-to-redo"))]
    NothingToRedo,
    #[error("{}", tr!("err-no-comment", id = .0.as_str()))]
    UnknownComment(String),
    #[error("{}", tr!("err-no-resource", id = .0.as_str()))]
    UnknownResource(String),
    #[error("{}", tr!("err-resource-taken", id = .0.as_str()))]
    ResourceIdTaken(String),
    #[error("{}", tr!("err-no-role", id = .0.as_str()))]
    UnknownRole(String),
    #[error("{}", tr!("err-role-taken", id = .0.as_str()))]
    RoleIdTaken(String),
    /// The activity and the resource (or role) already on it.
    #[error("{}", tr!("err-assignment-exists", activity = .0.as_str(), resource = .1.as_str()))]
    AssignmentExists(String, String),
    #[error("{}", tr!("err-no-assignment", activity = .0.as_str(), resource = .1.as_str()))]
    UnknownAssignment(String, String),
    /// A command that failed in a way no error foresaw; the project was put back as it was.
    #[error("{}", tr!("err-internal", command = .0.as_str()))]
    Internal(String),
    #[error(transparent)]
    Perth(#[from] PerthError),
    #[error(transparent)]
    P6(#[from] mkperth_p6xml::P6Error),
    #[error(transparent)]
    Schedule(#[from] ScheduleError),
}

/// The commands, by id: what `run` takes.
pub const COMMANDS: [&str; 37] = [
    "project.new",
    "project.open",
    "project.save",
    "project.export",
    "project.sample",
    "project.schedule",
    "activity.export-tex",
    "activity.add",
    "activity.edit",
    "activity.delete",
    "relationship.add",
    "relationship.delete",
    "project.options",
    "calendar.edit",
    "resource.add",
    "resource.edit",
    "resource.delete",
    "role.add",
    "role.delete",
    "assignment.add",
    "assignment.edit",
    "assignment.delete",
    "resource.level",
    "resource.unlevel",
    "comment.add",
    "comment.reply",
    "comment.edit",
    "comment.resolve",
    "comment.delete",
    "edit.undo",
    "edit.redo",
    "band.add",
    "band.delete",
    "marker.add",
    "marker.delete",
    "month-mark.add",
    "month-mark.delete",
];

#[derive(Debug, Clone)]
pub struct Session {
    pub project: Project,
    /// Where the project came from, if a file.
    pub path: Option<PathBuf>,
    /// The last schedule, or why it failed.
    pub schedule: Result<Schedule, ScheduleError>,
    /// What opening the project had to say.
    pub warnings: Vec<String>,
    /// Changes not saved yet.
    pub modified: bool,
    /// Each change's label and the project before it, oldest first.
    undo: Vec<(String, Project)>,
    redo: Vec<(String, Project)>,
}

impl Default for Session {
    /// The sample project, scheduled.
    fn default() -> Self {
        let project = sample::project();
        let schedule = mkperth_sched::schedule(&project);
        Session { project, path: None, schedule, warnings: Vec::new(), modified: false, undo: Vec::new(), redo: Vec::new() }
    }
}

/// Whether a path names a P6 XML file.
fn is_p6(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("xml"))
}

impl Session {
    /// Runs a command by id with its parameters (a JSON object; `null` for none).
    ///
    /// The last resort against a crash: a command that panics (a bug, never by design) is caught,
    /// the session is put back as it was before the command, and the panic is an error.
    pub fn run(&mut self, id: &str, params: &Value) -> Result<(), EngineError> {
        let before = (self.project.clone(), self.path.clone(), self.schedule.clone(), self.warnings.clone(), self.modified);
        let (undo, redo) = (self.undo.len(), self.redo.len());
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.run_command(id, params))) {
            Ok(result) => result,
            Err(_) => {
                (self.project, self.path, self.schedule, self.warnings, self.modified) = before;
                // A change half made may have pushed its undo step (or taken a redo one): back
                // to the stacks' depths before it, nothing after them kept.
                self.undo.truncate(undo);
                self.redo.truncate(redo);
                Err(EngineError::Internal(id.to_string()))
            }
        }
    }

    fn run_command(&mut self, id: &str, params: &Value) -> Result<(), EngineError> {
        let param = |name: &str| params.get(name).and_then(Value::as_str).ok_or_else(|| EngineError::Param(id.to_string(), name.to_string()));
        if let Some(result) = self
            .run_edit(id, params)
            .or_else(|| self.note_command(id, params))
            .or_else(|| self.resource_command(id, params))
            .or_else(|| self.comment_command(id, params))
        {
            return result;
        }
        match id {
            "project.open" => self.open(Path::new(param("path")?)),
            // Without a path, where it was opened from.
            "project.save" => {
                let path = match params.get("path").and_then(Value::as_str) {
                    Some(p) => PathBuf::from(p),
                    None => self.path.clone().ok_or_else(|| EngineError::Param(id.to_string(), "path".to_string()))?,
                };
                self.save(&path)
            }
            // A copy in the format of `path`'s extension: the project stays where it was, as it was.
            "project.export" => {
                let path = Path::new(param("path")?);
                let (text, warnings) = if is_p6(path) { mkperth_p6xml::write(&self.project) } else { mkperth_perth::write(&self.project) };
                write_atomic(path, text.as_bytes()).map_err(|e| EngineError::File(path.display().to_string(), e.to_string()))?;
                self.warnings = warnings;
                Ok(())
            }
            "project.sample" => {
                *self = Session::default();
                Ok(())
            }
            // An empty project from `start` (the engine reads no clock: the caller gives the day),
            // named `name`, on a Monday-to-Friday calendar; a fresh history, nothing to save.
            "project.new" => {
                let start = params.get("start").and_then(Value::as_str).ok_or_else(|| EngineError::Param(id.to_string(), "start".to_string()))?;
                let start = mkperth_core::Date::parse(start).ok_or_else(|| EngineError::Value(id.to_string(), "start".to_string()))?;
                let name = match params.get("name") {
                    None | Some(Value::Null) => String::new(),
                    Some(Value::String(n)) if n.chars().count() <= 2000 => n.clone(),
                    Some(_) => return Err(EngineError::Value(id.to_string(), "name".to_string())),
                };
                let project = sample::empty(name, start);
                let schedule = mkperth_sched::schedule(&project);
                *self = Session { project, path: None, schedule, warnings: Vec::new(), modified: false, undo: Vec::new(), redo: Vec::new() };
                Ok(())
            }
            "project.schedule" => {
                self.reschedule();
                Ok(())
            }
            // The calculation of an activity as LaTeX, dates as 2026-11-02.
            "activity.export-tex" => {
                let param = |name: &str| params.get(name).and_then(Value::as_str).ok_or_else(|| EngineError::Param(id.to_string(), name.to_string()));
                let (activity, path) = (param("id")?, param("path")?);
                self.export_latex(activity, Path::new(path), &|d| d.to_string())
            }
            // A command that panics, to prove the guard above.
            #[cfg(test)]
            "test.panic" => {
                self.project.activities.clear();
                self.undo.push(("half".into(), self.project.clone()));
                panic!("a bug in a command");
            }
            other => Err(EngineError::UnknownCommand(other.to_string())),
        }
    }

    /// Opens a project file and schedules it: P6 XML (`.xml`) or Perth.jl's JSON (anything
    /// else). On failure the open project stays.
    pub fn open(&mut self, path: &Path) -> Result<(), EngineError> {
        let text = std::fs::read_to_string(path).map_err(|e| EngineError::File(path.display().to_string(), e.to_string()))?;
        let (project, warnings) = if is_p6(path) {
            let read = mkperth_p6xml::read(&text)?;
            (read.project, read.warnings)
        } else {
            let read = mkperth_perth::read(&text)?;
            (read.project, read.warnings)
        };
        self.project = project;
        self.warnings = warnings;
        self.path = Some(path.to_path_buf());
        self.modified = false;
        self.undo.clear();
        self.redo.clear();
        self.reschedule();
        Ok(())
    }

    /// Writes the project as P6 XML (`.xml`) or Perth.jl's JSON, all or nothing; it becomes the project's path.
    /// Returns what Perth's format couldn't hold.
    pub fn save(&mut self, path: &Path) -> Result<(), EngineError> {
        let (text, warnings) = if is_p6(path) { mkperth_p6xml::write(&self.project) } else { mkperth_perth::write(&self.project) };
        write_atomic(path, text.as_bytes()).map_err(|e| EngineError::File(path.display().to_string(), e.to_string()))?;
        self.path = Some(path.to_path_buf());
        self.modified = false;
        self.warnings = warnings;
        Ok(())
    }

    /// How the scheduler reached the dates of the activity `id`.
    pub fn explain(&self, id: &str) -> Result<mkperth_sched::Explanation, EngineError> {
        Ok(mkperth_sched::explain(&self.project, id)?)
    }

    /// Writes the calculation of the activity `id` as a LaTeX document at `path`, dates written
    /// by `date`.
    pub fn export_latex(&self, id: &str, path: &Path, date: &dyn Fn(mkperth_core::Date) -> String) -> Result<(), EngineError> {
        let name = self.project.activities.iter().find(|a| a.id == id).map_or("", |a| a.name.as_str());
        let tex = self.explain(id)?.to_latex(&self.project.name, name, date);
        write_atomic(path, tex.as_bytes()).map_err(|e| EngineError::File(path.display().to_string(), e.to_string()))
    }

    /// Schedules the project again.
    pub fn reschedule(&mut self) {
        self.schedule = mkperth_sched::schedule(&self.project);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn edits_undo_and_redo() {
        let mut s = Session::default();
        let original = s.project.clone();
        // A new activity after A10, in its summary, then tied to A20.
        s.run("activity.add", &json!({"after": "A10", "name": "Soil test", "duration": 3})).unwrap();
        let added = s.project.activities.iter().position(|a| a.name == "Soil test").unwrap();
        let id = s.project.activities[added].id.clone();
        assert_eq!(s.project.activities[added - 1].id, "A10");
        assert_eq!(s.project.activities[added].parent.as_deref(), Some("W1"));
        s.run("relationship.add", &json!({"predecessor": id, "successor": "A20", "kind": "SS", "lag": 1})).unwrap();
        s.run(
            "activity.edit",
            &json!({"id": id, "duration": 7, "name": "Soil tests", "constraint": {"type": "start-on-or-after", "date": "2026-11-09"}}),
        )
        .unwrap();
        assert!(s.modified);
        assert_eq!(s.undo_label(), Some(format!("edit activity {id}").as_str()));
        // Renaming the ID renames it in relationships too.
        s.run("activity.edit", &json!({"id": id, "new_id": "A15"})).unwrap();
        assert!(s.project.relationships.iter().any(|r| r.predecessor == "A15" && r.successor == "A20"));
        // Deleting takes its relationships along.
        s.run("activity.delete", &json!({"id": "A15"})).unwrap();
        assert!(!s.project.relationships.iter().any(|r| r.predecessor == "A15"));
        // Undo all the way back restores the project exactly; redo all the way forward again.
        let end = s.project.clone();
        for _ in 0..5 {
            s.run("edit.undo", &Value::Null).unwrap();
        }
        assert_eq!(s.project, original);
        assert_eq!(s.run("edit.undo", &Value::Null), Err(EngineError::NothingToUndo));
        for _ in 0..5 {
            s.run("edit.redo", &Value::Null).unwrap();
        }
        assert_eq!(s.project, end);
        assert_eq!(s.run("edit.redo", &Value::Null), Err(EngineError::NothingToRedo));
    }

    #[test]
    fn scheduling_options_with_undo() {
        let mut s = Session::default();
        let critical = |s: &Session| s.schedule.as_ref().map(|x| x.activities.iter().filter(|a| a.critical).count()).unwrap_or(0);
        let before = critical(&s);
        s.run("project.options", &json!({"critical": "float", "threshold": 25})).unwrap();
        assert!(critical(&s) > before, "B10's float of 19 is under 25");
        s.run("project.options", &json!({"critical": "longest-path", "float": "smallest"})).unwrap();
        assert_eq!(s.project.critical_by, mkperth_core::CriticalBy::LongestPath);
        assert_eq!(s.project.float_kind, mkperth_core::FloatKind::Smallest);
        assert!(matches!(s.run("project.options", &json!({"float": "sideways"})), Err(EngineError::Value(..))));
        assert!(matches!(s.run("project.options", &json!({"progress": "sideways"})), Err(EngineError::Value(..))));
        assert!(matches!(s.run("project.options", &json!({"data_date": "2026-13-01"})), Err(EngineError::Value(..))));
        s.run("edit.undo", &Value::Null).unwrap();
        s.run("edit.undo", &Value::Null).unwrap();
        assert_eq!(critical(&s), before);
    }

    #[test]
    fn the_calculation_exports_as_latex() {
        let mut s = Session::default();
        let dir = std::env::temp_dir().join(format!("mkperth-engine-tex-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a10.tex");
        s.run("activity.export-tex", &json!({"id": "A10", "path": path.display().to_string()})).unwrap();
        let tex = std::fs::read_to_string(&path).unwrap();
        assert!(tex.contains("\\documentclass") && tex.contains("A10") && tex.contains("2026-11-"));
        assert!(matches!(s.run("activity.export-tex", &json!({"id": "nope", "path": path.display().to_string()})), Err(EngineError::Schedule(_))));
        assert!(matches!(s.run("activity.export-tex", &json!({"id": "A10"})), Err(EngineError::Param(..))));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_charts_notes_by_command() {
        let mut s = Session::default();
        let (bands, markers, months) = (s.project.bands.len(), s.project.markers.len(), s.project.month_marks.len());
        // Ends the wrong way round are turned; the list stays in date order.
        s.run("band.add", &json!({"name": "  Rainy   season ", "from": "2026-12-31", "to": "2026-10-01", "color": "#4a90d9"})).unwrap();
        let b = &s.project.bands[0];
        assert_eq!((b.name.as_str(), b.from.to_string(), b.to.to_string()), ("Rainy season", "2026-10-01".into(), "2026-12-31".into()));
        s.run("marker.add", &json!({"name": "Audit", "date": "2026-11-20", "label_at": 40})).unwrap();
        assert_eq!(s.project.markers[0].label_at, 40);
        // One mark a month: the new one replaces the old.
        s.run("month-mark.add", &json!({"month": "2027-01-15", "name": "recess"})).unwrap();
        assert_eq!(s.project.month_marks.len(), months);
        assert_eq!(s.project.month_marks[0].name, "recess");
        assert_eq!(s.project.month_marks[0].month.to_string(), "2027-01-01");
        for bad in [
            ("band.add", json!({"name": "", "from": "2026-10-01", "to": "2026-10-02"})),
            ("band.add", json!({"name": "x", "from": "2026-10-01", "to": "2026-10-02", "color": "blue"})),
            ("band.add", json!({"name": "x", "from": "2026-10-01"})),
            ("marker.add", json!({"name": "x", "date": "2026-10-01", "label_at": 101})),
            ("marker.add", json!({"name": "x".repeat(201), "date": "2026-10-01"})),
            ("band.delete", json!({"index": 99})),
            ("marker.delete", json!({"index": -1})),
            ("month-mark.delete", json!({})),
        ] {
            assert!(s.run(bad.0, &bad.1).is_err(), "{bad:?}");
        }
        s.run("band.delete", &json!({"index": 0})).unwrap();
        assert_eq!(s.project.bands.len(), bands);
        // Undo walks them all back.
        for _ in 0..4 {
            s.run("edit.undo", &Value::Null).unwrap();
        }
        assert_eq!((s.project.bands.len(), s.project.markers.len()), (bands, markers));
        assert_eq!(s.project.month_marks[0].name, "holidays");
    }

    #[test]
    fn a_secondary_constraint_pairs_with_its_primary() {
        let mut s = Session::default();
        let window = json!({"id": "A10", "constraint": {"type": "start-on-or-after", "date": "2026-11-04"},
            "secondary_constraint": {"type": "finish-on-or-before", "date": "2026-11-20"}});
        s.run("activity.edit", &window).unwrap();
        let a = s.project.activities.iter().find(|a| a.id == "A10").unwrap();
        assert!(a.constraint.is_some() && a.secondary_constraint.is_some());
        // Two on the start, a mandatory secondary, no primary at all: refused, the project as it was.
        let before = s.project.clone();
        for bad in [
            json!({"id": "A10", "secondary_constraint": {"type": "start-on-or-before", "date": "2026-11-06"}}),
            json!({"id": "A10", "secondary_constraint": {"type": "mandatory-finish", "date": "2026-11-20"}}),
            json!({"id": "A10", "secondary_constraint": {"type": "as-late-as-possible"}}),
            json!({"id": "A10", "constraint": null}),
            json!({"id": "A10", "constraint": {"type": "as-late-as-possible"}}),
        ] {
            assert_eq!(s.run("activity.edit", &bad), Err(EngineError::ConstraintPair("A10".into())), "{bad}");
        }
        assert_eq!(s.project, before);
        // Notes, kept as typed; too long is refused.
        s.run("activity.edit", &json!({"id": "A10", "notes": "Call the surveyor.\nTwo visits."})).unwrap();
        assert_eq!(s.project.activities.iter().find(|a| a.id == "A10").unwrap().notes, "Call the surveyor.\nTwo visits.");
        assert!(matches!(s.run("activity.edit", &json!({"id": "A10", "notes": "x".repeat(100_001)})), Err(EngineError::Value(..))));
        // An expected finish, then cleared.
        s.run("activity.edit", &json!({"id": "A10", "expected_finish": "2026-11-27"})).unwrap();
        assert_eq!(s.project.activities.iter().find(|a| a.id == "A10").unwrap().expected_finish, mkperth_core::Date::parse("2026-11-27"));
        s.run("activity.edit", &json!({"id": "A10", "expected_finish": null})).unwrap();
        assert_eq!(s.project.activities.iter().find(|a| a.id == "A10").unwrap().expected_finish, None);
    }

    #[test]
    fn progress_is_told_by_commands() {
        let mut s = Session::default();
        let original = s.project.clone();
        let status = |s: &Session, id: &str| {
            s.schedule.as_ref().ok().and_then(|x| x.activities.iter().find(|a| a.id == id).map(|a| (a.status, a.remaining_minutes)))
        };
        s.run("project.options", &json!({"data_date": "2026-11-10", "progress": "progress-override"})).unwrap();
        assert_eq!(s.project.data_date, mkperth_core::Date::parse("2026-11-10"));
        assert_eq!(s.project.progress_mode, mkperth_core::ProgressMode::ProgressOverride);
        s.run("activity.edit", &json!({"id": "A10", "actual_start": "2026-11-03", "actual_finish": "2026-11-06"})).unwrap();
        assert_eq!(status(&s, "A10"), Some((mkperth_sched::Status::Completed, 0)));
        s.run("activity.edit", &json!({"id": "A10", "actual_finish": null, "remaining": 2})).unwrap();
        assert_eq!(status(&s, "A10"), Some((mkperth_sched::Status::InProgress, 960)));
        assert!(matches!(
            s.run("activity.edit", &json!({"id": "A10", "actual_finish": "2026-11-02"})),
            Err(EngineError::Value(_, ref name)) if name == "actual_finish"
        ));
        assert!(matches!(s.run("activity.edit", &json!({"id": "A10", "remaining": -1})), Err(EngineError::Value(..))));
        for _ in 0..3 {
            s.run("edit.undo", &Value::Null).unwrap();
        }
        assert_eq!(s.project, original);
    }

    #[test]
    fn edits_that_would_break_the_project_are_refused() {
        let mut s = Session::default();
        let before = s.project.clone();
        // A loop: C50 already follows A10 through the network.
        let looped = s.run("relationship.add", &json!({"predecessor": "C50", "successor": "A10"}));
        assert!(matches!(looped, Err(EngineError::Schedule(ScheduleError::Loop(_)))), "{looped:?}");
        assert_eq!(s.run("activity.add", &json!({"id": "A10"})), Err(EngineError::DuplicateId("A10".into())));
        assert_eq!(s.run("activity.edit", &json!({"id": "A10", "new_id": "A20"})), Err(EngineError::DuplicateId("A20".into())));
        assert!(matches!(s.run("relationship.add", &json!({"predecessor": "A10", "successor": "A20"})), Err(EngineError::DuplicateRelationship(..))));
        assert!(matches!(
            s.run("relationship.delete", &json!({"predecessor": "A20", "successor": "A10"})),
            Err(EngineError::UnknownRelationship(..))
        ));
        assert!(matches!(s.run("activity.edit", &json!({"id": "nope", "name": "x"})), Err(EngineError::UnknownActivity(_))));
        assert!(matches!(s.run("activity.edit", &json!({"id": "A10", "duration": -1})), Err(EngineError::Value(..))));
        assert!(matches!(s.run("activity.edit", &json!({"id": "A10", "calendar": "Moon"})), Err(EngineError::Value(..))));
        assert!(matches!(
            s.run("activity.edit", &json!({"id": "A10", "constraint": {"type": "start-on", "date": "2026-02-30"}})),
            Err(EngineError::Value(..))
        ));
        assert_eq!(s.project, before, "nothing refused changed anything");
        assert!(!s.modified && s.undo_label().is_none());
    }

    /// Every command survives empty and hostile parameters: an error or a change, never a panic,
    /// and an error leaves the project as it was.
    #[test]
    fn every_command_survives_empty_and_hostile_params() {
        let hostile = [
            Value::Null,
            json!({}),
            json!([1, 2]),
            json!("text"),
            json!({"id": 7, "path": 3, "predecessor": null, "successor": [], "kind": "XX", "lag": 1e300, "duration": -5, "after": {}}),
            json!({"id": "A10", "duration": i64::MAX, "lag": i64::MIN, "constraint": {"type": "nope"}}),
            json!({"id": "x".repeat(5000), "name": "y".repeat(5000)}),
            json!({"predecessor": "A10", "successor": "A10"}),
        ];
        for command in COMMANDS {
            for params in &hostile {
                let mut s = Session::default();
                let before = s.project.clone();
                if s.run(command, params).is_err() {
                    assert_eq!(s.project, before, "{command} {params}");
                }
            }
        }
    }

    /// A command that panics is an error, and the session is as it was before it: the project,
    /// its schedule and the undo history.
    #[test]
    fn a_command_that_panics_puts_the_session_back() {
        let mut s = Session::default();
        s.run("activity.edit", &json!({"id": "A10", "name": "Survey"})).unwrap();
        let (project, undo) = (s.project.clone(), s.undo_label().map(str::to_string));
        assert_eq!(s.run("test.panic", &Value::Null), Err(EngineError::Internal("test.panic".into())));
        assert_eq!(s.project, project);
        assert!(s.schedule.is_ok());
        assert_eq!(s.undo_label().map(str::to_string), undo, "the half-made step is gone");
        // The session goes on: undo still takes back the edit before.
        s.run("edit.undo", &Value::Null).unwrap();
        assert_ne!(s.project, project);
    }

    /// An export writes a copy in the extension's format and leaves the project where it was.
    #[test]
    fn an_export_is_a_copy() {
        let dir = std::env::temp_dir().join(format!("mkperth-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = Session::default();
        s.run("activity.edit", &json!({"id": "A10", "name": "Survey"})).unwrap();
        for name in ["copy.xml", "copy.json"] {
            let path = dir.join(name);
            s.run("project.export", &json!({"path": path.display().to_string()})).unwrap();
            assert!(std::fs::read_to_string(&path).unwrap().contains("Survey"));
        }
        assert!(s.path.is_none() && s.modified, "still unsaved, still nowhere");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Several activities deleted as one change: one undo brings them all back.
    #[test]
    fn activities_are_deleted_together() {
        let mut s = Session::default();
        let n = s.project.activities.len();
        s.run("activity.delete", &json!({"ids": ["A10", "A20", "B10"]})).unwrap();
        assert_eq!(s.project.activities.len(), n - 3);
        assert_eq!(s.undo_label(), Some("delete 3 activities"));
        assert!(s.run("activity.delete", &json!({"ids": ["A30", "ZZ"]})).is_err(), "all or nothing");
        assert_eq!(s.project.activities.len(), n - 3);
        s.run("edit.undo", &Value::Null).unwrap();
        assert_eq!(s.project.activities.len(), n);
    }

    /// A new project is empty, from its start, with nothing to undo or save.
    #[test]
    fn a_new_project_starts_empty() {
        let mut s = Session::default();
        s.run("activity.edit", &json!({"id": "A10", "name": "Survey"})).unwrap();
        assert!(matches!(s.run("project.new", &json!({})), Err(EngineError::Param(..))), "the start is needed");
        assert!(matches!(s.run("project.new", &json!({"start": "soon"})), Err(EngineError::Value(..))));
        s.run("project.new", &json!({"start": "2026-11-06", "name": "Warehouse"})).unwrap();
        assert_eq!((s.project.name.as_str(), s.project.start.to_string().as_str()), ("Warehouse", "2026-11-06"));
        assert!(s.project.activities.is_empty() && s.path.is_none() && !s.modified && s.undo_label().is_none());
        assert!(s.schedule.is_ok(), "an empty project schedules");
    }

    /// The week, the holidays, the work periods and the region of a calendar, each undoable.
    /// A10 takes 5 days from Friday 6 November 2026: Monday to Friday, it works the 6th, 9th,
    /// 10th, 11th and 12th; seven days a week, the 6th to the 10th; with the 9th a holiday, the
    /// 6th, 7th, 8th, 10th and 11th.
    #[test]
    fn calendars_are_edited_by_command() {
        let day = |y, m, d| mkperth_core::Date::from_ymd(y, m, d).unwrap();
        let mut s = Session::default();
        s.run("project.new", &json!({"start": "2026-11-06"})).unwrap();
        s.run("activity.add", &json!({"id": "A10", "duration": 5})).unwrap();
        let finish = |s: &Session| s.schedule.as_ref().unwrap().activities[0].early_finish;
        assert_eq!(finish(&s), day(2026, 11, 12));
        s.run("calendar.edit", &json!({"week": [true, true, true, true, true, true, true]})).unwrap();
        assert_eq!(finish(&s), day(2026, 11, 10));
        s.run("calendar.edit", &json!({"holidays_add": ["2026-11-09"]})).unwrap();
        assert_eq!(finish(&s), day(2026, 11, 11));
        assert_eq!(s.undo_label(), Some("edit calendar Standard"));
        s.run("edit.undo", &Value::Null).unwrap();
        assert_eq!(finish(&s), day(2026, 11, 10));
        s.run("calendar.edit", &json!({"periods": [[480, 720], [780, 960]], "region": "brazil", "calendar": "Standard"})).unwrap();
        assert_eq!(s.project.calendar.periods[0], [(480, 720), (780, 960)]);
        assert_eq!(s.project.calendar.region, Some(mkperth_core::Region::Brazil));
        s.run("calendar.edit", &json!({"region": null, "worked_add": ["2026-11-14"], "worked_remove": ["2026-11-14"]})).unwrap();
        assert_eq!(s.project.calendar.region, None);
        // Refused, and the project as it was: no change asked for, no working day, periods that
        // overlap or run backwards, a calendar the project doesn't have, a date that isn't one.
        let before = s.project.clone();
        for params in [
            json!({}),
            json!({"week": [false, false, false, false, false, false, false]}),
            json!({"week": [true, true]}),
            json!({"periods": [[480, 720], [700, 900]]}),
            json!({"periods": [[720, 480]]}),
            json!({"periods": [[0, 1441]]}),
            json!({"calendar": "Night shift", "week": [true, true, true, true, true, true, true]}),
            json!({"holidays_add": ["2026-02-30"]}),
            json!({"region": "Atlantis"}),
        ] {
            assert!(s.run("calendar.edit", &params).is_err(), "{params}");
            assert_eq!(s.project, before, "{params}");
        }
    }

    #[test]
    fn the_sample_schedules_and_has_a_critical_path() {
        let s = Session::default();
        let schedule = s.schedule.as_ref().unwrap();
        assert!(schedule.activities.iter().any(|a| a.critical));
        assert!(schedule.activities.iter().any(|a| !a.critical));
        assert!(s.project.activities.iter().any(|a| a.parent.is_some()), "a WBS");
    }

    #[test]
    fn commands_and_their_errors() {
        let mut s = Session::default();
        assert_eq!(s.run("no.such", &Value::Null), Err(EngineError::UnknownCommand("no.such".into())));
        assert_eq!(s.run("project.open", &Value::Null), Err(EngineError::Param("project.open".into(), "path".into())));
        let missing = serde_json::json!({"path": "/nonexistent/x.json"});
        assert!(matches!(s.run("project.open", &missing), Err(EngineError::File(..))));
        assert!(s.run("project.schedule", &Value::Null).is_ok());
        // A file that isn't a project leaves the open one.
        let dir = std::env::temp_dir().join(format!("mkperth-engine-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bad = dir.join("bad.json");
        std::fs::write(&bad, "[1]").unwrap();
        let before = s.project.name.clone();
        assert!(s.open(&bad).is_err());
        assert_eq!(s.project.name, before);
        let good = dir.join("good.json");
        std::fs::write(&good, r#"{"name": "p", "tasks": [{"id": "a", "start": "2026-09-01", "duration": 3}], "calendar": ""}"#).unwrap();
        s.run("project.open", &serde_json::json!({"path": good.display().to_string()})).unwrap();
        assert_eq!((s.project.name.as_str(), s.schedule.as_ref().map(|x| x.activities.len())), ("p", Ok(1)));
        // Saved where it came from, and read back the same.
        s.run("project.save", &Value::Null).unwrap();
        let mut again = Session::default();
        again.open(&good).unwrap();
        assert_eq!(again.project.activities, s.project.activities);
        // The sample has no path: saving it needs one.
        let mut sample = Session::default();
        assert!(matches!(sample.run("project.save", &Value::Null), Err(EngineError::Param(..))));
        let to = dir.join("sample.json");
        sample.run("project.save", &serde_json::json!({"path": to.display().to_string()})).unwrap();
        assert_eq!(sample.path.as_deref(), Some(to.as_path()));
        assert!(Session::default().open(&to).is_ok());
        // The same project as P6 XML, by its extension, and back.
        let xml = dir.join("sample.xml");
        sample.run("project.save", &serde_json::json!({"path": xml.display().to_string()})).unwrap();
        assert!(std::fs::read_to_string(&xml).unwrap().contains("<APIBusinessObjects"));
        let mut from_p6 = Session::default();
        from_p6.open(&xml).unwrap();
        assert_eq!(from_p6.project.activities.len(), sample.project.activities.len());
        let finish = |s: &Session| s.schedule.as_ref().map(|x| x.finish).ok();
        assert_eq!(finish(&from_p6), finish(&sample), "the P6 copy schedules to the same finish");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
