//! Costs: what isn't a resource's work (expenses on activities, with their accrual) and the
//! accounts costs are charged to, in a hierarchy. Money comes in as numbers (`300.5`) and is
//! kept in hundredths of the currency. Each change is a command with undo.

use mkperth_core::{Accrual, CostAccount, Date, Expense, Price};
use mkperth_i18n::tr;
use serde_json::Value;

use crate::{EngineError, Session};

/// The most expenses or cost accounts a project may hold, and prices a resource may have.
pub(crate) const MAX_ITEMS: usize = 100_000;
const MAX_PRICES: usize = 1000;
/// The longest a name or an ID may be, in characters.
pub(crate) const MAX_TEXT: usize = 200;
/// The most an amount may be, in the currency.
pub(crate) const MAX_MONEY: f64 = 1e13;

pub(crate) fn bad(command: &str, param: &str) -> EngineError {
    EngineError::Value(command.into(), param.into())
}

pub(crate) fn text(params: &Value, command: &str, name: &str) -> Result<Option<String>, EngineError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.chars().count() <= MAX_TEXT => Ok(Some(s.trim().to_string())),
        Some(_) => Err(bad(command, name)),
    }
}

pub(crate) fn required(params: &Value, command: &str, name: &str) -> Result<String, EngineError> {
    text(params, command, name)?.filter(|s| !s.is_empty()).ok_or_else(|| EngineError::Param(command.into(), name.into()))
}

/// An amount of money as hundredths, from a number.
fn money_of(v: &Value) -> Option<i64> {
    v.as_f64().filter(|m| m.is_finite() && (0.0..=MAX_MONEY).contains(m)).map(|m| (m * 100.0).round() as i64)
}

/// An amount given as `name`, if given.
pub(crate) fn money(params: &Value, command: &str, name: &str) -> Result<Option<i64>, EngineError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => money_of(v).map(Some).ok_or_else(|| bad(command, name)),
    }
}

/// A resource's prices (`[{"from": "2026-10-01" or null, "price": 12.5}]`), if given.
pub(crate) fn prices(params: &Value, command: &str) -> Result<Option<Vec<Price>>, EngineError> {
    if let Some(p) = params.get("price").filter(|v| !v.is_null()) {
        return Ok(Some(vec![Price { from: None, per_unit: money_of(p).ok_or_else(|| bad(command, "price"))? }]));
    }
    match params.get("prices") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(items)) if items.len() <= MAX_PRICES => items
            .iter()
            .map(|i| {
                let from = match i.get("from") {
                    None | Some(Value::Null) => None,
                    Some(d) => Some(d.as_str().and_then(Date::parse).ok_or_else(|| bad(command, "prices"))?),
                };
                let per_unit = i.get("price").and_then(money_of).ok_or_else(|| bad(command, "prices"))?;
                Ok(Price { from, per_unit })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        Some(_) => Err(bad(command, "prices")),
    }
}

fn accrual(params: &Value, command: &str) -> Result<Option<Accrual>, EngineError> {
    text(params, command, "accrual")?.map(|a| Accrual::from_code(&a).ok_or_else(|| bad(command, "accrual"))).transpose()
}

impl Session {
    /// The cost commands; `None` when `id` isn't one of them.
    pub(crate) fn cost_command(&mut self, id: &str, params: &Value) -> Option<Result<(), EngineError>> {
        Some(match id {
            "expense.add" => self.add_expense(params),
            "expense.edit" => self.edit_expense(params),
            "expense.delete" => self.delete_expense(params),
            "cost-account.add" => self.add_cost_account(params),
            "cost-account.delete" => self.delete_cost_account(params),
            _ => return None,
        })
    }

