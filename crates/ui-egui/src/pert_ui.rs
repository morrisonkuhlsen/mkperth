//! PERT in the window: an activity's three estimates in Details, and the PERT pane, the analysis
//! beside the schedule (it never moves a date): the finish planned and expected, its spread,
//! the chance of finishing by a day, the days it finishes by with a chance, and the expected
//! path with each activity's estimates.

use mkperth_engine::core::{Activity, Date, Pert, Project};
use mkperth_engine::i18n::tr;
use mkperth_engine::sched::PertAnalysis;
use serde_json::{Value, json};

use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::{PerthApp, date_field, days_text, icon_button, show_date};

/// Hundredths of a day as the language writes days: `4,5`, `9`.
fn days(h: i64) -> String {
    days_text(h, 100)
}

/// The chances whose finishes the pane lists, with their z.
const CHANCES: [(u8, f64); 4] = [(50, 0.0), (80, 0.841_621), (90, 1.281_552), (95, 1.644_854)];

/// An activity's three estimates, typed in Details: what to set (`pert`'s value for
/// `activity.edit`) once a field is left with all three read, or all three emptied.
pub(crate) fn pert_fields(ui: &mut egui::Ui, a: &Activity) -> Option<Value> {
    let t = Tokens::get(ui.ctx());
    let mut out = None;
    theme::row(ui, &tr!("detail-pert"), |ui, _| {
        // Three short fields sharing the row, each after its letter.
        let gap = ui.spacing().item_spacing.x;
        let width = ((ui.available_width() - 3.0 * (12.0 + 2.0 * gap)) / 3.0 - 2.0 * ui.spacing().button_padding.x).floor().max(28.0);
        let now = a.pert.map(|p| [p.optimistic, p.most_likely, p.pessimistic]);
        let mut texts: [String; 3] = std::array::from_fn(|k| now.and_then(|n| n.get(k).copied()).map(days).unwrap_or_default());
        let mut left = false;
        for (k, (short, tip)) in
            [("O", tr!("pert-optimistic")), ("M", tr!("pert-most-likely")), ("P", tr!("pert-pessimistic"))].into_iter().enumerate()
        {
            let key = egui::Id::unique("pert-text").with((a.id.as_str(), k));
            let shown = texts.get(k).cloned().unwrap_or_default();
            // The text typed, while it is the same activity and estimate underneath.
            if let Some((was, typed)) = ui.data(|d| d.get_temp::<(String, String)>(key))
                && was == shown
                && let Some(slot) = texts.get_mut(k)
            {
                *slot = typed;
            }
            let name = ui.label(egui::RichText::new(short).size(11.5).color(t.text_dim)).on_hover_text(format!("{tip} · {}", tr!("detail-pert-tip")));
            let Some(text) = texts.get_mut(k) else { continue };
            let response = ui.add(egui::TextEdit::singleline(text).desired_width(width)).labelled_by(name.id);
            // Kept while the three are being typed, until they are read.
            left |= response.lost_focus();
            if response.changed() || response.has_focus() || response.lost_focus() {
                let typed = text.clone();
                ui.data_mut(|d| d.insert_temp(key, (shown, typed)));
            }
        }
        if left {
            let read: Vec<Option<f64>> =
                texts.iter().map(|x| x.trim().replace(',', ".").parse::<f64>().ok().filter(|d| d.is_finite() && *d >= 0.0)).collect();
            let empty = texts.iter().all(|x| x.trim().is_empty());
            if empty || read.iter().all(Option::is_some) {
                for k in 0..3usize {
                    ui.data_mut(|d| d.remove::<(String, String)>(egui::Id::unique("pert-text").with((a.id.as_str(), k))));
                }
            }
            if empty {
                out = a.pert.is_some().then_some(Value::Null);
            } else if let [Some(o), Some(m), Some(p)] = read.as_slice() {
                let hundredths = |d: f64| (d * 100.0).round() as i64;
                let new = Pert { optimistic: hundredths(*o), most_likely: hundredths(*m), pessimistic: hundredths(*p) };
                if Some(new) != a.pert {
                    out = Some(json!({"optimistic": o, "most_likely": m, "pessimistic": p}));
                }
            }
        }
    });
    if let Some(p) = a.pert {
        ui.horizontal(|ui| {
            ui.add_space(124.0);
            let text = tr!(
                "detail-pert-expected",
                te = format!("{:.1}", p.expected() / 100.0).as_str(),
                sigma = format!("{:.1}", p.sigma() / 100.0).as_str()
            );
            ui.label(egui::RichText::new(text).size(11.5).color(t.text_dim));
        });
    }
    ui.add_space(3.0);
    out
}

