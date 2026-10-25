//! The budget: the money first approved for the project and each WBS, the log of changes to it,
//! the spending plan month by month, the funding sources and what each gives. A WBS is named by
//! its summary activity's ID; none names the project. Money comes in as numbers (`300.5`, a cut
//! as `-300`) and is kept in hundredths of the currency. Each change is a command with undo.

use mkperth_core::{BudgetChange, ChangeStatus, Date, Fund, FundingSource, Project, SpendingMonth, WbsBudget};
use mkperth_i18n::tr;
use serde_json::Value;

use crate::costs::{MAX_ITEMS, MAX_MONEY, bad, money, required, text};
use crate::{EngineError, Session};

/// The most months a spending plan may hold: a century.
const MAX_MONTHS: usize = 1200;

/// The WBS named by `wbs` (`None`, absent or empty: the project).
fn wbs(params: &Value, command: &str) -> Result<Option<String>, EngineError> {
    Ok(text(params, command, "wbs")?.filter(|s| !s.is_empty()))
}

/// The WBS's activity is in the project.
fn known(p: &Project, wbs: Option<&str>) -> Result<(), EngineError> {
    match wbs {
        Some(id) if !p.activities.iter().any(|a| a.id == id) => Err(EngineError::UnknownActivity(id.to_string())),
        _ => Ok(()),
    }
}

/// An amount that may be a cut, as hundredths.
fn signed(params: &Value, command: &str, name: &str) -> Result<Option<i64>, EngineError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            v.as_f64().filter(|m| m.is_finite() && m.abs() <= MAX_MONEY).map(|m| Some((m * 100.0).round() as i64)).ok_or_else(|| bad(command, name))
        }
    }
}

fn date(params: &Value, command: &str, name: &str) -> Result<Option<Date>, EngineError> {
    text(params, command, name)?.filter(|s| !s.is_empty()).map(|d| Date::parse(&d).ok_or_else(|| bad(command, name))).transpose()
}

/// A month as its first day, from `2026-11` or any day in it.
fn month(params: &Value, command: &str) -> Result<Date, EngineError> {
    let m = required(params, command, "month")?;
    let day = if m.len() == 7 { Date::parse(&format!("{m}-01")) } else { Date::parse(&m) };
    let (y, mo, _) = day.ok_or_else(|| bad(command, "month"))?.ymd();
    Date::from_ymd(y, mo, 1).ok_or_else(|| bad(command, "month"))
}

fn status(params: &Value, command: &str) -> Result<Option<ChangeStatus>, EngineError> {
    text(params, command, "status")?.map(|s| ChangeStatus::from_code(&s).ok_or_else(|| bad(command, "status"))).transpose()
}

fn number(params: &Value, command: &str) -> Result<u32, EngineError> {
    match params.get("number") {
        None | Some(Value::Null) => Err(EngineError::Param(command.into(), "number".into())),
        Some(v) => v.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or_else(|| bad(command, "number")),
    }
}

/// The project's budget (`wbs` `None`) or a WBS's, made when it has none.
fn budget_of<'a>(p: &'a mut Project, wbs: &Option<String>) -> Option<&'a mut WbsBudget> {
    if p.budget.of(wbs.as_deref()).is_none() {
        p.budget.budgets.push(WbsBudget { wbs: wbs.clone(), ..WbsBudget::default() });
    }
    p.budget.budgets.iter_mut().find(|b| b.wbs == *wbs)
}

impl Session {
    /// The budget commands; `None` when `id` isn't one of them.
    pub(crate) fn budget_command(&mut self, id: &str, params: &Value) -> Option<Result<(), EngineError>> {
        Some(match id {
            "budget.set" => self.set_budget(params),
            "budget.plan" => self.plan_budget(params),
            "budget-change.add" => self.add_budget_change(params),
            "budget-change.edit" => self.edit_budget_change(params),
            "budget-change.delete" => self.delete_budget_change(params),
            "funding-source.add" => self.add_funding_source(params),
            "funding-source.delete" => self.delete_funding_source(params),
            "fund.set" => self.set_fund(params),
            _ => return None,
        })
    }

