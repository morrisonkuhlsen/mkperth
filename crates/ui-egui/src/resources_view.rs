//! The Resources view, from the rail: the project's resources in a table, the roles they fill,
//! and the selected resource's details and assignments. Every change is a `resource.*`,
//! `role.*` or `assignment.*` command, undoable as any other.

use mkperth_engine::core::{Date, Project, ResourceKind, UNIT};
use mkperth_engine::i18n::tr;
use mkperth_engine::sched::{Schedule, by_week, capacity, usage};
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
    /// The resource Details' Resources tab is about to assign.
    pub resource: String,
    /// Fields being typed for the selected resource or its assignments: (key, text).
    typing: Vec<(String, String)>,
}

/// Details' Resources tab: the assignments on `activity` (who, or which role, and the work
/// budgeted, done and left, typed in place), each removed, and a resource assigned from a list.
/// The commands to run.
pub(crate) fn activity_assignments(
    ui: &mut egui::Ui,
    t: &Tokens,
    project: &Project,
    activity: &str,
    form: &mut ResourceForm,
) -> Vec<(&'static str, Value)> {
    let mut pending = Vec::new();
    if project.activities.iter().any(|a| a.parent.as_deref() == Some(activity)) {
        ui.label(egui::RichText::new(tr!("details-resources-summary")).size(12.0).color(t.text_dim));
        return pending;
    }
    let on: Vec<_> = project.assignments.iter().filter(|a| a.activity == activity).collect();
    if on.is_empty() {
        ui.label(egui::RichText::new(tr!("details-resources-none")).size(12.0).color(t.text_dim));
    } else {
        egui::Grid::new("activity-assignments").striped(true).num_columns(5).spacing(egui::vec2(8.0, 6.0)).show(ui, |ui| {
            for head in ["assignment-who", "assignment-planned", "assignment-actual", "assignment-remaining"] {
                ui.label(egui::RichText::new(tr!(head)).size(11.5).color(t.header_text).strong());
            }
            ui.label("");
            ui.end_row();
            for a in on {
                let resource = a.resource.as_deref().and_then(|id| project.resources.iter().find(|r| r.id == id));
                let who = match (resource, &a.role) {
                    (Some(r), _) => {
                        let unit = if r.kind == ResourceKind::Material { r.unit.clone() } else { "h".to_string() };
                        format!("{} ({unit})", r.name)
                    }
                    (None, Some(role)) => {
                        let name = project.roles.iter().find(|r| r.id == *role).map_or(role.as_str(), |r| r.name.as_str());
                        tr!("assignment-role-only", role = name)
                    }
                    (None, None) => String::new(),
                };
                let l = ui.label(egui::RichText::new(who).size(12.0)).id;
                let target = match &a.resource {
                    Some(r) => json!({"activity": activity, "resource": r}),
                    None => json!({"activity": activity, "role": a.role}),
                };
                let key = |what: &str| format!("{activity}:{}:{what}", a.resource.clone().or(a.role.clone()).unwrap_or_default());
                for (what, value) in [("planned", a.planned), ("actual", a.actual), ("remaining", a.remaining_units())] {
                    if let Some(v) = field(ui, &mut form.typing, &key(what), units_text(value), l, 52.0) {
                        let mut params = target.clone();
                        if let Some(map) = params.as_object_mut() {
                            map.insert(what.into(), read_units(&v).map_or(json!("unreadable"), |u| json!(u)));
                        }
                        pending.push(("assignment.edit", params));
                    }
                }
                if crate::icon_button(ui, Icon::Close, &tr!("assignment-delete", activity = activity)) {
                    pending.push(("assignment.delete", target.clone()));
                }
                ui.end_row();
            }
        });
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let l = ui.label(egui::RichText::new(tr!("details-resources-add")).size(12.0).color(t.text_dim)).id;
        let shown = project.resources.iter().find(|r| r.id == form.resource).map_or_else(|| tr!("details-resources-pick"), |r| r.name.clone());
        let combo = egui::ComboBox::from_id_salt("details-assign").selected_text(shown).width(170.0).show_ui(ui, |ui| {
            for r in &project.resources {
                ui.selectable_value(&mut form.resource, r.id.clone(), format!("{} · {}", r.id, r.name));
            }
        });
        combo.response.labelled_by(l);
        if ui.add_enabled(!form.resource.is_empty(), egui::Button::new(tr!("assignment-add"))).clicked() {
            pending.push(("assignment.add", json!({"activity": activity, "resource": form.resource})));
            form.resource.clear();
        }
    });
    if project.resources.is_empty() {
        ui.label(egui::RichText::new(tr!("details-resources-go")).size(11.5).color(t.text_dim));
    }
    pending
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

