//! The Budget pane: the budget of the project or of one WBS, how it stands (original, current,
//! proposed, what its WBS take, what its plan places, what its activities cost), its spending plan
//! month by month, its log of changes, and the funding sources and what each gives the project.
//! Each change is a command (`budget.*`, `budget-change.*`, `funding-source.*`, `fund.set`).

use mkperth_engine::core::{ChangeStatus, Date};
use mkperth_engine::i18n::tr;
use serde_json::{Value, json};

use crate::costs_ui::{money_text, read_money, read_signed_money};
use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::{PerthApp, date_field, icon_button, show_date};

/// What the Budget pane is typing: the WBS shown (`None`, the project), and its forms.
#[derive(Debug, Clone, Default)]
pub struct BudgetForm {
    pub wbs: Option<String>,
    /// The original budget being typed, for the WBS it was begun on.
    pub original: Option<(Option<String>, String)>,
    pub month: String,
    pub month_amount: String,
    pub change_date: Option<Date>,
    pub change_amount: String,
    pub responsible: String,
    pub reason: String,
    pub source_id: String,
    pub source_name: String,
    pub source_parent: String,
    pub fund_source: String,
    pub fund_amount: String,
}

fn status_name(s: ChangeStatus) -> String {
    tr!(&format!("budget-status-{}", s.code()))
}

/// A month typed as `11/2026` or `2026-11`, as the command's `2026-11`.
fn read_month(text: &str) -> Option<String> {
    let t = text.trim();
    let (y, m) = match t.split_once('/') {
        Some((m, y)) => (y.trim().parse::<i32>().ok()?, m.trim().parse::<u32>().ok()?),
        None => {
            let (y, m) = t.split_once('-')?;
            (y.trim().parse::<i32>().ok()?, m.trim().parse::<u32>().ok()?)
        }
    };
    Date::from_ymd(y, m, 1).map(|_| format!("{y:04}-{m:02}"))
}

