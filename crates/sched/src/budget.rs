//! The money approved for the project and each WBS, and how it is planned to be spent. The
//! original budget is what was first approved; the current adds the approved changes in the
//! budget log, the proposed the pending ones too. A WBS's budget is distributed to the WBS under
//! it: what its children's current budgets don't take is undistributed. The spending plan is
//! the budget month by month: what it doesn't place is unplanned. The funds are what the
//! funding sources give the project: what they don't cover is unfunded. Beside all of it, what
//! the scheduled work is budgeted to cost (the activities' budgeted costs). Money in hundredths
//! of the currency.

use std::collections::{BTreeMap, HashMap, HashSet};

use mkperth_core::{ChangeStatus, Date, Project};

use crate::Schedule;

/// The project's budget, or a WBS's.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BudgetLine {
    /// The WBS (a summary activity, by ID); `None` for the project.
    pub wbs: Option<String>,
    /// How many WBS it sits under: 0 for the project, 1 for a WBS at the top.
    pub depth: usize,
    pub original: i64,
    /// The original and the approved changes.
    pub current: i64,
    /// The current and the pending changes.
    pub proposed: i64,
    /// The current budgets of the WBS right under it.
    pub distributed: i64,
    /// The spending plan's months, summed.
    pub planned: i64,
    /// What its activities are budgeted to cost, when there is a schedule.
    pub scheduled: Option<i64>,
}

impl BudgetLine {
    /// The current budget its WBS don't take: negative when they take more than it has.
    pub fn undistributed(&self) -> i64 {
        self.current.saturating_sub(self.distributed)
    }

    /// The current budget the spending plan doesn't place: negative when it plans more.
    pub fn unplanned(&self) -> i64 {
        self.current.saturating_sub(self.planned)
    }
}

/// The project's budget, then each WBS's in the project's order: every summary activity, and any
/// other activity with a budget or a change of its own.
pub fn budget(project: &Project, schedule: Option<&Schedule>) -> Vec<BudgetLine> {
    let b = &project.budget;
    let parents: HashSet<&str> = project.activities.iter().filter_map(|a| a.parent.as_deref()).collect();
    let own: HashSet<&str> = b.budgets.iter().filter_map(|x| x.wbs.as_deref()).chain(b.log.iter().filter_map(|c| c.wbs.as_deref())).collect();
    let index: HashMap<&str, usize> = project.activities.iter().enumerate().map(|(i, a)| (a.id.as_str(), i)).collect();
    let costs = schedule.map(|s| crate::costs(project, s));
    let line = |wbs: Option<&str>, depth: usize, scheduled: Option<i64>| {
        let original = b.of(wbs).map_or(0, |x| x.original);
        let changes = |status: ChangeStatus| {
            b.log.iter().filter(|c| c.wbs.as_deref() == wbs && c.status == status).fold(0i64, |s, c| s.saturating_add(c.amount))
        };
        let current = original.saturating_add(changes(ChangeStatus::Approved));
        let planned = b.of(wbs).map_or(0, |x| x.plan.iter().fold(0i64, |s, m| s.saturating_add(m.amount)));
        BudgetLine {
            wbs: wbs.map(str::to_string),
            depth,
            original,
            current,
            proposed: current.saturating_add(changes(ChangeStatus::Pending)),
            distributed: 0,
            planned,
            scheduled,
        }
    };
    let mut lines = vec![line(None, 0, schedule.map(|s| crate::project_cost(project, s).budget))];
    // Where each WBS's line is, to give its parent's line its current budget.
    let mut at: HashMap<&str, usize> = HashMap::new();
    for (i, a) in project.activities.iter().enumerate() {
        if !parents.contains(a.id.as_str()) && !own.contains(a.id.as_str()) {
            continue;
        }
        // Its depth: the summaries above it, never further than there are activities.
        let mut depth = 1;
        let mut up = a.parent.as_deref().and_then(|p| index.get(p).copied());
        while let Some(k) = up.filter(|_| depth <= project.activities.len()) {
            depth += 1;
            up = project.activities.get(k).and_then(|x| x.parent.as_deref()).and_then(|p| index.get(p).copied());
        }
        let scheduled = costs.as_ref().and_then(|c| c.get(i)).map(|c| c.budget);
        at.insert(a.id.as_str(), lines.len());
        lines.push(line(Some(a.id.as_str()), depth, scheduled));
    }
    // Each WBS's current budget is distributed from the one above it: its parent, or the project.
    for a in &project.activities {
        let Some(&k) = at.get(a.id.as_str()) else { continue };
        let current = lines.get(k).map_or(0, |l| l.current);
        let above = a.parent.as_deref().and_then(|p| at.get(p).copied()).unwrap_or(0);
        if let Some(l) = lines.get_mut(above) {
            l.distributed = l.distributed.saturating_add(current);
        }
    }
    lines
}

