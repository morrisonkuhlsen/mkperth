//! Every date with its calculation: what bounded an activity in each pass, which bound won,
//! where its calendar placed it, and how its floats were counted. Recorded by the scheduler
//! itself as it runs (`crate::explain`), so the explanation is the calculation, not a second one.

use mkperth_core::{Constraint, Date, RelationshipType};
use mkperth_i18n::tr;

use crate::Scheduled;

/// A moment on the schedule: the morning or the evening of a day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moment {
    pub date: Date,
    pub evening: bool,
}

/// Where a bound came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    ProjectStart,
    /// A relationship from a predecessor (forward) or to a successor (backward).
    Relationship {
        other: String,
        kind: RelationshipType,
        lag: i64,
    },
    Constraint(Constraint),
    ProjectFinish,
    MustFinishBy,
}

/// One bound on the activity: in the forward pass a start no earlier than `moment`, in the
/// backward pass a finish no later than it. For finish-to-finish and start-to-finish ties forward
/// (start-to-start and start-to-finish backward) `via` is the bound before it was turned around
/// through the activity's duration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bound {
    pub source: Source,
    pub moment: Moment,
    pub via: Option<Moment>,
    /// A mandatory constraint: it replaces the others instead of joining them.
    pub overrides: bool,
}

/// One limit on the free float: the working days before a successor, or before the project's
/// finish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slack {
    pub source: Source,
    pub days: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Explanation {
    pub id: String,
    pub calendar: String,
    /// Working days; 0 for a milestone or a summary.
    pub duration: i64,
    /// The children whose dates a summary takes; empty for any other activity.
    pub children: Vec<String>,
    pub early: Vec<Bound>,
    /// The latest of the early bounds (or the mandatory one), before the calendar placed it.
    pub earliest: Moment,
    pub early_start: Moment,
    pub early_finish: Moment,
    pub late: Vec<Bound>,
    /// The earliest of the late bounds (or the mandatory one), before the calendar placed it.
    pub latest: Moment,
    pub late_start: Moment,
    pub late_finish: Moment,
    pub free: Vec<Slack>,
    /// Days moved later by as late as possible.
    pub held_late: i64,
    /// The activity's row in the schedule.
    pub scheduled: Scheduled,
}

/// A time as a moment: `t` is the morning of day `t`, or the evening of day `t − 1`.
pub(crate) fn moment(t: i64, evening: bool) -> Option<Moment> {
    let day = if evening { t - 1 } else { t };
    i32::try_from(day).ok().and_then(Date::from_days).map(|date| Moment { date, evening })
}

fn kind_name(kind: RelationshipType) -> String {
    match kind {
        RelationshipType::FinishToStart => tr!("rel-fs"),
        RelationshipType::StartToStart => tr!("rel-ss"),
        RelationshipType::FinishToFinish => tr!("rel-ff"),
        RelationshipType::StartToFinish => tr!("rel-sf"),
    }
}

/// A constraint's name and date.
pub fn constraint_name(c: Constraint, date: &dyn Fn(Date) -> String) -> String {
    match c {
        Constraint::StartOnOrAfter(d) => tr!("con-start-on-or-after", date = date(d).as_str()),
        Constraint::StartOnOrBefore(d) => tr!("con-start-on-or-before", date = date(d).as_str()),
        Constraint::FinishOnOrAfter(d) => tr!("con-finish-on-or-after", date = date(d).as_str()),
        Constraint::FinishOnOrBefore(d) => tr!("con-finish-on-or-before", date = date(d).as_str()),
        Constraint::StartOn(d) => tr!("con-start-on", date = date(d).as_str()),
        Constraint::FinishOn(d) => tr!("con-finish-on", date = date(d).as_str()),
        Constraint::MandatoryStart(d) => tr!("con-mandatory-start", date = date(d).as_str()),
        Constraint::MandatoryFinish(d) => tr!("con-mandatory-finish", date = date(d).as_str()),
        Constraint::AsLateAsPossible => tr!("con-as-late-as-possible"),
    }
}

/// The text with its first letter in upper case.
fn capitalized(text: String) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => text,
    }
}

