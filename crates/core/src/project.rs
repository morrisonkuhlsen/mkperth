//! A project: its activities, the relationships between them, its calendar and the date its
//! schedule starts from.

use std::collections::BTreeMap;

use crate::{Calendar, Date};

/// What a file format holds that the model doesn't: each key namespaced by its format
/// (`perth.progress`), each value the field as that format wrote it (JSON text for Perth's).
/// Readers fill it, writers of the same format read it back, everyone else carries it along.
pub type Extra = BTreeMap<String, String>;

/// How a relationship ties its successor to its predecessor (the four kinds of the
/// precedence diagramming method).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RelationshipType {
    /// The successor starts after the predecessor finishes.
    #[default]
    FinishToStart,
    /// The successor starts after the predecessor starts.
    StartToStart,
    /// The successor finishes after the predecessor finishes.
    FinishToFinish,
    /// The successor finishes after the predecessor starts.
    StartToFinish,
}

impl RelationshipType {
    /// `FS`, `SS`, `FF` or `SF`.
    pub fn code(self) -> &'static str {
        match self {
            RelationshipType::FinishToStart => "FS",
            RelationshipType::StartToStart => "SS",
            RelationshipType::FinishToFinish => "FF",
            RelationshipType::StartToFinish => "SF",
        }
    }

    pub fn from_code(code: &str) -> Option<RelationshipType> {
        Some(match code.to_ascii_uppercase().as_str() {
            "FS" => RelationshipType::FinishToStart,
            "SS" => RelationshipType::StartToStart,
            "FF" => RelationshipType::FinishToFinish,
            "SF" => RelationshipType::StartToFinish,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    pub predecessor: String,
    pub successor: String,
    pub kind: RelationshipType,
    /// Working days added to the tie (negative: a lead).
    pub lag: i64,
    /// Working minutes, when the lag isn't whole days; it takes the place of `lag`.
    pub lag_minutes: Option<i64>,
    /// How the lag counts: working time (the default), elapsed time, or a share of the
    /// predecessor's duration.
    pub lag_kind: LagKind,
}

/// How a relationship's lag counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LagKind {
    /// Working time on the lag calendar: `lag`, or `lag_minutes`.
    #[default]
    Working,
    /// Elapsed time, every hour of every day: `lag_minutes` (or `lag` days of 24 hours).
    Elapsed,
    /// A share of the predecessor's duration, in hundredths of a percent (5 000 is half); `lag`
    /// and `lag_minutes` are not used.
    Percent(i64),
}

/// What an activity is, as far as scheduling cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ActivityKind {
    /// Work that takes its duration.
    #[default]
    Task,
    /// A moment that starts something: shown on the morning of its day.
    StartMilestone,
    /// A moment that ends something: shown on the evening of the working day before.
    FinishMilestone,
}

impl ActivityKind {
    pub fn is_milestone(self) -> bool {
        self != ActivityKind::Task
    }
}

/// A date an activity is held to, beyond its relationships.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Constraint {
    /// Starts no earlier than the morning of the date: moves the early dates only.
    StartOnOrAfter(Date),
    /// Starts no later than the morning of the date: moves the late dates only.
    StartOnOrBefore(Date),
    /// Finishes no earlier than the evening of the date: moves the early dates only.
    FinishOnOrAfter(Date),
    /// Finishes no later than the evening of the date: moves the late dates only.
    FinishOnOrBefore(Date),
    /// Starts on the date: no earlier for the early dates, no later for the late ones.
    StartOn(Date),
    /// Finishes on the date, the same way.
    FinishOn(Date),
    /// Starts on the date whatever its predecessors say, early and late.
    MandatoryStart(Date),
    /// Finishes on the date whatever its predecessors say, early and late.
    MandatoryFinish(Date),
    /// As late as its free float allows, without delaying any successor.
    AsLateAsPossible,
}

impl Constraint {
    /// Whether it bounds the start (`Some(true)`) or the finish (`Some(false)`); none for as late
    /// as possible.
    pub fn on_start(self) -> Option<bool> {
        match self {
            Constraint::StartOnOrAfter(_) | Constraint::StartOnOrBefore(_) | Constraint::StartOn(_) | Constraint::MandatoryStart(_) => Some(true),
            Constraint::FinishOnOrAfter(_) | Constraint::FinishOnOrBefore(_) | Constraint::FinishOn(_) | Constraint::MandatoryFinish(_) => {
                Some(false)
            }
            Constraint::AsLateAsPossible => None,
        }
    }

