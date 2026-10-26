//! MK Perth core types: civil dates, the calendar that says which days are worked, and a
//! project's activities and the relationships between them.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod calendar;
mod date;
mod holidays;
mod project;

pub use calendar::{Calendar, CalendarError, DEFAULT_PERIODS, MINUTES_PER_DAY, Period};
pub use date::{Date, Weekday};
pub use holidays::{Region, easter};
pub use project::{
    Accrual, Activity, ActivityCode, ActivityKind, Assignment, Band, Baseline, BaselineActivity, BudgetChange, Budgeting, ChangeStatus,
    Classification, CodeValue, Comment, Constraint, CostAccount, CriticalBy, DurationType, EarnedValue, EpsNode, EtcTechnique, EvTechnique, Expense,
    ExternalRelationship, Extra, FieldKind, FieldValue, FinancialPeriod, FloatKind, Fund, FundingSource, Indicator, LagCalendar, LagKind, LevelBy,
    Levelled, LevellingOptions, LevellingPriority, Limit, Marker, MonthMark, ObsNode, Organisation, PeriodRecord, Pert, Price, ProgressMode, Project,
    Relationship, RelationshipType, Resource, ResourceKind, Role, SpendingMonth, UNIT, UserField, WbsBudget,
};
