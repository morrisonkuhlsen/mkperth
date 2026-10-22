//! MK Perth's window: the Activities view — the activity table beside the Gantt chart, early and
//! late dates, floats and the critical path — over the engine's session. The UI only reads the
//! session and acts through `Session::run`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod theme;

use std::path::PathBuf;

use egui::{Align2, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use mkperth_engine::Session;
use mkperth_engine::core::{ActivityKind, Date};
use mkperth_engine::i18n::{Lang, current, tr};
use mkperth_engine::sched::Scheduled;
use serde_json::json;

pub use mkperth_engine::i18n;
pub use theme::Tokens;

/// Shows an open-file dialog; `None` when cancelled.
pub type PickOpen = Box<dyn Fn() -> Option<PathBuf>>;

/// What the platform provides: dialogs come in from the app, so the UI stays testable.
#[derive(Default)]
pub struct Services {
    pub pick_open: Option<PickOpen>,
}

const ROW: f32 = 24.0;
const HEADER: f32 = 44.0;
const STATUS: f32 = 26.0;

/// The table's columns: a message id and a width.
const COLUMNS: [(&str, f32); 9] = [
    ("col-id", 84.0),
    ("col-name", 230.0),
    ("col-duration", 54.0),
    ("col-early-start", 84.0),
    ("col-early-finish", 84.0),
    ("col-late-start", 84.0),
    ("col-late-finish", 84.0),
    ("col-total-float", 72.0),
    ("col-free-float", 72.0),
];

pub struct PerthApp {
    pub session: Session,
    pub services: Services,
    /// The activity selected, by index in the project.
    pub selected: Option<usize>,
    /// What the last action said.
    pub status: Option<Result<String, String>>,
}

impl Default for PerthApp {
    fn default() -> Self {
        PerthApp::new(Services::default())
    }
}

/// A row of the table: an activity's index and its depth in the WBS.
fn rows(session: &Session) -> Vec<(usize, usize)> {
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
    // Depth first, without recursion; a node is never visited twice, so a loop can't trap it.
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

/// A date as the language writes it.
pub fn show_date(d: Date) -> String {
    let (y, m, day) = d.ymd();
    match current() {
        Lang::PtBr => format!("{day:02}/{m:02}/{y:04}"),
        Lang::En => format!("{day:02}-{}-{:02}", tr!(&format!("month-{m}")), y.rem_euclid(100)),
    }
}

impl PerthApp {
    pub fn new(services: Services) -> Self {
        PerthApp { session: Session::default(), services, selected: None, status: None }
    }

    /// Runs a command, keeping what it said for the status line.
    pub fn run(&mut self, id: &str, params: serde_json::Value) {
        self.status = Some(match self.session.run(id, &params) {
            Ok(()) => Ok(match id {
                "project.open" | "project.sample" => tr!("status-opened", name = self.session.project.name.as_str()),
                _ => tr!("status-scheduled"),
            }),
            Err(e) => Err(e.to_string()),
        });
        if id != "project.schedule" {
            self.selected = None;
        }
    }

    pub fn open(&mut self, path: &std::path::Path) {
        self.run("project.open", json!({"path": path.display().to_string()}));
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        ui.painter().rect_filled(ui.max_rect().expand(8.0), 0.0, t.background);
        if ui.input(|i| i.key_pressed(egui::Key::F9)) {
            self.run("project.schedule", json!(null));
        }
        if ui.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::O))) {
            self.pick_and_open();
        }
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("MK Perth").color(t.text).size(16.0));
            ui.separator();
            ui.label(egui::RichText::new(&self.session.project.name).color(t.text_soft));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(tr!("btn-schedule")).on_hover_text("F9").clicked() {
                    self.run("project.schedule", json!(null));
                }
                if ui.button(tr!("btn-sample")).clicked() {
                    self.run("project.sample", json!(null));
                }
                if ui.button(tr!("btn-open")).on_hover_text("Ctrl+O").clicked() {
                    self.pick_and_open();
                }
            });
        });
        ui.add_space(4.0);
        let body = (ui.available_height() - STATUS).max(80.0);
        ui.allocate_ui(vec2(ui.available_width(), body), |ui| {
            ui.set_min_size(vec2(ui.available_width(), body));
            match &self.session.schedule {
                Ok(_) => self.activities(ui, &t),
                Err(e) => {
                    ui.add_space(20.0);
                    ui.colored_label(t.critical, e.to_string());
                }
            }
        });
        self.status_line(ui, &t);
    }

    fn pick_and_open(&mut self) {
        if let Some(path) = self.services.pick_open.as_ref().and_then(|pick| pick()) {
            self.open(&path);
        }
    }

    /// The table and the chart, one row per activity, scrolling together.
    fn activities(&mut self, ui: &mut egui::Ui, t: &Tokens) {
        let Ok(schedule) = &self.session.schedule else { return };
        let rows = rows(&self.session);
        let table_width: f32 = COLUMNS.iter().map(|(_, w)| w).sum();
        let full = ui.available_rect_before_wrap();
        let chart_left = full.left() + table_width + 1.0;
        let chart_width = (full.right() - chart_left).max(60.0);
        // The chart's span: the project's start to the latest late finish, with a margin.
        let mut first = self.session.project.start;
        let mut last = schedule.finish;
        for a in &schedule.activities {
            first = first.min(a.early_start).min(a.late_start);
            last = last.max(a.late_finish).max(a.early_finish);
        }
        let from = first.days() - 2;
        let days = (last.days() + 4 - from).max(7);
        let scale = chart_width / days as f32;
        let x_of = |days_since: i32, left: f32| left + (days_since - from) as f32 * scale;

        // The header: column titles; months and weeks over the chart.
        let (header, _) = ui.allocate_exact_size(vec2(full.width(), HEADER), Sense::hover());
        let painter = ui.painter_at(header);
        painter.rect_filled(header, 0.0, t.header);
        let mut x = header.left();
        for (id, w) in COLUMNS {
            let cell = Rect::from_min_size(pos2(x, header.top()), vec2(w, HEADER));
            painter.with_clip_rect(cell.shrink(2.0)).text(
                pos2(x + 6.0, header.center().y),
                Align2::LEFT_CENTER,
                tr!(id),
                FontId::proportional(12.0),
                t.text,
            );
            x += w;
            painter.vline(x, header.y_range(), Stroke::new(1.0, t.grid));
        }
        let chart_header = painter.with_clip_rect(Rect::from_min_max(pos2(chart_left, header.top()), header.max));
        for k in 0..=days {
            let Some(d) = Date::from_days(from + k) else { continue };
            let (y, m, day) = d.ymd();
            let xd = x_of(d.days(), chart_left);
            // A month's name at its first day; the first month's at the left edge only when its
            // name has room before the next one starts.
            let room = (32 - i32::try_from(day).unwrap_or(1)) as f32 * scale > 70.0;
            if day == 1 || (k == 0 && room) {
                chart_header.vline(xd, header.top()..=header.center().y, Stroke::new(1.0, t.grid));
                chart_header.text(
                    pos2(xd + 4.0, header.top() + 11.0),
                    Align2::LEFT_CENTER,
                    format!("{} {y}", tr!(&format!("month-{m}"))),
                    FontId::proportional(12.0),
                    t.text,
                );
            }
            if d.weekday().index() == 0 && scale * 7.0 >= 22.0 {
                chart_header.vline(xd, header.center().y..=header.bottom(), Stroke::new(1.0, t.grid));
                chart_header.text(
                    pos2(xd + 3.0, header.bottom() - 10.0),
                    Align2::LEFT_CENTER,
                    day.to_string(),
                    FontId::proportional(11.0),
                    t.text_soft,
                );
            }
        }

        let calendar = &self.session.project.calendar;
        let mut clicked = None;
        egui::ScrollArea::vertical().auto_shrink(false).show_rows(ui, ROW, rows.len(), |ui, range| {
            for (n, &(i, depth)) in rows.iter().enumerate().skip(range.start).take(range.len()) {
                let (rect, response) = ui.allocate_exact_size(vec2(full.width(), ROW), Sense::click());
                if response.clicked() {
                    clicked = Some(i);
                }
                let (Some(a), Some(s)) = (self.session.project.activities.get(i), schedule.activities.get(i)) else { continue };
                let painter = ui.painter_at(rect);
                let fill = if self.selected == Some(i) {
                    t.selection
                } else if n % 2 == 1 {
                    t.stripe
                } else {
                    t.background
                };
                painter.rect_filled(rect, 0.0, fill);
                // Non-working days of the project's calendar, shaded, when they're wide enough.
                if scale >= 3.0 {
                    for k in 0..days {
                        if let Some(d) = Date::from_days(from + k).filter(|d| !calendar.is_working(*d)) {
                            let xd = x_of(d.days(), chart_left);
                            painter.rect_filled(Rect::from_min_max(pos2(xd, rect.top()), pos2(xd + scale, rect.bottom())), 0.0, t.non_working);
                        }
                    }
                }
                let summary =
                    rows.iter().any(|&(c, _)| self.session.project.activities.get(c).and_then(|x| x.parent.as_deref()) == Some(a.id.as_str()));
                let duration = if summary { String::new() } else { a.working_days().to_string() };
                let critical_text = if s.critical && !summary { t.critical } else { t.text };
                let cells = [
                    (a.id.clone(), t.text, 0.0),
                    (a.name.clone(), if summary { t.summary } else { t.text }, depth as f32 * 14.0),
                    (duration, t.text, 0.0),
                    (show_date(s.early_start), t.text, 0.0),
                    (show_date(s.early_finish), t.text, 0.0),
                    (show_date(s.late_start), t.text_soft, 0.0),
                    (show_date(s.late_finish), t.text_soft, 0.0),
                    (s.total_float.to_string(), critical_text, 0.0),
                    (s.free_float.to_string(), t.text, 0.0),
                ];
                let mut x = rect.left();
                for ((text, color, indent), (_, w)) in cells.into_iter().zip(COLUMNS) {
                    let cell = Rect::from_min_size(pos2(x, rect.top()), vec2(w, ROW));
                    painter.with_clip_rect(cell.shrink(1.0)).text(
                        pos2(x + 6.0 + indent, rect.center().y),
                        Align2::LEFT_CENTER,
                        text,
                        FontId::proportional(12.5),
                        color,
                    );
                    x += w;
                    painter.vline(x, rect.y_range(), Stroke::new(1.0, t.grid));
                }
                bar(
                    &painter.with_clip_rect(Rect::from_min_max(pos2(chart_left, rect.top()), rect.max)),
                    a.kind,
                    summary,
                    s,
                    rect,
                    &|d: Date| x_of(d.days(), chart_left),
                    scale,
                    t,
                );
                painter.hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, t.grid_soft));
            }
        });
        if clicked.is_some() {
            self.selected = clicked;
        }
    }

    fn status_line(&self, ui: &mut egui::Ui, t: &Tokens) {
        ui.horizontal(|ui| {
            match &self.session.schedule {
                Ok(s) => {
                    let critical = s.activities.iter().filter(|a| a.critical).count();
                    ui.colored_label(
                        t.text_soft,
                        tr!("status-summary", activities = s.activities.len(), finish = show_date(s.finish).as_str(), critical = critical),
                    );
                    if !s.ignored.is_empty() {
                        ui.colored_label(t.warning, tr!("status-ignored", n = s.ignored.len()));
                    }
                }
                Err(_) => {
                    ui.colored_label(t.critical, tr!("status-not-scheduled"));
                }
            }
            if !self.session.warnings.is_empty() {
                ui.colored_label(t.warning, tr!("status-warnings", n = self.session.warnings.len())).on_hover_text(self.session.warnings.join("\n"));
            }
            if let Some(selected) = self.selected.and_then(|i| self.session.project.activities.get(i)) {
                ui.separator();
                ui.colored_label(t.text, format!("{} — {}", selected.id, selected.name));
            }
            match &self.status {
                Some(Ok(m)) => {
                    ui.separator();
                    ui.colored_label(t.text_soft, m);
                }
                Some(Err(m)) => {
                    ui.separator();
                    ui.colored_label(t.critical, m);
                }
                None => {}
            }
        });
    }
}

