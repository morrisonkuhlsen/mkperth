//! MK Perth's session: the open project and its last schedule. Every frontend — the window,
//! later the command line and MCP — acts through `Session::run`, a command by id with JSON
//! parameters, so they all behave the same.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

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
    #[error(transparent)]
    Perth(#[from] PerthError),
    #[error(transparent)]
    Schedule(#[from] ScheduleError),
}

/// The commands, by id: what `run` takes.
pub const COMMANDS: [&str; 4] = ["project.open", "project.save", "project.sample", "project.schedule"];

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
}

impl Default for Session {
    /// The sample project, scheduled.
    fn default() -> Self {
        let project = sample::project();
        let schedule = mkperth_sched::schedule(&project);
        Session { project, path: None, schedule, warnings: Vec::new(), modified: false }
    }
}

impl Session {
    /// Runs a command by id with its parameters (a JSON object; `null` for none).
    pub fn run(&mut self, id: &str, params: &Value) -> Result<(), EngineError> {
        let param = |name: &str| params.get(name).and_then(Value::as_str).ok_or_else(|| EngineError::Param(id.to_string(), name.to_string()));
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
            "project.sample" => {
                *self = Session::default();
                Ok(())
            }
            "project.schedule" => {
                self.reschedule();
                Ok(())
            }
            other => Err(EngineError::UnknownCommand(other.to_string())),
        }
    }

    /// Opens a Perth.jl project file and schedules it. On failure the open project stays.
    pub fn open(&mut self, path: &Path) -> Result<(), EngineError> {
        let text = std::fs::read_to_string(path).map_err(|e| EngineError::File(path.display().to_string(), e.to_string()))?;
        let read = mkperth_perth::read(&text)?;
        self.project = read.project;
        self.warnings = read.warnings;
        self.path = Some(path.to_path_buf());
        self.reschedule();
        Ok(())
    }

    /// Writes the project as Perth.jl's JSON, all or nothing; it becomes the project's path.
    /// Returns what Perth's format couldn't hold.
    pub fn save(&mut self, path: &Path) -> Result<(), EngineError> {
        let (text, warnings) = mkperth_perth::write(&self.project);
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

    /// Schedules the project again.
    pub fn reschedule(&mut self) {
        self.schedule = mkperth_sched::schedule(&self.project);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
