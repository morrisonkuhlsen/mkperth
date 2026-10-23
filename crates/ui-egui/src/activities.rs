//! The Activities view: the activity table beside the Gantt chart, one row per activity in WBS
//! order, only the visible rows painted, the table and the chart scrolling together.

use egui::{Align2, FontId, Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use mkperth_engine::Session;
use mkperth_engine::core::{ActivityKind, Date, RelationshipType};
use mkperth_engine::i18n::tr;
use mkperth_engine::sched::{Scheduled, Status};

use crate::show_date;
use crate::theme::Tokens;

const ROW: f32 = 24.0;
const HEADER: f32 = 44.0;

/// The table's columns: a message id and a width. The ID and the name are always shown.
pub const COLUMNS: [(&str, f32); 9] = [
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

/// A table cell that can be edited in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Id,
    Name,
    Duration,
}

/// The cell being edited: the activity's index, the column, the text so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Editing {
    pub index: usize,
    pub column: Column,
    pub text: String,
}

/// What happened in the view this frame.
#[derive(Debug, Default)]
pub(crate) struct Events {
    pub clicked: Option<usize>,
    /// An edit to make: the activity's index, the column, the new text.
    pub commit: Option<(usize, Column, String)>,
}

/// What the view shows, from the ribbon and the Filters pane.
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub critical_only: bool,
    pub late_bars: bool,
    /// Relationship lines between the bars.
    pub links: bool,
    /// The time scale in points per day; `None` fits the whole project in the chart.
    pub scale: Option<f32>,
    /// Days scrolled past the chart's first day (with a set scale).
    pub offset: f32,
    /// Rows whose ID or name holds this text (any case), with their summaries.
    pub search: String,
    /// The columns hidden, by their place in `COLUMNS`.
    pub hidden: [bool; 9],
    /// Today, drawn as a line on the chart.
    pub today: Option<Date>,
}

impl Options {
    /// The columns shown: their place in `COLUMNS`, message id and width.
    pub fn columns(&self) -> Vec<(usize, &'static str, f32)> {
        COLUMNS
            .iter()
            .enumerate()
            .filter(|(k, _)| *k < 2 || !self.hidden.get(*k).copied().unwrap_or(false))
            .map(|(k, (id, w))| (k, *id, *w))
            .collect()
    }
}

/// A note's colour: its own (`#rrggbb`), else the one given.
pub(crate) fn note_color(hex: &str, fallback: egui::Color32) -> egui::Color32 {
    let digits = hex.strip_prefix('#').filter(|h| h.len() == 6);
    let part = |k: usize| digits.and_then(|h| h.get(k..k + 2)).and_then(|p| u8::from_str_radix(p, 16).ok());
    match (part(0), part(2), part(4)) {
        (Some(r), Some(g), Some(b)) => egui::Color32::from_rgb(r, g, b),
        _ => fallback,
    }
}

/// The narrowest and the widest a day may be drawn, in points.
pub const MIN_SCALE: f32 = 0.5;
pub const MAX_SCALE: f32 = 80.0;

