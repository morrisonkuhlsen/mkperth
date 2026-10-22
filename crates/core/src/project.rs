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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Activity {
    /// Unique within the project (P6's Activity ID).
    pub id: String,
    pub name: String,
    /// Working days; 0 is a milestone.
    pub duration: i64,
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
