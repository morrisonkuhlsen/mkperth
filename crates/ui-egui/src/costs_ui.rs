//! Costs in the window: an activity's costs and expenses in Details, and the Costs pane (the
//! currency, the project's costs, its spending month by month and the cost accounts). Every
//! change is a command, undoable as any other.

use mkperth_engine::core::{Accrual, Project};
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
    for (name, value) in [
        (tr!("cost-budget"), c.budget),
        (tr!("cost-actual"), c.actual),
        (tr!("cost-remaining"), c.remaining),
        (tr!("cost-at-completion"), c.at_completion()),
        (tr!("cost-variance"), c.variance()),
    ] {
        theme::fact(ui, &name, &money_text(value, currency), if value < 0 { t.red } else { t.text });
    }
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
    theme::section(ui, &tr!("expenses"));
    let mine: Vec<_> = project.expenses.iter().filter(|e| e.activity == a.id).collect();
    if mine.is_empty() {
        theme::hint(ui, &tr!("detail-none"));
    }
    egui::Grid::new("expenses").striped(false).num_columns(5).spacing(egui::vec2(8.0, 4.0)).show(ui, |ui| {
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
    theme::section(ui, &tr!("expense-new"));
    theme::row(ui, &tr!("expense-name"), |ui, l| {
        ui.add(egui::TextEdit::singleline(&mut form.name).desired_width(theme::fill(ui))).labelled_by(l);
    });
    theme::row(ui, &tr!("cost-budget"), |ui, l| {
        ui.add(egui::TextEdit::singleline(&mut form.planned).desired_width(110.0)).labelled_by(l);
    });
    theme::row(ui, &tr!("expense-accrual"), |ui, l| {
        egui::ComboBox::from_id_salt("expense-accrual")
            .selected_text(accrual_name(form.accrual))
            .width(theme::fill(ui))
            .truncate()
            .show_ui(ui, |ui| {
                for x in Accrual::ALL {
                    ui.selectable_value(&mut form.accrual, x, accrual_name(x));
                }
            })
            .response
            .labelled_by(l);
    });
    if !project.cost_accounts.is_empty() {
        theme::row(ui, &tr!("expense-account"), |ui, l| {
            let shown = if form.account.is_empty() { tr!("expense-no-account") } else { form.account.clone() };
            egui::ComboBox::from_id_salt("expense-account")
                .selected_text(shown)
                .width(theme::fill(ui))
                .truncate()
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut form.account, String::new(), tr!("expense-no-account"));
                    for c in &project.cost_accounts {
                        ui.selectable_value(&mut form.account, c.id.clone(), format!("{} · {}", c.id, c.name));
                    }
                })
                .response
                .labelled_by(l);
        });
    }
    ui.add_space(4.0);
    let planned = read_money(&form.planned);
    ui.horizontal(|ui| {
        theme::under_fields(ui);
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
    });
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
            let heading = |ui: &mut egui::Ui, text: String| theme::section(ui, &text);
            theme::row(ui, &tr!("cost-currency"), |ui, l| {
                let mut text = form.currency.clone().unwrap_or_else(|| p.currency.clone());
                let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(70.0).hint_text("R$")).labelled_by(l);
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
                let months = mkperth_engine::sched::spending_by_month(p, s);
                if months.is_empty() {
                    theme::hint(ui, &tr!("detail-none"));
                }
                let most = months.iter().map(|(_, c)| *c).max().unwrap_or(1).max(1);
                for (m, c) in &months {
                    let (y, n, _) = m.ymd();
                    theme::row(ui, &format!("{} {y}", tr!(&format!("month-{n}"))), |ui, _| {
                        // The amount at the right edge, the bar in the space before it.
                        let amount = money_text(*c, &p.currency);
                        let galley = ui.painter().layout_no_wrap(amount, egui::FontId::proportional(12.5), t.text);
                        let bar_width = (ui.available_width() - galley.size().x - 12.0).max(10.0);
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(bar_width, 10.0), egui::Sense::hover());
                        let w = rect.width() * (*c as f32 / most as f32);
                        ui.painter().rect_filled(egui::Rect::from_min_size(rect.min, egui::vec2(w, rect.height())), 0.0, crate::splash::BRAND);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(galley.text()).size(12.5).color(t.text));
                        });
                    });
                }
            }

            heading(ui, tr!("cost-accounts"));
            if p.cost_accounts.is_empty() {
                theme::hint(ui, &tr!("detail-none"));
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
                    ui.vertical(|ui| {
                        if theme::list_item(ui, None, &c.id, &c.name, &tr!("cost-account-delete", id = c.id.as_str())) {
                            pending = Some(("cost-account.delete", json!({"id": c.id})));
                        }
                    });
                });
            }
            heading(ui, tr!("cost-account-new"));
            theme::row(ui, &tr!("cost-account-id"), |ui, l| {
                ui.add(egui::TextEdit::singleline(&mut form.id).desired_width(80.0)).labelled_by(l);
            });
            theme::row(ui, &tr!("cost-account-name"), |ui, l| {
                ui.add(egui::TextEdit::singleline(&mut form.name).desired_width(theme::fill(ui))).labelled_by(l);
            });
            theme::row(ui, &tr!("cost-account-parent"), |ui, l| {
                let shown = if form.parent.is_empty() { tr!("expense-no-account") } else { form.parent.clone() };
                egui::ComboBox::from_id_salt("cost-account-parent")
                    .selected_text(shown)
                    .width(theme::fill(ui))
                    .truncate()
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut form.parent, String::new(), tr!("expense-no-account"));
                        for c in &p.cost_accounts {
                            ui.selectable_value(&mut form.parent, c.id.clone(), format!("{} · {}", c.id, c.name));
                        }
                    })
                    .response
                    .labelled_by(l);
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                theme::under_fields(ui);
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
