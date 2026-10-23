//! Resources, roles and their assignments to activities: who and what does the work. Each change
//! is a command with undo; units come in as numbers (`7.5` hours) and are kept in hundredths.

use mkperth_core::{Assignment, Project, Resource, ResourceKind, Role, UNIT};
use mkperth_i18n::tr;
use serde_json::Value;

use crate::{EngineError, Session};

/// The most resources, roles or assignments a project may hold.
const MAX_ITEMS: usize = 100_000;
/// The longest an ID, a name or a unit may be, in characters.
const MAX_TEXT: usize = 200;
/// The most units a number may say (hours, or a material's unit).
const MAX_UNITS: f64 = 1e9;

fn bad(command: &str, param: &str) -> EngineError {
    EngineError::Value(command.into(), param.into())
}

/// A text parameter, if given: trimmed, not too long.
fn text(params: &Value, command: &str, name: &str) -> Result<Option<String>, EngineError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.chars().count() <= MAX_TEXT => Ok(Some(s.trim().to_string())),
        Some(_) => Err(bad(command, name)),
    }
}

fn required(params: &Value, command: &str, name: &str) -> Result<String, EngineError> {
    text(params, command, name)?.filter(|s| !s.is_empty()).ok_or_else(|| EngineError::Param(command.into(), name.into()))
}

/// A number of units (`7.5`), as hundredths, if given: finite, not below nothing.
fn units(params: &Value, command: &str, name: &str) -> Result<Option<i64>, EngineError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_f64()
            .filter(|u| u.is_finite() && (0.0..=MAX_UNITS).contains(u))
            .map(|u| Some((u * UNIT as f64).round() as i64))
            .ok_or_else(|| bad(command, name)),
    }
}

/// A list of IDs (`["ELEC", "CRANE"]`), if given.
fn ids(params: &Value, command: &str, name: &str) -> Result<Option<Vec<String>>, EngineError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(items)) if items.len() <= MAX_ITEMS => items
            .iter()
            .map(|i| i.as_str().filter(|s| !s.trim().is_empty() && s.chars().count() <= MAX_TEXT).map(|s| s.trim().to_string()))
            .collect::<Option<Vec<_>>>()
            .map(Some)
            .ok_or_else(|| bad(command, name)),
        Some(_) => Err(bad(command, name)),
    }
}

/// `prefix` and the first number after the ones taken: `R1`, `R2`…
fn next_id<'a>(prefix: &str, taken: impl Iterator<Item = &'a str>) -> String {
    let highest = taken.filter_map(|id| id.strip_prefix(prefix).and_then(|n| n.parse::<u64>().ok())).max().unwrap_or(0);
    format!("{prefix}{}", highest.saturating_add(1))
}

/// Which assignment `params` names: on `activity`, of `resource` (or, unstaffed, for `role`).
fn assignment_index(p: &Project, params: &Value, command: &str) -> Result<usize, EngineError> {
    let activity = required(params, command, "activity")?;
    let resource = text(params, command, "resource")?.filter(|s| !s.is_empty());
    let role = text(params, command, "role")?.filter(|s| !s.is_empty());
    if resource.is_none() && role.is_none() {
        return Err(EngineError::Param(command.into(), "resource".into()));
    }
    p.assignments
        .iter()
        .position(|a| a.activity == activity && if resource.is_some() { a.resource == resource } else { a.resource.is_none() && a.role == role })
        .ok_or_else(|| EngineError::UnknownAssignment(activity, resource.or(role).unwrap_or_default()))
}

impl Session {
    /// The resource commands; `None` when `id` isn't one of them.
    pub(crate) fn resource_command(&mut self, id: &str, params: &Value) -> Option<Result<(), EngineError>> {
        Some(match id {
            "resource.add" => self.add_resource(params),
            "resource.edit" => self.edit_resource(params),
            "resource.delete" => self.delete_resource(params),
            "role.add" => self.add_role(params),
            "role.delete" => self.delete_role(params),
            "assignment.add" => self.add_assignment(params),
            "assignment.edit" => self.edit_assignment(params),
            "assignment.delete" => self.delete_assignment(params),
            _ => return None,
        })
    }

