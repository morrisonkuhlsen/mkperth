//! The network, a precedence diagram: activities as boxes, relationships as arrows. The boxes
//! are laid out in columns by the topological order (each activity one column after the
//! furthest of its predecessors) and, in each column, in the order of their predecessors' rows,
//! so arrows cross less; without recursion over the network, so 5 000 activities lay out at
//! once. The window draws it on screen; here it is drawn as SVG, to print or keep.

use std::collections::HashMap;
use std::fmt::Write as _;

use mkperth_core::RelationshipType;
use mkperth_i18n::tr;

use crate::{EngineError, Session};

/// A box's width and height, and the gaps between columns and rows, in points.
pub const BOX: (f32, f32) = (168.0, 64.0);
pub const GAP: (f32, f32) = (56.0, 18.0);

/// Where each activity's box is: its column and its row, by the activity's index; summaries have
/// none (they take no part in the network).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    pub place: Vec<Option<(usize, usize)>>,
    pub columns: usize,
    pub rows: usize,
}

/// Lays the network out: columns by the longest chain of predecessors before each activity,
/// rows by the mean row of its predecessors (then its early start and ID). A loop can't trap it:
/// what the topological order never reaches goes in a column after the rest.
pub fn layout(session: &Session) -> Layout {
    let acts = &session.project.activities;
    let n = acts.len();
    let parents: std::collections::HashSet<&str> = acts.iter().filter_map(|a| a.parent.as_deref()).collect();
    let leaf: Vec<bool> = acts.iter().map(|a| !parents.contains(a.id.as_str())).collect();
    let index: HashMap<&str, usize> = acts.iter().enumerate().map(|(i, a)| (a.id.as_str(), i)).collect();
    let mut succ: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut pred: Vec<Vec<usize>> = vec![Vec::new(); n];
    for r in &session.project.relationships {
        let (Some(&a), Some(&b)) = (index.get(r.predecessor.as_str()), index.get(r.successor.as_str())) else { continue };
        if a != b && leaf.get(a) == Some(&true) && leaf.get(b) == Some(&true) {
            if let Some(s) = succ.get_mut(a) {
                s.push(b);
            }
            if let Some(p) = pred.get_mut(b) {
                p.push(a);
            }
        }
    }
    // Kahn's order, each column one after the furthest predecessor's.
    let mut waiting: Vec<usize> = pred.iter().map(Vec::len).collect();
    let mut column = vec![0usize; n];
    let mut queue: std::collections::VecDeque<usize> = (0..n).filter(|&i| leaf.get(i) == Some(&true) && waiting.get(i) == Some(&0)).collect();
    let mut done = vec![false; n];
    while let Some(i) = queue.pop_front() {
        if let Some(d) = done.get_mut(i) {
            *d = true;
        }
        let c = column.get(i).copied().unwrap_or(0);
        for &s in succ.get(i).map_or(&[][..], Vec::as_slice) {
            if let Some(x) = column.get_mut(s) {
                *x = (*x).max(c + 1);
            }
            if let Some(w) = waiting.get_mut(s) {
                *w = w.saturating_sub(1);
                if *w == 0 {
                    queue.push_back(s);
                }
            }
        }
    }
    let last = column.iter().copied().max().unwrap_or(0);
    for i in 0..n {
        if leaf.get(i) == Some(&true)
            && done.get(i) == Some(&false)
            && let Some(c) = column.get_mut(i)
        {
            *c = last + 1;
        }
    }
    let columns = (0..n).filter(|&i| leaf.get(i) == Some(&true)).map(|i| column.get(i).copied().unwrap_or(0) + 1).max().unwrap_or(0);
    // Rows, column by column: by the mean row of the predecessors already placed.
    let schedule = session.schedule.as_ref().ok();
    let start = |i: usize| schedule.and_then(|s| s.activities.get(i)).map(|s| s.early_start);
    let mut by_column: Vec<Vec<usize>> = vec![Vec::new(); columns];
    for i in (0..n).filter(|&i| leaf.get(i) == Some(&true)) {
        if let Some(c) = by_column.get_mut(column.get(i).copied().unwrap_or(0)) {
            c.push(i);
        }
    }
    let mut row = vec![usize::MAX; n];
    let mut rows = 0;
    for list in &mut by_column {
        let key = |i: usize| {
            let placed: Vec<usize> =
                pred.get(i).map_or(&[][..], Vec::as_slice).iter().filter_map(|p| row.get(*p).copied().filter(|r| *r != usize::MAX)).collect();
            if placed.is_empty() { f64::MAX } else { placed.iter().sum::<usize>() as f64 / placed.len() as f64 }
        };
        let mut keyed: Vec<(f64, Option<mkperth_core::Date>, &str, usize)> =
            list.iter().map(|&i| (key(i), start(i), acts.get(i).map_or("", |a| a.id.as_str()), i)).collect();
        keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(b.2)));
        *list = keyed.into_iter().map(|(_, _, _, i)| i).collect();
        for (r, &i) in list.iter().enumerate() {
            if let Some(x) = row.get_mut(i) {
                *x = r;
            }
        }
        rows = rows.max(list.len());
    }
    let place =
        (0..n).map(|i| (leaf.get(i) == Some(&true)).then(|| (column.get(i).copied().unwrap_or(0), row.get(i).copied().unwrap_or(0)))).collect();
    Layout { place, columns, rows }
}