/// Which activities are summaries (have children), in one pass.
pub(crate) fn summaries(acts: &[mkperth_engine::core::Activity]) -> Vec<bool> {
    let parents: std::collections::HashSet<&str> = acts.iter().filter_map(|a| a.parent.as_deref()).collect();
    acts.iter().map(|a| parents.contains(a.id.as_str())).collect()
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
    let summary = summaries(acts);
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

/// Paints the view in the space given. A double click on an ID, a name or a duration edits it in
/// place: Enter or leaving the cell keeps the text, Esc drops it.
pub(crate) fn show(ui: &mut egui::Ui, session: &Session, options: &mut Options, selected: Option<usize>, editing: &mut Option<Editing>) -> Events {
    let t = Tokens::get(ui.ctx());
    let mut events = Events::default();
    let Ok(schedule) = &session.schedule else { return events };
    let rows = visible(session, options);
    let acts = &session.project.activities;
    let columns = options.columns();
    let table_width: f32 = columns.iter().map(|(_, _, w)| w).sum();
    // Where a column starts, if shown.
    let left_of = |k: usize| columns.iter().position(|(c, _, _)| *c == k).map(|n| columns.iter().take(n).map(|(_, _, w)| w).sum::<f32>());
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
    // The time scale: the whole project fitted, or a scale the person chose and scrolled.
    let fit = chart_width / days as f32;
    let chart_area = Rect::from_min_max(pos2(chart_left, full.top()), full.max);
    let pointer = ui.input(|i| i.pointer.hover_pos()).filter(|p| chart_area.contains(*p));
    if let Some(p) = pointer {
        let (zoom, pan) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta.x));
        let scale = options.scale.unwrap_or(fit);
        if (zoom - 1.0).abs() > f32::EPSILON {
            // Ctrl and the wheel (or a pinch): zoom about the day under the pointer.
            let day = options.offset + (p.x - chart_left) / scale;
            let new = (scale * zoom).clamp(MIN_SCALE, MAX_SCALE);
            options.scale = Some(new);
            options.offset = day - (p.x - chart_left) / new;
        } else if pan.abs() > 0.0 && options.scale.is_some() {
            // Shift and the wheel, or a sideways swipe: scroll in time.
            options.offset -= pan / scale;
        }
    }
    let scale = options.scale.unwrap_or(fit);
    // Never scrolled past either end.
    let visible_days = chart_width / scale;
    options.offset = if options.scale.is_none() { 0.0 } else { options.offset.clamp(0.0, (days as f32 - visible_days).max(0.0)) };
    let offset = options.offset;
    let x_of = |d: i32, left: f32| left + ((d - from) as f32 - offset) * scale;
    // Only the days in view are drawn.
    let first_day = offset.floor() as i32;
    let last_day = (offset + visible_days).ceil() as i32 + 1;

    // The header: column titles; months, weeks and days over the chart, as the scale allows.
    let (header, header_response) = ui.allocate_exact_size(vec2(full.width(), HEADER), Sense::drag());
    let painter = ui.painter_at(header);
    painter.rect_filled(header, 0.0, t.header_bg);
    painter.hline(header.x_range(), header.bottom(), Stroke::new(1.0, t.header_line));
    let mut x = header.left();
    for &(_, id, w) in &columns {
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
    // Dragging the dates scrolls in time.
    if header_response.dragged() && header_response.interact_pointer_pos().is_some_and(|p| p.x > chart_left) {
        if options.scale.is_none() {
            options.scale = Some(scale);
        }
        options.offset -= header_response.drag_delta().x / scale;
    }
    let chart_header = painter.with_clip_rect(Rect::from_min_max(pos2(chart_left, header.top()), header.max));
    if let Some(d) = session.project.data_date {
        let xd = x_of(d.days(), chart_left);
        chart_header.vline(xd, header.y_range(), Stroke::new(1.5, t.green.gamma_multiply(0.7)));
    }
    // Today, at the middle of its day.
    if let Some(d) = options.today {
        let xd = x_of(d.days(), chart_left) + scale / 2.0;
        chart_header.vline(xd, header.y_range(), Stroke::new(1.5, t.orange.gamma_multiply(0.8)));
    }
    let mut month_end = f32::NEG_INFINITY;
    for k in first_day.max(0)..=last_day.min(days) {
        let Some(d) = Date::from_days(from + k) else { continue };
        let (y, m, day) = d.ymd();
        let xd = x_of(d.days(), chart_left);
        // A month's name at its first day (the first one in view at the left edge), when it
        // doesn't run into the one before.
        if day == 1 || k == first_day.max(0) {
            let mark = session.project.month_marks.iter().find(|x| x.month.ymd().0 == y && x.month.ymd().1 == m);
            let text = match mark.filter(|x| !x.name.is_empty()) {
                Some(x) => format!("{} {y} · {}", tr!(&format!("month-{m}")), x.name),
                None => format!("{} {y}", tr!(&format!("month-{m}"))),
            };
            // A marked month: its strip of the ruler painted, a line along its foot.
            if let Some(x) = mark {
                let color = note_color(&x.color, t.red);
                let first = x.month.days();
                let next = Date::from_ymd(if m == 12 { y + 1 } else { y }, if m == 12 { 1 } else { m + 1 }, 1).map_or(first + 31, |n| n.days());
                let strip = Rect::from_min_max(pos2(x_of(first, chart_left), header.top()), pos2(x_of(next, chart_left), header.center().y));
                chart_header.rect_filled(strip, 0.0, color.gamma_multiply(0.22));
                chart_header.hline(strip.x_range(), strip.bottom() - 1.0, Stroke::new(2.0, color));
            }
            let galley = chart_header.layout_no_wrap(text, FontId::proportional(12.0), t.header_text);
            let at = if day == 1 { xd } else { xd.max(chart_left) };
            if day == 1 {
                chart_header.vline(xd, header.top()..=header.center().y, Stroke::new(1.0, t.gridline));
            }
            // The name held at the left edge gives way to the next month's when they'd meet.
            let next = Date::from_ymd(if m == 12 { y + 1 } else { y }, if m == 12 { 1 } else { m + 1 }, 1).map(|n| x_of(n.days(), chart_left));
            let crowded = day != 1 && next.is_some_and(|n| n < at + 4.0 + galley.size().x + 8.0);
            if at + 4.0 > month_end && !crowded {
                month_end = at + 4.0 + galley.size().x + 8.0;
                chart_header.galley(pos2(at + 4.0, header.top() + 11.0 - galley.size().y / 2.0), galley, t.header_text);
            }
        }
        // Every day when a day is wide; Mondays when a week is.
        let every_day = scale >= 18.0;
        if every_day || (d.weekday().index() == 0 && scale * 7.0 >= 22.0) {
            chart_header.vline(xd, header.center().y..=header.bottom(), Stroke::new(1.0, t.gridline));
            chart_header.text(pos2(xd + 3.0, header.bottom() - 10.0), Align2::LEFT_CENTER, day.to_string(), FontId::proportional(11.0), t.text_dim);
        }
    }

    let calendar = &session.project.calendar;
    let data_date = session.project.data_date;
    let row_of: std::collections::HashMap<usize, usize> = rows.iter().enumerate().map(|(n, &(i, _))| (i, n)).collect();
    let index_of: std::collections::HashMap<&str, usize> = acts.iter().enumerate().map(|(i, a)| (a.id.as_str(), i)).collect();
    let summary_of = summaries(acts);
    let is_summary = |i: usize| summary_of.get(i).copied().unwrap_or(false);
    let mut clicked = None;
    let mut double = None;
    egui::ScrollArea::vertical().id_salt("activities").auto_shrink(false).show_rows(ui, ROW, rows.len(), |ui, range| {
        // In layers: the rows' backgrounds, then the relationship lines, then the cells, bars and
        // names over them.
        let mut first_top = None;
        let mut placed = Vec::with_capacity(range.len());
        for &(i, depth) in rows.iter().skip(range.start).take(range.len()) {
            let (rect, response) = ui.allocate_exact_size(vec2(full.width(), ROW), Sense::click());
            first_top.get_or_insert(rect.top());
            if response.clicked() {
                clicked = Some(i);
            }
            if response.double_clicked()
                && let Some(pos) = response.interact_pointer_pos()
            {
                let x = pos.x - rect.left();
                let (mut at, mut hit) = (0.0, None);
                for &(k, _, w) in &columns {
                    if x >= at && x < at + w {
                        hit = Some(k);
                    }
                    at += w;
                }
                double = match hit {
                    Some(0) => Some((i, Column::Id)),
                    Some(1) => Some((i, Column::Name)),
                    Some(2) => Some((i, Column::Duration)),
                    _ => None,
                };
            }
            if acts.get(i).is_none() || schedule.activities.get(i).is_none() {
                continue;
            }
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, if selected == Some(i) { t.accent_soft } else { t.grid_bg });
            // Non-working days of the project's calendar, shaded, when they're wide enough.
            if scale >= 8.0 {
                for k in first_day.max(0)..last_day.min(days) {
                    if let Some(d) = Date::from_days(from + k).filter(|d| !calendar.is_working(*d)) {
                        let xd = x_of(d.days(), chart_left);
                        painter.rect_filled(Rect::from_min_max(pos2(xd, rect.top()), pos2(xd + scale, rect.bottom())), 0.0, t.header_bg);
                    }
                }
            }
            // The bands behind the bars, and the markers' lines.
            let chart_row = painter.with_clip_rect(Rect::from_min_max(pos2(chart_left, rect.top()), rect.max));
            for b in &session.project.bands {
                let color = note_color(&b.color, t.blue);
                let (l, r) = (x_of(b.from.days(), chart_left), x_of(b.to.days(), chart_left) + scale);
                chart_row.rect_filled(Rect::from_min_max(pos2(l, rect.top()), pos2(r, rect.bottom())), 0.0, color.gamma_multiply(0.16));
                chart_row.vline(l, rect.y_range(), Stroke::new(1.0, color.gamma_multiply(0.45)));
            }
            for m in &session.project.markers {
                let x = x_of(m.date.days(), chart_left) + scale / 2.0;
                let color = note_color(&m.color, t.orange);
                chart_row.extend(egui::Shape::dashed_line(&[pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.2, color), 3.0, 3.0));
            }
            if let Some(d) = options.today {
                let xd = x_of(d.days(), chart_left) + scale / 2.0;
                if xd >= chart_left {
                    painter.vline(xd, rect.y_range(), Stroke::new(1.0, t.orange.gamma_multiply(0.6)));
                }
            }
            // The data date: a line down the chart where progress was told.
            if let Some(d) = data_date {
                let xd = x_of(d.days(), chart_left);
                if xd >= chart_left {
                    painter.vline(xd, rect.y_range(), Stroke::new(1.5, t.green.gamma_multiply(0.7)));
                }
            }
            placed.push((i, depth, rect));
        }
        // The names of bands and markers, turned along their lines, over the rows in view.
        let view = ui.clip_rect().intersect(Rect::from_min_max(pos2(chart_left, ui.clip_rect().top()), full.max));
        let names = ui.painter().with_clip_rect(view);
        let turned = |x: f32, y: f32, text: &str, color: egui::Color32| {
            let galley = names.layout_no_wrap(text.to_string(), FontId::monospace(10.5), color);
            // A quarter turn clockwise: it reads downwards, its height to the left of `x`.
            names.add(egui::epaint::TextShape::new(pos2(x + galley.size().y + 2.0, y), galley, color).with_angle(std::f32::consts::FRAC_PI_2));
        };
        for b in &session.project.bands {
            turned(x_of(b.from.days(), chart_left) + 3.0, view.top() + 8.0, &b.name, note_color(&b.color, t.text_dim));
        }
        for m in &session.project.markers {
            let y = view.top() + 6.0 + (view.height() - 60.0).max(0.0) * f32::from(m.label_at) / 100.0;
            turned(x_of(m.date.days(), chart_left) + scale / 2.0 + 1.0, y, &m.name, note_color(&m.color, t.orange));
        }
        // The relationship lines, under the bars, from the predecessor's start or finish to the
        // successor's; rows scrolled out of view still anchor them, clipped to the chart.
        if options.links
            && let Some(top) = first_top
        {
            let clip = Rect::from_min_max(pos2(chart_left, ui.clip_rect().top()), pos2(full.right(), ui.clip_rect().bottom()));
            let painter = ui.painter().with_clip_rect(clip);
            let y_of = |n: usize| top + (n as f32 - range.start as f32) * ROW + ROW / 2.0 - 2.0;
            // Where an activity's work begins and ends on the chart.
            let ends = |i: usize| -> Option<(f32, f32)> {
                let (a, s) = (acts.get(i)?, schedule.activities.get(i)?);
                let start = x_of(s.early_start.days(), chart_left);
                let finish = x_of(s.early_finish.days(), chart_left) + scale;
                Some(match a.kind {
                    ActivityKind::StartMilestone => (start, start),
                    ActivityKind::FinishMilestone => (finish, finish),
                    ActivityKind::Task if a.working_days() == 0 => (start, start),
                    ActivityKind::Task => (start, finish),
                })
            };
            for r in &session.project.relationships {
                let (Some(&p), Some(&q)) = (index_of.get(r.predecessor.as_str()), index_of.get(r.successor.as_str())) else { continue };
                if is_summary(p) || is_summary(q) {
                    continue;
                }
                let (Some(&np), Some(&nq)) = (row_of.get(&p), row_of.get(&q)) else { continue };
                // Only the lines that touch the rows in view.
                let (lo, hi) = (np.min(nq), np.max(nq));
                if hi < range.start || lo > range.end {
                    continue;
                }
                let (Some(pe), Some(qe)) = (ends(p), ends(q)) else { continue };
                let from_finish = matches!(r.kind, RelationshipType::FinishToStart | RelationshipType::FinishToFinish);
                let to_start = matches!(r.kind, RelationshipType::FinishToStart | RelationshipType::StartToStart);
                let x0 = if from_finish { pe.1 } else { pe.0 };
                let x1 = if to_start { qe.0 } else { qe.1 };
                let (y0, y1) = (y_of(np), y_of(nq));
                let critical = schedule.activities.get(p).is_some_and(|s| s.critical) && schedule.activities.get(q).is_some_and(|s| s.critical);
                let color = if critical { t.red.gamma_multiply(0.85) } else { t.text_dim.gamma_multiply(0.75) };
                link(&painter, (x0, y0), (x1, y1), from_finish, to_start, color);
            }
        }
        for (i, depth, rect) in placed {
            let (Some(a), Some(s)) = (acts.get(i), schedule.activities.get(i)) else { continue };
            let painter = ui.painter_at(rect);
            let summary = is_summary(i);
            let day = session
                .project
                .activities
                .get(i)
                .and_then(|a| a.calendar.as_deref())
                .and_then(|n| session.project.calendars.iter().find(|c| c.name == n))
                .map_or(session.project.calendar.day_minutes(), |c| c.day_minutes());
            let duration = if summary { String::new() } else { crate::days_text(a.working_minutes(day), day) };
            let float_color = if s.critical && !summary { t.red } else { t.cell_text };
            let cells = [
                (a.id.clone(), t.cell_text, 0.0),
                (a.name.clone(), if summary { t.text } else { t.cell_text }, depth as f32 * 14.0),
                (duration, t.cell_text, 0.0),
                (show_date(s.early_start), t.cell_text, 0.0),
                (show_date(s.early_finish), t.cell_text, 0.0),
                (show_date(s.late_start), t.text_dim, 0.0),
                (show_date(s.late_finish), t.text_dim, 0.0),
                (crate::days_text(s.total_float_minutes, day), float_color, 0.0),
                (crate::days_text(s.free_float_minutes, day), t.cell_text, 0.0),
            ];
            let mut x = rect.left();
            for (k, (text, color, indent)) in cells.into_iter().enumerate() {
                let Some(&(_, _, w)) = columns.iter().find(|(c, _, _)| *c == k) else { continue };
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
            // The cell being edited: a text box over it.
            if let Some(e) = editing.as_mut().filter(|e| e.index == i) {
                let k = match e.column {
                    Column::Id => 0,
                    Column::Name => 1,
                    Column::Duration => 2,
                };
                // A hidden column isn't edited.
                let (Some(left), Some(width)) = (left_of(k), COLUMNS.get(k).map(|c| c.1)) else {
                    *editing = None;
                    continue;
                };
                let cell = Rect::from_min_size(pos2(rect.left() + left, rect.top() + 1.0), vec2(width, ROW - 2.0));
                let edit = ui.put(cell, egui::TextEdit::singleline(&mut e.text).id_salt(("cell", i)).margin(vec2(4.0, 2.0)));
                if !edit.has_focus() && !edit.lost_focus() {
                    edit.request_focus();
                }
                if ui.input(|inp| inp.key_pressed(egui::Key::Escape)) {
                    *editing = None;
                } else if edit.lost_focus() {
                    events.commit = Some((e.index, e.column, e.text.clone()));
                    *editing = None;
                }
            }
            let chart = painter.with_clip_rect(Rect::from_min_max(pos2(chart_left, rect.top()), rect.max));
            painter.hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, t.gridline));
            let background = if selected == Some(i) { t.accent_soft } else { t.grid_bg };
            let look = Look { summary, selected: selected == Some(i), float: options.late_bars, background, data_date };
            bar(&chart, a.kind, &a.name, s, rect, &|d: Date| x_of(d.days(), chart_left), scale, look, &t);
        }
    });
    // A thin scroll bar under the chart when part of it is out of view.
    if options.scale.is_some() && visible_days < days as f32 {
        let track = Rect::from_min_max(pos2(chart_left + 2.0, full.bottom() - 9.0), pos2(full.right() - 2.0, full.bottom() - 3.0));
        let response = ui.interact(track, ui.id().with("time-scroll"), Sense::drag());
        let share = (visible_days / days as f32).clamp(0.05, 1.0);
        let thumb_width = track.width() * share;
        let at = track.left() + (track.width() - thumb_width) * (options.offset / (days as f32 - visible_days).max(1.0));
        let thumb = Rect::from_min_size(pos2(at, track.top()), vec2(thumb_width, track.height()));
        ui.painter().rect_filled(track, 3.0, t.header_bg);
        ui.painter().rect_filled(thumb, 3.0, if response.hovered() || response.dragged() { t.text_dim } else { t.header_line });
        if response.dragged() {
            options.offset += response.drag_delta().x / (track.width() - thumb_width).max(1.0) * (days as f32 - visible_days);
        }
    }
    if rows.is_empty() {
        ui.painter().text(full.center(), Align2::CENTER_CENTER, tr!("activities-none"), FontId::proportional(13.0), t.text_dim);
    }
    if let Some((i, column)) = double {
        let summary = is_summary(i);
        if let Some(a) = acts.get(i).filter(|_| !(summary && column == Column::Duration)) {
            let text = match column {
                Column::Id => a.id.clone(),
                Column::Name => a.name.clone(),
                Column::Duration => {
                    let day = session.project.calendar.day_minutes();
                    crate::days_text(a.working_minutes(day), day)
                }
            };
            *editing = Some(Editing { index: i, column, text });
        }
    }
    events.clicked = clicked;
    events
}