    /// A resource: `name` (required), `id` (else `R1`, `R2`…), `kind` (labor, nonlabor,
    /// material), a material's `unit`, the `calendar` it works on and its `max_units` an hour
    /// (1 for one full time).
    fn add_resource(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "resource.add";
        let name = required(params, C, "name")?;
        let id =
            text(params, C, "id")?.filter(|s| !s.is_empty()).unwrap_or_else(|| next_id("R", self.project.resources.iter().map(|r| r.id.as_str())));
        let kind = text(params, C, "kind")?.map(|k| ResourceKind::from_code(&k).ok_or_else(|| bad(C, "kind"))).transpose()?.unwrap_or_default();
        let unit = text(params, C, "unit")?.unwrap_or_default();
        let calendar = text(params, C, "calendar")?.filter(|s| !s.is_empty());
        let max_units = units(params, C, "max_units")?.unwrap_or(UNIT);
        self.change(tr!("undo-add-resource", id = id.as_str()), |p| {
            if p.resources.len() >= MAX_ITEMS {
                return Err(bad(C, "id"));
            }
            if p.resources.iter().any(|r| r.id == id) {
                return Err(EngineError::ResourceIdTaken(id.clone()));
            }
            if let Some(c) = &calendar
                && *c != p.calendar.name
                && !p.calendars.iter().any(|x| x.name == *c)
            {
                return Err(bad(C, "calendar"));
            }
            p.resources.push(Resource { id: id.clone(), name, kind, unit, calendar, max_units, ..Resource::default() });
            Ok(())
        })
    }

    /// Changes a resource found by `id`: `new_id` (everywhere it is named), `name`, `kind`,
    /// `unit`, `calendar` (`null` for the project's), `max_units` and the `roles` it fills.
    fn edit_resource(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "resource.edit";
        let id = required(params, C, "id")?;
        let new_id = text(params, C, "new_id")?;
        let name = text(params, C, "name")?;
        let kind = text(params, C, "kind")?.map(|k| ResourceKind::from_code(&k).ok_or_else(|| bad(C, "kind"))).transpose()?;
        let unit = text(params, C, "unit")?;
        let calendar = match params.get("calendar") {
            None => None,
            Some(Value::Null) => Some(None),
            Some(_) => Some(text(params, C, "calendar")?.filter(|s| !s.is_empty())),
        };
        let max_units = units(params, C, "max_units")?;
        let roles = ids(params, C, "roles")?;
        self.change(tr!("undo-edit-resource", id = id.as_str()), |p| {
            let k = p.resources.iter().position(|r| r.id == id).ok_or_else(|| EngineError::UnknownResource(id.clone()))?;
            if let Some(Some(c)) = &calendar
                && *c != p.calendar.name
                && !p.calendars.iter().any(|x| x.name == *c)
            {
                return Err(bad(C, "calendar"));
            }
            if let Some(rs) = &roles
                && let Some(missing) = rs.iter().find(|r| !p.roles.iter().any(|x| x.id == **r))
            {
                return Err(EngineError::UnknownRole(missing.clone()));
            }
            if let Some(new) = new_id.as_deref().filter(|n| *n != id) {
                if new.is_empty() {
                    return Err(bad(C, "new_id"));
                }
                if p.resources.iter().any(|r| r.id == new) {
                    return Err(EngineError::ResourceIdTaken(new.to_string()));
                }
                for a in p.assignments.iter_mut().filter(|a| a.resource.as_deref() == Some(id.as_str())) {
                    a.resource = Some(new.to_string());
                }
            }
            let Some(r) = p.resources.get_mut(k) else { return Err(EngineError::UnknownResource(id.clone())) };
            if let Some(new) = new_id.filter(|n| !n.is_empty()) {
                r.id = new;
            }
            if let Some(n) = name.filter(|n| !n.is_empty()) {
                r.name = n;
            }
            if let Some(k) = kind {
                r.kind = k;
            }
            if let Some(u) = unit {
                r.unit = u;
            }
            if let Some(c) = calendar {
                r.calendar = c;
            }
            if let Some(m) = max_units {
                r.max_units = m;
            }
            if let Some(rs) = roles {
                r.roles = rs;
            }
            Ok(())
        })
    }

