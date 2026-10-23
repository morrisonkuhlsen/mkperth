//! Costs in the window: an activity's costs and expenses in Details, and the Costs pane (the
//! currency, the project's costs, its spending month by month and the cost accounts). Every
//! change is a command, undoable as any other.

use mkperth_engine::core::{Accrual, Date, Project};
use mkperth_engine::i18n::{self, Lang, tr};
use mkperth_engine::sched::{Cost, Schedule};
use serde_json::{Value, json};

use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::{PerthApp, icon_button};

/// Hundredths of the currency as the language writes money, the symbol first when there is one:
/// `R$ 1.234,50`, `US$ 1,234.50`.
pub fn money_text(cents: i64, currency: &str) -> String {
    let (group, point) = match i18n::current() {
        Lang::PtBr => ('.', ','),
        Lang::En => (',', '.'),
    };
    money_with(cents, currency, group, point)
}

/// Money with the separators given.
fn money_with(cents: i64, currency: &str, group: char, point: char) -> String {
    let whole = (cents.unsigned_abs() / 100).to_string();
    let mut grouped = String::new();
    for (k, c) in whole.chars().enumerate() {
        if k > 0 && (whole.len() - k).is_multiple_of(3) {
            grouped.push(group);
        }
        grouped.push(c);
    }
    let sign = if cents < 0 { "-" } else { "" };
    let number = format!("{sign}{grouped}{point}{:02}", cents.unsigned_abs() % 100);
    if currency.is_empty() { number } else { format!("{currency} {number}") }
}

/// Money typed (`1.234,50`, `1234.5`) as hundredths; `None` if it isn't a number.
fn read_money(text: &str) -> Option<f64> {
    let t = text.trim();
    // A comma after the last point is the decimal one (Portuguese), else points are.
    let normal = if t.rfind(',') > t.rfind('.') { t.replace('.', "").replace(',', ".") } else { t.replace(',', "") };
    normal.parse::<f64>().ok().filter(|m| m.is_finite() && *m >= 0.0)
}

/// The typed form for a new expense.
#[derive(Debug, Clone, Default)]
pub struct ExpenseForm {
    pub name: String,
    pub planned: String,
    pub accrual: Accrual,
    pub account: String,
}

fn accrual_name(a: Accrual) -> String {
    tr!(&format!("accrual-{}", a.code()))
}

/// An activity's costs, facts in a grid.
fn cost_facts(ui: &mut egui::Ui, t: &Tokens, c: Cost, currency: &str) {
    egui::Grid::new("cost-facts").num_columns(2).spacing(egui::vec2(12.0, 4.0)).show(ui, |ui| {
        for (name, value) in [
            (tr!("cost-budget"), c.budget),
            (tr!("cost-actual"), c.actual),
            (tr!("cost-remaining"), c.remaining),
            (tr!("cost-at-completion"), c.at_completion()),
            (tr!("cost-variance"), c.variance()),
        ] {
            ui.label(egui::RichText::new(name).size(12.0).color(t.text_dim));
            let color = if value < 0 { t.red } else { t.text };
            ui.label(egui::RichText::new(money_text(value, currency)).size(12.5).color(color));
            ui.end_row();
        }
    });
}