/// How a bar is drawn beyond its dates.
#[derive(Clone, Copy)]
struct Look {
    summary: bool,
    selected: bool,
    /// The total float as a light bar to the late finish.
    float: bool,
    /// The row's background, behind the name so lines passing under it don't cross the text.
    background: egui::Color32,
    /// Where the work done ends on an activity under way.
    data_date: Option<Date>,
}

/// A colour a little darker, for a bar's edge.
fn darker(c: egui::Color32) -> egui::Color32 {
    let f = |v: u8| (f32::from(v) * 0.78) as u8;
    egui::Color32::from_rgb(f(c.r()), f(c.g()), f(c.b()))
}

/// An activity's bar: rounded, red on the critical path, its float a light bar to the late
/// finish, its name beside it; a summary as a thin bracket over its span; a milestone as a
/// diamond.
#[allow(clippy::too_many_arguments)]
fn bar(
    painter: &egui::Painter,
    kind: ActivityKind,
    name: &str,
    s: &Scheduled,
    row: Rect,
    x_of: &dyn Fn(Date) -> f32,
    scale: f32,
    look: Look,
    t: &Tokens,
) {
    let mid = row.center().y - 2.0;
    let span = |a: Date, b: Date| (x_of(a), x_of(b) + scale);
    let label = |x: f32| {
        let galley = painter.layout_no_wrap(name.to_string(), FontId::proportional(11.0), t.text_dim);
        let at = pos2(x + 6.0, mid - galley.size().y / 2.0);
        painter.rect_filled(Rect::from_min_size(at - vec2(2.0, 0.0), galley.size() + vec2(4.0, 0.0)), 2.0, look.background);
        painter.galley(at, galley, t.text_dim);
    };
    if look.summary {
        let (l, r) = span(s.early_start, s.early_finish);
        let color = t.text.gamma_multiply(0.8);
        painter.rect_filled(Rect::from_min_max(pos2(l, mid - 3.0), pos2(r, mid + 1.0)), 1.0, color);
        for x in [l, r] {
            painter.add(egui::Shape::convex_polygon(
                vec![pos2(x - 3.5, mid + 1.0), pos2(x + 3.5, mid + 1.0), pos2(x, mid + 6.0)],
                color,
                Stroke::NONE,
            ));
        }
        return;
    }
    // What happened in green: a completed activity whole, an activity under way up to the data date.
    let color = if s.status == Status::Completed {
        t.green
    } else if s.critical {
        t.red
    } else {
        t.blue
    };
    if kind.is_milestone() || s.early_start > s.early_finish {
        // A start milestone at the morning of its day, a finish milestone at the evening.
        let x = if kind == ActivityKind::FinishMilestone { x_of(s.early_finish) + scale } else { x_of(s.early_start) };
        let r = 6.5;
        let diamond = vec![pos2(x, mid - r), pos2(x + r, mid), pos2(x, mid + r), pos2(x - r, mid)];
        let edge = if look.selected { Stroke::new(2.0, t.accent) } else { Stroke::new(1.0, darker(color)) };
        painter.add(egui::Shape::convex_polygon(diamond, color, edge));
        label(x + r);
        return;
    }
    let (l, r) = span(s.early_start, s.early_finish);
    let r = r.max(l + 3.0);
    // The float first, under the bar's end: a light band to the late finish, with a tick there.
    if look.float && s.total_float > 0 {
        let (_, lr) = span(s.late_start, s.late_finish);
        let band = Rect::from_min_max(pos2(r - 2.0, mid - 2.0), pos2(lr, mid + 2.0));
        painter.rect_filled(band, 2.0, color.gamma_multiply(0.28));
        painter.vline(lr, (mid - 4.0)..=(mid + 4.0), Stroke::new(1.0, color.gamma_multiply(0.6)));
    }
    let body = Rect::from_min_max(pos2(l, mid - 6.0), pos2(r, mid + 6.0));
    let edge = if look.selected { Stroke::new(2.0, t.accent) } else { Stroke::new(1.0, darker(color)) };
    painter.rect(body, 3.0, color, edge, StrokeKind::Inside);
    // A highlight along the top, for a little depth.
    painter.rect_filled(
        Rect::from_min_max(pos2(l + 2.0, mid - 5.0), pos2((r - 2.0).max(l + 2.0), mid - 3.5)),
        1.0,
        egui::Color32::from_white_alpha(46),
    );
    if s.status == Status::InProgress
        && let Some(d) = look.data_date
    {
        let done = x_of(d).clamp(l, r);
        if done > l {
            painter.rect_filled(
                Rect::from_min_max(pos2(l, mid - 6.0), pos2(done, mid + 6.0)),
                egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 },
                t.green,
            );
        }
    }
    let end = if look.float && s.total_float > 0 { span(s.late_start, s.late_finish).1 } else { r };
    label(end);
}