    /// Deletes the resource `id` and its assignments.
    fn delete_resource(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "resource.delete";
        let id = required(params, C, "id")?;
        self.change(tr!("undo-delete-resource", id = id.as_str()), |p| {
            let k = p.resources.iter().position(|r| r.id == id).ok_or_else(|| EngineError::UnknownResource(id.clone()))?;
            p.resources.remove(k);
            p.assignments.retain(|a| a.resource.as_deref() != Some(id.as_str()));
            Ok(())
        })
    }

    /// A role: `name` (required) and `id` (else `RL1`, `RL2`…).
    fn add_role(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "role.add";
        let name = required(params, C, "name")?;
        let id = text(params, C, "id")?.filter(|s| !s.is_empty()).unwrap_or_else(|| next_id("RL", self.project.roles.iter().map(|r| r.id.as_str())));
        self.change(tr!("undo-add-role", id = id.as_str()), |p| {
            if p.roles.len() >= MAX_ITEMS {
                return Err(bad(C, "id"));
            }
            if p.roles.iter().any(|r| r.id == id) {
                return Err(EngineError::RoleIdTaken(id.clone()));
            }
            p.roles.push(Role { id: id.clone(), name, ..Role::default() });
            Ok(())
        })
    }

    /// Deletes the role `id`: resources no longer fill it, assignments no longer name it, and an
    /// assignment that was only that role, not staffed, goes.
    fn delete_role(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "role.delete";
        let id = required(params, C, "id")?;
        self.change(tr!("undo-delete-role", id = id.as_str()), |p| {
            let k = p.roles.iter().position(|r| r.id == id).ok_or_else(|| EngineError::UnknownRole(id.clone()))?;
            p.roles.remove(k);
            for r in &mut p.resources {
                r.roles.retain(|x| *x != id);
            }
            p.assignments.retain(|a| a.resource.is_some() || a.role.as_deref() != Some(id.as_str()));
            for a in &mut p.assignments {
                if a.role.as_deref() == Some(id.as_str()) {
                    a.role = None;
                }
            }
            Ok(())
        })
    }

    /// Puts a `resource` (or a `role` still to staff, or both) to work on `activity`. `planned`
    /// units default to the activity's working hours at the resource's units an hour (none for a
    /// material); `actual` to none. One assignment per resource on an activity; summaries take
    /// none (their work is their children's).
    fn add_assignment(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "assignment.add";
        let activity = required(params, C, "activity")?;
        let resource = text(params, C, "resource")?.filter(|s| !s.is_empty());
        let role = text(params, C, "role")?.filter(|s| !s.is_empty());
        if resource.is_none() && role.is_none() {
            return Err(EngineError::Param(C.into(), "resource".into()));
        }
        let planned = units(params, C, "planned")?;
        let actual = units(params, C, "actual")?.unwrap_or(0);
        let who = resource.clone().or(role.clone()).unwrap_or_default();
        self.change(tr!("undo-add-assignment", resource = who.as_str(), activity = activity.as_str()), |p| {
            if p.assignments.len() >= MAX_ITEMS {
                return Err(bad(C, "activity"));
            }
            let a = p.activities.iter().find(|a| a.id == activity).ok_or_else(|| EngineError::UnknownActivity(activity.clone()))?;
            if p.activities.iter().any(|x| x.parent.as_deref() == Some(activity.as_str())) {
                return Err(bad(C, "activity"));
            }
            let r = match &resource {
                Some(id) => Some(p.resources.iter().find(|r| r.id == *id).ok_or_else(|| EngineError::UnknownResource(id.clone()))?),
                None => None,
            };
            if let Some(role) = &role
                && !p.roles.iter().any(|x| x.id == *role)
            {
                return Err(EngineError::UnknownRole(role.clone()));
            }
            let taken = p
                .assignments
                .iter()
                .any(|x| x.activity == activity && if resource.is_some() { x.resource == resource } else { x.resource.is_none() && x.role == role });
            if taken {
                return Err(EngineError::AssignmentExists(activity.clone(), who.clone()));
            }
            // The activity's working hours at the resource's units an hour (one, for a role).
            let planned = planned.unwrap_or_else(|| match r {
                Some(r) if r.kind == ResourceKind::Material => 0,
                _ => {
                    let minutes = a.working_minutes(p.calendar.day_minutes());
                    let per_hour = r.map_or(UNIT, |r| r.max_units);
                    minutes.saturating_mul(per_hour) / 60
                }
            });
            p.assignments.push(Assignment {
                activity: activity.clone(),
                resource: resource.clone(),
                role: role.clone(),
                planned,
                actual,
                remaining: None,
                ..Assignment::default()
            });
            Ok(())
        })
    }

