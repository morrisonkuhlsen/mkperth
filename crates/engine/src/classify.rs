//! Classifying activities: activity codes with their values in a hierarchy, user-defined fields
//! of a kind each, and an activity's value of each. Each change is a command with undo.

use std::collections::HashSet;

use mkperth_core::{ActivityCode, CodeValue, Date, EpsNode, FieldKind, FieldValue, Indicator, ObsNode, UserField};
use mkperth_i18n::tr;
use serde_json::{Map, Value};

use crate::costs::{MAX_MONEY, MAX_TEXT, bad, required, text};
use crate::{EngineError, Session};

/// The most codes or fields a project may have, and values a code may have.
const MAX_KINDS: usize = 1000;
const MAX_VALUES: usize = 100_000;

/// A field's value from JSON, by the field's kind: text, a number, a whole number, money, a
/// date (`2026-11-03`) or an indicator (`red`, `yellow`, `green`, `blue`).
fn field_value(kind: FieldKind, v: &Value) -> Option<FieldValue> {
    let number = || v.as_f64().filter(|x| x.is_finite());
    Some(match kind {
        FieldKind::Text => FieldValue::Text(v.as_str().filter(|t| t.chars().count() <= MAX_TEXT)?.to_string()),
        FieldKind::Number => FieldValue::Number((number().filter(|x| x.abs() <= 1e12)? * 1e6).round() as i64),
        FieldKind::Integer => FieldValue::Integer(v.as_i64()?),
        FieldKind::Cost => FieldValue::Cost((number().filter(|x| x.abs() <= MAX_MONEY)? * 100.0).round() as i64),
        FieldKind::Date => FieldValue::Date(v.as_str().and_then(Date::parse)?),
        FieldKind::Indicator => FieldValue::Indicator(v.as_str().and_then(Indicator::from_code)?),
    })
}

/// A map of names to values (`{"Phase": "DES", "Area": null}`), if given.
fn map<'a>(params: &'a Value, command: &str, name: &str) -> Result<Option<&'a Map<String, Value>>, EngineError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(m)) if m.len() <= MAX_KINDS => Ok(Some(m)),
        Some(_) => Err(bad(command, name)),
    }
}

impl Session {
    /// The classification commands; `None` when `id` isn't one of them.
    pub(crate) fn classify_command(&mut self, id: &str, params: &Value) -> Option<Result<(), EngineError>> {
        Some(match id {
            "code.add" => self.add_code(params),
            "code.delete" => self.delete_code(params),
            "code-value.add" => self.add_code_value(params),
            "code-value.delete" => self.delete_code_value(params),
            "field.add" => self.add_field(params),
            "field.delete" => self.delete_field(params),
            "activity.classify" => self.classify(params),
            "obs.add" => self.add_obs(params),
            "obs.delete" => self.delete_obs(params),
            "project.organise" => self.organise(params),
            _ => return None,
        })
    }

    /// An activity code, by its `name` (unique).
    fn add_code(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "code.add";
        let name = required(params, C, "name")?;
        self.change(tr!("undo-add-code", name = name.as_str()), |p| {
            let c = &mut p.classification;
            if c.codes.len() >= MAX_KINDS || c.code(&name).is_some() {
                return Err(bad(C, "name"));
            }
            c.codes.push(ActivityCode { name: name.clone(), values: Vec::new() });
            Ok(())
        })
    }

    /// Deletes the code `name`, every activity's value of it, and levelling's order by it.
    fn delete_code(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "code.delete";
        let name = required(params, C, "name")?;
        self.change(tr!("undo-delete-code", name = name.as_str()), |p| {
            let before = p.classification.codes.len();
            p.classification.codes.retain(|c| c.name != name);
            if p.classification.codes.len() == before {
                return Err(bad(C, "name"));
            }
            for a in &mut p.activities {
                a.codes.remove(&name);
            }
            p.levelling.order.retain(|b| *b != mkperth_core::LevelBy::Code(name.clone()));
            Ok(())
        })
    }

    /// A value of the code `code`: its `value` (unique in the code), `description`, and the value
    /// it sits under (`parent`).
    fn add_code_value(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "code-value.add";
        let code = required(params, C, "code")?;
        let value = required(params, C, "value")?;
        let description = text(params, C, "description")?.unwrap_or_default();
        let parent = text(params, C, "parent")?.filter(|s| !s.is_empty());
        self.change(tr!("undo-add-code-value", value = value.as_str(), code = code.as_str()), |p| {
            let c = p.classification.codes.iter_mut().find(|c| c.name == code).ok_or_else(|| bad(C, "code"))?;
            if c.values.len() >= MAX_VALUES || c.values.iter().any(|v| v.code == value) {
                return Err(bad(C, "value"));
            }
            if let Some(parent) = &parent
                && !c.values.iter().any(|v| v.code == *parent)
            {
                return Err(bad(C, "parent"));
            }
            c.values.push(CodeValue { code: value.clone(), description, parent });
            Ok(())
        })
    }

