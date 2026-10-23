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
    pub extra: Extra,
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
    pub extra: Extra,
}

impl Assignment {
    /// The work left: as given, else the budget less what is done (never below nothing).
    pub fn remaining_units(&self) -> i64 {
        self.remaining.unwrap_or_else(|| self.planned.saturating_sub(self.actual).max(0))
    }
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
    pub extra: Extra,
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
