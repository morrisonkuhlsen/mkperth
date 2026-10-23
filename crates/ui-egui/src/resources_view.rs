//! The Resources view, from the rail: the project's resources in a table, the roles they fill,
//! and the selected resource's details and assignments. Every change is a `resource.*`,
//! `role.*` or `assignment.*` command, undoable as any other.

use mkperth_engine::core::{ResourceKind, UNIT};
use mkperth_engine::i18n::tr;
use serde_json::{Value, json};

use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::{PerthApp, icon_button};

/// What the Resources view is typing: a new resource's name and kind, a new role's name, the
/// activity a new assignment goes to, and the selected resource's fields as typed.
#[derive(Debug, Clone, Default)]
pub struct ResourceForm {
    pub name: String,
    pub kind: ResourceKind,
    pub role: String,
    pub activity: String,
    /// Fields being typed for the selected resource or its assignments: (key, text).
    typing: Vec<(String, String)>,
}

/// Hundredths of a unit as text: `40`, `12.5`.
pub fn units_text(u: i64) -> String {
    if u % UNIT == 0 { (u / UNIT).to_string() } else { format!("{:.2}", u as f64 / UNIT as f64).trim_end_matches('0').to_string() }
}

/// Units typed as `12.5` or `12,5`, as the number commands take.
fn read_units(text: &str) -> Option<f64> {
    text.trim().replace(',', ".").parse::<f64>().ok().filter(|u| u.is_finite() && *u >= 0.0)
}

fn kind_name(k: ResourceKind) -> String {
    match k {
        ResourceKind::Labor => tr!("resource-labor"),
        ResourceKind::Nonlabor => tr!("resource-nonlabor"),
        ResourceKind::Material => tr!("resource-material"),
    }
}

/// A one-line field that keeps what is typed under `key` until it loses focus, then returns it.
fn field(ui: &mut egui::Ui, typing: &mut Vec<(String, String)>, key: &str, shown: String, label: egui::Id, width: f32) -> Option<String> {
    let at = typing.iter().position(|(k, _)| k == key);
    let mut text = at.and_then(|k| typing.get(k)).map_or(shown.clone(), |(_, t)| t.clone());
    let response = ui.add(egui::TextEdit::singleline(&mut text).desired_width(width)).labelled_by(label);
    if response.lost_focus() {
        if let Some(k) = at {
            typing.remove(k);
        }
        return (text != shown).then_some(text);
    }
    if response.changed() {
        match at.and_then(|k| typing.get_mut(k)) {
            Some((_, t)) => *t = text,
            None => typing.push((key.to_string(), text)),
        }
    }
    None
}