    /// Deletes the value `value` of the code `code`: the values under it go under its own
    /// parent, and the activities that had it have none.
    fn delete_code_value(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "code-value.delete";
        let code = required(params, C, "code")?;
        let value = required(params, C, "value")?;
        self.change(tr!("undo-delete-code-value", value = value.as_str(), code = code.as_str()), |p| {
            let c = p.classification.codes.iter_mut().find(|c| c.name == code).ok_or_else(|| bad(C, "code"))?;
            let k = c.values.iter().position(|v| v.code == value).ok_or_else(|| bad(C, "value"))?;
            let gone = c.values.remove(k);
            for v in c.values.iter_mut().filter(|v| v.parent.as_deref() == Some(value.as_str())) {
                v.parent = gone.parent.clone();
            }
            for a in &mut p.activities {
                if a.codes.get(&code) == Some(&value) {
                    a.codes.remove(&code);
                }
            }
            Ok(())
        })
    }

    /// A user-defined field: its `name` (unique) and `kind` (text, number, integer, cost, date,
    /// indicator).
    fn add_field(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "field.add";
        let name = required(params, C, "name")?;
        let kind = FieldKind::from_code(&required(params, C, "kind")?).ok_or_else(|| bad(C, "kind"))?;
        self.change(tr!("undo-add-field", name = name.as_str()), |p| {
            let c = &mut p.classification;
            if c.fields.len() >= MAX_KINDS || c.field(&name).is_some() {
                return Err(bad(C, "name"));
            }
            c.fields.push(UserField { name: name.clone(), kind });
            Ok(())
        })
    }

    /// Deletes the field `name`, and every activity's value of it.
    fn delete_field(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "field.delete";
        let name = required(params, C, "name")?;
        self.change(tr!("undo-delete-field", name = name.as_str()), |p| {
            let before = p.classification.fields.len();
            p.classification.fields.retain(|f| f.name != name);
            if p.classification.fields.len() == before {
                return Err(bad(C, "name"));
            }
            for a in &mut p.activities {
                a.fields.remove(&name);
            }
            Ok(())
        })
    }

    /// Classifies the activity `id` (or each of `activities`): `codes` gives each code's value
    /// (`null` for none), `fields` each field's (`null` for none), by the field's kind.
    fn classify(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "activity.classify";
        let ids: Vec<String> = match (params.get("activities"), text(params, C, "id")?) {
            (Some(Value::Array(items)), _) if !items.is_empty() && items.len() <= 100_000 => {
                items.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().ok_or_else(|| bad(C, "activities"))?
            }
            (None, Some(id)) => vec![id],
            _ => return Err(EngineError::Param(C.into(), "id".into())),
        };
        let codes = map(params, C, "codes")?.cloned();
        let fields = map(params, C, "fields")?.cloned();
        // Who answers for it: an OBS node's name, or `null` for no one.
        let responsible = match params.get("responsible") {
            None => None,
            Some(Value::Null) => Some(None),
            Some(_) => Some(Some(required(params, C, "responsible")?)),
        };
        if codes.is_none() && fields.is_none() && responsible.is_none() {
            return Err(EngineError::Param(C.into(), "codes".into()));
        }
        let label = match ids.as_slice() {
            [one] => tr!("undo-classify", id = one.as_str()),
            many => tr!("undo-classify-many", n = many.len()),
        };
        self.change(label, |p| {
            // Each value checked against its code or field before any activity changes.
            let mut set_codes: Vec<(String, Option<String>)> = Vec::new();
            for (name, v) in codes.iter().flatten() {
                let code = p.classification.code(name).ok_or_else(|| bad(C, "codes"))?;
                let value = match v {
                    Value::Null => None,
                    Value::String(s) if code.values.iter().any(|x| x.code == *s) => Some(s.clone()),
                    _ => return Err(bad(C, "codes")),
                };
                set_codes.push((name.clone(), value));
            }
            let mut set_fields: Vec<(String, Option<FieldValue>)> = Vec::new();
            for (name, v) in fields.iter().flatten() {
                let field = p.classification.field(name).ok_or_else(|| bad(C, "fields"))?;
                let value = match v {
                    Value::Null => None,
                    v => Some(field_value(field.kind, v).ok_or_else(|| bad(C, "fields"))?),
                };
                set_fields.push((name.clone(), value));
            }
            if let Some(Some(r)) = &responsible
                && !p.organisation.obs.iter().any(|o| o.name == *r)
            {
                return Err(bad(C, "responsible"));
            }
            let wanted: HashSet<&str> = ids.iter().map(String::as_str).collect();
            let mut found = 0usize;
            for a in p.activities.iter_mut().filter(|a| wanted.contains(a.id.as_str())) {
                found += 1;
                for (name, value) in &set_codes {
                    match value {
                        Some(v) => a.codes.insert(name.clone(), v.clone()),
                        None => a.codes.remove(name),
                    };
                }
                for (name, value) in &set_fields {
                    match value {
                        Some(v) => a.fields.insert(name.clone(), v.clone()),
                        None => a.fields.remove(name),
                    };
                }
                if let Some(r) = &responsible {
                    a.responsible = r.clone();
                }
            }
            if found != wanted.len() {
                let missing = ids.iter().find(|id| !p.activities.iter().any(|a| a.id == **id)).cloned().unwrap_or_default();
                return Err(EngineError::UnknownActivity(missing));
            }
            Ok(())
        })
    }