    /// The `original` budget of the project, or of the WBS `wbs`.
    fn set_budget(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "budget.set";
        let wbs = wbs(params, C)?;
        let original = money(params, C, "original")?.ok_or_else(|| EngineError::Param(C.into(), "original".into()))?;
        let label = tr!("undo-budget");
        self.change(label, |p| {
            known(p, wbs.as_deref())?;
            if p.budget.budgets.len() >= MAX_ITEMS && p.budget.of(wbs.as_deref()).is_none() {
                return Err(bad(C, "wbs"));
            }
            budget_of(p, &wbs).ok_or_else(|| EngineError::Internal(C.into()))?.original = original;
            Ok(())
        })
    }

    /// What the spending plan of the project (or of `wbs`) spends in `month` (`2026-11`): the
    /// `amount`, or nothing when it is 0.
    fn plan_budget(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "budget.plan";
        let wbs = wbs(params, C)?;
        let month = month(params, C)?;
        let amount = money(params, C, "amount")?.ok_or_else(|| EngineError::Param(C.into(), "amount".into()))?;
        let label = tr!("undo-spending-plan");
        self.change(label, |p| {
            known(p, wbs.as_deref())?;
            if p.budget.budgets.len() >= MAX_ITEMS && p.budget.of(wbs.as_deref()).is_none() {
                return Err(bad(C, "wbs"));
            }
            let plan = &mut budget_of(p, &wbs).ok_or_else(|| EngineError::Internal(C.into()))?.plan;
            plan.retain(|m| m.month != month);
            if amount != 0 {
                if plan.len() >= MAX_MONTHS {
                    return Err(bad(C, "month"));
                }
                plan.push(SpendingMonth { month, amount });
                plan.sort_by_key(|m| m.month);
            }
            Ok(())
        })
    }

    /// A change in the budget log, numbered after the highest there: to the project or `wbs`, on `date`,
    /// its `amount` (negative for a cut), `status` (pending, approved, not-approved),
    /// `responsible` and `reason`.
    fn add_budget_change(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "budget-change.add";
        let wbs = wbs(params, C)?;
        let date = date(params, C, "date")?.ok_or_else(|| EngineError::Param(C.into(), "date".into()))?;
        let amount = signed(params, C, "amount")?.ok_or_else(|| EngineError::Param(C.into(), "amount".into()))?;
        let status = status(params, C)?.unwrap_or_default();
        let responsible = text(params, C, "responsible")?.unwrap_or_default();
        let reason = text(params, C, "reason")?.unwrap_or_default();
        let label = tr!("undo-add-budget-change");
        self.change(label, |p| {
            known(p, wbs.as_deref())?;
            if p.budget.log.len() >= MAX_ITEMS {
                return Err(bad(C, "amount"));
            }
            let number = p.budget.log.iter().map(|c| c.number).max().unwrap_or(0).checked_add(1).ok_or_else(|| bad(C, "number"))?;
            p.budget.log.push(BudgetChange { number, wbs, date, amount, status, responsible, reason });
            Ok(())
        })
    }

    /// Changes the change `number`: its `date`, `amount`, `status`, `responsible`, `reason`.
    fn edit_budget_change(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "budget-change.edit";
        let number = number(params, C)?;
        let date = date(params, C, "date")?;
        let amount = signed(params, C, "amount")?;
        let status = status(params, C)?;
        let responsible = text(params, C, "responsible")?;
        let reason = text(params, C, "reason")?;
        self.change(tr!("undo-edit-budget-change", number = number), |p| {
            let c = p.budget.log.iter_mut().find(|c| c.number == number).ok_or_else(|| bad(C, "number"))?;
            if let Some(d) = date {
                c.date = d;
            }
            if let Some(a) = amount {
                c.amount = a;
            }
            if let Some(s) = status {
                c.status = s;
            }
            if let Some(r) = responsible {
                c.responsible = r;
            }
            if let Some(r) = reason {
                c.reason = r;
            }
            Ok(())
        })
    }

