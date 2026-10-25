//! The Codes and fields pane: the project's activity codes with their values, its user-defined
//! fields with their kinds; and Details' classification of an activity. Each change is a command.

use mkperth_engine::core::{Activity, Classification, FieldKind, FieldValue, Indicator};
use mkperth_engine::i18n::tr;
use serde_json::{Value, json};

use crate::costs_ui::read_signed_money;
use crate::icons::Icon;
use crate::theme;
use crate::{PerthApp, date_field, icon_button};

/// What the Codes and fields pane is typing.
#[derive(Debug, Clone, Default)]
pub struct CodesForm {
    pub code: String,
    /// The code whose values are shown, and the value being added to it.
    pub open: String,
    pub value: String,
    pub description: String,
    pub parent: String,
    pub field: String,
    pub kind: FieldKind,
}

fn kind_name(k: FieldKind) -> String {
    tr!(&format!("field-kind-{}", k.code()))
}

impl PerthApp {
    pub(crate) fn codes_pane(&mut self, ui: &mut egui::Ui) {
        if theme::pane_header(ui, Icon::Labels, &tr!("pane-codes"), &tr!("pane-codes-tip"), |ui| {
            icon_button(ui, Icon::ChevronRight, &tr!("pane-close"))
        }) {
            self.side = None;
        }
        let c = &self.session.project.classification;
        let form = &mut self.codes_form;
        if !form.open.is_empty() && c.code(&form.open).is_none() {
            form.open.clear();
        }
        if form.open.is_empty()
            && let Some(first) = c.codes.first()
        {
            form.open = first.name.clone();
        }
        let mut pending: Vec<(&str, Value)> = Vec::new();
        egui::ScrollArea::vertical().id_salt("codes-pane").auto_shrink(false).show(ui, |ui| {
            // The codes; the one open shows its values.
            theme::section(ui, &tr!("codes-codes"));
            if c.codes.is_empty() {
                theme::hint(ui, &tr!("codes-none"));
            }
            for code in &c.codes {
                let detail = tr!("codes-values-count", n = code.values.len());
                ui.horizontal(|ui| {
                    if ui.selectable_label(form.open == code.name, &code.name).clicked() {
                        form.open = code.name.clone();
                    }
                    ui.label(egui::RichText::new(detail).size(12.0).color(theme::Tokens::get(ui.ctx()).text_dim));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icon_button(ui, Icon::Close, &tr!("codes-delete", name = code.name.as_str())) {
                            pending.push(("code.delete", json!({"name": code.name})));
                        }
                    });
                });
            }
            theme::row(ui, &tr!("codes-new"), |ui, label| {
                ui.add(egui::TextEdit::singleline(&mut form.code).desired_width(theme::fill(ui) - 70.0).hint_text(tr!("codes-new-hint")))
                    .labelled_by(label);
                if ui.add_enabled(!form.code.trim().is_empty(), egui::Button::new(tr!("codes-add"))).clicked() {
                    pending.push(("code.add", json!({"name": form.code.trim()})));
                    form.open = form.code.trim().to_string();
                    form.code.clear();
                }
            });

            if let Some(code) = c.code(&form.open) {
                theme::section(ui, &tr!("codes-values-of", name = code.name.as_str()));
                if code.values.is_empty() {
                    theme::hint(ui, &tr!("detail-none"));
                }
                // In their hierarchy: each under its parent, indented.
                let depth = |v: &str| {
                    let (mut d, mut at) = (0usize, code.values.iter().find(|x| x.code == v).and_then(|x| x.parent.clone()));
                    while let Some(p) = at.filter(|_| d < code.values.len()) {
                        d += 1;
                        at = code.values.iter().find(|x| x.code == p).and_then(|x| x.parent.clone());
                    }
                    d
                };
                for v in &code.values {
                    ui.horizontal(|ui| {
                        ui.add_space(depth(&v.code) as f32 * 14.0);
                        ui.vertical(|ui| {
                            if theme::list_item(ui, None, &v.code, &v.description, &tr!("codes-value-delete", value = v.code.as_str())) {
                                pending.push(("code-value.delete", json!({"code": code.name, "value": v.code})));
                            }
                        });
                    });
                }
                theme::row(ui, &tr!("codes-value"), |ui, label| {
                    ui.add(egui::TextEdit::singleline(&mut form.value).desired_width(90.0)).labelled_by(label);
                });
                theme::row(ui, &tr!("codes-description"), |ui, label| {
                    ui.add(egui::TextEdit::singleline(&mut form.description).desired_width(theme::fill(ui))).labelled_by(label);
                });
                theme::row(ui, &tr!("cost-account-parent"), |ui, label| {
                    let shown = if form.parent.is_empty() { tr!("expense-no-account") } else { form.parent.clone() };
                    egui::ComboBox::from_id_salt("code-value-parent")
                        .selected_text(shown)
                        .width(theme::fill(ui))
                        .truncate()
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut form.parent, String::new(), tr!("expense-no-account"));
                            for v in &code.values {
                                ui.selectable_value(&mut form.parent, v.code.clone(), &v.code);
                            }
                        })
                        .response
                        .labelled_by(label);
                });
                ui.horizontal(|ui| {
                    theme::under_fields(ui);
                    if ui.add_enabled(!form.value.trim().is_empty(), egui::Button::new(tr!("codes-value-add"))).clicked() {
                        let parent = (!form.parent.is_empty()).then(|| form.parent.clone());
                        pending.push((
                            "code-value.add",
                            json!({"code": code.name, "value": form.value.trim(), "description": form.description.trim(), "parent": parent}),
                        ));
                        form.value.clear();
                        form.description.clear();
                    }
                });
            }

            // The user-defined fields.
            theme::section(ui, &tr!("codes-fields"));
            if c.fields.is_empty() {
                theme::hint(ui, &tr!("detail-none"));
            }
            for f in &c.fields {
                if theme::list_item(ui, None, &f.name, &kind_name(f.kind), &tr!("codes-field-delete", name = f.name.as_str())) {
                    pending.push(("field.delete", json!({"name": f.name})));
                }
            }
            theme::row(ui, &tr!("codes-field-name"), |ui, label| {
                ui.add(egui::TextEdit::singleline(&mut form.field).desired_width(theme::fill(ui))).labelled_by(label);
            });
            theme::row(ui, &tr!("codes-field-kind"), |ui, label| {
                egui::ComboBox::from_id_salt("field-kind")
                    .selected_text(kind_name(form.kind))
                    .width(theme::fill(ui))
                    .truncate()
                    .show_ui(ui, |ui| {
                        for k in FieldKind::ALL {
                            ui.selectable_value(&mut form.kind, k, kind_name(k));
                        }
                    })
                    .response
                    .labelled_by(label);
            });
            ui.horizontal(|ui| {
                theme::under_fields(ui);
                if ui.add_enabled(!form.field.trim().is_empty(), egui::Button::new(tr!("codes-field-add"))).clicked() {
                    pending.push(("field.add", json!({"name": form.field.trim(), "kind": form.kind.code()})));
                    form.field.clear();
                }
            });
        });
        for (command, params) in pending {
            self.run(command, params);
        }
    }
}