    /// Changes the assignment named by `activity` and `resource` (or `role`): `planned`,
    /// `actual`, `remaining` (`null` for the budget less what is done), and `staff` (a resource
    /// to fill an unstaffed role).
    fn edit_assignment(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "assignment.edit";
        let planned = units(params, C, "planned")?;
        let actual = units(params, C, "actual")?;
        let remaining = match params.get("remaining") {
            None => None,
            Some(Value::Null) => Some(None),
            Some(_) => Some(units(params, C, "remaining")?),
        };
        let staff = text(params, C, "staff")?.filter(|s| !s.is_empty());
        let activity = text(params, C, "activity")?.unwrap_or_default();
        self.change(tr!("undo-edit-assignment", activity = activity.as_str()), |p| {
            let k = assignment_index(p, params, C)?;
            if let Some(s) = &staff {
                if !p.resources.iter().any(|r| r.id == *s) {
                    return Err(EngineError::UnknownResource(s.clone()));
                }
                if p.assignments.iter().any(|x| x.activity == activity && x.resource.as_deref() == Some(s.as_str())) {
                    return Err(EngineError::AssignmentExists(activity.clone(), s.clone()));
                }
            }
            let Some(a) = p.assignments.get_mut(k) else { return Err(bad(C, "activity")) };
            if let Some(u) = planned {
                a.planned = u;
            }
            if let Some(u) = actual {
                a.actual = u;
            }
            if let Some(u) = remaining {
                a.remaining = u;
            }
            if let Some(s) = staff {
                a.resource = Some(s);
            }
            Ok(())
        })
    }

