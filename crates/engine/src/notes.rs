//! The chart's notes: bands, markers and marked months. They never move a task; each change is
//! a command with undo, the lists kept in date order.

use mkperth_core::{Band, Date, Marker, MonthMark};
use mkperth_i18n::tr;
use serde_json::Value;

use crate::{EngineError, Session};

/// The most of each kind a project may hold.
const MAX_NOTES: usize = 10_000;
/// The longest a note's name may be, in characters.
const MAX_NAME: usize = 200;

fn bad(command: &str, param: &str) -> EngineError {
    EngineError::Value(command.into(), param.into())
}

fn name(params: &Value, command: &str, required: bool) -> Result<String, EngineError> {
    match params.get("name") {
        None | Some(Value::Null) if !required => Ok(String::new()),
        Some(Value::String(s)) if s.chars().count() <= MAX_NAME && (!required || !s.trim().is_empty()) => {
            Ok(s.split_whitespace().collect::<Vec<_>>().join(" "))
        }
        None | Some(Value::Null) => Err(EngineError::Param(command.into(), "name".into())),
        Some(_) => Err(bad(command, "name")),
    }
}

fn date(params: &Value, command: &str, key: &str) -> Result<Date, EngineError> {
    match params.get(key) {
        None | Some(Value::Null) => Err(EngineError::Param(command.into(), key.into())),
        Some(v) => v.as_str().and_then(Date::parse).ok_or_else(|| bad(command, key)),
    }
}

/// A colour: none, or `#` and six hex digits.
fn color(params: &Value, command: &str) -> Result<String, EngineError> {
    match params.get("color") {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(s)) if s.is_empty() => Ok(String::new()),
        Some(Value::String(s)) if s.len() == 7 && s.starts_with('#') && s.chars().skip(1).all(|c| c.is_ascii_hexdigit()) => Ok(s.clone()),
        Some(_) => Err(bad(command, "color")),
    }
}

fn index(params: &Value, command: &str, len: usize) -> Result<usize, EngineError> {
    match params.get("index") {
        None | Some(Value::Null) => Err(EngineError::Param(command.into(), "index".into())),
        Some(v) => v.as_u64().and_then(|n| usize::try_from(n).ok()).filter(|n| *n < len).ok_or_else(|| bad(command, "index")),
    }
}

impl Session {
    pub(crate) fn note_command(&mut self, id: &str, params: &Value) -> Option<Result<(), EngineError>> {
        Some(match id {
            "band.add" => self.add_band(params),
            "marker.add" => self.add_marker(params),
            "month-mark.add" => self.add_month_mark(params),
            "band.delete" | "marker.delete" | "month-mark.delete" => self.delete_note(id, params),
            _ => return None,
        })
    }

    /// `band.add {name, from, to, color?}`: a stretch shaded behind the bars; ends given the
    /// wrong way round are turned.
    fn add_band(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "band.add";
        let (name, color) = (name(params, C, true)?, color(params, C)?);
        let (a, b) = (date(params, C, "from")?, date(params, C, "to")?);
        let (from, to) = (a.min(b), a.max(b));
        self.change(tr!("undo-add-band", name = name.as_str()), |p| {
            if p.bands.len() >= MAX_NOTES {
                return Err(bad(C, "name"));
            }
            p.bands.push(Band { name, from, to, color });
            p.bands.sort_by_key(|x| (x.from, x.to, x.name.to_lowercase()));
            Ok(())
        })
    }

    /// `marker.add {name, date, color?, label_at?}`: a named day drawn as a line; its name
    /// `label_at` percent down the chart.
    fn add_marker(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "marker.add";
        let (name, color, day) = (name(params, C, true)?, color(params, C)?, date(params, C, "date")?);
        let label_at = match params.get("label_at") {
            None | Some(Value::Null) => 0,
            Some(v) => v.as_u64().filter(|n| *n <= 100).and_then(|n| u8::try_from(n).ok()).ok_or_else(|| bad(C, "label_at"))?,
        };
        self.change(tr!("undo-add-marker", name = name.as_str()), |p| {
            if p.markers.len() >= MAX_NOTES {
                return Err(bad(C, "name"));
            }
            p.markers.push(Marker { name, date: day, color, label_at });
            p.markers.sort_by_key(|x| (x.date, x.name.to_lowercase()));
            Ok(())
        })
    }

    /// `month-mark.add {month, name?, color?}`: the month of `month` painted in the ruler; one
    /// mark per month, the new one in place of the old.
    fn add_month_mark(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "month-mark.add";
        let (name, color, day) = (name(params, C, false)?, color(params, C)?, date(params, C, "month")?);
        let (y, m, _) = day.ymd();
        let month = Date::from_ymd(y, m, 1).ok_or_else(|| bad(C, "month"))?;
        self.change(tr!("undo-add-month-mark"), |p| {
            p.month_marks.retain(|x| x.month != month);
            if p.month_marks.len() >= MAX_NOTES {
                return Err(bad(C, "month"));
            }
            p.month_marks.push(MonthMark { month, name, color });
            p.month_marks.sort_by_key(|x| x.month);
            Ok(())
        })
    }

    /// `band.delete`, `marker.delete`, `month-mark.delete {index}`: by its place in the list.
    fn delete_note(&mut self, command: &str, params: &Value) -> Result<(), EngineError> {
        let len = match command {
            "band.delete" => self.project.bands.len(),
            "marker.delete" => self.project.markers.len(),
            _ => self.project.month_marks.len(),
        };
        let at = index(params, command, len)?;
        let kind = command.to_string();
        self.change(tr!("undo-delete-note"), move |p| {
            match kind.as_str() {
                "band.delete" => drop(p.bands.remove(at)),
                "marker.delete" => drop(p.markers.remove(at)),
                _ => drop(p.month_marks.remove(at)),
            }
            Ok(())
        })
    }
}
