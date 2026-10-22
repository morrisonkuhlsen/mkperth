//! The Activities view: the activity table beside the Gantt chart, one row per activity in WBS
//! order, only the visible rows painted, the table and the chart scrolling together.

use egui::{Align2, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use mkperth_engine::Session;
use mkperth_engine::core::{ActivityKind, Date};
use mkperth_engine::i18n::tr;
use mkperth_engine::sched::Scheduled;

use crate::show_date;
use crate::theme::Tokens;

const ROW: f32 = 24.0;
const HEADER: f32 = 44.0;

/// The table's columns: a message id and a width.
const COLUMNS: [(&str, f32); 9] = [
    ("col-id", 84.0),
    ("col-name", 230.0),
    ("col-duration", 60.0),
    ("col-early-start", 84.0),
    ("col-early-finish", 84.0),
    ("col-late-start", 84.0),
    ("col-late-finish", 84.0),
    ("col-total-float", 72.0),
    ("col-free-float", 72.0),
];

/// What the view shows, from the ribbon and the Filters pane.
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub critical_only: bool,
    pub late_bars: bool,
    /// Rows whose ID or name holds this text (any case), with their summaries.
    pub search: String,
}

/// The rows: each activity's index and its depth in the WBS, depth first.
pub(crate) fn rows(session: &Session) -> Vec<(usize, usize)> {
    let acts = &session.project.activities;
    let index: std::collections::HashMap<&str, usize> = acts.iter().enumerate().map(|(i, a)| (a.id.as_str(), i)).collect();
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); acts.len()];
    let mut top = Vec::new();
    for (i, a) in acts.iter().enumerate() {
        match a.parent.as_deref().and_then(|p| index.get(p)) {
            Some(&p) if p != i => {
                if let Some(c) = children.get_mut(p) {
                    c.push(i);
                }
            }
            _ => top.push(i),
        }
    }
    // Without recursion; a node is never visited twice, so a loop can't trap it.
    let mut out = Vec::with_capacity(acts.len());
    let mut seen = vec![false; acts.len()];
    let mut stack: Vec<(usize, usize)> = top.into_iter().rev().map(|i| (i, 0)).collect();
    while let Some((i, depth)) = stack.pop() {
        if seen.get(i).copied().unwrap_or(true) {
            continue;
        }
        if let Some(s) = seen.get_mut(i) {
            *s = true;
        }
        out.push((i, depth));
        for &c in children.get(i).map_or(&[][..], Vec::as_slice).iter().rev() {
            stack.push((c, depth + 1));
        }
    }
    out
}

/// The rows the options let through: an activity that matches, and the summaries above it.
pub(crate) fn visible(session: &Session, options: &Options) -> Vec<(usize, usize)> {
    let all = rows(session);
    let Ok(schedule) = &session.schedule else { return all };
    let needle = options.search.trim().to_lowercase();
    let acts = &session.project.activities;
    let summary: Vec<bool> = (0..acts.len()).map(|i| acts.iter().any(|c| c.parent.as_deref() == acts.get(i).map(|a| a.id.as_str()))).collect();
    let matches = |i: usize| {
        let (Some(a), Some(s)) = (acts.get(i), schedule.activities.get(i)) else { return false };
        (!options.critical_only || s.critical)
            && (needle.is_empty() || a.id.to_lowercase().contains(&needle) || a.name.to_lowercase().contains(&needle))
    };
    let mut keep = vec![false; acts.len()];
    let index: std::collections::HashMap<&str, usize> = acts.iter().enumerate().map(|(i, a)| (a.id.as_str(), i)).collect();
    for i in 0..acts.len() {
        if summary.get(i) == Some(&false) && matches(i) {
            // The activity and every summary above it.
            let mut at = Some(i);
            let mut steps = 0;
            while let Some(k) = at {
                if keep.get(k) == Some(&true) || steps > acts.len() {
                    break;
                }
                if let Some(slot) = keep.get_mut(k) {
                    *slot = true;
                }
                at = acts.get(k).and_then(|a| a.parent.as_deref()).and_then(|p| index.get(p).copied());
                steps += 1;
            }
        }
    }
    all.into_iter().filter(|(i, _)| keep.get(*i) == Some(&true)).collect()
}