    fn delete_assignment(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "assignment.delete";
        let activity = text(params, C, "activity")?.unwrap_or_default();
        self.change(tr!("undo-delete-assignment", activity = activity.as_str()), |p| {
            let k = assignment_index(p, params, C)?;
            p.assignments.remove(k);
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

    /// A10 in the sample takes 5 days of 8 hours: 40 hours for one full time, 80 for two.
    #[test]
    fn resources_roles_and_assignments() {
        let mut s = Session::default();
        run(&mut s, "role.add", json!({"id": "MASON", "name": "Mason"}));
        run(&mut s, "resource.add", json!({"name": "Ana"}));
        run(&mut s, "resource.add", json!({"name": "Crane", "kind": "nonlabor", "max_units": 2}));
        run(&mut s, "resource.add", json!({"id": "CONC", "name": "Concrete", "kind": "material", "unit": "m³"}));
        let ids: Vec<&str> = s.project.resources.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["R1", "R2", "CONC"]);
        run(&mut s, "resource.edit", json!({"id": "R1", "roles": ["MASON"]}));
        run(&mut s, "assignment.add", json!({"activity": "A10", "resource": "R1"}));
        run(&mut s, "assignment.add", json!({"activity": "A10", "resource": "R2"}));
        run(&mut s, "assignment.add", json!({"activity": "A10", "resource": "CONC", "planned": 12.5}));
        run(&mut s, "assignment.add", json!({"activity": "A20", "role": "MASON"}));
        let planned: Vec<i64> = s.project.assignments.iter().map(|a| a.planned).collect();
        assert_eq!(planned, [4000, 8000, 1250, 12000], "40 h, 80 h, 12.5 m³, and A20's 15 days at one an hour");
        // Work done, and what is left: the budget less it, or as told.
        run(&mut s, "assignment.edit", json!({"activity": "A10", "resource": "R1", "actual": 16}));
        assert_eq!(s.project.assignments[0].remaining_units(), 2400);
        run(&mut s, "assignment.edit", json!({"activity": "A10", "resource": "R1", "remaining": 30}));
        assert_eq!(s.project.assignments[0].remaining_units(), 3000);
        // The role staffed by a resource.
        run(&mut s, "assignment.edit", json!({"activity": "A20", "role": "MASON", "staff": "R1"}));
        assert_eq!(s.project.assignments[3].resource.as_deref(), Some("R1"));
        // Renamed activities and resources carry their assignments; deleted ones take them.
        run(&mut s, "activity.edit", json!({"id": "A10", "new_id": "A11"}));
        run(&mut s, "resource.edit", json!({"id": "R2", "new_id": "CRANE"}));
        assert_eq!(s.project.assignments[1].activity, "A11");
        assert_eq!(s.project.assignments[1].resource.as_deref(), Some("CRANE"));
        run(&mut s, "resource.delete", json!({"id": "CRANE"}));
        assert_eq!(s.project.assignments.len(), 3);
        run(&mut s, "activity.delete", json!({"id": "A11"}));
        assert_eq!(s.project.assignments.len(), 1, "only A20's is left");
        // Deleting a role: no resource fills it, no assignment names it.
        run(&mut s, "role.delete", json!({"id": "MASON"}));
        assert!(s.project.resources[0].roles.is_empty() && s.project.assignments[0].role.is_none());
        // Undo takes each change back.
        run(&mut s, "edit.undo", Value::Null);
        assert_eq!(s.project.roles.len(), 1);
    }

    #[test]
    fn what_is_refused_leaves_the_project_as_it_was() {
        let mut s = Session::default();
        run(&mut s, "resource.add", json!({"id": "R1", "name": "Ana"}));
        run(&mut s, "assignment.add", json!({"activity": "A10", "resource": "R1"}));
        let before = s.project.clone();
        for (id, params) in [
            ("resource.add", json!({})),
            ("resource.add", json!({"id": "R1", "name": "Twin"})),
            ("resource.add", json!({"name": "X", "kind": "robot"})),
            ("resource.add", json!({"name": "X", "max_units": -1})),
            ("resource.add", json!({"name": "X", "calendar": "Moon"})),
            ("resource.edit", json!({"id": "R9", "name": "X"})),
            ("resource.edit", json!({"id": "R1", "roles": ["NOPE"]})),
            ("assignment.add", json!({"activity": "A10", "resource": "R1"})),
            ("assignment.add", json!({"activity": "W1", "resource": "R1"})),
            ("assignment.add", json!({"activity": "A10"})),
            ("assignment.add", json!({"activity": "ZZ", "resource": "R1"})),
            ("assignment.add", json!({"activity": "A20", "resource": "R1", "planned": "lots"})),
            ("assignment.edit", json!({"activity": "A20", "resource": "R1", "actual": 1})),
            ("assignment.delete", json!({"activity": "A10", "role": "MASON"})),
            ("role.delete", json!({"id": "MASON"})),
        ] {
            assert!(s.run(id, &params).is_err(), "{id} {params}");
            assert_eq!(s.project, before, "{id} {params}");
        }
    }
}