/// What the funding sources give the project, and what of its current budget they don't cover
/// (negative when they give more).
pub fn funding(project: &Project) -> (i64, i64) {
    let funded = project.budget.funds.iter().fold(0i64, |s, f| s.saturating_add(f.amount));
    let current = budget(project, None).first().map_or(0, |l| l.current);
    (funded, current.saturating_sub(funded))
}

/// The scheduled work's budgeted cost month by month, each from its first day: what the
/// spending plan is set against.
pub fn spending_by_month(project: &Project, schedule: &Schedule) -> Vec<(Date, i64)> {
    let mut months: BTreeMap<Date, i64> = BTreeMap::new();
    for (d, c) in crate::spending(project, schedule) {
        let (y, m, _) = d.ymd();
        if let Some(first) = Date::from_ymd(y, m, 1) {
            let slot = months.entry(first).or_insert(0);
            *slot = slot.saturating_add(c);
        }
    }
    months.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use mkperth_core::{Activity, BudgetChange, Budgeting, Calendar, Expense, Fund, SpendingMonth, WbsBudget};

    use super::*;

    fn day(y: i32, m: u32, d: u32) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    fn money(units: i64) -> i64 {
        units * 100
    }

    /// The WBS: W1 over A; W2 over W21 (over B) and C. Budgets first approved: the project
    /// 10 000, W1 4 000, W2 5 000, W21 2 000. The log: #1 the project +1 500 approved; #2 W2 +800
    /// approved; #3 W1 −300 pending; #4 the project +200 not approved; #5 W21 +500 pending.
    ///
    /// - the project: current 10 000 + 1 500 = 11 500 (#4 never counts), proposed 11 500; its
    ///   WBS take W1's 4 000 and W2's 5 800 = 9 800, so 1 700 is undistributed.
    /// - W1: current 4 000, proposed 4 000 − 300 = 3 700; no WBS under it, all undistributed.
    /// - W2: current 5 000 + 800 = 5 800, proposed 5 800; W21 takes 2 000, 3 800 left.
    /// - W21: current 2 000, proposed 2 500.
    ///
    /// The project's spending plan: November 3 000, December 4 000, January 4 500 = 11 500, the
    /// whole current budget; W2's: November 1 000, 4 800 unplanned. Funds: the bank 7 000, the
    /// owner 4 000 = 11 000, 500 unfunded.
    fn project() -> Project {
        let a = |id: &str, parent: Option<&str>| Activity { id: id.into(), duration: 5, parent: parent.map(str::to_string), ..Activity::default() };
        let wbs = |w: Option<&str>, original: i64, plan: &[(u32, i32, i64)]| WbsBudget {
            wbs: w.map(str::to_string),
            original: money(original),
            plan: plan.iter().map(|&(m, y, amount)| SpendingMonth { month: day(y, m, 1), amount: money(amount) }).collect(),
        };
        let change = |number: u32, w: Option<&str>, amount: i64, status: ChangeStatus| BudgetChange {
            number,
            wbs: w.map(str::to_string),
            date: day(2026, 11, 2),
            amount: money(amount),
            status,
            responsible: String::new(),
            reason: String::new(),
        };
        Project {
            name: "budget".into(),
            start: day(2026, 11, 2),
            calendar: Calendar::default(),
            calendars: Vec::new(),
            lag_calendar: Default::default(),
            float_kind: Default::default(),
            critical_by: Default::default(),
            data_date: None,
            progress_mode: Default::default(),
            must_finish_by: None,
            activities: vec![a("W1", None), a("A", Some("W1")), a("W2", None), a("W21", Some("W2")), a("B", Some("W21")), a("C", Some("W2"))],
            relationships: Vec::new(),
            bands: Vec::new(),
            markers: Vec::new(),
            month_marks: Vec::new(),
            resources: Vec::new(),
            roles: Vec::new(),
            assignments: Vec::new(),
            comments: Vec::new(),
            levelling: Default::default(),
            baselines: Vec::new(),
            baseline_in_use: None,
            expenses: Vec::new(),
            cost_accounts: Vec::new(),
            currency: String::new(),
            budget: Budgeting {
                budgets: vec![
                    wbs(None, 10_000, &[(11, 2026, 3_000), (12, 2026, 4_000), (1, 2027, 4_500)]),
                    wbs(Some("W1"), 4_000, &[]),
                    wbs(Some("W2"), 5_000, &[(11, 2026, 1_000)]),
                    wbs(Some("W21"), 2_000, &[]),
                ],
                log: vec![
                    change(1, None, 1_500, ChangeStatus::Approved),
                    change(2, Some("W2"), 800, ChangeStatus::Approved),
                    change(3, Some("W1"), -300, ChangeStatus::Pending),
                    change(4, None, 200, ChangeStatus::NotApproved),
                    change(5, Some("W21"), 500, ChangeStatus::Pending),
                ],
                sources: Vec::new(),
                funds: vec![Fund { source: "BANK".into(), amount: money(7_000) }, Fund { source: "OWN".into(), amount: money(4_000) }],
            },
            extra: Default::default(),
        }
    }

    #[test]
    fn budgets_reconcile_by_hand() {
        let lines = budget(&project(), None);
        let row = |l: &BudgetLine| {
            (l.wbs.clone().unwrap_or_default(), l.depth, l.current / 100, l.proposed / 100, l.undistributed() / 100, l.unplanned() / 100)
        };
        let rows: Vec<_> = lines.iter().map(row).collect();
        assert_eq!(
            rows,
            [
                (String::new(), 0, 11_500, 11_500, 1_700, 0),
                ("W1".into(), 1, 4_000, 3_700, 4_000, 4_000),
                ("W2".into(), 1, 5_800, 5_800, 3_800, 4_800),
                ("W21".into(), 2, 2_000, 2_500, 2_000, 2_000),
            ]
        );
        // Each level reconciles: what a budget distributes and leaves undistributed is all of it.
        for l in &lines {
            assert_eq!(l.distributed + l.undistributed(), l.current);
        }
        assert_eq!(lines[0].original, money(10_000));
        assert_eq!(funding(&project()), (money(11_000), money(500)));
    }

    /// The scheduled work beside the budget: a permit of 1 000 on A, at its start (Monday
    /// 2 November 2026), is W1's and the project's budgeted cost, and November's spending.
    #[test]
    fn the_scheduled_cost_beside_it() {
        let mut p = project();
        p.expenses.push(Expense {
            activity: "A".into(),
            name: "Permit".into(),
            planned: money(1_000),
            accrual: mkperth_core::Accrual::Start,
            ..Expense::default()
        });
        let s = crate::schedule(&p).unwrap();
        let lines = budget(&p, Some(&s));
        assert_eq!(lines[0].scheduled, Some(money(1_000)));
        assert_eq!(lines[1].scheduled, Some(money(1_000)));
        assert_eq!(lines[2].scheduled, Some(0));
        assert_eq!(spending_by_month(&p, &s), [(day(2026, 11, 1), money(1_000))]);
    }

    /// A leaf with a budget of its own is a line too; a WBS whose parent is gone counts under
    /// the project.
    #[test]
    fn odd_wbs() {
        let mut p = project();
        p.budget.budgets.push(WbsBudget { wbs: Some("C".into()), original: money(300), plan: Vec::new() });
        let lines = budget(&p, None);
        let c = lines.iter().find(|l| l.wbs.as_deref() == Some("C")).unwrap();
        assert_eq!((c.depth, c.current), (2, money(300)));
        assert_eq!(lines[2].distributed, money(2_300), "W2 gives W21 and C theirs");
        p.activities[3].parent = Some("gone".into());
        let lines = budget(&p, None);
        assert_eq!(lines[0].distributed, money(4_000 + 5_800 + 2_000));
    }
}