/// Paints the view in the space given; returns the activity clicked, if any.
pub(crate) fn show(ui: &mut egui::Ui, session: &Session, options: &Options, selected: Option<usize>) -> Option<usize> {
    let t = Tokens::get(ui.ctx());
    let Ok(schedule) = &session.schedule else { return None };
    let rows = visible(session, options);
    let acts = &session.project.activities;
    let table_width: f32 = COLUMNS.iter().map(|(_, w)| w).sum();
    let full = ui.available_rect_before_wrap();
    let chart_left = full.left() + table_width + 1.0;
    let chart_width = (full.right() - chart_left).max(60.0);
    let mut first = session.project.start;
    let mut last = schedule.finish;
    for a in &schedule.activities {
        first = first.min(a.early_start).min(a.late_start);
        last = last.max(a.late_finish).max(a.early_finish);
    }
    let from = first.days() - 2;
    let days = (last.days() + 4 - from).max(7);
    let scale = chart_width / days as f32;
    let x_of = |d: i32, left: f32| left + (d - from) as f32 * scale;

    // The header: column titles; months and weeks over the chart.
    let (header, _) = ui.allocate_exact_size(vec2(full.width(), HEADER), Sense::hover());
    let painter = ui.painter_at(header);
    painter.rect_filled(header, 0.0, t.header_bg);
    painter.hline(header.x_range(), header.bottom(), Stroke::new(1.0, t.header_line));
    let mut x = header.left();
    for (id, w) in COLUMNS {
        let cell = Rect::from_min_size(pos2(x, header.top()), vec2(w, HEADER));
        painter.with_clip_rect(cell.shrink(2.0)).text(
            pos2(x + 6.0, header.center().y),
            Align2::LEFT_CENTER,
            tr!(id),
            FontId::proportional(12.0),
            t.header_text,
        );
        x += w;
        painter.vline(x, header.y_range(), Stroke::new(1.0, t.gridline));
    }
    let chart_header = painter.with_clip_rect(Rect::from_min_max(pos2(chart_left, header.top()), header.max));
    for k in 0..=days {
        let Some(d) = Date::from_days(from + k) else { continue };
        let (y, m, day) = d.ymd();
        let xd = x_of(d.days(), chart_left);
        // A month's name at its first day; the first month's at the left edge only when it has
        // room before the next one starts.
        let room = (32 - i32::try_from(day).unwrap_or(1)) as f32 * scale > 70.0;
        if day == 1 || (k == 0 && room) {
            chart_header.vline(xd, header.top()..=header.center().y, Stroke::new(1.0, t.gridline));
            chart_header.text(
                pos2(xd + 4.0, header.top() + 11.0),
                Align2::LEFT_CENTER,
                format!("{} {y}", tr!(&format!("month-{m}"))),
                FontId::proportional(12.0),
                t.header_text,
            );
        }
        if d.weekday().index() == 0 && scale * 7.0 >= 22.0 {
            chart_header.vline(xd, header.center().y..=header.bottom(), Stroke::new(1.0, t.gridline));
            chart_header.text(pos2(xd + 3.0, header.bottom() - 10.0), Align2::LEFT_CENTER, day.to_string(), FontId::proportional(11.0), t.text_dim);
        }
    }

    let calendar = &session.project.calendar;
    let mut clicked = None;
    egui::ScrollArea::vertical().id_salt("activities").auto_shrink(false).show_rows(ui, ROW, rows.len(), |ui, range| {
        for &(i, depth) in rows.iter().skip(range.start).take(range.len()) {
            let (rect, response) = ui.allocate_exact_size(vec2(full.width(), ROW), Sense::click());
            if response.clicked() {
                clicked = Some(i);
            }
            let (Some(a), Some(s)) = (acts.get(i), schedule.activities.get(i)) else { continue };
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, if selected == Some(i) { t.accent_soft } else { t.grid_bg });
            // Non-working days of the project's calendar, shaded, when they're wide enough.
            if scale >= 3.0 {
                for k in 0..days {
                    if let Some(d) = Date::from_days(from + k).filter(|d| !calendar.is_working(*d)) {
                        let xd = x_of(d.days(), chart_left);
                        painter.rect_filled(Rect::from_min_max(pos2(xd, rect.top()), pos2(xd + scale, rect.bottom())), 0.0, t.header_bg);
                    }
                }
            }
            let summary = acts.iter().any(|c| c.parent.as_deref() == Some(a.id.as_str()));
            let duration = if summary { String::new() } else { a.working_days().to_string() };
            let float_color = if s.critical && !summary { t.red } else { t.cell_text };
            let cells = [
                (a.id.clone(), t.cell_text, 0.0),
                (a.name.clone(), if summary { t.text } else { t.cell_text }, depth as f32 * 14.0),
                (duration, t.cell_text, 0.0),
                (show_date(s.early_start), t.cell_text, 0.0),
                (show_date(s.early_finish), t.cell_text, 0.0),
                (show_date(s.late_start), t.text_dim, 0.0),
                (show_date(s.late_finish), t.text_dim, 0.0),
                (s.total_float.to_string(), float_color, 0.0),
                (s.free_float.to_string(), t.cell_text, 0.0),
            ];
            let mut x = rect.left();
            for ((text, color, indent), (_, w)) in cells.into_iter().zip(COLUMNS) {
                let cell = Rect::from_min_size(pos2(x, rect.top()), vec2(w, ROW));
                let painter = painter.with_clip_rect(cell.shrink(1.0));
                painter.text(pos2(x + 6.0 + indent, rect.center().y), Align2::LEFT_CENTER, &text, FontId::proportional(12.5), color);
                if summary {
                    // Bold by drawing twice: the built-in fonts have no bold face.
                    painter.text(pos2(x + 6.5 + indent, rect.center().y), Align2::LEFT_CENTER, &text, FontId::proportional(12.5), color);
                }
                x += w;
                painter.vline(x, rect.y_range(), Stroke::new(1.0, t.gridline));
            }
            let chart = painter.with_clip_rect(Rect::from_min_max(pos2(chart_left, rect.top()), rect.max));
            bar(&chart, a.kind, summary, s, rect, &|d: Date| x_of(d.days(), chart_left), scale, options.late_bars, &t);
            painter.hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, t.gridline));
        }
    });
    if rows.is_empty() {
        ui.painter().text(full.center(), Align2::CENTER_CENTER, tr!("activities-none"), FontId::proportional(13.0), t.text_dim);
    }
    clicked
}