    /// Whether `secondary` may stand beside `primary`: a dated primary, and a secondary that is
    /// neither mandatory nor as late as possible, on the other end of the activity (a start's
    /// with a finish's, as a window to work in).
    pub fn pairs(primary: Constraint, secondary: Constraint) -> bool {
        let soft = !matches!(secondary, Constraint::MandatoryStart(_) | Constraint::MandatoryFinish(_) | Constraint::AsLateAsPossible);
        match (primary.on_start(), secondary.on_start()) {
            (Some(a), Some(b)) => soft && a != b,
            _ => false,
        }
    }
}

/// How an activity's duration, its assignments' units and their units an hour hold together
/// (units = units an hour × the activity's working hours) when one of them is changed: the two
/// the type names stay, the third is worked out again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DurationType {
    /// A new duration changes the units; new units change the units an hour.
    #[default]
    FixedDurationAndRate,
    /// A new duration or new units change the units an hour.
    FixedDurationAndUnits,
    /// A new duration changes the units; new units change the duration.
    FixedRate,
    /// A new duration changes the units an hour; new units or units an hour change the duration.
    FixedUnits,
}

impl DurationType {
    pub const ALL: [DurationType; 4] =
        [DurationType::FixedDurationAndRate, DurationType::FixedDurationAndUnits, DurationType::FixedRate, DurationType::FixedUnits];

    /// The commands' word for it.
    pub fn code(self) -> &'static str {
        match self {
            DurationType::FixedDurationAndRate => "fixed-duration-and-rate",
            DurationType::FixedDurationAndUnits => "fixed-duration-and-units",
            DurationType::FixedRate => "fixed-rate",
            DurationType::FixedUnits => "fixed-units",
        }
    }

    pub fn from_code(code: &str) -> Option<DurationType> {
        DurationType::ALL.into_iter().find(|d| d.code() == code)
    }

    /// Whether a new duration keeps the units (and changes the units an hour).
    pub fn keeps_units_on_duration(self) -> bool {
        matches!(self, DurationType::FixedDurationAndUnits | DurationType::FixedUnits)
    }

    /// Whether new units (or, for fixed units, a new rate) move the duration.
    pub fn moves_duration(self) -> bool {
        matches!(self, DurationType::FixedRate | DurationType::FixedUnits)
    }
}

/// How much an activity matters when resources are levelled: of two wanting a resource on the
/// same day, the higher goes first (P6's levelling priority).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub enum LevellingPriority {
    Top,
    High,
    #[default]
    Normal,
    Low,
    Lowest,
}

impl LevellingPriority {
    /// From the highest to the lowest.
    pub const ALL: [LevellingPriority; 5] =
        [LevellingPriority::Top, LevellingPriority::High, LevellingPriority::Normal, LevellingPriority::Low, LevellingPriority::Lowest];

    /// The commands' word for it.
    pub fn code(self) -> &'static str {
        match self {
            LevellingPriority::Top => "top",
            LevellingPriority::High => "high",
            LevellingPriority::Normal => "normal",
            LevellingPriority::Low => "low",
            LevellingPriority::Lowest => "lowest",
        }
    }

    pub fn from_code(code: &str) -> Option<LevellingPriority> {
        LevellingPriority::ALL.into_iter().find(|p| p.code() == code)
    }
}

/// What decides, among activities wanting an over-allocated resource on the same day, which keep
/// it: each in turn, the next only to break a tie (the activity's ID breaks the last).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LevelBy {
    /// The higher levelling priority first.
    Priority,
    /// The smaller total float first.
    TotalFloat,
    /// The earlier early start first.
    EarlyStart,
    /// The earlier late start first.
    LateStart,
    /// The shorter first.
    Duration,
    /// By the activity code of this name: the value earlier in the code's list of values first,
    /// an activity without one last (P6's activity codes as levelling priorities).
    Code(String),
}

impl LevelBy {
    /// The ways that need nothing named.
    pub const ALL: [LevelBy; 5] = [LevelBy::Priority, LevelBy::TotalFloat, LevelBy::EarlyStart, LevelBy::LateStart, LevelBy::Duration];

    /// The commands' word for it: `priority`, …, or `code:` and the code's name.
    pub fn code(&self) -> String {
        match self {
            LevelBy::Priority => "priority".into(),
            LevelBy::TotalFloat => "total-float".into(),
            LevelBy::EarlyStart => "early-start".into(),
            LevelBy::LateStart => "late-start".into(),
            LevelBy::Duration => "duration".into(),
            LevelBy::Code(name) => format!("code:{name}"),
        }
    }

    pub fn from_code(code: &str) -> Option<LevelBy> {
        match code.strip_prefix("code:") {
            Some(name) if !name.trim().is_empty() => Some(LevelBy::Code(name.to_string())),
            Some(_) => None,
            None => LevelBy::ALL.into_iter().find(|b| b.code() == code),
        }
    }
}

