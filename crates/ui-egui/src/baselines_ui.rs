//! The Baselines pane: the schedule frozen under a name (`baseline.assign`), the baseline it is
//! compared with (`baseline.use`), and baselines deleted (`baseline.delete`), each undoable.

use mkperth_engine::i18n::tr;
use serde_json::{Value, json};

use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::{PerthApp, icon_button};

impl PerthApp {
    pub(crate) fn baselines_pane(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        if theme::pane_header(ui, Icon::Calendar, &tr!("pane-baselines"), &tr!("pane-baselines-tip"), |ui| {
            icon_button(ui, Icon::ChevronRight, &tr!("pane-close"))
        }) {
            self.side = None;
        }
        let p = &self.session.project;
        let mut pending: Option<(&str, Value)> = None;
        let name = &mut self.baseline_name;
        egui::ScrollArea::vertical().id_salt("baselines-pane").auto_shrink(false).show(ui, |ui| {
            let heading = |ui: &mut egui::Ui, text: String| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(text).size(12.5).color(t.text).strong());
            };
            heading(ui, tr!("baseline-new"));
            ui.horizontal(|ui| {
                let label = ui.label(egui::RichText::new(tr!("baseline-name")).size(12.0).color(t.text_dim));
                ui.add(egui::TextEdit::singleline(name).desired_width(170.0).hint_text(tr!("baseline-numbered", n = p.baselines.len() + 1)))
                    .labelled_by(label.id);
            });
            if ui.button(tr!("baseline-assign")).on_hover_text(tr!("baseline-assign-tip")).clicked() {
                let given = name.trim();
                pending = Some(("baseline.assign", if given.is_empty() { json!({}) } else { json!({ "name": given }) }));
                name.clear();
            }

            heading(ui, tr!("baseline-list"));
            if ui.radio(p.baseline().is_none(), tr!("baseline-none")).clicked() && p.baseline_in_use.is_some() {
                pending = Some(("baseline.use", json!({ "name": null })));
            }
            for b in &p.baselines {
                ui.horizontal(|ui| {
                    let on = p.baseline_in_use.as_deref() == Some(b.name.as_str());
                    if ui.radio(on, b.name.as_str()).clicked() && !on {
                        pending = Some(("baseline.use", json!({ "name": b.name })));
                    }
                    ui.label(egui::RichText::new(tr!("baseline-count", n = b.activities.len())).size(11.5).color(t.text_dim));
                    if icon_button(ui, Icon::Close, &tr!("baseline-delete", name = b.name.as_str())) {
                        pending = Some(("baseline.delete", json!({ "name": b.name })));
                    }
                });
            }
            ui.add_space(10.0);
            ui.label(egui::RichText::new(tr!("baseline-help")).size(11.5).color(t.text_dim));
        });
        if let Some((command, params)) = pending {
            self.run(command, params);
        }
    }
}