/// An activity's bars: the early one (red on the critical path), the late one thin beneath it;
/// a summary as a bracket; a milestone as a diamond.
#[allow(clippy::too_many_arguments)]
fn bar(
    painter: &egui::Painter,
    kind: ActivityKind,
    summary: bool,
    s: &Scheduled,
    row: Rect,
    x_of: &dyn Fn(Date) -> f32,
    scale: f32,
    late: bool,
    t: &Tokens,
) {
    let mid = row.center().y;
    let span = |a: Date, b: Date| (x_of(a), x_of(b) + scale);
    if summary {
        let (l, r) = span(s.early_start, s.early_finish);
        let top = mid - 5.0;
        painter.rect_filled(Rect::from_min_max(pos2(l, top), pos2(r, top + 4.0)), 0.0, t.text);
        for x in [l, r] {
            painter.add(egui::Shape::convex_polygon(vec![pos2(x - 4.0, top), pos2(x + 4.0, top), pos2(x, top + 9.0)], t.text, Stroke::NONE));
        }
        return;
    }
    let color = if s.critical { t.red } else { t.blue };
    if kind.is_milestone() {
        // A start milestone at the morning of its day, a finish milestone at the evening.
        let x = if kind == ActivityKind::FinishMilestone { x_of(s.early_finish) + scale } else { x_of(s.early_start) };
        let r = 6.0;
        painter.add(egui::Shape::convex_polygon(vec![pos2(x, mid - r), pos2(x + r, mid), pos2(x, mid + r), pos2(x - r, mid)], color, Stroke::NONE));
        return;
    }
    let (l, r) = span(s.early_start, s.early_finish);
    painter.rect(Rect::from_min_max(pos2(l, mid - 7.0), pos2(r.max(l + 2.0), mid + 3.0)), 0.0, color, Stroke::NONE, StrokeKind::Inside);
    if late {
        let (ll, lr) = span(s.late_start, s.late_finish);
        painter.rect_filled(Rect::from_min_max(pos2(ll, mid + 5.0), pos2(lr.max(ll + 2.0), mid + 8.0)), 0.0, t.text_dim.gamma_multiply(0.6));
        // The total float, from the early finish to the late finish.
        if s.total_float > 0 {
            painter.line_segment([Pos2::new(r, mid - 2.0), Pos2::new(lr, mid - 2.0)], Stroke::new(1.0, t.text_dim));
        }
    }
}