/// How resources are levelled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevellingOptions {
    /// Delay an activity only as far as its total float allows: the project's finish stays, and
    /// what can't be resolved within float stays over-allocated.
    pub within_float: bool,
    /// The resources levelled, by their IDs; every one (but materials) when empty.
    pub resources: Vec<String>,
    /// Which activities keep a resource first.
    pub order: Vec<LevelBy>,
}

impl Default for LevellingOptions {
    fn default() -> Self {
        LevellingOptions { within_float: false, resources: Vec::new(), order: vec![LevelBy::Priority, LevelBy::TotalFloat, LevelBy::EarlyStart] }
    }
}

/// An activity as a baseline froze it: its dates and its duration then.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineActivity {
    pub id: String,
    pub start: Date,
    pub finish: Date,
    /// Working minutes.
    pub duration: i64,
    /// Its budgeted cost then, in hundredths of the currency.
    pub cost: i64,
}

/// The plan frozen, to measure the schedule against: each activity's dates when it was taken.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Baseline {
    /// Unique among the project's baselines.
    pub name: String,
    pub activities: Vec<BaselineActivity>,
}

/// Three estimates of an activity's duration, in hundredths of a working day (`UNIT` is a day),
/// for PERT's analysis: they never move its dates, which its duration sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pert {
    pub optimistic: i64,
    pub most_likely: i64,
    pub pessimistic: i64,
}

impl Pert {
    /// The expected duration, (O + 4M + P) / 6, in hundredths of a day.
    pub fn expected(&self) -> f64 {
        (self.optimistic as f64 + 4.0 * self.most_likely as f64 + self.pessimistic as f64) / 6.0
    }

    /// The standard deviation, (P − O) / 6, in hundredths of a day.
    pub fn sigma(&self) -> f64 {
        (self.pessimistic - self.optimistic) as f64 / 6.0
    }

    /// Whether the estimates are in order, none below nothing: O ≤ M ≤ P.
    pub fn ordered(&self) -> bool {
        0 <= self.optimistic && self.optimistic <= self.most_likely && self.most_likely <= self.pessimistic
    }
}

/// Where levelling held an activity: to start on or after `start`, so that `resource` stays
/// within its limit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Levelled {
    pub start: Date,
    pub resource: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Activity {
    /// Unique within the project (P6's Activity ID).
    pub id: String,
    pub name: String,
    pub kind: ActivityKind,
    /// Working days. A milestone takes none, whatever this says; a task of 0 is shown as a
    /// start milestone.
    pub duration: i64,
    /// Working minutes, when the duration isn't whole days (from hours); it takes the place of
    /// `duration`.
    pub duration_minutes: Option<i64>,
    /// The day work began, once it has.
    pub actual_start: Option<Date>,
    /// The day work ended, once it has.
    pub actual_finish: Option<Date>,
    /// Working minutes left for an activity under way; when `None`, its duration less the work
    /// since its actual start up to the data date.
    pub remaining_minutes: Option<i64>,
    /// The day it is expected to finish: its work (what is left, once started) runs from its
    /// start to the evening of that day.
    pub expected_finish: Option<Date>,
    pub constraint: Option<Constraint>,
    /// A second constraint beside the first: typically a start on or after with a finish on or
    /// before. Both hold.
    pub secondary_constraint: Option<Constraint>,
    /// The name of the calendar it works on; the project's when `None`.
    pub calendar: Option<String>,
    /// The activity it sits under in the WBS. An activity with children is a summary: its dates
    /// come from theirs, and it takes no part in the network itself.
    pub parent: Option<String>,
    /// Free text: what the team keeps about it.
    pub notes: String,
    /// Which of its duration, its assignments' units and their units an hour hold when one
    /// changes.
    pub duration_type: DurationType,
    /// Its resources each work their own part on their own calendar, and it finishes when the
    /// last of them does (P6's resource dependent activities); else it works its duration on
    /// its calendar, whoever is on it.
    pub resource_dependent: bool,
    /// Which of two activities keeps a resource both want on a day when resources are levelled.
    pub levelling_priority: LevellingPriority,
    /// Where the last levelling held it; it starts no earlier until levelling is run again or
    /// cleared.
    pub levelled: Option<Levelled>,
    /// Its three estimates for PERT's analysis, when it has them.
    pub pert: Option<Pert>,
    /// How much of its work is done, as those doing it judge it (P6's physical percent
    /// complete), in hundredths of a percent: 0 to 10 000.
    pub percent_complete: Option<i64>,
    /// Its value of each activity code, by the code's name: one value per code.
    pub codes: BTreeMap<String, String>,
    /// Its value of each user-defined field, by the field's name.
    pub fields: BTreeMap<String, FieldValue>,
    /// Who answers for it: a node of the project's OBS, by name (P6 keeps one on each WBS).
    pub responsible: Option<String>,
    pub extra: Extra,
}