/// Text as SVG holds it.
fn esc(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Working days as the language writes them: whole, or to two decimals.
fn days_text(minutes: i64, day: i64) -> String {
    let d = day.max(1);
    let text =
        if minutes % d == 0 { (minutes / d).to_string() } else { format!("{:.2}", minutes as f64 / d as f64).trim_end_matches('0').to_string() };
    mkperth_i18n::decimal(&text)
}

impl Session {
    /// The network as an SVG drawing to print: the project's name and finish over the boxes
    /// (each its ID, name, early dates and total float, a red edge on the critical path) and the
    /// arrows between them, as the network view draws them in light colours. Dates as `date`
    /// writes them.
    pub fn network_svg(&self, date: &dyn Fn(mkperth_core::Date) -> String) -> Result<String, EngineError> {
        let schedule = self.schedule.as_ref().map_err(|e| EngineError::Schedule(e.clone()))?;
        let lay = layout(self);
        let acts = &self.project.activities;
        let (margin, head) = (24.0f32, 56.0f32);
        let (cell_x, cell_y) = (BOX.0 + GAP.0, BOX.1 + GAP.1);
        let width = margin * 2.0 + (lay.columns as f32 * cell_x - GAP.0).max(BOX.0 * 2.0);
        let height = margin * 2.0 + head + (lay.rows as f32 * cell_y - GAP.1).max(0.0);
        let corner = |i: usize| lay.place.get(i).copied().flatten().map(|(c, r)| (margin + c as f32 * cell_x, margin + head + r as f32 * cell_y));
        let critical = |i: usize| schedule.activities.get(i).is_some_and(|s| s.critical);
        let (red, border, ink, dim, grey) = ("#C93C3C", "#C9CAD3", "#22232A", "#626470", "#8A8C96");
        let mut out = String::new();
        let _ = write!(
            out,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\" font-family=\"sans-serif\">\n"
        );
        let _ = writeln!(out, "<title>{}</title>", esc(&tr!("network-svg-title", name = self.project.name.as_str())));
        let _ = writeln!(
            out,
            "<defs><marker id=\"head\" viewBox=\"0 0 10 10\" refX=\"10\" refY=\"5\" markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\"><path d=\"M0,0 L10,5 L0,10 z\" fill=\"{grey}\"/></marker>\
             <marker id=\"head-critical\" viewBox=\"0 0 10 10\" refX=\"10\" refY=\"5\" markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\"><path d=\"M0,0 L10,5 L0,10 z\" fill=\"{red}\"/></marker></defs>"
        );
        let _ = writeln!(out, "<rect width=\"100%\" height=\"100%\" fill=\"#FFFFFF\"/>");
        let critical_count = schedule.activities.iter().filter(|s| s.critical).count();
        let _ = writeln!(
            out,
            "<text x=\"{margin}\" y=\"{}\" font-size=\"16\" font-weight=\"bold\" fill=\"{ink}\">{}</text>",
            margin + 14.0,
            esc(&self.project.name)
        );
        let line = tr!("network-svg-line", date = date(schedule.finish).as_str(), n = critical_count);
        let _ = writeln!(out, "<text x=\"{margin}\" y=\"{}\" font-size=\"11\" fill=\"{dim}\">{}</text>", margin + 34.0, esc(&line));
        // The arrows: out of the predecessor's side, into the successor's, with an elbow between.
        let index: HashMap<&str, usize> = acts.iter().enumerate().map(|(i, a)| (a.id.as_str(), i)).collect();
        for r in &self.project.relationships {
            let (Some(&a), Some(&b)) = (index.get(r.predecessor.as_str()), index.get(r.successor.as_str())) else { continue };
            let (Some((ax, ay)), Some((bx, by))) = (corner(a), corner(b)) else { continue };
            let from_start = matches!(r.kind, RelationshipType::StartToStart | RelationshipType::StartToFinish);
            let onto_finish = matches!(r.kind, RelationshipType::FinishToFinish | RelationshipType::StartToFinish);
            let from = (if from_start { ax } else { ax + BOX.0 }, ay + BOX.1 / 2.0);
            let to = (if onto_finish { bx + BOX.0 } else { bx }, by + BOX.1 / 2.0);
            let bend = if to.0 > from.0 + 8.0 { (from.0 + to.0) / 2.0 } else { from.0 + 10.0 };
            let (color, head) = if critical(a) && critical(b) { (red, "head-critical") } else { (grey, "head") };
            let _ = writeln!(
                out,
                "<polyline points=\"{},{} {bend},{} {bend},{} {},{}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"1.2\" marker-end=\"url(#{head})\"/>",
                from.0, from.1, from.1, to.1, to.0, to.1
            );
        }
        // The boxes, each its own clipped drawing: the ID and name over the dates, the float at
        // the right.
        let day = self.project.calendar.day_minutes();
        for (i, a) in acts.iter().enumerate() {
            let (Some((x, y)), Some(s)) = (corner(i), schedule.activities.get(i)) else { continue };
            let (w, h) = BOX;
            let edge = if s.critical { red } else { border };
            let _ = writeln!(out, "<svg x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\">");
            let _ = writeln!(
                out,
                "<rect x=\"0.6\" y=\"0.6\" width=\"{}\" height=\"{}\" fill=\"#FFFFFF\" stroke=\"{edge}\" stroke-width=\"1.2\"/>",
                w - 1.2,
                h - 1.2
            );
            if s.critical {
                let _ = writeln!(out, "<rect width=\"4\" height=\"{h}\" fill=\"{red}\"/>");
            }
            let float = tr!("network-float", float = days_text(s.total_float_minutes, day).as_str());
            let _ = writeln!(
                out,
                "<text x=\"{}\" y=\"18\" font-size=\"11\" text-anchor=\"end\" fill=\"{}\">{}</text>",
                w - 7.0,
                if s.critical { red } else { dim },
                esc(&float)
            );
            let _ = writeln!(out, "<text x=\"9\" y=\"18\" font-size=\"12.5\" font-weight=\"bold\" fill=\"{ink}\">{}</text>", esc(&a.id));
            let _ = writeln!(out, "<text x=\"9\" y=\"35\" font-size=\"11.5\" fill=\"{dim}\">{}</text>", esc(&a.name));
            let dates = format!("{} – {}", date(s.early_start), date(s.early_finish));
            let _ = writeln!(out, "<text x=\"9\" y=\"{}\" font-size=\"11\" fill=\"{ink}\">{}</text>\n</svg>", h - 9.0, esc(&dates));
        }
        out.push_str("</svg>\n");
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use mkperth_core::{Activity, Relationship, RelationshipType};

    use super::*;

    /// The sample: every relationship runs from a column to a later one, no two boxes share a
    /// place, summaries have none; the project's start is in the first column.
    #[test]
    fn the_sample_laid_out() {
        let s = Session::default();
        let lay = layout(&s);
        let place = |id: &str| lay.place[s.project.activities.iter().position(|a| a.id == id).unwrap()];
        assert_eq!(place("W1"), None, "a summary has no box");
        assert_eq!(place("M0").map(|p| p.0), Some(0));
        for r in &s.project.relationships {
            if let (Some(a), Some(b)) = (place(&r.predecessor), place(&r.successor)) {
                assert!(a.0 < b.0, "{} → {}", r.predecessor, r.successor);
            }
        }
        let mut seen = std::collections::HashSet::new();
        assert!(lay.place.iter().flatten().all(|p| seen.insert(*p)), "no two boxes in one place");
    }

    /// 5 000 activities, each tied to the one before and some to one further back, lay out in
    /// well under a second; the chain makes 5 000 columns.
    #[test]
    #[allow(clippy::disallowed_methods)] // the clock, in a test that runs natively only
    fn five_thousand_activities_lay_out_at_once() {
        let mut s = Session::default();
        let n = 5_000;
        s.project.activities = (0..n).map(|k| Activity { id: format!("A{k}"), duration: 1, ..Activity::default() }).collect();
        s.project.relationships = (1..n)
            .flat_map(|k| {
                let tie = |a: usize| Relationship {
                    predecessor: format!("A{a}"),
                    successor: format!("A{k}"),
                    kind: RelationshipType::FinishToStart,
                    lag: 0,
                    lag_minutes: None,
                    lag_kind: Default::default(),
                };
                std::iter::once(tie(k - 1)).chain((k % 7 == 0).then(|| tie(k / 2)))
            })
            .collect();
        s.reschedule();
        let started = std::time::Instant::now();
        let lay = layout(&s);
        assert!(started.elapsed().as_secs_f64() < 1.0, "{:?}", started.elapsed());
        assert_eq!(lay.columns, n);
    }

    /// The sample drawn for print: a box per activity that isn't a summary, an arrow per
    /// relationship between them, red on the critical path; names escaped; and an XML document
    /// that ends where it should.
    #[test]
    fn the_sample_drawn_as_svg() {
        let mut s = Session::default();
        s.project.activities[1].name = "Survey & <plan>".into();
        s.reschedule();
        let svg = s.network_svg(&|d| d.to_string()).unwrap();
        assert!(svg.starts_with("<?xml") && svg.ends_with("</svg>\n"));
        let leaves = s.project.activities.iter().filter(|a| !s.project.activities.iter().any(|b| b.parent.as_deref() == Some(a.id.as_str()))).count();
        assert_eq!(svg.matches("<svg x=").count(), leaves);
        let ties = layout(&s);
        let drawn = s
            .project
            .relationships
            .iter()
            .filter(|r| {
                let at = |id: &str| s.project.activities.iter().position(|a| a.id == id).and_then(|i| ties.place[i]);
                at(&r.predecessor).is_some() && at(&r.successor).is_some()
            })
            .count();
        assert_eq!(svg.matches("<polyline").count(), drawn);
        assert!(svg.contains("Survey &amp; &lt;plan&gt;") && !svg.contains("<plan>"));
        assert!(svg.contains("head-critical") && svg.contains("2027-04-20"), "the finish");
        // Not scheduled, nothing drawn: the reason.
        s.project.relationships.push(Relationship {
            predecessor: "A20".into(),
            successor: "M0".into(),
            kind: RelationshipType::FinishToStart,
            lag: 0,
            lag_minutes: None,
            lag_kind: Default::default(),
        });
        s.reschedule();
        assert!(matches!(s.network_svg(&|d| d.to_string()), Err(EngineError::Schedule(_))));
    }
}
