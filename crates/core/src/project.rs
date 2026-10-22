//! A project: its activities, the relationships between them, its calendar and the date its
//! schedule starts from.

use crate::{Calendar, Date};

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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Activity {
    /// Unique within the project (P6's Activity ID).
    pub id: String,
    pub name: String,
    pub kind: ActivityKind,
    /// Working days. A milestone takes none, whatever this says; a task of 0 is shown as a
    /// start milestone.
    pub duration: i64,
}

impl Activity {
    /// The working days it takes: none for a milestone.
    pub fn working_days(&self) -> i64 {
        if self.kind.is_milestone() { 0 } else { self.duration }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub name: String,
    /// The schedule starts here: no activity starts before it.
    pub start: Date,
    pub calendar: Calendar,
    pub activities: Vec<Activity>,
    pub relationships: Vec<Relationship>,
}