    /// An expense on `activity`: its `name` (unique on the activity), `planned` and `actual`
    /// cost, `remaining` (else the planned less the actual), `accrual` (start, end, uniform)
    /// and the cost `account` it is charged to.
    fn add_expense(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "expense.add";
        let activity = required(params, C, "activity")?;
        let name = required(params, C, "name")?;
        let (planned, actual, remaining) = (money(params, C, "planned")?, money(params, C, "actual")?, money(params, C, "remaining")?);
        let accrual = accrual(params, C)?.unwrap_or_default();
        let account = text(params, C, "account")?.filter(|s| !s.is_empty());
        self.change(tr!("undo-add-expense", activity = activity.as_str()), |p| {
            if !p.activities.iter().any(|a| a.id == activity) {
                return Err(EngineError::UnknownActivity(activity.clone()));
            }
            if p.expenses.len() >= MAX_ITEMS || p.expenses.iter().any(|e| e.activity == activity && e.name == name) {
                return Err(bad(C, "name"));
            }
            if let Some(a) = &account
                && !p.cost_accounts.iter().any(|c| c.id == *a)
            {
                return Err(bad(C, "account"));
            }
            p.expenses.push(Expense {
                activity: activity.clone(),
                name: name.clone(),
                account,
                planned: planned.unwrap_or(0),
                actual: actual.unwrap_or(0),
                remaining,
                accrual,
            });
            Ok(())
        })
    }

    /// Changes the expense `name` on `activity`: `new_name`, `planned`, `actual`, `remaining`
    /// (`null` for the planned less the actual), `accrual`, `account` (`null` or empty for none).
    fn edit_expense(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "expense.edit";
        let activity = required(params, C, "activity")?;
        let name = required(params, C, "name")?;
        let new_name = text(params, C, "new_name")?.filter(|s| !s.is_empty());
        let (planned, actual) = (money(params, C, "planned")?, money(params, C, "actual")?);
        let remaining = match params.get("remaining") {
            None => None,
            Some(Value::Null) => Some(None),
            Some(v) => Some(Some(money_of(v).ok_or_else(|| bad(C, "remaining"))?)),
        };
        let accrual = accrual(params, C)?;
        let account = match params.get("account") {
            None => None,
            Some(Value::Null) => Some(None),
            Some(_) => Some(text(params, C, "account")?.filter(|s| !s.is_empty())),
        };
        self.change(tr!("undo-edit-expense", activity = activity.as_str()), |p| {
            if let Some(Some(a)) = &account
                && !p.cost_accounts.iter().any(|c| c.id == *a)
            {
                return Err(bad(C, "account"));
            }
            if let Some(n) = &new_name
                && *n != name
                && p.expenses.iter().any(|e| e.activity == activity && e.name == *n)
            {
                return Err(bad(C, "new_name"));
            }
            let e = p.expenses.iter_mut().find(|e| e.activity == activity && e.name == name).ok_or_else(|| bad(C, "name"))?;
            if let Some(n) = new_name {
                e.name = n;
            }
            if let Some(m) = planned {
                e.planned = m;
            }
            if let Some(m) = actual {
                e.actual = m;
            }
            if let Some(m) = remaining {
                e.remaining = m;
            }
            if let Some(a) = accrual {
                e.accrual = a;
            }
            if let Some(a) = account {
                e.account = a;
            }
            Ok(())
        })
    }

    fn delete_expense(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "expense.delete";
        let activity = required(params, C, "activity")?;
        let name = required(params, C, "name")?;
        self.change(tr!("undo-delete-expense", activity = activity.as_str()), |p| {
            let before = p.expenses.len();
            p.expenses.retain(|e| !(e.activity == activity && e.name == name));
            if p.expenses.len() == before {
                return Err(bad(C, "name"));
            }
            Ok(())
        })
    }

    /// A cost account: `id` and `name` (required), under the account `parent`.
    fn add_cost_account(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "cost-account.add";
        let id = required(params, C, "id")?;
        let name = required(params, C, "name")?;
        let parent = text(params, C, "parent")?.filter(|s| !s.is_empty());
        self.change(tr!("undo-add-cost-account", id = id.as_str()), |p| {
            if p.cost_accounts.len() >= MAX_ITEMS || p.cost_accounts.iter().any(|c| c.id == id) {
                return Err(bad(C, "id"));
            }
            if let Some(parent) = &parent
                && !p.cost_accounts.iter().any(|c| c.id == *parent)
            {
                return Err(bad(C, "parent"));
            }
            p.cost_accounts.push(CostAccount { id: id.clone(), name, parent });
            Ok(())
        })
    }