impl Activity {
    /// The working days it takes: none for a milestone; a part day counts as one.
    pub fn working_days(&self) -> i64 {
        if self.kind.is_milestone() {
            0
        } else if let Some(m) = self.duration_minutes {
            if m > 0 { m.saturating_add(479) / 480 } else { 0 }
        } else {
            self.duration
        }
    }

    /// The working minutes it takes on a calendar of `day_minutes` a day: none for a milestone.
    pub fn working_minutes(&self, day_minutes: i64) -> i64 {
        if self.kind.is_milestone() { 0 } else { self.duration_minutes.unwrap_or_else(|| self.duration.saturating_mul(day_minutes)) }
    }
}

/// Units are kept in hundredths, so 7.5 hours or 0.25 m³ are whole numbers: `UNIT` is one.
pub const UNIT: i64 = 100;

/// What a resource is, as P6 has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ResourceKind {
    /// People: their work is counted in hours.
    #[default]
    Labor,
    /// Equipment and the like: also counted in hours.
    Nonlabor,
    /// What is used up, counted in its own unit (m³, t, bags).
    Material,
}

impl ResourceKind {
    /// `labor`, `nonlabor` or `material`: the commands' and files' word for it.
    pub fn code(self) -> &'static str {
        match self {
            ResourceKind::Labor => "labor",
            ResourceKind::Nonlabor => "nonlabor",
            ResourceKind::Material => "material",
        }
    }

    pub fn from_code(code: &str) -> Option<ResourceKind> {
        Some(match code.to_ascii_lowercase().replace(['-', ' '], "").as_str() {
            "labor" | "labour" => ResourceKind::Labor,
            "nonlabor" | "nonlabour" | "equipment" => ResourceKind::Nonlabor,
            "material" => ResourceKind::Material,
            _ => return None,
        })
    }
}

/// Who or what does the work: a person, a crew, a machine, a material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    /// Unique among the resources (P6's Resource ID).
    pub id: String,
    pub name: String,
    pub kind: ResourceKind,
    /// A material's unit of measure (`m³`); hours for the others, so empty.
    pub unit: String,
    /// The calendar it works on; the project's when `None`.
    pub calendar: Option<String>,
    /// The most it does at once, in hundredths of a unit per hour: `UNIT` is one person (or
    /// machine) full time, `2 * UNIT` two of them.
    pub max_units: i64,
    /// The roles it can fill, by their IDs.
    pub roles: Vec<String>,
    /// Its price per unit (an hour, or a material's unit) over time, by the day each takes
    /// effect; the first holds before any.
    pub prices: Vec<Price>,
    pub extra: Extra,
}

/// A price per unit, in hundredths of the currency (cents), from a day on (`None`: from the
/// start of time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Price {
    pub from: Option<Date>,
    pub per_unit: i64,
}

impl Resource {
    /// Its price per unit on `day`: the last to take effect by then, else the first.
    pub fn price_on(&self, day: Date) -> i64 {
        let mut chosen: Option<&Price> = None;
        for p in &self.prices {
            if p.from.is_none_or(|f| f <= day) && chosen.is_none_or(|c| c.from <= p.from) {
                chosen = Some(p);
            }
        }
        chosen.or(self.prices.first()).map_or(0, |p| p.per_unit)
    }
}

/// When an expense's cost is counted: all on the activity's first day, all on its last, or
/// spread evenly over its working days.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Accrual {
    Start,
    End,
    #[default]
    Uniform,
}

impl Accrual {
    pub const ALL: [Accrual; 3] = [Accrual::Start, Accrual::End, Accrual::Uniform];

    /// The commands' word for it.
    pub fn code(self) -> &'static str {
        match self {
            Accrual::Start => "start",
            Accrual::End => "end",
            Accrual::Uniform => "uniform",
        }
    }

    pub fn from_code(code: &str) -> Option<Accrual> {
        Accrual::ALL.into_iter().find(|a| a.code() == code)
    }
}

/// A cost of an activity that isn't a resource's work: a permit, a rental, freight. Money in
/// hundredths of the currency.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Expense {
    /// The activity's ID.
    pub activity: String,
    /// Unique among the activity's expenses.
    pub name: String,
    /// The cost account it is charged to, by ID.
    pub account: Option<String>,
    pub planned: i64,
    pub actual: i64,
    /// What is left; when `None`, the planned cost less the actual.
    pub remaining: Option<i64>,
    pub accrual: Accrual,
}