    /// A node of the OBS: its `name` (unique), `description`, and the node it sits under.
    fn add_obs(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "obs.add";
        let name = required(params, C, "name")?;
        let description = text(params, C, "description")?.unwrap_or_default();
        let parent = text(params, C, "parent")?.filter(|s| !s.is_empty());
        self.change(tr!("undo-add-obs", name = name.as_str()), |p| {
            let obs = &mut p.organisation.obs;
            if obs.len() >= MAX_VALUES || obs.iter().any(|o| o.name == name) {
                return Err(bad(C, "name"));
            }
            if let Some(parent) = &parent
                && !obs.iter().any(|o| o.name == *parent)
            {
                return Err(bad(C, "parent"));
            }
            obs.push(ObsNode { name: name.clone(), description, parent });
            Ok(())
        })
    }

    /// Deletes the OBS node `name`: those under it go under its own parent; the project and the
    /// activities it answered for have no one.
    fn delete_obs(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "obs.delete";
        let name = required(params, C, "name")?;
        self.change(tr!("undo-delete-obs", name = name.as_str()), |p| {
            let o = &mut p.organisation;
            let k = o.obs.iter().position(|x| x.name == name).ok_or_else(|| bad(C, "name"))?;
            let gone = o.obs.remove(k);
            for x in o.obs.iter_mut().filter(|x| x.parent.as_deref() == Some(name.as_str())) {
                x.parent = gone.parent.clone();
            }
            if o.responsible.as_deref() == Some(name.as_str()) {
                o.responsible = None;
            }
            for a in p.activities.iter_mut().filter(|a| a.responsible.as_deref() == Some(name.as_str())) {
                a.responsible = None;
            }
            Ok(())
        })
    }

