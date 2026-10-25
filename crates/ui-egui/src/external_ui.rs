//! Details' ties to other projects: the activity's relationships with other projects' activities,
//! each deleted; a new one made by choosing the other project's file and one of its activities.

use mkperth_engine::Session;
use mkperth_engine::core::{Activity, RelationshipType};
use mkperth_engine::i18n::tr;
use serde_json::{Value, json};

use crate::theme;

/// What the ties' form holds: the other project's file as written, its activities, the one
/// chosen, which way the tie runs, the lag.
#[derive(Debug, Clone, Default)]
pub struct ExternalForm {
    pub path: String,
    pub activities: Vec<(String, String)>,
    pub other: String,
    pub outgoing: bool,
    pub lag: i64,
    pub error: Option<String>,
}

/// The section, and the command it asks for.
pub(crate) fn section(
    ui: &mut egui::Ui,
    session: &Session,
    a: &Activity,
    form: &mut ExternalForm,
    pick: Option<&dyn Fn() -> Option<std::path::PathBuf>>,
) -> Option<(&'static str, Value)> {
    let mut out = None;
    theme::section(ui, &tr!("external-title"));
    let mine: Vec<(usize, &mkperth_engine::core::ExternalRelationship)> =
        session.project.external.iter().enumerate().filter(|(_, x)| x.here == a.id).collect();
    if mine.is_empty() {
        theme::hint(ui, &tr!("detail-none"));
    }
    for (k, x) in mine {
        let arrow = if x.incoming { tr!("external-from", other = x.there.as_str()) } else { tr!("external-to", other = x.there.as_str()) };
        let lag = if x.lag == 0 { String::new() } else { format!(" {:+}", x.lag) };
        if theme::list_item(ui, None, &arrow, &format!("{}{lag} · {}", x.kind.code(), x.project), &tr!("external-delete")) {
            out = Some(("relationship.delete-external", json!({"index": k})));
        }
    }
    theme::row(ui, &tr!("external-project"), |ui, label| {
        let shown = if form.path.is_empty() { tr!("external-choose") } else { form.path.clone() };
        let r = ui.add(egui::Button::new(egui::RichText::new(shown).size(12.5)).truncate()).labelled_by(label);
        if r.clicked()
            && let Some(path) = pick.and_then(|p| p())
        {
            form.path = session.written_path(&path);
            form.other.clear();
            match session.other_activities(&form.path) {
                Ok(list) => {
                    form.activities = list;
                    form.error = None;
                }
                Err(e) => {
                    form.activities.clear();
                    form.error = Some(e.to_string());
                }
            }
        }
    });
    if let Some(e) = &form.error {
        theme::hint(ui, e);
    }
    if !form.activities.is_empty() {
        theme::row(ui, &tr!("external-activity"), |ui, label| {
            let shown = if form.other.is_empty() { tr!("external-choose") } else { form.other.clone() };
            egui::ComboBox::from_id_salt("external-activity")
                .selected_text(shown)
                .width(theme::fill(ui))
                .truncate()
                .show_ui(ui, |ui| {
                    for (id, name) in &form.activities {
                        ui.selectable_value(&mut form.other, id.clone(), format!("{id} · {name}"));
                    }
                })
                .response
                .labelled_by(label);
        });
        theme::row(ui, &tr!("external-way"), |ui, label| {
            let name = |out: bool| if out { tr!("external-way-out") } else { tr!("external-way-in") };
            egui::ComboBox::from_id_salt("external-way")
                .selected_text(name(form.outgoing))
                .width(theme::fill(ui))
                .truncate()
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut form.outgoing, false, name(false));
                    ui.selectable_value(&mut form.outgoing, true, name(true));
                })
                .response
                .labelled_by(label);
        });
        theme::row(ui, &tr!("external-lag"), |ui, label| {
            ui.add(egui::DragValue::new(&mut form.lag).range(-999..=999)).labelled_by(label);
        });
        ui.horizontal(|ui| {
            theme::under_fields(ui);
            if ui.add_enabled(!form.other.is_empty(), egui::Button::new(tr!("external-add"))).clicked() {
                out = Some((
                    "relationship.add-external",
                    json!({"project": form.path, "activity": a.id, "other": form.other, "direction": if form.outgoing { "out" } else { "in" },
                        "type": RelationshipType::FinishToStart.code(), "lag": form.lag}),
                ));
                form.other.clear();
            }
        });
    }
    out
}