/// A relationship line: out of the predecessor (rightwards from a finish, leftwards from a
/// start), down or up to the successor's row, into it (from the left to a start, from the right
/// to a finish), an arrowhead at the end.
fn link(painter: &egui::Painter, from: (f32, f32), to: (f32, f32), from_finish: bool, to_start: bool, color: egui::Color32) {
    let (x0, y0) = from;
    let (x1, y1) = to;
    let stub = 6.0;
    let out = if from_finish { x0 + stub } else { x0 - stub };
    let into = if to_start { x1 - stub } else { x1 + stub };
    let down = if y1 >= y0 { 1.0 } else { -1.0 };
    let mut points = vec![pos2(x0, y0)];
    let direct = if to_start { into >= out } else { into <= out };
    if direct {
        // Straight down beside the target, then in.
        let x = if to_start { out.max(into) } else { out.min(into) };
        points.extend([pos2(x, y0), pos2(x, y1), pos2(x1, y1)]);
    } else {
        // Around: out, half a row towards the target, across, then down and in.
        let mid = y0 + down * (ROW / 2.0);
        points.extend([pos2(out, y0), pos2(out, mid), pos2(into, mid), pos2(into, y1), pos2(x1, y1)]);
    }
    painter.add(egui::Shape::line(points, Stroke::new(1.2, color)));
    let head = if to_start { -1.0 } else { 1.0 };
    painter.add(egui::Shape::convex_polygon(
        vec![pos2(x1, y1), pos2(x1 + head * 5.0, y1 - 3.5), pos2(x1 + head * 5.0, y1 + 3.5)],
        color,
        Stroke::NONE,
    ));
}
