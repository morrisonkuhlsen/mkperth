//! The schedule written out for scripts and people: JSON and CSV with fixed keys, ISO dates and
//! floats in working days with a point; a table in the person's language.

use mkperth_i18n::tr;
use mkperth_sched::{Scheduled, Status};
use serde_json::{Value, json};

use crate::{EngineError, Session};

/// Working minutes as days of `day` minutes, rounded to two decimals.
fn days(minutes: i64, day: i64) -> f64 {
    (minutes as f64 / day.max(1) as f64 * 100.0).round() / 100.0
}

fn status_code(s: Status) -> &'static str {
    match s {
        Status::NotStarted => "not-started",
        Status::InProgress => "in-progress",
        Status::Completed => "completed",
    }
}

impl Session {
    /// Each activity with its schedule and the minutes of its calendar's working day.
    fn rows(&self) -> Result<Vec<(&mkperth_core::Activity, &Scheduled, i64)>, EngineError> {
        let schedule = self.schedule.as_ref().map_err(|e| EngineError::Schedule(e.clone()))?;
        let p = &self.project;
        Ok(p.activities
            .iter()
            .zip(&schedule.activities)
            .map(|(a, s)| {
                let day = a.calendar.as_deref().and_then(|n| p.calendars.iter().find(|c| c.name == n)).unwrap_or(&p.calendar).day_minutes();
                (a, s, day)
            })
            .collect())
    }

    /// The schedule as JSON: the project's name and finish, and every activity's dates, floats
    /// (working days), status and whether it is critical.
    pub fn schedule_json(&self) -> Result<Value, EngineError> {
        let schedule = self.schedule.as_ref().map_err(|e| EngineError::Schedule(e.clone()))?;
        let activities: Vec<Value> = self
            .rows()?
            .into_iter()
            .map(|(a, s, day)| {
                json!({
                    "id": a.id,
                    "name": a.name,
                    "early_start": s.early_start.to_string(),
                    "early_finish": s.early_finish.to_string(),
                    "late_start": s.late_start.to_string(),
                    "late_finish": s.late_finish.to_string(),
                    "total_float": days(s.total_float_minutes, day),
                    "free_float": days(s.free_float_minutes, day),
                    "critical": s.critical,
                    "longest_path": s.longest_path,
                    "status": status_code(s.status),
                })
            })
            .collect();
        Ok(json!({ "name": self.project.name, "finish": schedule.finish.to_string(), "activities": activities }))
    }

    /// The schedule as CSV (RFC 4180): a header of fixed names, a row per activity.
    pub fn schedule_csv(&self) -> Result<String, EngineError> {
        let field = |s: &str| if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() };
        let mut out = String::from("id,name,early_start,early_finish,late_start,late_finish,total_float,free_float,critical,longest_path,status\r\n");
        for (a, s, day) in self.rows()? {
            out.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{}\r\n",
                field(&a.id),
                field(&a.name),
                s.early_start,
                s.early_finish,
                s.late_start,
                s.late_finish,
                days(s.total_float_minutes, day),
                days(s.free_float_minutes, day),
                s.critical,
                s.longest_path,
                status_code(s.status)
            ));
        }
        Ok(out)
    }

    /// The schedule as a table for reading, its headings and decimal mark in the language set,
    /// dates as `date` writes them; a star marks the critical activities.
    pub fn schedule_table(&self, date: &dyn Fn(mkperth_core::Date) -> String) -> Result<String, EngineError> {
        let schedule = self.schedule.as_ref().map_err(|e| EngineError::Schedule(e.clone()))?;
        let number = |x: f64| mkperth_i18n::decimal(&x.to_string());
        let mut rows: Vec<Vec<String>> = vec![
            ["col-id", "col-name", "col-early-start", "col-early-finish", "col-late-start", "col-late-finish", "col-total-float", "col-free-float"]
                .iter()
                .map(|k| tr!(k))
                .collect(),
        ];
        for (a, s, day) in self.rows()? {
            rows.push(vec![
                format!("{}{}", if s.critical { "*" } else { "" }, a.id),
                a.name.clone(),
                date(s.early_start),
                date(s.early_finish),
                date(s.late_start),
                date(s.late_finish),
                number(days(s.total_float_minutes, day)),
                number(days(s.free_float_minutes, day)),
            ]);
        }
        // Each column as wide as its widest cell, the name cut at 40 characters.
        let cell = |c: &String, k: usize| {
            if k == 1 && c.chars().count() > 40 { format!("{}…", c.chars().take(39).collect::<String>()) } else { c.clone() }
        };
        let widths: Vec<usize> = (0..8).map(|k| rows.iter().map(|r| r.get(k).map_or(0, |c| cell(c, k).chars().count())).max().unwrap_or(0)).collect();
        let mut out = String::new();
        for r in &rows {
            let line: Vec<String> =
                r.iter().enumerate().map(|(k, c)| format!("{:w$}", cell(c, k), w = widths.get(k).copied().unwrap_or(0))).collect();
            out.push_str(line.join("  ").trim_end());
            out.push('\n');
        }
        out.push_str(&tr!("cli-finish", date = date(schedule.finish).as_str(), n = schedule.activities.iter().filter(|a| a.critical).count()));
        out.push('\n');
        Ok(out)
    }
}