/// The resource's work by week as bars, red where it is more than it can do; what it can do as a
/// dashed line over each week (not for a material, which has no hours). A week's figures show
/// under the pointer.
fn histogram(ui: &mut egui::Ui, t: &Tokens, project: &Project, schedule: &Schedule, resource: &str, with_capacity: bool) {
    let used = by_week(&usage(project, schedule, resource));
    let can = if with_capacity { by_week(&capacity(project, resource, project.start, schedule.finish)) } else { Vec::new() };
    let mut weeks: std::collections::BTreeMap<Date, (i64, Option<i64>)> = std::collections::BTreeMap::new();
    for (w, u) in &can {
        weeks.entry(*w).or_insert((0, None)).1 = Some(*u);
    }
    for (w, u) in &used {
        weeks.entry(*w).or_insert((0, None)).0 = *u;
    }
    if used.is_empty() {
        ui.label(egui::RichText::new(tr!("resource-usage-none")).size(12.0).color(t.text_dim));
        return;
    }
    let top = weeks.values().map(|(u, c)| (*u).max(c.unwrap_or(0))).max().unwrap_or(UNIT).max(UNIT) as f32 * 1.1;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 150.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let gutter = 44.0;
    let plot = egui::Rect::from_min_max(egui::pos2(rect.left() + gutter, rect.top() + 6.0), egui::pos2(rect.right() - 6.0, rect.bottom() - 20.0));
    painter.hline(plot.x_range(), plot.bottom(), egui::Stroke::new(1.0, t.border));
    painter.text(
        egui::pos2(plot.left() - 6.0, plot.top()),
        egui::Align2::RIGHT_TOP,
        units_text(top as i64),
        egui::FontId::proportional(11.0),
        t.text_dim,
    );
    painter.text(egui::pos2(plot.left() - 6.0, plot.bottom()), egui::Align2::RIGHT_BOTTOM, "0", egui::FontId::proportional(11.0), t.text_dim);
    let n = weeks.len().max(1) as f32;
    let slot = plot.width() / n;
    let y_of = |u: i64| plot.bottom() - plot.height() * (u as f32 / top);
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let mut last_month = 0;
    for (k, (week, (u, c))) in weeks.iter().enumerate() {
        let x = plot.left() + slot * k as f32;
        let bar = egui::Rect::from_min_max(egui::pos2(x + slot * 0.15, y_of(*u)), egui::pos2(x + slot * 0.85, plot.bottom()));
        let over = c.is_some_and(|c| *u > c);
        painter.rect_filled(bar, 2.0, if over { t.red } else { crate::splash::BRAND });
        if let Some(c) = c {
            painter.add(egui::Shape::dashed_line(
                &[egui::pos2(x + 1.0, y_of(*c)), egui::pos2(x + slot - 1.0, y_of(*c))],
                egui::Stroke::new(1.2, t.text_dim),
                3.0,
                2.0,
            ));
        }
        // A month's name under its first week, when there is room for it.
        let (_, month, _) = week.ymd();
        if month != last_month && slot * 4.0 > 30.0 {
            last_month = month;
            painter.text(
                egui::pos2(x, plot.bottom() + 4.0),
                egui::Align2::LEFT_TOP,
                tr!(&format!("month-{month}")),
                egui::FontId::proportional(11.0),
                t.text_dim,
            );
        }
        let column = egui::Rect::from_min_max(egui::pos2(x, plot.top()), egui::pos2(x + slot, plot.bottom()));
        if pointer.is_some_and(|p| column.contains(p)) {
            painter.rect_filled(column, 0.0, t.hover.gamma_multiply(0.5));
            let line = match c {
                Some(c) => tr!("resource-usage-week-of", week = crate::show_date(*week), used = units_text(*u), capacity = units_text(*c)),
                None => tr!("resource-usage-week", week = crate::show_date(*week), used = units_text(*u)),
            };
            egui::Tooltip::always_open(ui.ctx().clone(), ui.layer_id(), egui::Id::unique("usage-tip"), egui::PopupAnchor::Pointer).show(|ui| {
                ui.label(line);
            });
        }
    }
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
        let top = (height * 0.4).max(200.0);
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
            // Details, assignments and the histogram scroll when the window is short.
            egui::ScrollArea::vertical().id_salt("resource-detail").auto_shrink(false).show(ui, |ui| {
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
                // Its work week by week against what it can do.
                if let Ok(schedule) = &self.session.schedule {
                    ui.add_space(12.0);
                    ui.separator();
                    let unit = if r.kind == ResourceKind::Material { r.unit.clone() } else { tr!("resource-hours") };
                    ui.label(egui::RichText::new(tr!("resource-usage", unit = unit.as_str())).size(13.5).strong());
                    histogram(ui, &t, p, schedule, &id, r.kind != ResourceKind::Material);
                }
            });
        });
        for (command, params) in pending {
            self.run(command, params);
        }
    }
}