impl PerthApp {
    /// The view in the space the activities take in theirs.
    pub(crate) fn resources_view(&mut self, ui: &mut egui::Ui, height: f32) {
        let t = Tokens::get(ui.ctx());
        let mut pending: Vec<(&str, Value)> = Vec::new();
        let p = &self.session.project;
        if self.resource_selected.is_some_and(|i| i >= p.resources.len()) {
            self.resource_selected = None;
        }
        let selected = self.resource_selected;
        let form = &mut self.resource_form;
        let label = |ui: &mut egui::Ui, text: String| ui.label(egui::RichText::new(text).size(12.0).color(t.text_dim)).id;
        // The table of resources, then the roles.
        let top = (height * 0.55).max(220.0);
        theme::card(&t).show(ui, |ui| {
            ui.set_min_size(egui::vec2(ui.available_width(), top));
            theme::pane_header(ui, Icon::People, &tr!("resources-view"), &tr!("resources-view-tip"), |_| {});
            egui::ScrollArea::vertical().id_salt("resources-table").max_height(top - 110.0).auto_shrink([false, true]).show(ui, |ui| {
                egui::Grid::new("resources").striped(true).num_columns(9).spacing(egui::vec2(14.0, 6.0)).show(ui, |ui| {
                    for head in [
                        "resource-id",
                        "resource-name",
                        "resource-kind",
                        "resource-unit",
                        "resource-max",
                        "resource-calendar",
                        "resource-roles",
                        "resource-assigned",
                    ] {
                        ui.label(egui::RichText::new(tr!(head)).size(12.0).color(t.header_text).strong());
                    }
                    ui.label("");
                    ui.end_row();
                    for (k, r) in p.resources.iter().enumerate() {
                        if ui.selectable_label(selected == Some(k), &r.id).clicked() {
                            self.resource_selected = Some(k);
                        }
                        ui.label(&r.name);
                        ui.label(kind_name(r.kind));
                        ui.label(if r.kind == ResourceKind::Material { r.unit.clone() } else { tr!("resource-hours") });
                        ui.label(units_text(r.max_units));
                        ui.label(r.calendar.clone().unwrap_or_else(|| p.calendar.name.clone()));
                        ui.label(r.roles.join(", "));
                        let assigned: i64 = p.assignments.iter().filter(|a| a.resource.as_deref() == Some(r.id.as_str())).map(|a| a.planned).sum();
                        ui.label(units_text(assigned));
                        if icon_button(ui, Icon::Close, &tr!("resource-delete", id = r.id.as_str())) {
                            pending.push(("resource.delete", json!({"id": r.id})));
                        }
                        ui.end_row();
                    }
                });
                if p.resources.is_empty() {
                    ui.label(egui::RichText::new(tr!("resources-none")).size(12.0).color(t.text_dim));
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let name = label(ui, tr!("resource-new"));
                ui.add(egui::TextEdit::singleline(&mut form.name).desired_width(180.0).hint_text(tr!("resource-name"))).labelled_by(name);
                let kind = egui::ComboBox::from_id_salt("resource-new-kind").selected_text(kind_name(form.kind)).width(120.0).show_ui(ui, |ui| {
                    for k in [ResourceKind::Labor, ResourceKind::Nonlabor, ResourceKind::Material] {
                        ui.selectable_value(&mut form.kind, k, kind_name(k));
                    }
                });
                kind.response.labelled_by(name);
                if ui.add_enabled(!form.name.trim().is_empty(), egui::Button::new(tr!("resource-add"))).clicked() {
                    pending.push(("resource.add", json!({"name": form.name.trim(), "kind": form.kind.code()})));
                    form.name.clear();
                }
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                let roles = label(ui, tr!("roles"));
                for r in &p.roles {
                    ui.label(egui::RichText::new(format!("{} · {}", r.id, r.name)).size(12.0));
                    if icon_button(ui, Icon::Close, &tr!("role-delete", id = r.id.as_str())) {
                        pending.push(("role.delete", json!({"id": r.id})));
                    }
                }
                ui.add(egui::TextEdit::singleline(&mut form.role).desired_width(140.0).hint_text(tr!("role-new"))).labelled_by(roles);
                if ui.add_enabled(!form.role.trim().is_empty(), egui::Button::new(tr!("role-add"))).clicked() {
                    pending.push(("role.add", json!({"name": form.role.trim()})));
                    form.role.clear();
                }
            });
        });
        ui.add_space(6.0);
        // The selected resource: its details beside its assignments.
        theme::card(&t).show(ui, |ui| {
            ui.set_min_size(egui::vec2(ui.available_width(), ui.available_height()));
            let Some(r) = selected.and_then(|k| p.resources.get(k)) else {
                ui.label(egui::RichText::new(tr!("resource-pick")).color(t.text_dim));
                return;
            };
            let id = r.id.clone();
            ui.columns(2, |cols| {
                let [details, work] = cols else { return };
                details.label(egui::RichText::new(format!("{} · {}", r.id, r.name)).size(13.5).strong());
                details.add_space(6.0);
                egui::Grid::new("resource-details").num_columns(2).spacing(egui::vec2(10.0, 6.0)).show(details, |ui| {
                    let l = label(ui, tr!("resource-name"));
                    if let Some(v) = field(ui, &mut form.typing, "name", r.name.clone(), l, 200.0).filter(|v| !v.trim().is_empty()) {
                        pending.push(("resource.edit", json!({"id": id, "name": v})));
                    }
                    ui.end_row();
                    let l = label(ui, tr!("resource-kind"));
                    let mut kind = r.kind;
                    let combo = egui::ComboBox::from_id_salt("resource-kind").selected_text(kind_name(kind)).width(140.0).show_ui(ui, |ui| {
                        for k in [ResourceKind::Labor, ResourceKind::Nonlabor, ResourceKind::Material] {
                            ui.selectable_value(&mut kind, k, kind_name(k));
                        }
                    });
                    combo.response.labelled_by(l);
                    if kind != r.kind {
                        pending.push(("resource.edit", json!({"id": id, "kind": kind.code()})));
                    }
                    ui.end_row();
                    if r.kind == ResourceKind::Material {
                        let l = label(ui, tr!("resource-unit"));
                        if let Some(v) = field(ui, &mut form.typing, "unit", r.unit.clone(), l, 80.0) {
                            pending.push(("resource.edit", json!({"id": id, "unit": v})));
                        }
                        ui.end_row();
                    }
                    let l = label(ui, tr!("resource-max"));
                    if let Some(v) = field(ui, &mut form.typing, "max", units_text(r.max_units), l, 80.0) {
                        match read_units(&v) {
                            Some(u) => pending.push(("resource.edit", json!({"id": id, "max_units": u}))),
                            None => pending.push(("resource.edit", json!({"id": id, "max_units": "unreadable"}))),
                        }
                    }
                    ui.end_row();
                    let l = label(ui, tr!("resource-calendar"));
                    let names: Vec<String> = std::iter::once(p.calendar.name.clone()).chain(p.calendars.iter().map(|c| c.name.clone())).collect();
                    let current = r.calendar.clone().unwrap_or_else(|| p.calendar.name.clone());
                    let mut chosen = current.clone();
                    let combo = egui::ComboBox::from_id_salt("resource-calendar").selected_text(current.clone()).width(160.0).show_ui(ui, |ui| {
                        for n in &names {
                            ui.selectable_value(&mut chosen, n.clone(), n);
                        }
                    });
                    combo.response.labelled_by(l);
                    if chosen != current {
                        let value = if chosen == p.calendar.name { Value::Null } else { json!(chosen) };
                        pending.push(("resource.edit", json!({"id": id, "calendar": value})));
                    }
                    ui.end_row();
                });
                if !p.roles.is_empty() {
                    details.add_space(6.0);
                    details.label(egui::RichText::new(tr!("resource-fills")).size(12.0).color(t.text_dim));
                    details.horizontal_wrapped(|ui| {
                        for role in &p.roles {
                            let mut on = r.roles.contains(&role.id);
                            if ui.checkbox(&mut on, &role.name).clicked() {
                                let mut roles = r.roles.clone();
                                if on {
                                    roles.push(role.id.clone());
                                } else {
                                    roles.retain(|x| *x != role.id);
                                }
                                pending.push(("resource.edit", json!({"id": id, "roles": roles})));
                            }
                        }
                    });
                }

                // Its assignments: the work budgeted, done and left on each activity.
                let unit = if r.kind == ResourceKind::Material { r.unit.clone() } else { tr!("resource-hours") };
                work.label(egui::RichText::new(tr!("resource-assignments", unit = unit.as_str())).size(13.5).strong());
                work.add_space(6.0);
                egui::Grid::new("resource-assignments").striped(true).num_columns(6).spacing(egui::vec2(12.0, 6.0)).show(work, |ui| {
                    for head in ["assignment-activity", "assignment-role", "assignment-planned", "assignment-actual", "assignment-remaining"] {
                        ui.label(egui::RichText::new(tr!(head)).size(12.0).color(t.header_text).strong());
                    }
                    ui.label("");
                    ui.end_row();
                    for a in p.assignments.iter().filter(|a| a.resource.as_deref() == Some(id.as_str())) {
                        let name = p.activities.iter().find(|x| x.id == a.activity).map_or(String::new(), |x| x.name.clone());
                        let l = ui.label(format!("{} · {name}", a.activity)).id;
                        ui.label(a.role.clone().unwrap_or_default());
                        let key = |what: &str| format!("{}:{what}", a.activity);
                        for (what, value) in [("planned", a.planned), ("actual", a.actual), ("remaining", a.remaining_units())] {
                            if let Some(v) = field(ui, &mut form.typing, &key(what), units_text(value), l, 60.0) {
                                let units = read_units(&v).map_or(json!("unreadable"), |u| json!(u));
                                pending.push(("assignment.edit", json!({"activity": a.activity, "resource": id, what: units})));
                            }
                        }
                        if icon_button(ui, Icon::Close, &tr!("assignment-delete", activity = a.activity.as_str())) {
                            pending.push(("assignment.delete", json!({"activity": a.activity, "resource": id})));
                        }
                        ui.end_row();
                    }
                });
                work.add_space(6.0);
                work.horizontal(|ui| {
                    let l = label(ui, tr!("assignment-new"));
                    let summary = |x: &str| p.activities.iter().any(|a| a.parent.as_deref() == Some(x));
                    let shown = if form.activity.is_empty() { tr!("assignment-pick") } else { form.activity.clone() };
                    let combo =
                        egui::ComboBox::from_id_salt("assignment-activity").selected_text(shown).width(220.0).height(300.0).show_ui(ui, |ui| {
                            for a in p.activities.iter().filter(|a| !summary(&a.id)) {
                                ui.selectable_value(&mut form.activity, a.id.clone(), format!("{} · {}", a.id, a.name));
                            }
                        });
                    combo.response.labelled_by(l);
                    if ui.add_enabled(!form.activity.is_empty(), egui::Button::new(tr!("assignment-add"))).clicked() {
                        pending.push(("assignment.add", json!({"activity": form.activity, "resource": id})));
                        form.activity.clear();
                    }
                });
            });
        });
        for (command, params) in pending {
            self.run(command, params);
        }
    }
}