    fn delete_budget_change(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "budget-change.delete";
        let number = number(params, C)?;
        self.change(tr!("undo-delete-budget-change", number = number), |p| {
            let before = p.budget.log.len();
            p.budget.log.retain(|c| c.number != number);
            if p.budget.log.len() == before {
                return Err(bad(C, "number"));
            }
            Ok(())
        })
    }

    /// A funding source: `id` and `name` (required), under the source `parent`.
    fn add_funding_source(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "funding-source.add";
        let id = required(params, C, "id")?;
        let name = required(params, C, "name")?;
        let parent = text(params, C, "parent")?.filter(|s| !s.is_empty());
        self.change(tr!("undo-add-funding-source", id = id.as_str()), |p| {
            if p.budget.sources.len() >= MAX_ITEMS || p.budget.sources.iter().any(|s| s.id == id) {
                return Err(bad(C, "id"));
            }
            if let Some(parent) = &parent
                && !p.budget.sources.iter().any(|s| s.id == *parent)
            {
                return Err(bad(C, "parent"));
            }
            p.budget.sources.push(FundingSource { id: id.clone(), name, parent });
            Ok(())
        })
    }

    /// Deletes the funding source `id`: the sources under it go under its own parent, and what
    /// it gave the project goes with it.
    fn delete_funding_source(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "funding-source.delete";
        let id = required(params, C, "id")?;
        self.change(tr!("undo-delete-funding-source", id = id.as_str()), |p| {
            let k = p.budget.sources.iter().position(|s| s.id == id).ok_or_else(|| bad(C, "id"))?;
            let gone = p.budget.sources.remove(k);
            for s in p.budget.sources.iter_mut().filter(|s| s.parent.as_deref() == Some(id.as_str())) {
                s.parent = gone.parent.clone();
            }
            p.budget.funds.retain(|f| f.source != id);
            Ok(())
        })
    }