impl Expense {
    /// What is left: as given, else the planned less the actual (never below nothing).
    pub fn remaining_cost(&self) -> i64 {
        self.remaining.unwrap_or_else(|| self.planned.saturating_sub(self.actual).max(0))
    }
}

/// An account costs are charged to, in a hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CostAccount {
    pub id: String,
    pub name: String,
    /// The account it sits under, by ID.
    pub parent: Option<String>,
}

/// What became of a proposed change to a budget: only an approved one counts in the current
/// budget; a pending one counts in the proposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ChangeStatus {
    #[default]
    Pending,
    Approved,
    NotApproved,
}

impl ChangeStatus {
    pub const ALL: [ChangeStatus; 3] = [ChangeStatus::Pending, ChangeStatus::Approved, ChangeStatus::NotApproved];

    /// The commands' word for it.
    pub fn code(self) -> &'static str {
        match self {
            ChangeStatus::Pending => "pending",
            ChangeStatus::Approved => "approved",
            ChangeStatus::NotApproved => "not-approved",
        }
    }

    pub fn from_code(code: &str) -> Option<ChangeStatus> {
        ChangeStatus::ALL.into_iter().find(|s| s.code() == code)
    }
}

/// A change to the project's budget or a WBS's, in the budget log. Money in hundredths of the
/// currency; a cut is negative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetChange {
    /// Its number in the log: from 1, each after the highest there.
    pub number: u32,
    /// The WBS (a summary activity, by ID); `None` for the project.
    pub wbs: Option<String>,
    pub date: Date,
    pub amount: i64,
    pub status: ChangeStatus,
    pub responsible: String,
    pub reason: String,
}

/// What a month of the spending plan is to spend, from its first day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpendingMonth {
    /// The month's first day.
    pub month: Date,
    pub amount: i64,
}

/// The money first approved for the project or a WBS, and how it is planned to be spent.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WbsBudget {
    /// The WBS (a summary activity, by ID); `None` for the project.
    pub wbs: Option<String>,
    pub original: i64,
    /// The spending plan, month by month, in date order.
    pub plan: Vec<SpendingMonth>,
}

/// Where money comes from: a loan, a grant, the owner's capital; in a hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FundingSource {
    pub id: String,
    pub name: String,
    /// The source it sits under, by ID.
    pub parent: Option<String>,
}

/// An amount a funding source gives the project.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Fund {
    /// The funding source, by ID.
    pub source: String,
    pub amount: i64,
}

/// The money approved and planned: the budgets, their log of changes, the funding sources and
/// what each gives the project.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Budgeting {
    pub budgets: Vec<WbsBudget>,
    pub log: Vec<BudgetChange>,
    pub sources: Vec<FundingSource>,
    pub funds: Vec<Fund>,
}

impl Budgeting {
    /// The project's budget (`None`) or a WBS's, if it has one.
    pub fn of(&self, wbs: Option<&str>) -> Option<&WbsBudget> {
        self.budgets.iter().find(|b| b.wbs.as_deref() == wbs)
    }

    /// Nothing approved, planned or funded.
    pub fn is_empty(&self) -> bool {
        self == &Budgeting::default()
    }
}

/// How an activity's earned value is measured: by its percent complete judged (physical), by
/// its duration or its units spent, or by fixed shares: nothing until it finishes (0/100), half
/// when it starts (50/50), or a given share when it starts (in hundredths of a percent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EvTechnique {
    #[default]
    Physical,
    Duration,
    Units,
    ZeroHundred,
    FiftyFifty,
    Custom(i64),
}

impl EvTechnique {
    /// The commands' word for it; a custom share is given beside it.
    pub fn code(self) -> &'static str {
        match self {
            EvTechnique::Physical => "physical",
            EvTechnique::Duration => "duration",
            EvTechnique::Units => "units",
            EvTechnique::ZeroHundred => "0-100",
            EvTechnique::FiftyFifty => "50-50",
            EvTechnique::Custom(_) => "custom",
        }
    }

    /// The technique for `code`; a custom one starts with `share`.
    pub fn from_code(code: &str, share: i64) -> Option<EvTechnique> {
        Some(match code {
            "physical" => EvTechnique::Physical,
            "duration" => EvTechnique::Duration,
            "units" => EvTechnique::Units,
            "0-100" => EvTechnique::ZeroHundred,
            "50-50" => EvTechnique::FiftyFifty,
            "custom" => EvTechnique::Custom(share.clamp(0, 10_000)),
            _ => return None,
        })
    }
}

