//! Relationships with other projects: kept in this one, naming the other project's file and
//! activity. The projects they name are read from their files (relative to this project's
//! folder) and scheduled with this one; one that can't be read is left out, with a warning.

use std::path::{Path, PathBuf};

use mkperth_core::{ExternalRelationship, Project, RelationshipType};
use mkperth_i18n::tr;
use mkperth_sched::{Link, Schedule, ScheduleError};
use serde_json::Value;

use crate::costs::{bad, required, text};
use crate::{EngineError, Session};

/// The most relationships to other projects a project may keep.
const MAX_EXTERNAL: usize = 100_000;

fn kind_of(params: &Value, command: &str) -> Result<RelationshipType, EngineError> {
    Ok(match text(params, command, "type")?.as_deref() {
        None | Some("FS") => RelationshipType::FinishToStart,
        Some("SS") => RelationshipType::StartToStart,
        Some("FF") => RelationshipType::FinishToFinish,
        Some("SF") => RelationshipType::StartToFinish,
        Some(_) => return Err(bad(command, "type")),
    })
}

/// Reads a project file: P6 XML (`.xml`) or Perth's JSON.
fn read(path: &Path) -> Result<Project, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    if crate::is_p6(path) {
        mkperth_p6xml::read(&text).map(|r| r.project).map_err(|e| e.to_string())
    } else {
        mkperth_perth::read(&text).map(|r| r.project).map_err(|e| e.to_string())
    }
}

impl Session {
    /// The external relationship commands; `None` when `id` isn't one of them.
    pub(crate) fn external_command(&mut self, id: &str, params: &Value) -> Option<Result<(), EngineError>> {
        Some(match id {
            "relationship.add-external" => self.add_external(params),
            "relationship.delete-external" => self.delete_external(params),
            _ => return None,
        })
    }

    /// Where a project file written as `written` is: beside this project's file when relative.
    fn resolve(&self, written: &str) -> PathBuf {
        let path = Path::new(written);
        match self.path.as_deref().and_then(Path::parent) {
            Some(folder) if path.is_relative() => folder.join(path),
            _ => path.to_path_buf(),
        }
    }

    /// The activities of the project in the file at `path`, by ID and name, to tie to.
    pub fn other_activities(&self, path: &str) -> Result<Vec<(String, String)>, EngineError> {
        let p = read(&self.resolve(path)).map_err(|e| EngineError::File(path.to_string(), e))?;
        Ok(p.activities.into_iter().map(|a| (a.id, a.name)).collect())
    }

    /// `path` as an external relationship writes it: relative to this project's folder when it
    /// is inside it, else as given.
    pub fn written_path(&self, path: &Path) -> String {
        match self.path.as_deref().and_then(Path::parent).and_then(|folder| path.strip_prefix(folder).ok()) {
            Some(rel) => rel.display().to_string(),
            None => path.display().to_string(),
        }
    }

    /// Reads the projects this one's external relationships name; one that can't be read says
    /// why in the warnings and is left out.
    pub(crate) fn load_others(&mut self) {
        let mut written: Vec<String> = self.project.external.iter().map(|x| x.project.clone()).collect();
        written.sort();
        written.dedup();
        self.others.clear();
        for w in written {
            match read(&self.resolve(&w)) {
                Ok(p) => self.others.push((w, p)),
                Err(e) => self.warnings.push(tr!("warn-other-project", path = w.as_str(), detail = e.as_str())),
            }
        }
    }

    /// Schedules `project`, with the other projects it is tied to when there are any.
    pub(crate) fn compute(&self, project: &Project) -> Result<Schedule, ScheduleError> {
        if project.external.is_empty() || self.others.is_empty() {
            return mkperth_sched::schedule(project);
        }
        let mut projects: Vec<&Project> = vec![project];
        projects.extend(self.others.iter().map(|(_, p)| p));
        let place = |written: &str| self.others.iter().position(|(w, _)| w == written).map(|k| k + 1);
        let links: Vec<Link> = project
            .external
            .iter()
            .filter_map(|x| {
                let there = (place(&x.project)?, x.there.clone());
                let here = (0, x.here.clone());
                let (predecessor, successor) = if x.incoming { (there, here) } else { (here, there) };
                Some(Link { predecessor, successor, kind: x.kind, lag: x.lag, lag_minutes: x.lag_minutes })
            })
            .collect();
        let mut all = mkperth_sched::schedule_together(&projects, &links)?;
        if all.is_empty() {
            return mkperth_sched::schedule(project);
        }
        Ok(all.swap_remove(0))
    }