    /// What the funding source `source` gives the project: the `amount`, or nothing when 0.
    fn set_fund(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "fund.set";
        let source = required(params, C, "source")?;
        let amount = money(params, C, "amount")?.ok_or_else(|| EngineError::Param(C.into(), "amount".into()))?;
        self.change(tr!("undo-fund", source = source.as_str()), |p| {
            if !p.budget.sources.iter().any(|s| s.id == source) {
                return Err(bad(C, "source"));
            }
            p.budget.funds.retain(|f| f.source != source);
            if amount != 0 {
                p.budget.funds.push(Fund { source: source.clone(), amount });
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn run(s: &mut Session, id: &str, params: Value) {
        if let Err(e) = s.run(id, &params) {
            panic!("{id} {params}: {e}");
        }
    }

    /// The sample's WBS: W1 (Design), W2 (Structure), W3 (Finishes). The project approved at
    /// 1 000 000, Design 300 000 and Structure 500 000; a change of +50 000 approved for Structure
    /// and one of −20 000 pending for Design. The project's current 1 000 000 is distributed
    /// 300 000 + 550 000 = 850 000, 150 000 left; Design's proposed is 280 000. A plan of
    /// 400 000 in November and 600 000 in December places all of it; the bank's 700 000 and the
    /// owner's 250 000 leave 50 000 unfunded.
    #[test]
    fn budget_log_plan_and_funds() {
        let mut s = Session::default();
        run(&mut s, "budget.set", json!({"original": 1_000_000}));
        run(&mut s, "budget.set", json!({"wbs": "W1", "original": 300_000}));
        run(&mut s, "budget.set", json!({"wbs": "W2", "original": 500_000}));
        run(&mut s, "budget-change.add", json!({"wbs": "W2", "date": "2026-11-10", "amount": 50_000, "status": "approved", "reason": "piling"}));
        run(&mut s, "budget-change.add", json!({"wbs": "W1", "date": "2026-11-12", "amount": -20_000, "responsible": "PM"}));
        run(&mut s, "budget.plan", json!({"month": "2026-11", "amount": 400_000}));
        run(&mut s, "budget.plan", json!({"month": "2026-12-15", "amount": 600_000}));
        run(&mut s, "funding-source.add", json!({"id": "BANK", "name": "Bank loan"}));
        run(&mut s, "funding-source.add", json!({"id": "OWN", "name": "Owner"}));
        run(&mut s, "fund.set", json!({"source": "BANK", "amount": 700_000}));
        run(&mut s, "fund.set", json!({"source": "OWN", "amount": 250_000}));
        let lines = mkperth_sched::budget(&s.project, s.schedule.as_ref().ok());
        let line = |w: Option<&str>| lines.iter().find(|l| l.wbs.as_deref() == w).unwrap().clone();
        assert_eq!((line(None).current, line(None).undistributed(), line(None).unplanned()), (100_000_000, 15_000_000, 0));
        assert_eq!((line(Some("W1")).current, line(Some("W1")).proposed), (30_000_000, 28_000_000));
        assert_eq!(line(Some("W2")).current, 55_000_000);
        assert_eq!(s.project.budget.log.iter().map(|c| c.number).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(mkperth_sched::funding(&s.project), (95_000_000, 5_000_000));
        assert_eq!(s.project.budget.of(None).unwrap().plan[1].month, Date::from_ymd(2026, 12, 1).unwrap());
        // The pending cut approved; a month set to 0 leaves the plan.
        run(&mut s, "budget-change.edit", json!({"number": 2, "status": "approved"}));
        assert_eq!(mkperth_sched::budget(&s.project, None)[1].current, 28_000_000);
        run(&mut s, "budget.plan", json!({"month": "2026-11", "amount": 0}));
        assert_eq!(s.project.budget.of(None).unwrap().plan.len(), 1);
        // A change is numbered after the highest in the log.
        run(&mut s, "budget-change.delete", json!({"number": 1}));
        run(&mut s, "budget-change.add", json!({"wbs": "W2", "date": "2026-11-20", "amount": 1}));
        assert_eq!(s.project.budget.log.iter().map(|c| c.number).collect::<Vec<_>>(), [2, 3]);
        // A WBS renamed carries its budget and changes; deleted, they go with it.
        run(&mut s, "activity.edit", json!({"id": "W2", "new_id": "W20"}));
        assert!(s.project.budget.of(Some("W20")).is_some());
        assert_eq!(s.project.budget.log[1].wbs.as_deref(), Some("W20"));
        run(&mut s, "activity.delete", json!({"id": "W20"}));
        assert!(s.project.budget.of(Some("W20")).is_none());
        assert_eq!(s.project.budget.log.iter().map(|c| c.number).collect::<Vec<_>>(), [2], "W1's change is left");
        // A source deleted takes its fund.
        run(&mut s, "funding-source.delete", json!({"id": "BANK"}));
        assert_eq!(mkperth_sched::funding(&s.project).0, 25_000_000);
        // Refused, the project untouched.
        let before = s.project.clone();
        assert!(s.run("budget.set", &json!({"wbs": "nope", "original": 1})).is_err());
        assert!(s.run("budget.set", &json!({"original": -1})).is_err());
        assert!(s.run("budget.plan", &json!({"month": "November", "amount": 1})).is_err());
        assert!(s.run("budget-change.add", &json!({"amount": 1})).is_err());
        assert!(s.run("budget-change.add", &json!({"date": "2026-11-20", "amount": 1, "status": "maybe"})).is_err());
        assert!(s.run("budget-change.edit", &json!({"number": 99, "amount": 1})).is_err());
        assert!(s.run("fund.set", &json!({"source": "nope", "amount": 1})).is_err());
        assert!(s.run("funding-source.add", &json!({"id": "OWN", "name": "again"})).is_err());
        assert_eq!(s.project, before);
        // Undo puts the last change back.
        run(&mut s, "edit.undo", json!(null));
        assert!(s.project.budget.sources.iter().any(|x| x.id == "BANK"));
    }
}