/// Details' Costs tab: the activity's costs, its expenses (the actual typed in place, each
/// removed) and a new one. The command to run.
pub(crate) fn costs_tab(
    ui: &mut egui::Ui,
    t: &Tokens,
    project: &Project,
    schedule: &Schedule,
    i: usize,
    form: &mut ExpenseForm,
) -> Option<(&'static str, Value)> {
    let a = project.activities.get(i)?;
    let mut pending = None;
    if let Some(c) = mkperth_engine::sched::costs(project, schedule).get(i).copied() {
        cost_facts(ui, t, c, &project.currency);
    }
    ui.add_space(8.0);
    ui.label(egui::RichText::new(tr!("expenses")).size(12.5).color(t.text).strong());
    let mine: Vec<_> = project.expenses.iter().filter(|e| e.activity == a.id).collect();
    if mine.is_empty() {
        ui.label(egui::RichText::new(tr!("detail-none")).size(12.0).color(t.text_dim));
    }
    egui::Grid::new("expenses").striped(true).num_columns(5).spacing(egui::vec2(8.0, 4.0)).show(ui, |ui| {
        for e in &mine {
            let name = ui.label(egui::RichText::new(&e.name).size(12.0)).id;
            ui.label(egui::RichText::new(money_text(e.planned, "")).size(12.0));
            // The actual cost, typed in place.
            let key = egui::Id::unique("expense-actual").with((e.activity.as_str(), e.name.as_str()));
            let shown = money_text(e.actual, "");
            let mut text = ui.data(|d| d.get_temp::<(String, String)>(key)).filter(|(was, _)| *was == shown).map_or(shown.clone(), |(_, x)| x);
            let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(64.0)).labelled_by(name).on_hover_text(tr!("cost-actual"));
            if r.lost_focus() {
                ui.data_mut(|d| d.remove::<(String, String)>(key));
                if text != shown
                    && let Some(m) = read_money(&text)
                {
                    pending = Some(("expense.edit", json!({"activity": e.activity, "name": e.name, "actual": m})));
                }
            } else if r.changed() {
                let typed = text.clone();
                ui.data_mut(|d| d.insert_temp(key, (shown, typed)));
            }
            ui.label(egui::RichText::new(accrual_name(e.accrual)).size(11.5).color(t.text_dim));
            if icon_button(ui, Icon::Close, &tr!("expense-delete", name = e.name.as_str())) {
                pending = Some(("expense.delete", json!({"activity": e.activity, "name": e.name})));
            }
            ui.end_row();
        }
    });
    ui.add_space(6.0);
    ui.label(egui::RichText::new(tr!("expense-new")).size(12.0).color(t.text_dim));
    ui.horizontal(|ui| {
        let l = ui.label(egui::RichText::new(tr!("expense-name")).size(11.5).color(t.text_dim)).id;
        ui.add(egui::TextEdit::singleline(&mut form.name).desired_width(110.0)).labelled_by(l);
        let l = ui.label(egui::RichText::new(tr!("cost-budget")).size(11.5).color(t.text_dim)).id;
        ui.add(egui::TextEdit::singleline(&mut form.planned).desired_width(70.0)).labelled_by(l);
    });
    ui.horizontal(|ui| {
        let l = ui.label(egui::RichText::new(tr!("expense-accrual")).size(11.5).color(t.text_dim)).id;
        egui::ComboBox::from_id_salt("expense-accrual")
            .selected_text(accrual_name(form.accrual))
            .width(110.0)
            .show_ui(ui, |ui| {
                for x in Accrual::ALL {
                    ui.selectable_value(&mut form.accrual, x, accrual_name(x));
                }
            })
            .response
            .labelled_by(l);
        if !project.cost_accounts.is_empty() {
            let l = ui.label(egui::RichText::new(tr!("expense-account")).size(11.5).color(t.text_dim)).id;
            let shown = if form.account.is_empty() { tr!("expense-no-account") } else { form.account.clone() };
            egui::ComboBox::from_id_salt("expense-account")
                .selected_text(shown)
                .width(90.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut form.account, String::new(), tr!("expense-no-account"));
                    for c in &project.cost_accounts {
                        ui.selectable_value(&mut form.account, c.id.clone(), format!("{} · {}", c.id, c.name));
                    }
                })
                .response
                .labelled_by(l);
        }
    });
    let planned = read_money(&form.planned);
    if ui.add_enabled(!form.name.trim().is_empty() && planned.is_some(), egui::Button::new(tr!("expense-add"))).clicked() {
        let mut params = json!({"activity": a.id, "name": form.name.trim(), "planned": planned, "accrual": form.accrual.code()});
        if !form.account.is_empty()
            && let Some(m) = params.as_object_mut()
        {
            m.insert("account".into(), json!(form.account));
        }
        pending = Some(("expense.add", params));
        *form = ExpenseForm::default();
    }
    pending
}

/// The typed form for a new cost account, and the currency being typed.
#[derive(Debug, Clone, Default)]
pub struct CostsForm {
    pub id: String,
    pub name: String,
    pub parent: String,
    pub currency: Option<String>,
}

