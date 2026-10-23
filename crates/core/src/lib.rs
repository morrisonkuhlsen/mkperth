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
    Accrual, Activity, ActivityKind, Assignment, Band, Baseline, BaselineActivity, Comment, Constraint, CostAccount, CriticalBy, DurationType,
    Expense, Extra, FloatKind, LagCalendar, LevelBy, Levelled, LevellingOptions, LevellingPriority, Marker, MonthMark, Pert, Price, ProgressMode,
    Project, Relationship, RelationshipType, Resource, ResourceKind, Role, UNIT,
};