    /// Deletes the cost account `id`: the accounts under it go under its own parent, and the
    /// expenses charged to it are charged to none.
    fn delete_cost_account(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "cost-account.delete";
        let id = required(params, C, "id")?;
        self.change(tr!("undo-delete-cost-account", id = id.as_str()), |p| {
            let k = p.cost_accounts.iter().position(|c| c.id == id).ok_or_else(|| bad(C, "id"))?;
            let gone = p.cost_accounts.remove(k);
            for c in p.cost_accounts.iter_mut().filter(|c| c.parent.as_deref() == Some(id.as_str())) {
                c.parent = gone.parent.clone();
            }
            for e in p.expenses.iter_mut().filter(|e| e.account.as_deref() == Some(id.as_str())) {
                e.account = None;
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

    /// A10 takes 5 days of 8 hours; Ana on it for 40 hours at 25.00, then 30.00 from its third
    /// day (Thursday 5 November 2026: A10 is the 3rd to the 9th, the 2nd a holiday): 16 × 25 +
    /// 24 × 30 = 1 120.00. A permit of 450.50 at its start: 1 570.50 budgeted, and so its summary.
    #[test]
    fn prices_expenses_and_accounts() {
        let mut s = Session::default();
        run(&mut s, "cost-account.add", json!({"id": "1", "name": "Site"}));
        run(&mut s, "cost-account.add", json!({"id": "1.1", "name": "Permits", "parent": "1"}));
        run(
            &mut s,
            "resource.add",
            json!({"id": "ANA", "name": "Ana", "prices": [{"from": null, "price": 25}, {"from": "2026-11-05", "price": 30}]}),
        );
        run(&mut s, "assignment.add", json!({"activity": "A10", "resource": "ANA"}));
        run(&mut s, "expense.add", json!({"activity": "A10", "name": "Permit", "planned": 450.5, "accrual": "start", "account": "1.1"}));
        let k = s.project.activities.iter().position(|a| a.id == "A10").unwrap();
        let w = s.project.activities.iter().position(|a| a.id == "W1").unwrap();
        let costs = mkperth_sched::costs(&s.project, s.schedule.as_ref().unwrap());
        assert_eq!(costs[k].budget, 157_050);
        assert_eq!(costs[w].budget, 157_050);
        run(&mut s, "expense.edit", json!({"activity": "A10", "name": "Permit", "actual": 450.5, "new_name": "Permit (council)"}));
        assert_eq!(s.project.expenses[0].remaining_cost(), 0);
        // The account deleted: its child goes under its parent's place, the expense to none.
        run(&mut s, "cost-account.delete", json!({"id": "1"}));
        assert_eq!(s.project.cost_accounts[0].parent, None);
        run(&mut s, "cost-account.delete", json!({"id": "1.1"}));
        assert_eq!(s.project.expenses[0].account, None);
        // Renamed and deleted activities carry and take their expenses.
        run(&mut s, "activity.edit", json!({"id": "A10", "new_id": "A11"}));
        assert_eq!(s.project.expenses[0].activity, "A11");
        run(&mut s, "activity.delete", json!({"id": "A11"}));
        assert!(s.project.expenses.is_empty());
        // Refused.
        let before = s.project.clone();
        assert!(s.run("expense.add", &json!({"activity": "A20", "name": "x", "planned": -1})).is_err());
        assert!(s.run("expense.add", &json!({"activity": "nope", "name": "x"})).is_err());
        assert!(s.run("expense.add", &json!({"activity": "A20", "name": "x", "accrual": "monthly"})).is_err());
        assert!(s.run("resource.edit", &json!({"id": "ANA", "prices": [{"from": "soon", "price": 1}]})).is_err());
        assert!(s.run("project.options", &json!({"currency": "a currency far too long"})).is_err());
        assert_eq!(s.project, before);
        run(&mut s, "project.options", json!({"currency": "R$"}));
        assert_eq!(s.project.currency, "R$");
    }
}