    /// Where the project sits and who answers for it: `eps`, its EPS from the top down
    /// (`[{"id": "MK", "name": "Morrison Kühlsen"}, …]`, empty for none), and `responsible`, an
    /// OBS node's name (`null` for no one).
    fn organise(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "project.organise";
        let eps = match params.get("eps") {
            None => None,
            Some(Value::Array(items)) if items.len() <= 100 => Some(
                items
                    .iter()
                    .map(|i| {
                        let field =
                            |k: &str| i.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty() && s.chars().count() <= MAX_TEXT);
                        Some(EpsNode { id: field("id")?.to_string(), name: field("name").unwrap_or_default().to_string() })
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| bad(C, "eps"))?,
            ),
            Some(_) => return Err(bad(C, "eps")),
        };
        let responsible = match params.get("responsible") {
            None => None,
            Some(Value::Null) => Some(None),
            Some(_) => Some(Some(required(params, C, "responsible")?)),
        };
        if eps.is_none() && responsible.is_none() {
            return Err(EngineError::Param(C.into(), "eps".into()));
        }
        self.change(tr!("undo-organise"), |p| {
            if let Some(Some(r)) = &responsible
                && !p.organisation.obs.iter().any(|o| o.name == *r)
            {
                return Err(bad(C, "responsible"));
            }
            if let Some(e) = eps {
                p.organisation.eps = e;
            }
            if let Some(r) = responsible {
                p.organisation.responsible = r;
            }
            Ok(())
        })
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

    /// A Phase code with a value under another, a Risk indicator and a Cost field; activities
    /// classified one by one and together; a value deleted, its child moved up and the
    /// activities that had it left with none; refused inputs leave the project as it was.
    #[test]
    fn codes_fields_and_activities() {
        let mut s = Session::default();
        run(&mut s, "code.add", json!({"name": "Phase"}));
        run(&mut s, "code-value.add", json!({"code": "Phase", "value": "DES", "description": "Design"}));
        run(&mut s, "code-value.add", json!({"code": "Phase", "value": "DES.ARC", "description": "Architecture", "parent": "DES"}));
        run(&mut s, "code-value.add", json!({"code": "Phase", "value": "BLD", "description": "Build"}));
        run(&mut s, "field.add", json!({"name": "Risk", "kind": "indicator"}));
        run(&mut s, "field.add", json!({"name": "Allowance", "kind": "cost"}));
        run(&mut s, "field.add", json!({"name": "Inspected", "kind": "date"}));
        run(&mut s, "activity.classify", json!({"activities": ["A10", "A20"], "codes": {"Phase": "DES"}, "fields": {"Risk": "green"}}));
        run(
            &mut s,
            "activity.classify",
            json!({"id": "A20", "codes": {"Phase": "DES.ARC"}, "fields": {"Allowance": 1250.5, "Inspected": "2026-11-20"}}),
        );
        let a = |s: &Session, id: &str| s.project.activities.iter().find(|a| a.id == id).unwrap().clone();
        assert_eq!(a(&s, "A10").codes.get("Phase").map(String::as_str), Some("DES"));
        assert_eq!(a(&s, "A20").fields.get("Allowance"), Some(&FieldValue::Cost(125_050)));
        assert_eq!(a(&s, "A20").fields.get("Risk"), Some(&FieldValue::Indicator(Indicator::Green)));
        // A value deleted: its child moves up, the activities with it have none.
        run(&mut s, "code-value.delete", json!({"code": "Phase", "value": "DES"}));
        assert_eq!(s.project.classification.code("Phase").unwrap().values.iter().find(|v| v.code == "DES.ARC").unwrap().parent, None);
        assert!(a(&s, "A10").codes.is_empty());
        assert_eq!(a(&s, "A20").codes.get("Phase").map(String::as_str), Some("DES.ARC"));
        // Cleared with null; a field deleted takes its values.
        run(&mut s, "activity.classify", json!({"id": "A20", "fields": {"Risk": null}}));
        assert!(!a(&s, "A20").fields.contains_key("Risk"));
        run(&mut s, "field.delete", json!({"name": "Allowance"}));
        assert!(!a(&s, "A20").fields.contains_key("Allowance"));
        // Refused, untouched.
        let before = s.project.clone();
        for (id, params) in [
            ("activity.classify", json!({"id": "A30", "codes": {"Phase": "NOPE"}})),
            ("activity.classify", json!({"id": "A30", "fields": {"Risk": "purple"}})),
            ("activity.classify", json!({"id": "A30", "fields": {"Inspected": "soon"}})),
            ("activity.classify", json!({"activities": ["A30", "ZZZ"], "codes": {"Phase": "BLD"}})),
            ("activity.classify", json!({"id": "A30"})),
            ("code.add", json!({"name": "Phase"})),
            ("code-value.add", json!({"code": "Phase", "value": "X", "parent": "nope"})),
            ("field.add", json!({"name": "Odd", "kind": "colour"})),
        ] {
            assert!(s.run(id, &params).is_err(), "{id} {params}");
        }
        assert_eq!(s.project, before);
        // The OBS: a node under another, the project's responsible and an activity's; deleted,
        // its child goes up and no one answers where it did.
        run(&mut s, "obs.add", json!({"name": "Engineering"}));
        run(&mut s, "obs.add", json!({"name": "Structures", "parent": "Engineering"}));
        run(
            &mut s,
            "project.organise",
            json!({"eps": [{"id": "MK", "name": "Morrison Kühlsen"}, {"id": "MK.B", "name": "Buildings"}], "responsible": "Engineering"}),
        );
        run(&mut s, "activity.classify", json!({"id": "W2", "responsible": "Structures"}));
        assert_eq!(s.project.organisation.eps.len(), 2);
        assert_eq!(a(&s, "W2").responsible.as_deref(), Some("Structures"));
        assert!(s.run("activity.classify", &json!({"id": "W2", "responsible": "Nobody"})).is_err());
        run(&mut s, "obs.delete", json!({"name": "Engineering"}));
        assert_eq!((s.project.organisation.obs[0].parent.clone(), s.project.organisation.responsible.clone()), (None, None));
        run(&mut s, "obs.delete", json!({"name": "Structures"}));
        assert_eq!(a(&s, "W2").responsible, None);
        // A code deleted takes its values off the activities.
        run(&mut s, "code.delete", json!({"name": "Phase"}));
        assert!(s.project.activities.iter().all(|a| a.codes.is_empty()));
        run(&mut s, "edit.undo", json!(null));
        assert_eq!(a(&s, "A20").codes.get("Phase").map(String::as_str), Some("DES.ARC"));
    }
}
