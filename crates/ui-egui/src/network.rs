//! The network view: a precedence diagram, activities as boxes, relationships as arrows. The
//! boxes are laid out in columns by the topological order (each activity one column after the
//! furthest of its predecessors) and, in each column, in the order of their predecessors' rows,
//! so arrows cross less. All of it without recursion over the network, so 5 000 activities lay
//! out at once; only the boxes and arrows in view are drawn. A relationship is drawn by dragging
//! from a box's end (its start at the left, its finish at the right) onto another box's.

use std::collections::HashMap;

use egui::{Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};
use mkperth_engine::Session;
use mkperth_engine::i18n::tr;

use crate::show_date;
use crate::theme::Tokens;

/// A box's size and the gaps between columns and rows, at a zoom of 1.
const BOX: Vec2 = vec2(168.0, 64.0);
const GAP: Vec2 = vec2(56.0, 18.0);

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
        let mut keyed: Vec<(f64, Option<mkperth_engine::core::Date>, &str, usize)> =
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

/// The ends of a box a relationship is drawn from: a dot at each side's middle, and how near the
/// pointer must be to take it.
const HANDLE: f32 = 4.5;
const HANDLE_REACH: f32 = 9.0;

/// The end of box `b` at `p`: `Some(true)` its start (the left dot), `Some(false)` its finish
/// (the right one), `None` neither.
pub fn handle_at(b: Rect, p: egui::Pos2, zoom: f32) -> Option<bool> {
    let reach = HANDLE_REACH * zoom.max(0.6);
    if p.distance(b.left_center()) <= reach {
        Some(true)
    } else if p.distance(b.right_center()) <= reach {
        Some(false)
    } else {
        None
    }
}

/// The kind of a relationship drawn from one box's start or finish onto the other's left half
/// (its start) or right half (its finish): FS, FF, SS or SF.
pub fn tie_kind(from_start: bool, onto_finish: bool) -> &'static str {
    match (from_start, onto_finish) {
        (false, false) => "FS",
        (false, true) => "FF",
        (true, false) => "SS",
        (true, true) => "SF",
    }
}

/// What the view keeps between frames: where it is scrolled and how near.
#[derive(Debug, Clone)]
pub struct NetworkView {
    pub offset: Vec2,
    pub zoom: f32,
    /// A relationship being drawn: the activity it comes from, and whether from its start.
    linking: Option<(usize, bool)>,
    /// The layout and the project it was made for.
    cache: Option<(mkperth_engine::core::Project, Layout)>,
}

impl Default for NetworkView {
    fn default() -> Self {
        NetworkView { offset: vec2(16.0, 16.0), zoom: 1.0, linking: None, cache: None }
    }
}

