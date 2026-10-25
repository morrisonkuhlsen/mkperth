//! The Levelling pane: which resources are over their limit and on which days, how they are
//! levelled (within float or not, which resources, who keeps a resource first), and the
//! activities the last levelling held. Levelling and clearing it are `resource.level` and
//! `resource.unlevel`, undoable as any other change.

use mkperth_engine::core::{LevelBy, LevellingOptions, ResourceKind};
use mkperth_engine::i18n::tr;
use serde_json::json;

use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::{PerthApp, Status, icon_button, show_date};

/// The options as the pane edits them until Level is pressed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelForm {
    within_float: bool,
    /// The resources levelled, by ID; every one when empty.
    resources: Vec<String>,
    /// Each way of choosing who keeps a resource, in order, and whether it counts.
    order: Vec<(LevelBy, bool)>,
}

impl LevelForm {
    /// The form for the project's options: the ways it uses first, in its order, then the others.
    fn of(o: &LevellingOptions) -> LevelForm {
        let mut order: Vec<(LevelBy, bool)> = o.order.iter().map(|b| (*b, true)).collect();
        order.extend(LevelBy::ALL.into_iter().filter(|b| !o.order.contains(b)).map(|b| (b, false)));
        LevelForm { within_float: o.within_float, resources: o.resources.clone(), order }
    }
}

impl PerthApp {
    /// After levelling, said in the status bar: how many wait, the finish now, and what is left
    /// over.
    pub(crate) fn level_status(&mut self) {
        if !matches!(self.status, Some(Status::Info(_))) {
            return;
        }
        let held = self.session.project.activities.iter().filter(|a| a.levelled.is_some()).count();
        if let Ok(s) = &self.session.schedule {
            let left = mkperth_engine::sched::overloads(&self.session.project, s).len();
            let mut text = tr!("level-done", n = held, finish = show_date(s.finish).as_str());
            if left > 0 {
                text = format!("{text} {}", tr!("level-left", n = left));
            }
            self.status = Some(if left > 0 { Status::Warning(text) } else { Status::Info(text) });
        }
    }

    pub(crate) fn levelling_pane(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        if theme::pane_header(ui, Icon::Level, &tr!("pane-levelling"), &tr!("pane-levelling-tip"), |ui| {
            icon_button(ui, Icon::ChevronRight, &tr!("pane-close"))
        }) {
            self.side = None;
        }
        let p = &self.session.project;
        // A form begun on other options (an undo, another file) starts again from the project's.
        if self.level_form.as_ref().is_none_or(|(base, _)| *base != p.levelling) {
            self.level_form = Some((p.levelling.clone(), LevelForm::of(&p.levelling)));
        }
        let Some((_, form)) = self.level_form.as_mut() else { return };
        let over = match &self.session.schedule {
            Ok(s) => mkperth_engine::sched::overloads(p, s),
            Err(_) => Vec::new(),
        };
        let levellable: Vec<(String, String)> =
            p.resources.iter().filter(|r| r.kind != ResourceKind::Material).map(|r| (r.id.clone(), r.name.clone())).collect();
        let (mut level, mut clear, mut pick) = (false, false, None);
        egui::ScrollArea::vertical().id_salt("levelling-pane").auto_shrink(false).show(ui, |ui| {
            let heading = |ui: &mut egui::Ui, text: String| theme::section(ui, &text);
            let dim = |ui: &mut egui::Ui, text: String| theme::hint(ui, &text);
            // What is over its limit now, resource by resource.
            heading(ui, tr!("level-over"));
            if over.is_empty() {
                dim(ui, tr!("level-none-over"));
            }
            for r in &p.resources {
                let days: Vec<_> = over.iter().filter(|o| o.resource == r.id).collect();
                if let Some(first) = days.first() {
                    let text = tr!("level-over-line", name = r.name.as_str(), n = days.len(), from = show_date(first.day).as_str());
                    ui.label(egui::RichText::new(text).size(12.0).color(t.text));
                }
            }

            heading(ui, tr!("level-options"));
            ui.checkbox(&mut form.within_float, tr!("level-within-float")).on_hover_text(tr!("level-within-float-tip"));

            heading(ui, tr!("level-resources"));
            let mut all = form.resources.is_empty();
            if ui.checkbox(&mut all, tr!("level-all-resources")).changed() {
                form.resources = if all { Vec::new() } else { levellable.iter().map(|(id, _)| id.clone()).collect() };
            }
            if !all {
                for (id, name) in &levellable {
                    let mut on = form.resources.contains(id);
                    // The last one chosen stays: none at all would be every one.
                    let last = on && form.resources.len() == 1;
                    if ui.add_enabled(!last, egui::Checkbox::new(&mut on, format!("{id} — {name}"))).changed() {
                        if on {
                            form.resources.push(id.clone());
                        } else {
                            form.resources.retain(|r| r != id);
                        }
                    }
                }
            }

            heading(ui, tr!("level-order"));
            dim(ui, tr!("level-order-tip"));
            let mut swap = None;
            let n = form.order.len();
            for k in 0..n {
                ui.horizontal(|ui| {
                    let Some((by, on)) = form.order.get_mut(k) else { return };
                    let name = tr!(&format!("level-by-{}", by.code()));
                    ui.checkbox(on, name.clone());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if k + 1 < n && icon_button(ui, Icon::ChevronDown, &tr!("level-later", name = name.as_str())) {
                            swap = Some(k);
                        }
                        if k > 0 && icon_button(ui, Icon::ChevronUp, &tr!("level-sooner", name = name.as_str())) {
                            swap = Some(k - 1);
                        }
                    });
                });
            }
            if let Some(k) = swap
                && k + 1 < form.order.len()
            {
                form.order.swap(k, k + 1);
            }

            ui.add_space(10.0);
            let held: Vec<(usize, String)> = p
                .activities
                .iter()
                .enumerate()
                .filter_map(|(i, a)| {
                    a.levelled
                        .as_ref()
                        .map(|l| (i, tr!("level-held-line", id = a.id.as_str(), date = show_date(l.start).as_str(), resource = l.resource.as_str())))
                })
                .collect();
            ui.horizontal(|ui| {
                level = ui.button(tr!("level-run")).on_hover_text(tr!("level-run-tip")).clicked();
                clear = ui.add_enabled(!held.is_empty(), egui::Button::new(tr!("level-clear"))).on_hover_text(tr!("level-clear-tip")).clicked();
            });

            // What the last levelling held, each a way to its activity.
            heading(ui, tr!("level-held"));
            if held.is_empty() {
                dim(ui, tr!("detail-none"));
            }
            for (i, text) in held {
                if ui.link(egui::RichText::new(text).size(12.0)).clicked() {
                    pick = Some(i);
                }
            }
        });
        if level {
            let order: Vec<&str> = form.order.iter().filter(|(_, on)| *on).map(|(b, _)| b.code()).collect();
            let params = json!({"within_float": form.within_float, "resources": form.resources, "order": order});
            self.run("resource.level", params);
            self.level_status();
        }
        if clear {
            self.run("resource.unlevel", serde_json::Value::Null);
        }
        if let Some(i) = pick {
            self.selected = Some(i);
            self.marked = std::iter::once(i).collect();
        }
    }
}
