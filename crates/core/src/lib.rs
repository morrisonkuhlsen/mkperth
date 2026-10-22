//! MK Perth core types: civil dates, the calendar that says which days are worked, and a
//! project's activities and the relationships between them.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod calendar;
mod date;
mod project;

pub use calendar::{Calendar, CalendarError};
pub use date::{Date, Weekday};
pub use project::{Activity, ActivityKind, Constraint, LagCalendar, Project, Relationship, RelationshipType};