/// Details' classification: each code's value chosen, each field's typed (a date picked, an
/// indicator chosen). The `activity.classify` parameters to run.
pub(crate) fn classify_fields(ui: &mut egui::Ui, c: &Classification, a: &Activity, currency: &str) -> Option<Value> {
    if c.is_empty() {
        return None;
    }
    let mut out = None;
    theme::section(ui, &tr!("details-classification"));
    for code in &c.codes {
        theme::row(ui, &code.name, |ui, label| {
            let current = a.codes.get(&code.name).cloned();
            let mut chosen = current.clone();
            egui::ComboBox::from_id_salt(("classify-code", &code.name))
                .selected_text(current.clone().unwrap_or_else(|| "—".into()))
                .width(theme::fill(ui))
                .truncate()
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut chosen, None, "—");
                    for v in &code.values {
                        let text = if v.description.is_empty() { v.code.clone() } else { format!("{} — {}", v.code, v.description) };
                        ui.selectable_value(&mut chosen, Some(v.code.clone()), text);
                    }
                })
                .response
                .labelled_by(label);
            if chosen != current {
                out = Some(json!({"id": a.id, "codes": {code.name.clone(): chosen}}));
            }
        });
    }
    for f in &c.fields {
        theme::row(ui, &f.name, |ui, label| {
            let current = a.fields.get(&f.name);
            let set = |v: Value| Some(json!({"id": a.id, "fields": {f.name.clone(): v}}));
            match f.kind {
                FieldKind::Date => {
                    let day = match current {
                        Some(FieldValue::Date(d)) => Some(*d),
                        _ => None,
                    };
                    if let Some(d) = date_field(ui, egui::Id::unique("classify-date").with((&a.id, &f.name)), label, day, 110.0) {
                        out = set(d.map_or(Value::Null, |d| json!(d.to_string())));
                    }
                }
                FieldKind::Indicator => {
                    let now = match current {
                        Some(FieldValue::Indicator(i)) => Some(*i),
                        _ => None,
                    };
                    let mut chosen = now;
                    let name = |i: Option<Indicator>| i.map_or_else(|| "—".to_string(), |i| tr!(&format!("indicator-{}", i.code())));
                    egui::ComboBox::from_id_salt(("classify-indicator", &f.name))
                        .selected_text(name(now))
                        .width(theme::fill(ui))
                        .truncate()
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut chosen, None, "—");
                            for i in Indicator::ALL {
                                ui.selectable_value(&mut chosen, Some(i), name(Some(i)));
                            }
                        })
                        .response
                        .labelled_by(label);
                    if chosen != now {
                        out = set(chosen.map_or(Value::Null, |i| json!(i.code())));
                    }
                }
                kind => {
                    // Typed: kept while typing, read on leaving the field.
                    let key = egui::Id::unique("classify-text").with((&a.id, &f.name));
                    let shown =
                        current.map(|v| crate::activities::field_text(v, if kind == FieldKind::Cost { "" } else { currency })).unwrap_or_default();
                    let mut text =
                        ui.data(|d| d.get_temp::<(String, String)>(key)).filter(|(was, _)| *was == shown).map_or(shown.clone(), |(_, x)| x);
                    let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(theme::fill(ui))).labelled_by(label);
                    if r.lost_focus() {
                        ui.data_mut(|d| d.remove::<(String, String)>(key));
                        if text != shown {
                            let t = text.trim();
                            out = set(if t.is_empty() {
                                Value::Null
                            } else {
                                match kind {
                                    FieldKind::Text => json!(t),
                                    FieldKind::Integer => t.parse::<i64>().map_or(json!("unreadable"), |n| json!(n)),
                                    _ => read_signed_money(t).map_or(json!("unreadable"), |n| json!(n)),
                                }
                            });
                        }
                    } else if r.changed() {
                        let typed = text.clone();
                        ui.data_mut(|d| d.insert_temp(key, (shown, typed)));
                    }
                }
            }
        });
    }
    out
}