impl PerthApp {
    /// The Costs pane: the currency, the project's costs, its spending month by month, and the
    /// cost accounts.
    pub(crate) fn costs_pane(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        if theme::pane_header(ui, Icon::Dictionary, &tr!("pane-costs"), &tr!("pane-costs-tip"), |ui| {
            icon_button(ui, Icon::ChevronRight, &tr!("pane-close"))
        }) {
            self.side = None;
        }
        let p = &self.session.project;
        let form = &mut self.costs_form;
        let mut pending: Option<(&str, Value)> = None;
        egui::ScrollArea::vertical().id_salt("costs-pane").auto_shrink(false).show(ui, |ui| {
            let heading = |ui: &mut egui::Ui, text: String| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(text).size(12.5).color(t.text).strong());
            };
            ui.horizontal(|ui| {
                let l = ui.label(egui::RichText::new(tr!("cost-currency")).size(12.0).color(t.text_dim)).id;
                let mut text = form.currency.clone().unwrap_or_else(|| p.currency.clone());
                let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(50.0).hint_text("R$")).labelled_by(l);
                if r.lost_focus() {
                    if text.trim() != p.currency {
                        pending = Some(("project.options", json!({"currency": text.trim()})));
                    }
                    form.currency = None;
                } else if r.changed() {
                    form.currency = Some(text);
                }
            });
            if let Ok(s) = &self.session.schedule {
                heading(ui, tr!("cost-project"));
                cost_facts(ui, &t, mkperth_engine::sched::project_cost(p, s), &p.currency);
                // The budget month by month: the spending plan.
                heading(ui, tr!("cost-by-month"));
                let mut months: Vec<(Date, i64)> = Vec::new();
                for (d, c) in mkperth_engine::sched::spending(p, s) {
                    let (y, m, _) = d.ymd();
                    let first = Date::from_ymd(y, m, 1).unwrap_or(d);
                    match months.last_mut() {
                        Some((at, sum)) if *at == first => *sum += c,
                        _ => months.push((first, c)),
                    }
                }
                if months.is_empty() {
                    ui.label(egui::RichText::new(tr!("detail-none")).size(12.0).color(t.text_dim));
                }
                let most = months.iter().map(|(_, c)| *c).max().unwrap_or(1).max(1);
                egui::Grid::new("cost-months").num_columns(3).spacing(egui::vec2(10.0, 3.0)).show(ui, |ui| {
                    for (m, c) in &months {
                        let (y, n, _) = m.ymd();
                        ui.label(egui::RichText::new(format!("{} {y}", tr!(&format!("month-{n}")))).size(12.0).color(t.text_dim));
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(90.0, 10.0), egui::Sense::hover());
                        let w = rect.width() * (*c as f32 / most as f32);
                        ui.painter().rect_filled(egui::Rect::from_min_size(rect.min, egui::vec2(w, rect.height())), 0.0, crate::splash::BRAND);
                        ui.label(egui::RichText::new(money_text(*c, &p.currency)).size(12.0));
                        ui.end_row();
                    }
                });
            }

            heading(ui, tr!("cost-accounts"));
            if p.cost_accounts.is_empty() {
                ui.label(egui::RichText::new(tr!("detail-none")).size(12.0).color(t.text_dim));
            }
            // In their hierarchy: each under its parent, indented.
            let depth = |id: &str| {
                let (mut d, mut at) = (0usize, p.cost_accounts.iter().find(|c| c.id == id).and_then(|c| c.parent.clone()));
                while let Some(parent) = at.filter(|_| d < p.cost_accounts.len()) {
                    d += 1;
                    at = p.cost_accounts.iter().find(|c| c.id == parent).and_then(|c| c.parent.clone());
                }
                d
            };
            for c in &p.cost_accounts {
                ui.horizontal(|ui| {
                    ui.add_space(depth(&c.id) as f32 * 14.0);
                    ui.label(egui::RichText::new(format!("{} · {}", c.id, c.name)).size(12.0));
                    if icon_button(ui, Icon::Close, &tr!("cost-account-delete", id = c.id.as_str())) {
                        pending = Some(("cost-account.delete", json!({"id": c.id})));
                    }
                });
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let l = ui.label(egui::RichText::new(tr!("cost-account-id")).size(11.5).color(t.text_dim)).id;
                ui.add(egui::TextEdit::singleline(&mut form.id).desired_width(50.0)).labelled_by(l);
                let l = ui.label(egui::RichText::new(tr!("cost-account-name")).size(11.5).color(t.text_dim)).id;
                ui.add(egui::TextEdit::singleline(&mut form.name).desired_width(110.0)).labelled_by(l);
            });
            ui.horizontal(|ui| {
                let l = ui.label(egui::RichText::new(tr!("cost-account-parent")).size(11.5).color(t.text_dim)).id;
                let shown = if form.parent.is_empty() { tr!("expense-no-account") } else { form.parent.clone() };
                egui::ComboBox::from_id_salt("cost-account-parent")
                    .selected_text(shown)
                    .width(90.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut form.parent, String::new(), tr!("expense-no-account"));
                        for c in &p.cost_accounts {
                            ui.selectable_value(&mut form.parent, c.id.clone(), format!("{} · {}", c.id, c.name));
                        }
                    })
                    .response
                    .labelled_by(l);
                if ui.add_enabled(!form.id.trim().is_empty() && !form.name.trim().is_empty(), egui::Button::new(tr!("cost-account-add"))).clicked() {
                    let parent = (!form.parent.is_empty()).then(|| form.parent.clone());
                    pending = Some(("cost-account.add", json!({"id": form.id.trim(), "name": form.name.trim(), "parent": parent})));
                    form.id.clear();
                    form.name.clear();
                }
            });
        });
        if let Some((command, params)) = pending {
            self.run(command, params);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_as_each_language_writes_it() {
        assert_eq!(money_with(123_450, "US$", ',', '.'), "US$ 1,234.50");
        assert_eq!(money_with(-5, "", ',', '.'), "-0.05");
        assert_eq!(money_with(123_456_789, "R$", '.', ','), "R$ 1.234.567,89");
        assert_eq!(read_money("1.234,50"), Some(1234.5));
        assert_eq!(read_money("1,234.50"), Some(1234.5));
        assert_eq!(read_money("30,5"), Some(30.5));
        assert_eq!(read_money("x"), None);
    }
}