/// How the cost to complete is estimated: what is left as the schedule prices it, or the budget
/// still to earn at the performance so far: as planned (a factor of 1), at the cost performance
/// (1/CPI), or at cost and schedule performance together (1/(CPI × SPI)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EtcTechnique {
    #[default]
    Remaining,
    Planned,
    Cpi,
    CpiSpi,
}

impl EtcTechnique {
    pub const ALL: [EtcTechnique; 4] = [EtcTechnique::Remaining, EtcTechnique::Planned, EtcTechnique::Cpi, EtcTechnique::CpiSpi];

    pub fn code(self) -> &'static str {
        match self {
            EtcTechnique::Remaining => "remaining",
            EtcTechnique::Planned => "pf-1",
            EtcTechnique::Cpi => "cpi",
            EtcTechnique::CpiSpi => "cpi-spi",
        }
    }

    pub fn from_code(code: &str) -> Option<EtcTechnique> {
        EtcTechnique::ALL.into_iter().find(|t| t.code() == code)
    }
}

/// A financial period: the stretch of days performance is stored for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinancialPeriod {
    /// Unique among the periods.
    pub name: String,
    pub start: Date,
    pub finish: Date,
}

/// What an activity did in one financial period, its own amounts (not to date), as P6 keeps
/// them: its planned and earned value in the period, its actual cost by kind (labour, non-labour,
/// material, expenses) and its actual units of labour and non-labour work. Money in hundredths
/// of the currency, units in hundredths. To date is the sum over the periods up to it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PeriodRecord {
    pub period: String,
    pub activity: String,
    pub planned: i64,
    pub earned: i64,
    pub labor_cost: i64,
    pub nonlabor_cost: i64,
    pub material_cost: i64,
    pub expense_cost: i64,
    pub labor_units: i64,
    pub nonlabor_units: i64,
}

impl PeriodRecord {
    /// The actual cost in the period, every kind together.
    pub fn actual(&self) -> i64 {
        self.labor_cost.saturating_add(self.nonlabor_cost).saturating_add(self.material_cost).saturating_add(self.expense_cost)
    }
}

/// Earned value: how it is measured and estimated, the financial periods and what was stored
/// for each.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EarnedValue {
    pub technique: EvTechnique,
    pub etc: EtcTechnique,
    /// In date order.
    pub periods: Vec<FinancialPeriod>,
    pub records: Vec<PeriodRecord>,
}

impl EarnedValue {
    /// Nothing set, no period, nothing stored.
    pub fn is_empty(&self) -> bool {
        self == &EarnedValue::default()
    }
}

/// An activity code: a way to classify activities (a phase, an area, a responsible), with the
/// values it can take, in a hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActivityCode {
    /// Unique among the codes.
    pub name: String,
    pub values: Vec<CodeValue>,
}

/// A value of an activity code: its short code, what it means, and the value it sits under.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CodeValue {
    /// Unique within its code.
    pub code: String,
    pub description: String,
    pub parent: Option<String>,
}

/// What a user-defined field holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FieldKind {
    #[default]
    Text,
    /// A number, kept in millionths.
    Number,
    Integer,
    /// Money, in hundredths of the currency.
    Cost,
    Date,
    /// A traffic light: red, yellow, green or blue.
    Indicator,
}

impl FieldKind {
    pub const ALL: [FieldKind; 6] = [FieldKind::Text, FieldKind::Number, FieldKind::Integer, FieldKind::Cost, FieldKind::Date, FieldKind::Indicator];

    /// The commands' word for it.
    pub fn code(self) -> &'static str {
        match self {
            FieldKind::Text => "text",
            FieldKind::Number => "number",
            FieldKind::Integer => "integer",
            FieldKind::Cost => "cost",
            FieldKind::Date => "date",
            FieldKind::Indicator => "indicator",
        }
    }

    pub fn from_code(code: &str) -> Option<FieldKind> {
        FieldKind::ALL.into_iter().find(|k| k.code() == code)
    }
}

/// A traffic light's colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Indicator {
    Red,
    Yellow,
    Green,
    Blue,
}

impl Indicator {
    pub const ALL: [Indicator; 4] = [Indicator::Red, Indicator::Yellow, Indicator::Green, Indicator::Blue];

    pub fn code(self) -> &'static str {
        match self {
            Indicator::Red => "red",
            Indicator::Yellow => "yellow",
            Indicator::Green => "green",
            Indicator::Blue => "blue",
        }
    }

    pub fn from_code(code: &str) -> Option<Indicator> {
        Indicator::ALL.into_iter().find(|i| i.code() == code)
    }
}

/// A user-defined field: its name and what it holds.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UserField {
    /// Unique among the fields.
    pub name: String,
    pub kind: FieldKind,
}