/// What happened in the view this frame: an activity clicked, or double-clicked to open; a
/// relationship drawn from one activity to another, with its kind (`FS`, `SS`, `FF`, `SF`).
#[derive(Debug, Default)]
pub struct NetworkEvents {
    pub clicked: Option<usize>,
    pub open: Option<usize>,
    pub tie: Option<(usize, usize, &'static str)>,
}

/// Paints the network in `rect`'s space: drag to move it, Ctrl and the wheel to zoom about the
/// pointer, a click to choose a box, a drag from a box's end onto another to tie them.
pub fn show(ui: &mut egui::Ui, session: &Session, view: &mut NetworkView, selected: Option<usize>) -> NetworkEvents {
    let t = Tokens::get(ui.ctx());
    let mut events = NetworkEvents::default();
    if view.cache.as_ref().is_none_or(|(p, _)| *p != session.project) {
        view.cache = Some((session.project.clone(), layout(session)));
    }
    let Some((_, lay)) = &view.cache else { return events };
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, t.grid_bg);
    // Where each box is now, before this frame's moves (a drag begins where it was pressed).
    let place_now = |i: usize, offset: Vec2, zoom: f32| {
        lay.place.get(i).copied().flatten().map(|(c, r)| {
            let cell = (BOX + GAP) * zoom;
            Rect::from_min_size(rect.min + offset + vec2(c as f32 * cell.x, r as f32 * cell.y), BOX * zoom)
        })
    };
    let n = session.project.activities.len();
    let box_under = |p: egui::Pos2, offset: Vec2, zoom: f32| (0..n).find(|&i| place_now(i, offset, zoom).is_some_and(|b| b.contains(p)));
    if response.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin());
        view.linking = origin
            .and_then(|o| (0..n).find_map(|i| place_now(i, view.offset, view.zoom).and_then(|b| handle_at(b, o, view.zoom)).map(|start| (i, start))));
    }
    if response.dragged() && view.linking.is_none() {
        view.offset += response.drag_delta();
    }
    if let Some(p) = response.hover_pos() {
        let zoom = ui.input(|i| i.zoom_delta());
        if (zoom - 1.0).abs() > f32::EPSILON {
            let new = (view.zoom * zoom).clamp(0.2, 2.5);
            // About the pointer: the point under it stays under it.
            let at = (p - rect.min - view.offset) / view.zoom;
            view.zoom = new;
            view.offset = p - rect.min - at * new;
        } else {
            view.offset += ui.input(|i| i.smooth_scroll_delta);
        }
    }
    let z = view.zoom;
    let cell = (BOX + GAP) * z;
    let corner = |c: usize, r: usize| rect.min + view.offset + vec2(c as f32 * cell.x, r as f32 * cell.y);
    let box_of = |i: usize| lay.place.get(i).copied().flatten().map(|(c, r)| Rect::from_min_size(corner(c, r), BOX * z));
    let schedule = session.schedule.as_ref().ok();
    let acts = &session.project.activities;
    let critical = |i: usize| schedule.and_then(|s| s.activities.get(i)).is_some_and(|s| s.critical);
    let view_rect = rect.expand(BOX.x * z);
    // The arrows: out of the predecessor's side, into the successor's, with an elbow between.
    let index: HashMap<&str, usize> = acts.iter().enumerate().map(|(i, a)| (a.id.as_str(), i)).collect();
    for r in &session.project.relationships {
        let (Some(&a), Some(&b)) = (index.get(r.predecessor.as_str()), index.get(r.successor.as_str())) else { continue };
        let (Some(ra), Some(rb)) = (box_of(a), box_of(b)) else { continue };
        if !view_rect.intersects(ra.union(rb)) {
            continue;
        }
        use mkperth_engine::core::RelationshipType as K;
        let from = if matches!(r.kind, K::StartToStart | K::StartToFinish) { ra.left_center() } else { ra.right_center() };
        let to = if matches!(r.kind, K::FinishToFinish | K::StartToFinish) { rb.right_center() } else { rb.left_center() };
        let color = if critical(a) && critical(b) { t.red } else { t.text_dim.gamma_multiply(0.7) };
        let mid = (from.x + to.x) / 2.0;
        let bend = if to.x > from.x + 8.0 { mid } else { from.x + 10.0 * z };
        let points = vec![from, pos2(bend, from.y), pos2(bend, to.y), to];
        painter.add(egui::Shape::line(points, Stroke::new(1.2, color)));
        let back = if to.x >= bend { -1.0 } else { 1.0 };
        let head = 6.0 * z.max(0.6);
        painter.add(egui::Shape::convex_polygon(
            vec![to, pos2(to.x + back * head, to.y - head * 0.6), pos2(to.x + back * head, to.y + head * 0.6)],
            color,
            Stroke::NONE,
        ));
    }
    // The boxes in view: the ID and name over the dates, the float at the right.
    let pointer = response.interact_pointer_pos();
    let hover = response.hover_pos();
    let mut hit = None;
    for (i, a) in acts.iter().enumerate() {
        let Some(b) = box_of(i) else { continue };
        if !rect.intersects(b) {
            continue;
        }
        let s = schedule.and_then(|s| s.activities.get(i));
        let on = selected == Some(i);
        let edge = if critical(i) { t.red } else { t.border };
        painter.rect_filled(b, 0.0, if on { t.accent_soft } else { t.paper });
        painter.rect_stroke(b, 0.0, Stroke::new(if on { 2.0 } else { 1.2 }, if on { t.accent } else { edge }), StrokeKind::Inside);
        if critical(i) {
            painter.rect_filled(Rect::from_min_size(b.min, vec2(4.0 * z, b.height())), 0.0, t.red);
        }
        if z < 0.45 && b.width() > 28.0 {
            // Far out: the ID alone, as large as reads.
            let size = (12.5 * z).max(9.0).min(b.height() * 0.8);
            painter.with_clip_rect(b.shrink(1.0)).text(b.center(), Align2::CENTER_CENTER, &a.id, FontId::proportional(size), t.text);
        }
        if z >= 0.45 {
            let font = |size: f32| FontId::proportional(size * z);
            let clip = painter.with_clip_rect(b.shrink(2.0));
            let left = b.left() + 9.0 * z;
            clip.text(pos2(left, b.top() + 6.0 * z), Align2::LEFT_TOP, &a.id, font(12.5), t.text);
            clip.text(pos2(left + 0.5, b.top() + 6.0 * z), Align2::LEFT_TOP, &a.id, font(12.5), t.text);
            clip.text(pos2(left, b.top() + 23.0 * z), Align2::LEFT_TOP, &a.name, font(11.5), t.text_dim);
            if let Some(s) = s {
                let dates = format!("{} – {}", show_date(s.early_start), show_date(s.early_finish));
                clip.text(pos2(left, b.bottom() - 6.0 * z), Align2::LEFT_BOTTOM, dates, font(11.0), t.cell_text);
                let float = crate::days_text(s.total_float_minutes, session.project.calendar.day_minutes());
                let color: Color32 = if s.critical { t.red } else { t.text_dim };
                clip.text(
                    pos2(b.right() - 7.0 * z, b.top() + 6.0 * z),
                    Align2::RIGHT_TOP,
                    tr!("network-float", float = float.as_str()),
                    font(11.0),
                    color,
                );
            }
        }
        // The ends a relationship is drawn from, on the box under the pointer and the one
        // being drawn from.
        let near = hover.is_some_and(|p| b.expand(HANDLE_REACH * z).contains(p));
        if near || view.linking.is_some_and(|(from, _)| from == i) {
            for end in [b.left_center(), b.right_center()] {
                let lit = hover.is_some_and(|p| handle_at(b, p, z).is_some() && p.distance(end) <= HANDLE_REACH * z.max(0.6));
                painter.circle(end, HANDLE * z.max(0.6), if lit { t.accent } else { t.paper }, Stroke::new(1.2, t.accent));
            }
        }
        if pointer.is_some_and(|p| b.contains(p)) {
            hit = Some(i);
        }
    }
    // The relationship being drawn: a line from the end it leaves to the pointer, the box it
    // would tie to outlined; let go over a box, it is tied.
    if let Some((from, start)) = view.linking {
        let pointer = pointer.or(hover);
        let target = pointer.and_then(|p| box_under(p, view.offset, view.zoom).filter(|&b| b != from).map(|b| (b, p)));
        if let (Some(b), Some(p)) = (box_of(from), pointer) {
            let end = if start { b.left_center() } else { b.right_center() };
            painter.line_segment([end, p], Stroke::new(1.6, t.accent));
        }
        if let Some((to, p)) = target
            && let Some(b) = box_of(to)
        {
            let onto_finish = p.x > b.center().x;
            painter.rect_stroke(b, 0.0, Stroke::new(2.0, t.accent), StrokeKind::Outside);
            let end = if onto_finish { b.right_center() } else { b.left_center() };
            painter.circle_filled(end, HANDLE * z.max(0.6), t.accent);
            painter.text(b.center_top() - vec2(0.0, 4.0), Align2::CENTER_BOTTOM, tie_kind(start, onto_finish), FontId::proportional(12.0), t.accent);
            if response.drag_stopped() {
                events.tie = Some((from, to, tie_kind(start, onto_finish)));
            }
        }
        if response.drag_stopped() || !response.dragged() {
            view.linking = None;
        }
    }
    if response.clicked() {
        events.clicked = hit;
    }
    if response.double_clicked() {
        events.open = hit;
    }
    // In the corner: nearer, further, and the whole network in view.
    let (columns, rows) = (lay.columns, lay.rows);
    let mut bar = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_size(pos2(rect.right() - 108.0, rect.top() + 8.0), vec2(100.0, 28.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    bar.spacing_mut().item_spacing.x = 2.0;
    let paper = egui::Frame::new().fill(t.paper).stroke(Stroke::new(1.0, t.border)).inner_margin(egui::Margin::same(2));
    paper.show(&mut bar, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let mut zoom_by = |ui: &mut egui::Ui, icon, name: String, factor: f32| {
                if crate::icon_button(ui, icon, &name) {
                    let centre = rect.center() - rect.min;
                    let at = (centre - view.offset) / view.zoom;
                    view.zoom = (view.zoom * factor).clamp(0.2, 2.5);
                    view.offset = centre - at * view.zoom;
                }
            };
            zoom_by(ui, crate::icons::Icon::Minus, tr!("network-zoom-out"), 0.8);
            zoom_by(ui, crate::icons::Icon::Plus, tr!("network-zoom-in"), 1.25);
            if crate::icon_button(ui, crate::icons::Icon::Scale, &tr!("network-fit")) {
                let whole = vec2(columns as f32 * (BOX.x + GAP.x), rows as f32 * (BOX.y + GAP.y)) + vec2(32.0, 32.0);
                view.zoom = (rect.width() / whole.x).min(rect.height() / whole.y).clamp(0.2, 1.0);
                view.offset = vec2(16.0, 16.0);
            }
        });
    });
    if columns == 0 {
        painter.text(rect.center(), Align2::CENTER_CENTER, tr!("network-empty"), FontId::proportional(13.0), t.text_dim);
    }
    events
}

#[cfg(test)]
mod tests {
    use mkperth_engine::core::{Activity, Relationship, RelationshipType};

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

    /// A box's ends and the four kinds drawn between them: from the right dot (its finish) or the
    /// left (its start), onto the other's left half (its start) or right half (its finish).
    #[test]
    fn the_ends_and_the_kinds() {
        let b = Rect::from_min_size(pos2(100.0, 100.0), BOX);
        assert_eq!(handle_at(b, b.left_center() + vec2(3.0, 2.0), 1.0), Some(true));
        assert_eq!(handle_at(b, b.right_center() - vec2(5.0, 0.0), 1.0), Some(false));
        assert_eq!(handle_at(b, b.center(), 1.0), None);
        assert_eq!(handle_at(b, b.right_center() + vec2(20.0, 0.0), 1.0), None);
        assert_eq!([tie_kind(false, false), tie_kind(false, true), tie_kind(true, false), tie_kind(true, true)], ["FS", "FF", "SS", "SF"]);
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
}