    /// A relationship between `activity` here and `other` in the project file `project`: the
    /// other drives this one when `direction` is `in` (the default), this one the other when
    /// `out`; its `type` (FS, SS, FF, SF) and `lag` in working days.
    fn add_external(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "relationship.add-external";
        let written = required(params, C, "project")?;
        let here = required(params, C, "activity")?;
        let there = required(params, C, "other")?;
        let incoming = match text(params, C, "direction")?.as_deref() {
            None | Some("in") => true,
            Some("out") => false,
            Some(_) => return Err(bad(C, "direction")),
        };
        let kind = kind_of(params, C)?;
        let lag = match params.get("lag") {
            None | Some(Value::Null) => 0,
            Some(v) => v.as_i64().filter(|l| l.abs() <= mkperth_sched::MAX_SPAN).ok_or_else(|| bad(C, "lag"))?,
        };
        // The other project read now, to check its activity is there.
        let other = read(&self.resolve(&written)).map_err(|e| EngineError::File(written.clone(), e))?;
        if !other.activities.iter().any(|a| a.id == there) {
            return Err(bad(C, "other"));
        }
        let label = tr!("undo-add-external", activity = here.as_str(), other = there.as_str());
        self.change(label, |p| {
            if !p.activities.iter().any(|a| a.id == here) {
                return Err(EngineError::UnknownActivity(here.clone()));
            }
            let same = |x: &ExternalRelationship| x.project == written && x.here == here && x.there == there && x.incoming == incoming;
            if p.external.len() >= MAX_EXTERNAL || p.external.iter().any(same) {
                return Err(bad(C, "other"));
            }
            p.external.push(ExternalRelationship {
                project: written.clone(),
                here: here.clone(),
                there: there.clone(),
                incoming,
                kind,
                lag,
                lag_minutes: None,
            });
            Ok(())
        })?;
        // Scheduled with the project it now names.
        self.load_others();
        self.reschedule();
        Ok(())
    }

    /// Deletes the external relationship at `index` in the project's list.
    fn delete_external(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "relationship.delete-external";
        let index = params.get("index").and_then(Value::as_u64).and_then(|n| usize::try_from(n).ok()).ok_or_else(|| bad(C, "index"))?;
        self.change(tr!("undo-delete-external"), |p| {
            if index >= p.external.len() {
                return Err(bad(C, "index"));
            }
            p.external.remove(index);
            Ok(())
        })?;
        self.load_others();
        self.reschedule();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn run(s: &mut Session, id: &str, params: Value) {
        if let Err(e) = s.run(id, &params) {
            panic!("{id} {params}: {e}");
        }
    }

    /// Two projects in files, P (A then B) and Q (X then Y), tied from Q: A → Y and X → B with a
    /// lag of 5, as in the scheduler's hand-worked test. Q's Y then runs from the 9th to the
    /// 12th, after P's A; deleted, it runs from the 5th again. A file that is gone is left out,
    /// with a warning.
    #[test]
    fn projects_tied_through_their_files() {
        let dir = std::env::temp_dir().join(format!("mkperth-external-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let day = |d: u32| mkperth_core::Date::from_ymd(2026, 11, d).unwrap();
        let mut p = Session::default();
        run(&mut p, "project.new", json!({"start": "2026-11-02", "name": "P"}));
        run(&mut p, "activity.add", json!({"id": "A", "name": "A", "duration": 5}));
        run(&mut p, "activity.add", json!({"id": "B", "name": "B", "duration": 2}));
        run(&mut p, "relationship.add", json!({"activities": ["A", "B"]}));
        run(&mut p, "project.save", json!({"path": dir.join("p.json").display().to_string()}));
        let mut q = Session::default();
        run(&mut q, "project.new", json!({"start": "2026-11-02", "name": "Q"}));
        run(&mut q, "activity.add", json!({"id": "X", "name": "X", "duration": 3}));
        run(&mut q, "activity.add", json!({"id": "Y", "name": "Y", "duration": 4}));
        run(&mut q, "relationship.add", json!({"activities": ["X", "Y"]}));
        run(&mut q, "project.save", json!({"path": dir.join("q.json").display().to_string()}));
        let y = |s: &Session| {
            let k = s.project.activities.iter().position(|a| a.id == "Y").unwrap();
            let a = &s.schedule.as_ref().unwrap().activities[k];
            (a.early_start, a.early_finish)
        };
        assert_eq!(y(&q), (day(5), day(10)));
        run(&mut q, "relationship.add-external", json!({"project": "p.json", "activity": "Y", "other": "A"}));
        run(&mut q, "relationship.add-external", json!({"project": "p.json", "activity": "X", "other": "B", "direction": "out", "lag": 5}));
        assert_eq!(y(&q), (day(9), day(12)));
        let x = q.project.activities.iter().position(|a| a.id == "X").unwrap();
        assert_eq!(q.schedule.as_ref().unwrap().activities[x].total_float, 0, "X drives P's B");
        // Saved and opened again, the ties hold.
        run(&mut q, "project.save", json!({}));
        let mut again = Session::default();
        run(&mut again, "project.open", json!({"path": dir.join("q.json").display().to_string()}));
        assert_eq!(y(&again), (day(9), day(12)));
        // Refused: an activity the other project hasn't, a file that isn't there.
        assert!(again.run("relationship.add-external", &json!({"project": "p.json", "activity": "Y", "other": "nope"})).is_err());
        assert!(again.run("relationship.add-external", &json!({"project": "gone.json", "activity": "Y", "other": "A"})).is_err());
        // Deleted, Y runs from the 5th again.
        run(&mut again, "relationship.delete-external", json!({"index": 0}));
        assert_eq!(y(&again), (day(5), day(10)));
        // P's file gone: scheduled alone, with a warning.
        let _ = std::fs::remove_file(dir.join("p.json"));
        run(&mut again, "project.schedule", json!(null));
        assert!(again.warnings.iter().any(|w| w.contains("p.json")), "{:?}", again.warnings);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