/// An activity's value of a user-defined field, of the field's kind.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FieldValue {
    Text(String),
    /// Millionths.
    Number(i64),
    Integer(i64),
    /// Hundredths of the currency.
    Cost(i64),
    Date(Date),
    Indicator(Indicator),
}

impl FieldValue {
    pub fn kind(&self) -> FieldKind {
        match self {
            FieldValue::Text(_) => FieldKind::Text,
            FieldValue::Number(_) => FieldKind::Number,
            FieldValue::Integer(_) => FieldKind::Integer,
            FieldValue::Cost(_) => FieldKind::Cost,
            FieldValue::Date(_) => FieldKind::Date,
            FieldValue::Indicator(_) => FieldKind::Indicator,
        }
    }
}

/// How the project classifies its activities: its activity codes and its user-defined fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Classification {
    pub codes: Vec<ActivityCode>,
    pub fields: Vec<UserField>,
}

impl Classification {
    pub fn is_empty(&self) -> bool {
        self.codes.is_empty() && self.fields.is_empty()
    }

    pub fn code(&self, name: &str) -> Option<&ActivityCode> {
        self.codes.iter().find(|c| c.name == name)
    }

    pub fn field(&self, name: &str) -> Option<&UserField> {
        self.fields.iter().find(|f| f.name == name)
    }
}

/// A relationship between an activity here and one in another project, kept in this one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalRelationship {
    /// The other project's file, as written: relative to this project's folder, or absolute.
    pub project: String,
    /// The activity here, and the one in the other project.
    pub here: String,
    pub there: String,
    /// The other project's activity drives this one's (it is the predecessor); else this one's
    /// drives the other's.
    pub incoming: bool,
    pub kind: RelationshipType,
    /// Working days; `lag_minutes` takes its place when set.
    pub lag: i64,
    pub lag_minutes: Option<i64>,
}

/// A node of the enterprise project structure the project sits in.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EpsNode {
    pub id: String,
    pub name: String,
}

/// A node of the organisational breakdown structure: who answers for parts of the work.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObsNode {
    /// Unique among the nodes.
    pub name: String,
    pub description: String,
    /// The node it sits under, by name.
    pub parent: Option<String>,
}

/// Where the project sits in the enterprise and who answers for it: its place in the EPS (from
/// the top down to the node holding it), the OBS, and the node responsible for the whole.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Organisation {
    pub eps: Vec<EpsNode>,
    pub obs: Vec<ObsNode>,
    pub responsible: Option<String>,
}

impl Organisation {
    pub fn is_empty(&self) -> bool {
        self == &Organisation::default()
    }
}

impl Default for Resource {
    fn default() -> Self {
        Resource {
            id: String::new(),
            name: String::new(),
            kind: ResourceKind::Labor,
            unit: String::new(),
            calendar: None,
            max_units: UNIT,
            roles: Vec::new(),
            prices: Vec::new(),
            extra: Extra::new(),
        }
    }
}

/// A kind of work a resource can fill (an electrician, a crane operator): an activity may ask
/// for a role before a resource is chosen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Role {
    /// Unique among the roles.
    pub id: String,
    pub name: String,
    pub extra: Extra,
}

/// A resource, or a role still to be staffed, put to work on an activity.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Assignment {
    /// The activity's ID.
    pub activity: String,
    /// The resource's ID; `None` for a role not staffed yet.
    pub resource: Option<String>,
    /// The role it fills, if one is named.
    pub role: Option<String>,
    /// Budgeted work, in hundredths of the resource's unit (hours for people and equipment).
    pub planned: i64,
    /// Work done so far, the same way.
    pub actual: i64,
    /// Work left; when `None`, the budget less what is done.
    pub remaining: Option<i64>,
    /// Units an hour, in hundredths; when `None`, the budget over the activity's working hours.
    pub rate: Option<i64>,
    pub extra: Extra,
}

impl Assignment {
    /// The work left: as given, else the budget less what is done (never below nothing).
    pub fn remaining_units(&self) -> i64 {
        self.remaining.unwrap_or_else(|| self.planned.saturating_sub(self.actual).max(0))
    }

    /// Its units an hour on an activity of `minutes` working minutes: as given, else its budget
    /// over those hours.
    pub fn rate_over(&self, minutes: i64) -> i64 {
        self.rate.unwrap_or_else(|| if minutes > 0 { self.planned.saturating_mul(60) / minutes } else { 0 })
    }
}

/// A remark on an activity, as a spreadsheet's comments: who wrote it and when, in threads of
/// replies, the thread resolved once its point is dealt with.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Comment {
    /// Unique in the project: `C1`, `C2`…
    pub id: String,
    /// The activity's ID.
    pub activity: String,
    pub author: String,
    /// When it was written, as the app that wrote it saw the time (`2026-10-23 14:05`).
    pub at: String,
    pub text: String,
    /// The comment that opened the thread it answers; `None` opens one.
    pub reply_to: Option<String>,
    /// A thread's dealt with (said on its first comment).
    pub resolved: bool,
}