impl PerthApp {
    pub(crate) fn budget_pane(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        if theme::pane_header(ui, Icon::Bars, &tr!("pane-budget"), &tr!("pane-budget-tip"), |ui| {
            icon_button(ui, Icon::ChevronRight, &tr!("pane-close"))
        }) {
            self.side = None;
        }
        let p = &self.session.project;
        let lines = mkperth_engine::sched::budget(p, self.session.schedule.as_ref().ok());
        let form = &mut self.budget_form;
        // A WBS gone (deleted, another file) shows the project again.
        if form.wbs.as_deref().is_some_and(|w| !lines.iter().any(|l| l.wbs.as_deref() == Some(w))) {
            form.wbs = None;
        }
        let Some(line) = lines.iter().find(|l| l.wbs == form.wbs).cloned() else { return };
        let today = self.options.today;
        let money = |m: i64| money_text(m, &p.currency);
        let mut pending: Vec<(&str, Value)> = Vec::new();
        let wbs_param = json!(form.wbs);
        egui::ScrollArea::vertical().id_salt("budget-pane").auto_shrink(false).show(ui, |ui| {
            // Whose budget: the project, or a WBS in its place in the tree.
            let of = theme::section(ui, &tr!("budget-of"));
            let name = |w: Option<&str>| match w {
                None => tr!("budget-project"),
                Some(id) => p.activities.iter().find(|a| a.id == id).map_or_else(|| id.to_string(), |a| format!("{id} · {}", a.name)),
            };
            egui::ComboBox::from_id_salt("budget-of")
                .selected_text(name(form.wbs.as_deref()))
                .width(theme::fill(ui))
                .truncate()
                .show_ui(ui, |ui| {
                    for l in &lines {
                        let indent = "   ".repeat(l.depth.saturating_sub(1));
                        let text = if l.wbs.is_none() { name(None) } else { format!("{indent}{}", name(l.wbs.as_deref())) };
                        ui.selectable_value(&mut form.wbs, l.wbs.clone(), text);
                    }
                })
                .response
                .labelled_by(of);

            // How it stands.
            theme::section(ui, &tr!("budget-summary"));
            theme::row(ui, &tr!("budget-original"), |ui, label| {
                // Empty while there is none: typing into "0,00" would add to it.
                let shown = if line.original == 0 { String::new() } else { money_text(line.original, "") };
                let mut text = form.original.as_ref().filter(|(w, _)| *w == form.wbs).map_or(shown.clone(), |(_, x)| x.clone());
                let r = ui
                    .add(
                        egui::TextEdit::singleline(&mut text)
                            .desired_width(theme::fill(ui))
                            .align(egui::Align2::RIGHT_CENTER)
                            .hint_text(money_text(0, "")),
                    )
                    .labelled_by(label);
                if r.lost_focus() {
                    form.original = None;
                    if text != shown {
                        match read_money(&text) {
                            Some(m) => pending.push(("budget.set", json!({"wbs": wbs_param, "original": m}))),
                            None => pending.push(("budget.set", json!({"wbs": wbs_param, "original": "unreadable"}))),
                        }
                    }
                } else if r.changed() {
                    form.original = Some((form.wbs.clone(), text));
                }
            });
            let red = |m: i64| if m < 0 { t.red } else { t.text };
            theme::fact(ui, &tr!("budget-current"), &money(line.current), t.text);
            theme::fact(ui, &tr!("budget-proposed"), &money(line.proposed), t.text);
            theme::fact(ui, &tr!("budget-distributed"), &money(line.distributed), t.text);
            theme::fact(ui, &tr!("budget-undistributed"), &money(line.undistributed()), red(line.undistributed()));
            if let Some(cost) = line.scheduled {
                let over = line.current.saturating_sub(cost);
                theme::fact(ui, &tr!("budget-scheduled"), &money(cost), red(over));
            }

            // The spending plan, month by month.
            theme::section(ui, &tr!("budget-plan"));
            let plan = p.budget.of(form.wbs.as_deref()).map(|b| b.plan.clone()).unwrap_or_default();
            if plan.is_empty() {
                theme::hint(ui, &tr!("detail-none"));
            }
            for m in &plan {
                let (y, n, _) = m.month.ymd();
                let when = format!("{} {y}", tr!(&format!("month-{n}")));
                if theme::list_item(ui, None, &when, &money(m.amount), &tr!("budget-plan-delete")) {
                    pending.push(("budget.plan", json!({"wbs": wbs_param, "month": format!("{y:04}-{n:02}"), "amount": 0})));
                }
            }
            theme::fact(ui, &tr!("budget-planned"), &money(line.planned), t.text);
            theme::fact(ui, &tr!("budget-unplanned"), &money(line.unplanned()), if line.unplanned() == 0 { t.text } else { t.red });
            theme::row(ui, &tr!("budget-month"), |ui, label| {
                ui.add(egui::TextEdit::singleline(&mut form.month).desired_width(70.0).hint_text("11/2026")).labelled_by(label);
                ui.add(egui::TextEdit::singleline(&mut form.month_amount).desired_width(theme::fill(ui)).hint_text(tr!("budget-amount")))
                    .labelled_by(label);
            });
            ui.horizontal(|ui| {
                theme::under_fields(ui);
                let (month, amount) = (read_month(&form.month), read_money(&form.month_amount));
                if ui.add_enabled(month.is_some() && amount.is_some(), egui::Button::new(tr!("budget-plan-add"))).clicked() {
                    pending.push(("budget.plan", json!({"wbs": wbs_param, "month": month, "amount": amount})));
                    form.month.clear();
                    form.month_amount.clear();
                }
            });

            // The log of changes to this budget.
            theme::section(ui, &tr!("budget-log"));
            let changes: Vec<_> = p.budget.log.iter().filter(|c| c.wbs == form.wbs).collect();
            if changes.is_empty() {
                theme::hint(ui, &tr!("detail-none"));
            }
            for c in changes {
                ui.horizontal(|ui| {
                    ui.set_min_height(22.0);
                    let sign = if c.amount > 0 { "+" } else { "" };
                    let label = ui.label(egui::RichText::new(format!("#{} {sign}{}", c.number, money(c.amount))).size(12.5).color(t.text)).id;
                    ui.label(egui::RichText::new(show_date(c.date)).size(12.0).color(t.text_dim));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icon_button(ui, Icon::Close, &tr!("budget-change-delete", number = c.number)) {
                            pending.push(("budget-change.delete", json!({"number": c.number})));
                        }
                        let mut status = c.status;
                        egui::ComboBox::from_id_salt(("budget-status", c.number))
                            .selected_text(status_name(status))
                            .width(100.0)
                            .truncate()
                            .show_ui(ui, |ui| {
                                for s in ChangeStatus::ALL {
                                    ui.selectable_value(&mut status, s, status_name(s));
                                }
                            })
                            .response
                            .labelled_by(label);
                        if status != c.status {
                            pending.push(("budget-change.edit", json!({"number": c.number, "status": status.code()})));
                        }
                    });
                });
                let why = [c.reason.as_str(), c.responsible.as_str()].iter().filter(|x| !x.is_empty()).copied().collect::<Vec<_>>().join(" · ");
                if !why.is_empty() {
                    theme::hint(ui, &why);
                }
            }
            theme::row(ui, &tr!("budget-change-date"), |ui, label| {
                if let Some(d) = date_field(ui, egui::Id::unique("budget-change-date"), label, form.change_date.or(today), 110.0) {
                    form.change_date = d;
                }
            });
            theme::row(ui, &tr!("budget-amount"), |ui, label| {
                ui.add(egui::TextEdit::singleline(&mut form.change_amount).desired_width(theme::fill(ui)).hint_text(tr!("budget-amount-hint")))
                    .labelled_by(label);
            });
            theme::row(ui, &tr!("budget-responsible"), |ui, label| {
                ui.add(egui::TextEdit::singleline(&mut form.responsible).desired_width(theme::fill(ui))).labelled_by(label);
            });
            theme::row(ui, &tr!("budget-reason"), |ui, label| {
                ui.add(egui::TextEdit::singleline(&mut form.reason).desired_width(theme::fill(ui))).labelled_by(label);
            });
            ui.horizontal(|ui| {
                theme::under_fields(ui);
                let (date, amount) = (form.change_date.or(today), read_signed_money(&form.change_amount));
                if ui.add_enabled(date.is_some() && amount.is_some(), egui::Button::new(tr!("budget-change-add"))).clicked() {
                    pending.push((
                        "budget-change.add",
                        json!({"wbs": wbs_param, "date": date.map(|d| d.to_string()), "amount": amount,
                            "responsible": form.responsible.trim(), "reason": form.reason.trim()}),
                    ));
                    form.change_amount.clear();
                    form.reason.clear();
                }
            });

            // The money's sources: the project's alone, as P6 keeps them.
            if form.wbs.is_none() {
                theme::section(ui, &tr!("budget-funding"));
                let (funded, unfunded) = mkperth_engine::sched::funding(p);
                if p.budget.funds.is_empty() {
                    theme::hint(ui, &tr!("detail-none"));
                }
                for f in &p.budget.funds {
                    // Its ID: the name is in the sources' list below.
                    let name = f.source.clone();
                    let share = if line.current > 0 {
                        format!("{} · {:.0}%", money(f.amount), f.amount as f64 / line.current as f64 * 100.0)
                    } else {
                        money(f.amount)
                    };
                    if theme::list_item(ui, None, &name, &share, &tr!("budget-fund-delete")) {
                        pending.push(("fund.set", json!({"source": f.source, "amount": 0})));
                    }
                }
                theme::fact(ui, &tr!("budget-funded"), &money(funded), t.text);
                theme::fact(ui, &tr!("budget-unfunded"), &money(unfunded), if unfunded == 0 { t.text } else { t.red });
                if !p.budget.sources.is_empty() {
                    theme::row(ui, &tr!("budget-fund-source"), |ui, label| {
                        let shown = if form.fund_source.is_empty() { tr!("budget-choose") } else { form.fund_source.clone() };
                        egui::ComboBox::from_id_salt("budget-fund-source")
                            .selected_text(shown)
                            .width(theme::fill(ui))
                            .truncate()
                            .show_ui(ui, |ui| {
                                for s in &p.budget.sources {
                                    ui.selectable_value(&mut form.fund_source, s.id.clone(), format!("{} · {}", s.id, s.name));
                                }
                            })
                            .response
                            .labelled_by(label);
                    });
                    theme::row(ui, &tr!("budget-amount"), |ui, label| {
                        ui.add(egui::TextEdit::singleline(&mut form.fund_amount).desired_width(theme::fill(ui))).labelled_by(label);
                    });
                    ui.horizontal(|ui| {
                        theme::under_fields(ui);
                        let amount = read_money(&form.fund_amount);
                        if ui.add_enabled(!form.fund_source.is_empty() && amount.is_some(), egui::Button::new(tr!("budget-fund-add"))).clicked() {
                            pending.push(("fund.set", json!({"source": form.fund_source, "amount": amount})));
                            form.fund_amount.clear();
                        }
                    });
                }

                // The sources themselves, in their hierarchy.
                theme::section(ui, &tr!("budget-sources"));
                if p.budget.sources.is_empty() {
                    theme::hint(ui, &tr!("detail-none"));
                }
                let depth = |id: &str| {
                    let (mut d, mut at) = (0usize, p.budget.sources.iter().find(|s| s.id == id).and_then(|s| s.parent.clone()));
                    while let Some(parent) = at.filter(|_| d < p.budget.sources.len()) {
                        d += 1;
                        at = p.budget.sources.iter().find(|s| s.id == parent).and_then(|s| s.parent.clone());
                    }
                    d
                };
                for s in &p.budget.sources {
                    ui.horizontal(|ui| {
                        ui.add_space(depth(&s.id) as f32 * 14.0);
                        ui.vertical(|ui| {
                            if theme::list_item(ui, None, &s.id, &s.name, &tr!("budget-source-delete", id = s.id.as_str())) {
                                pending.push(("funding-source.delete", json!({"id": s.id})));
                            }
                        });
                    });
                }
                theme::row(ui, &tr!("cost-account-id"), |ui, label| {
                    ui.add(egui::TextEdit::singleline(&mut form.source_id).desired_width(80.0)).labelled_by(label);
                });
                theme::row(ui, &tr!("cost-account-name"), |ui, label| {
                    ui.add(egui::TextEdit::singleline(&mut form.source_name).desired_width(theme::fill(ui))).labelled_by(label);
                });
                theme::row(ui, &tr!("cost-account-parent"), |ui, label| {
                    let shown = if form.source_parent.is_empty() { tr!("expense-no-account") } else { form.source_parent.clone() };
                    egui::ComboBox::from_id_salt("budget-source-parent")
                        .selected_text(shown)
                        .width(theme::fill(ui))
                        .truncate()
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut form.source_parent, String::new(), tr!("expense-no-account"));
                            for s in &p.budget.sources {
                                ui.selectable_value(&mut form.source_parent, s.id.clone(), format!("{} · {}", s.id, s.name));
                            }
                        })
                        .response
                        .labelled_by(label);
                });
                ui.horizontal(|ui| {
                    theme::under_fields(ui);
                    let ready = !form.source_id.trim().is_empty() && !form.source_name.trim().is_empty();
                    if ui.add_enabled(ready, egui::Button::new(tr!("budget-source-add"))).clicked() {
                        let parent = (!form.source_parent.is_empty()).then(|| form.source_parent.clone());
                        pending.push(("funding-source.add", json!({"id": form.source_id.trim(), "name": form.source_name.trim(), "parent": parent})));
                        form.source_id.clear();
                        form.source_name.clear();
                    }
                });
            }
        });
        for (command, params) in pending {
            self.run(command, params);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn months_as_typed() {
        assert_eq!(read_month("11/2026").as_deref(), Some("2026-11"));
        assert_eq!(read_month(" 2027-1 ").as_deref(), Some("2027-01"));
        for bad in ["13/2026", "2026", "nov/2026", ""] {
            assert_eq!(read_month(bad), None, "{bad}");
        }
    }
}