impl Explanation {
    /// The calculation in words, line by line, dates written by `date`.
    pub fn lines(&self, date: &dyn Fn(Date) -> String) -> Vec<String> {
        let at =
            |m: Moment| if m.evening { tr!("ex-evening", date = date(m.date).as_str()) } else { tr!("ex-morning", date = date(m.date).as_str()) };
        let lag = |l: i64| if l == 0 { String::new() } else { tr!("ex-lag", lag = format!("{l:+}").as_str()) };
        let source = |s: &Source| match s {
            Source::ProjectStart => tr!("ex-src-project-start"),
            Source::ProjectFinish => tr!("ex-src-project-finish"),
            Source::MustFinishBy => tr!("ex-src-must-finish-by"),
            Source::Constraint(c) => constraint_name(*c, date),
            Source::Relationship { other, kind, lag: l } => {
                tr!("ex-src-relationship", kind = kind_name(*kind).as_str(), other = other.as_str(), lag = lag(*l).as_str())
            }
        };
        let mut out = Vec::new();
        if !self.children.is_empty() {
            out.push(tr!("ex-summary", ids = self.children.join(", ").as_str()));
            out.push(tr!(
                "ex-summary-dates",
                es = date(self.scheduled.early_start).as_str(),
                ef = date(self.scheduled.early_finish).as_str(),
                ls = date(self.scheduled.late_start).as_str(),
                lf = date(self.scheduled.late_finish).as_str()
            ));
            out.push(tr!("ex-total-float", calendar = self.calendar.as_str(), n = self.scheduled.total_float));
            return out;
        }
        // Each bound under its pass, indented by two spaces (the catalogs keep no leading space).
        out.push(tr!("ex-forward"));
        for b in &self.early {
            out.push(
                "  ".to_string()
                    + &capitalized(match (b.via, b.overrides) {
                        (_, true) => tr!("ex-early-mandatory", source = source(&b.source).as_str(), at = at(b.moment).as_str()),
                        (Some(v), _) => tr!("ex-early-via", source = source(&b.source).as_str(), via = at(v).as_str(), at = at(b.moment).as_str()),
                        (None, _) => tr!("ex-early-bound", source = source(&b.source).as_str(), at = at(b.moment).as_str()),
                    }),
            );
        }
        out.push(tr!("ex-earliest", at = at(self.earliest).as_str()));
        if self.early_start != self.earliest {
            out.push(tr!("ex-placed", calendar = self.calendar.as_str(), at = at(self.early_start).as_str()));
        }
        if self.duration > 0 {
            out.push(tr!("ex-early-finish", n = self.duration, calendar = self.calendar.as_str(), at = at(self.early_finish).as_str()));
        }
        out.push(tr!("ex-backward"));
        for b in &self.late {
            out.push(
                "  ".to_string()
                    + &capitalized(match (b.via, b.overrides) {
                        (_, true) => tr!("ex-late-mandatory", source = source(&b.source).as_str(), at = at(b.moment).as_str()),
                        (Some(v), _) => tr!("ex-late-via", source = source(&b.source).as_str(), via = at(v).as_str(), at = at(b.moment).as_str()),
                        (None, _) => tr!("ex-late-bound", source = source(&b.source).as_str(), at = at(b.moment).as_str()),
                    }),
            );
        }
        out.push(tr!("ex-latest", at = at(self.latest).as_str()));
        if self.late_finish != self.latest {
            out.push(tr!("ex-placed-late", calendar = self.calendar.as_str(), at = at(self.late_finish).as_str()));
        }
        if self.duration > 0 {
            out.push(tr!("ex-late-start", n = self.duration, calendar = self.calendar.as_str(), at = at(self.late_start).as_str()));
        }
        out.push(tr!("ex-total-float", calendar = self.calendar.as_str(), n = self.scheduled.total_float));
        let free: Vec<String> = self.free.iter().map(|s| tr!("ex-slack", source = source(&s.source).as_str(), n = s.days)).collect();
        out.push(tr!("ex-free-float", n = self.scheduled.free_float, list = free.join("; ").as_str()));
        if self.held_late > 0 {
            out.push(tr!("ex-held-late", n = self.held_late));
        }
        if self.scheduled.critical {
            out.push(tr!("ex-critical"));
        }
        out
    }
}