impl PerthApp {
    /// The analysis for the project as it is, worked out again only when the project changed.
    fn pert_analysis(&mut self) -> Option<Result<PertAnalysis, String>> {
        let p = &self.session.project;
        if !p.activities.iter().any(|a| a.pert.is_some()) {
            return None;
        }
        if self.pert_cache.as_ref().is_none_or(|(was, _)| was != p) {
            let result = mkperth_engine::sched::pert(p).map_err(|e| e.to_string());
            self.pert_cache = Some((p.clone(), result));
        }
        self.pert_cache.as_ref().map(|(_, r)| r.clone())
    }

    pub(crate) fn pert_pane(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        if theme::pane_header(ui, Icon::Pert, &tr!("pane-pert"), &tr!("pane-pert-tip"), |ui| icon_button(ui, Icon::ChevronRight, &tr!("pane-close")))
        {
            self.side = None;
        }
        let analysis = self.pert_analysis();
        let project: &Project = &self.session.project;
        let mut target = self.pert_target;
        let mut pick = None;
        egui::ScrollArea::vertical().id_salt("pert-pane").auto_shrink(false).show(ui, |ui| {
            let heading = |ui: &mut egui::Ui, text: String| theme::section(ui, &text);
            let fact = |ui: &mut egui::Ui, name: String, value: String| theme::fact(ui, &name, &value, t.text);
            theme::hint(ui, &tr!("pert-only-analysis"));
            let a = match analysis {
                None => {
                    ui.add_space(6.0);
                    theme::hint(ui, &tr!("pert-none"));
                    return;
                }
                Some(Err(e)) => {
                    ui.label(egui::RichText::new(e).size(12.0).color(t.red));
                    return;
                }
                Some(Ok(a)) => a,
            };
            heading(ui, tr!("pert-finish"));
            fact(ui, tr!("pert-planned-finish"), show_date(a.planned_finish));
            fact(ui, tr!("pert-expected-finish"), show_date(a.expected_finish));
            fact(ui, tr!("pert-expected-duration"), tr!("pert-days-sigma", t = a.expected_days, sigma = format!("{:.1}", a.sigma).as_str()));

            heading(ui, tr!("pert-chance-by"));
            theme::row(ui, &tr!("pert-target"), |ui, label| {
                if let Some(d) = date_field(ui, egui::Id::unique("pert-target"), label, Some(target.unwrap_or(a.planned_finish)), 110.0) {
                    target = d;
                }
            });
            let day: Date = target.unwrap_or(a.planned_finish);
            let chance = a.chance(project, day) * 100.0;
            ui.label(
                egui::RichText::new(tr!("pert-chance", date = show_date(day).as_str(), percent = format!("{chance:.0}").as_str()))
                    .size(13.0)
                    .color(t.text),
            );

            heading(ui, tr!("pert-finishes-by"));
            for (percent, z) in CHANCES {
                fact(ui, format!("{percent}%"), show_date(a.finish_at(project, z)));
            }

            heading(ui, tr!("pert-path"));
            egui::Grid::new("pert-path").striped(false).spacing(egui::vec2(10.0, 4.0)).show(ui, |ui| {
                for head in ["col-id", "pert-o", "pert-m", "pert-p", "pert-te", "pert-sigma"] {
                    ui.label(egui::RichText::new(tr!(head)).size(11.5).color(t.header_text).strong());
                }
                ui.end_row();
                for &i in &a.path {
                    let Some(act) = project.activities.get(i) else { continue };
                    if ui.link(egui::RichText::new(act.id.as_str()).size(12.0)).clicked() {
                        pick = Some(i);
                    }
                    match (act.pert, a.activities.iter().find(|x| x.index == i)) {
                        (Some(p), Some(x)) => {
                            for text in [
                                days(p.optimistic),
                                days(p.most_likely),
                                days(p.pessimistic),
                                format!("{:.1}", x.expected),
                                format!("{:.1}", x.sigma),
                            ] {
                                ui.label(egui::RichText::new(text).size(12.0));
                            }
                        }
                        _ => {
                            ui.label(egui::RichText::new(tr!("pert-no-estimates")).size(11.5).color(t.text_dim));
                        }
                    }
                    ui.end_row();
                }
            });
        });
        self.pert_target = target;
        if let Some(i) = pick {
            self.selected = Some(i);
            self.marked = std::iter::once(i).collect();
        }
    }
}