/// Which calendar a relationship's lag counts its working days on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LagCalendar {
    /// The predecessor's.
    #[default]
    Predecessor,
    /// The successor's.
    Successor,
    /// Every day, around the clock.
    TwentyFourHour,
    /// The project's.
    Project,
}

/// How total float is measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FloatKind {
    /// Late finish minus early finish.
    #[default]
    Finish,
    /// Late start minus early start.
    Start,
    /// The smaller of the two.
    Smallest,
}

/// How the remaining work of an activity that started before its predecessors allowed is
/// scheduled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ProgressMode {
    /// Its remaining work still waits for its predecessors.
    #[default]
    RetainedLogic,
    /// Its remaining work goes on from the data date, whatever its predecessors.
    ProgressOverride,
}

/// What makes an activity critical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CriticalBy {
    /// Total float at or below this many working days.
    TotalFloat(i64),
    /// On the longest path: the chain of driving relationships back from the project's finish.
    LongestPath,
}

impl Default for CriticalBy {
    fn default() -> Self {
        CriticalBy::TotalFloat(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub name: String,
    /// The schedule starts here: no activity starts before it.
    pub start: Date,
    pub calendar: Calendar,
    /// Other calendars activities may name.
    pub calendars: Vec<Calendar>,
    pub lag_calendar: LagCalendar,
    pub float_kind: FloatKind,
    pub critical_by: CriticalBy,
    /// The day progress was measured to: remaining work is scheduled from it, and nothing not
    /// yet started starts before it.
    pub data_date: Option<Date>,
    pub progress_mode: ProgressMode,
    /// The evening the project must finish by: the late dates count back from it instead of
    /// from the latest early finish.
    pub must_finish_by: Option<Date>,
    pub activities: Vec<Activity>,
    pub relationships: Vec<Relationship>,
    /// Notes on the chart that never move a task: stretches shaded behind the bars, named days
    /// drawn as lines, months painted in the ruler.
    pub bands: Vec<Band>,
    pub markers: Vec<Marker>,
    pub month_marks: Vec<MonthMark>,
    /// Who and what does the work, the roles they fill, and their assignments to activities.
    pub resources: Vec<Resource>,
    pub roles: Vec<Role>,
    pub assignments: Vec<Assignment>,
    /// Remarks on activities, in the order they were written.
    pub comments: Vec<Comment>,
    /// How resources are levelled.
    pub levelling: LevellingOptions,
    /// The plans frozen so far, oldest first.
    pub baselines: Vec<Baseline>,
    /// The baseline the schedule is compared with, by name: its bars and the variances.
    pub baseline_in_use: Option<String>,
    /// The costs that aren't resources' work, and the accounts costs are charged to.
    pub expenses: Vec<Expense>,
    pub cost_accounts: Vec<CostAccount>,
    /// The currency's symbol (`R$`, `US$`), written before amounts; none when empty.
    pub currency: String,
    /// The money approved and planned for the project and its WBS.
    pub budget: Budgeting,
    /// How earned value is measured, and the performance stored period by period.
    pub earned_value: EarnedValue,
    /// The activity codes and user-defined fields activities are classified by.
    pub classification: Classification,
    /// Relationships to and from other projects' activities: those projects are scheduled with
    /// this one.
    pub external: Vec<ExternalRelationship>,
    /// Its place in the EPS, the OBS and who answers for it.
    pub organisation: Organisation,
    pub extra: Extra,
}

impl Project {
    /// The baseline the schedule is compared with, if it is still there.
    pub fn baseline(&self) -> Option<&Baseline> {
        let name = self.baseline_in_use.as_deref()?;
        self.baselines.iter().find(|b| b.name == name)
    }
}

/// A stretch of days shaded behind the bars, its name along its left edge: a rainy season, a
/// shutdown, a sprint. Bands may overlap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Band {
    pub name: String,
    pub from: Date,
    pub to: Date,
    /// A hex colour (`#4a90d9`); empty picks one.
    pub color: String,
}

/// A named day drawn as a line down the chart: an audit, a delivery, a visit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    pub name: String,
    pub date: Date,
    pub color: String,
    /// How far down the chart its name is written, in percent (0, the top, to 100).
    pub label_at: u8,
}

/// A whole month painted in the ruler, with a word: the month of the holidays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthMark {
    /// Its first day.
    pub month: Date,
    pub name: String,
    pub color: String,
}