/// An activity's bars: the early one (red on the critical path), the late one thin beneath it;
/// a summary as a bracket; a milestone as a diamond.
#[allow(clippy::too_many_arguments)]
fn bar(painter: &egui::Painter, kind: ActivityKind, summary: bool, s: &Scheduled, row: Rect, x_of: &dyn Fn(Date) -> f32, scale: f32, t: &Tokens) {
    let mid = row.center().y;
    let span = |a: Date, b: Date| (x_of(a), x_of(b) + scale);
    if summary {
        let (l, r) = span(s.early_start, s.early_finish);
        let top = mid - 5.0;
        painter.rect_filled(Rect::from_min_max(pos2(l, top), pos2(r, top + 4.0)), 0.0, t.summary);
        for x in [l, r] {
            painter.add(egui::Shape::convex_polygon(vec![pos2(x - 4.0, top), pos2(x + 4.0, top), pos2(x, top + 9.0)], t.summary, Stroke::NONE));
        }
        return;
    }
    let color = if s.critical { t.critical } else { t.bar };
    if kind.is_milestone() {
        // A start milestone at the morning of its day, a finish milestone at the evening.
        let x = if kind == ActivityKind::FinishMilestone { x_of(s.early_finish) + scale } else { x_of(s.early_start) };
        let r = 6.0;
        painter.add(egui::Shape::convex_polygon(vec![pos2(x, mid - r), pos2(x + r, mid), pos2(x, mid + r), pos2(x - r, mid)], color, Stroke::NONE));
        return;
    }
    let (l, r) = span(s.early_start, s.early_finish);
    painter.rect(Rect::from_min_max(pos2(l, mid - 7.0), pos2(r.max(l + 2.0), mid + 3.0)), 2.0, color, Stroke::NONE, StrokeKind::Inside);
    let (ll, lr) = span(s.late_start, s.late_finish);
    painter.rect_filled(Rect::from_min_max(pos2(ll, mid + 5.0), pos2(lr.max(ll + 2.0), mid + 8.0)), 1.0, t.late);
    // The total float, from the early finish to the late finish.
    if s.total_float > 0 {
        painter.line_segment([Pos2::new(r, mid - 2.0), Pos2::new(lr, mid - 2.0)], Stroke::new(1.0, t.text_soft));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_follow_the_wbs_depth_first() {
        let s = Session::default();
        let r = rows(&s);
        assert_eq!(r.len(), s.project.activities.len());
        let id = |k: usize| s.project.activities[r[k].0].id.as_str();
        assert_eq!((id(0), r[0].1), ("W1", 0));
        assert_eq!((id(1), r[1].1), ("M0", 1), "children under their summary");
    }

    #[test]
    fn dates_in_each_language() {
        let d = Date::from_ymd(2026, 11, 2).unwrap();
        assert_eq!(show_date(d), "02-Nov-26");
        i18n::set(Lang::PtBr);
        assert_eq!(show_date(d), "02/11/2026");
        i18n::set(Lang::En);
    }
}
