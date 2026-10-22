//! MK Perth's window, in the house's layout (MK Sigma's): a title row and a ribbon of tabs; a
//! rail of views on the left; the Activities view (the table beside the Gantt chart) with the
//! selected activity's calculation under it; its details on the right; a Filters strip at the
//! edge; a status bar with the zoom. The UI only reads the session and acts through
//! `Session::run`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod activities;
pub mod icons;
mod ribbon;
pub mod theme;

use std::path::{Path, PathBuf};

use mkperth_engine::Session;
use mkperth_engine::core::{ActivityKind, Date, RelationshipType};
use mkperth_engine::i18n::{Lang, current, tr};
use mkperth_engine::sched::constraint_name;
use serde_json::json;

pub use activities::Options;
pub use mkperth_engine::i18n;
pub use ribbon::Tab;
pub use theme::Tokens;

use icons::Icon;

/// Shows an open-file dialog; `None` when cancelled.
pub type PickOpen = Box<dyn Fn() -> Option<PathBuf>>;
/// Shows a save-file dialog suggesting a file name; `None` when cancelled.
pub type PickSave = Box<dyn Fn(&str) -> Option<PathBuf>>;

/// What the platform provides: dialogs come in from the app, so the UI stays testable.
#[derive(Default)]
pub struct Services {
    pub pick_open: Option<PickOpen>,
    pub pick_save: Option<PickSave>,
}

/// The views the rail switches between.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Activities,
}

/// What the last action said, for the status bar.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Info(String),
    Warning(String),
    Error(String),
}

const STATUS_HEIGHT: f32 = 24.0;
const RAIL_WIDTH: f32 = 44.0;
const FILTERS_WIDTH: f32 = 240.0;
const FILTERS_STRIP: f32 = 30.0;
const CALCULATION_HEIGHT: f32 = 230.0;

pub struct PerthApp {
    pub session: Session,
    pub services: Services,
    /// The activity selected, by index in the project.
    pub selected: Option<usize>,
    pub status: Option<Status>,
    pub tab: Tab,
    pub ribbon_collapsed: bool,
    pub view: View,
    pub options: Options,
    pub filters_open: bool,
    /// Save after every change (only when the project has a file).
    pub autosave: bool,
    /// The interface's zoom, in percent.
    pub zoom: u32,
    /// The selected activity's calculation, in words: (its index, the lines).
    calculation: Option<(usize, Vec<String>)>,
    themed: bool,
    title: String,
}

impl Default for PerthApp {
    fn default() -> Self {
        PerthApp::new(Services::default())
    }
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
        PerthApp {
            session: Session::default(),
            services,
            selected: None,
            status: None,
            tab: Tab::default(),
            ribbon_collapsed: false,
            view: View::default(),
            options: Options { late_bars: true, ..Options::default() },
            filters_open: false,
            autosave: false,
            zoom: 100,
            calculation: None,
            themed: false,
            title: String::new(),
        }
    }

    /// Runs a command, keeping what it said for the status bar.
    pub fn run(&mut self, id: &str, params: serde_json::Value) {
        self.status = Some(match self.session.run(id, &params) {
            Ok(()) => match id {
                "project.open" | "project.sample" if !self.session.warnings.is_empty() => Status::Warning(format!(
                    "{} {}",
                    tr!("status-opened", name = self.session.project.name.as_str()),
                    self.session.warnings.join("; ")
                )),
                "project.open" | "project.sample" => Status::Info(tr!("status-opened", name = self.session.project.name.as_str())),
                "project.save" => {
                    Status::Info(tr!("status-saved", path = self.session.path.as_deref().map(|p| p.display().to_string()).unwrap_or_default()))
                }
                _ => Status::Info(tr!("status-scheduled")),
            },
            Err(e) => Status::Error(e.to_string()),
        });
        if matches!(id, "project.open" | "project.sample") {
            self.selected = None;
        }
        self.calculation = None;
    }

    pub fn open(&mut self, path: &Path) {
        self.run("project.open", json!({"path": path.display().to_string()}));
    }

    fn pick_and_open(&mut self) {
        if let Some(path) = self.services.pick_open.as_ref().and_then(|pick| pick()) {
            self.open(&path);
        }
    }

    pub fn save(&mut self) {
        if self.session.path.is_some() {
            self.run("project.save", json!(null));
        } else {
            self.save_as();
        }
    }

    pub fn save_as(&mut self) {
        let suggested = format!("{}.json", self.session.project.name.replace(['/', '\\'], "-"));
        if let Some(path) = self.services.pick_save.as_ref().and_then(|pick| pick(&suggested)) {
            self.run("project.save", json!({"path": path.display().to_string()}));
        }
    }

    fn act(&mut self, ctx: &egui::Context, action: ribbon::Action) {
        use ribbon::Action;
        match action {
            Action::Open => self.pick_and_open(),
            Action::Save => self.save(),
            Action::SaveAs => self.save_as(),
            Action::Sample => self.run("project.sample", json!(null)),
            Action::Schedule => self.run("project.schedule", json!(null)),
            Action::CriticalOnly => self.options.critical_only = !self.options.critical_only,
            Action::LateBars => self.options.late_bars = !self.options.late_bars,
            Action::View(view) => self.view = view,
            Action::Theme(preference) => ctx.set_theme(preference),
            Action::ToggleAutoSave => self.autosave = !self.autosave,
            Action::Search => self.filters_open = true,
            Action::Language(lang) => {
                i18n::set(lang);
                self.calculation = None;
            }
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        let shortcut = |modifiers, key| ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(modifiers, key)));
        if shortcut(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::S) {
            self.save_as();
        } else if shortcut(egui::Modifiers::COMMAND, egui::Key::S) {
            self.save();
        }
        if shortcut(egui::Modifiers::COMMAND, egui::Key::O) {
            self.pick_and_open();
        }
        if shortcut(egui::Modifiers::COMMAND, egui::Key::F1) {
            self.ribbon_collapsed = !self.ribbon_collapsed;
        }
        if shortcut(egui::Modifiers::NONE, egui::Key::F9) {
            self.run("project.schedule", json!(null));
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        if !self.themed {
            theme::apply(ui.ctx());
            self.themed = true;
        }
        self.handle_input(ui.ctx());
        // The project's name: Perth.jl names its files by id, so the name says more than the file.
        let name = if self.session.project.name.is_empty() {
            self.session.path.as_deref().and_then(Path::file_name).map_or_else(|| tr!("untitled"), |n| n.to_string_lossy().into_owned())
        } else {
            self.session.project.name.clone()
        };
        let document = format!("{}{name}", if self.session.modified { "• " } else { "" });
        let title = format!("{document} — MK Perth");
        if title != self.title {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        let t = Tokens::get(ui.ctx());
        let whole = ui.max_rect().expand(8.0);
        ui.painter().rect_filled(whole, 0.0, t.window);
        // The chrome — title row, tabs and ribbon — is one surface, painted once its height is
        // known.
        let chrome = ui.painter().add(egui::Shape::Noop);
        ui.spacing_mut().item_spacing.y = 0.0;
        let state = ribbon::State {
            title: &document,
            autosave: self.autosave,
            critical_only: self.options.critical_only,
            late_bars: self.options.late_bars,
            view: self.view,
        };
        let mut actions = Vec::new();
        actions.extend(ribbon::title_bar(ui, &state));
        actions.extend(ribbon::ribbon(ui, &mut self.tab, &mut self.ribbon_collapsed, &state));
        let bottom = ui.cursor().top();
        ui.painter().set(chrome, egui::Shape::rect_filled(egui::Rect::from_min_max(whole.min, egui::pos2(whole.right(), bottom)), 0.0, t.ribbon));
        ui.painter().hline(whole.x_range(), bottom, egui::Stroke::new(1.0, t.border));
        for action in actions {
            self.act(ui.ctx(), action);
        }

        let body = (ui.available_height() - STATUS_HEIGHT - 8.0).max(100.0);
        ui.add_space(6.0);
        let gap = 6.0;
        let filters_width = if self.filters_open { FILTERS_WIDTH } else { FILTERS_STRIP };
        let room = ui.available_width() - RAIL_WIDTH - filters_width - 2.0 * gap;
        let left_width = ((room - gap) * 0.72).floor();
        let right_width = room - gap - left_width;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(gap, 0.0);
            let top_down = egui::Layout::top_down(egui::Align::Min);
            self.view_rail(ui, body);
            ui.allocate_ui_with_layout(egui::vec2(left_width, body), top_down, |ui| {
                ui.set_width(left_width);
                ui.set_height(body);
                self.left_pane(ui, body);
            });
            ui.allocate_ui_with_layout(egui::vec2(right_width, body), top_down, |ui| {
                ui.set_width(right_width);
                ui.set_height(body);
                theme::card(&t).fill(t.paper).show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    self.details(ui);
                });
            });
            ui.allocate_ui_with_layout(egui::vec2(filters_width, body), top_down, |ui| {
                ui.set_width(filters_width);
                ui.set_height(body);
                self.filters_pane(ui);
            });
        });
        ui.add_space(2.0);
        self.status_line(ui);
        let zoom = self.zoom as f32 / 100.0;
        if (ui.ctx().zoom_factor() - zoom).abs() > 0.001 {
            ui.ctx().set_zoom_factor(zoom);
        }
        if self.autosave && self.session.modified && self.session.path.is_some() {
            self.run("project.save", json!(null));
        }
    }

    /// The rail of views down the left edge.
    fn view_rail(&mut self, ui: &mut egui::Ui, height: f32) {
        let t = Tokens::get(ui.ctx());
        let (rect, _) = ui.allocate_exact_size(egui::vec2(RAIL_WIDTH, height), egui::Sense::hover());
        ui.painter().rect_filled(rect, 0.0, t.ribbon);
        ui.painter().vline(rect.right(), rect.y_range(), egui::Stroke::new(1.0, t.border));
        let views = [(Icon::Gantt, tr!("activities-view"), tr!("activities-view-tip"), View::Activities)];
        for (k, (icon, name, tip, view)) in views.into_iter().enumerate() {
            let on = self.view == view;
            let slot = egui::Rect::from_min_size(rect.min + egui::vec2(0.0, 4.0 + 42.0 * k as f32), egui::vec2(RAIL_WIDTH - 1.0, 40.0));
            let response = ui.interact(slot, ui.id().with(("rail", k)), egui::Sense::click());
            response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, &name));
            let painter = ui.painter();
            if on {
                painter.rect_filled(slot, 0.0, t.accent_soft);
                painter.rect_filled(egui::Rect::from_min_size(slot.min, egui::vec2(3.0, slot.height())), 0.0, t.accent);
            } else if response.hovered() {
                painter.rect_filled(slot, 0.0, t.hover);
            }
            icons::paint(painter, egui::Rect::from_center_size(slot.center(), egui::vec2(20.0, 20.0)), icon, &t);
            if response.on_hover_text(tip).clicked() {
                self.view = view;
            }
        }
    }

    /// The Activities view over the selected activity's calculation.
    fn left_pane(&mut self, ui: &mut egui::Ui, height: f32) {
        let t = Tokens::get(ui.ctx());
        let table_height = (height - CALCULATION_HEIGHT - 6.0).max(140.0);
        let grid = egui::Frame::new().fill(t.grid_bg).stroke(egui::Stroke::new(1.0, t.border)).inner_margin(egui::Margin::same(1));
        let mut clicked = None;
        grid.show(ui, |ui| {
            let size = egui::vec2(ui.available_width(), table_height - 2.0);
            ui.allocate_ui(size, |ui| {
                ui.set_min_size(size);
                match &self.session.schedule {
                    Ok(_) => clicked = activities::show(ui, &self.session, &self.options, self.selected),
                    Err(e) => {
                        ui.add_space(20.0);
                        ui.colored_label(t.danger, format!("  {e}"));
                    }
                }
            });
        });
        if clicked.is_some() && clicked != self.selected {
            self.selected = clicked;
            self.calculation = None;
        }
        ui.add_space(6.0);
        let mut schedule = false;
        theme::card(&t).show(ui, |ui| {
            ui.set_min_size(egui::vec2(ui.available_width(), ui.available_height()));
            schedule = theme::pane_header(ui, Icon::List, &tr!("pane-calculation"), &tr!("pane-calculation-tip"), |ui| {
                ribbon::accent_button(ui, Icon::Run, &tr!("schedule")).on_hover_text(tr!("schedule-tip")).clicked()
            });
            let Some(i) = self.selected else {
                ui.label(egui::RichText::new(tr!("pane-calculation-empty")).color(t.text_dim));
                return;
            };
            if self.calculation.as_ref().is_none_or(|(k, _)| *k != i) {
                let id = self.session.project.activities.get(i).map(|a| a.id.clone()).unwrap_or_default();
                let lines = match self.session.explain(&id) {
                    Ok(e) => e.lines(&show_date),
                    Err(e) => vec![e.to_string()],
                };
                self.calculation = Some((i, lines));
            }
            if let Some((_, lines)) = &self.calculation {
                egui::ScrollArea::vertical().id_salt("calculation").auto_shrink(false).show(ui, |ui| {
                    // Headings and results flush; each bound indented under its pass.
                    for (k, line) in lines.iter().enumerate() {
                        if let Some(bound) = line.strip_prefix("  ") {
                            ui.indent(("bound", k), |ui| {
                                ui.label(egui::RichText::new(format!("•  {bound}")).size(12.5).color(t.text));
                            });
                        } else {
                            ui.label(egui::RichText::new(line).size(12.5).color(t.text_dim));
                        }
                        ui.add_space(3.0);
                    }
                });
            }
        });
        if schedule {
            self.run("project.schedule", json!(null));
        }
    }

    /// The selected activity's facts, relationships and dates.
    fn details(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        theme::pane_header(ui, Icon::Info, &tr!("pane-details"), "", |_| {});
        let project = &self.session.project;
        let Some((i, a)) = self.selected.and_then(|i| project.activities.get(i).map(|a| (i, a))) else {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new(tr!("pane-details-empty")).size(14.0).color(t.text_dim));
                ui.label(egui::RichText::new(tr!("pane-details-empty-tip")).size(12.0).color(t.text_dim));
            });
            return;
        };
        let scheduled = self.session.schedule.as_ref().ok().and_then(|s| s.activities.get(i));
        let summary = project.activities.iter().any(|c| c.parent.as_deref() == Some(a.id.as_str()));
        let kind = if summary {
            tr!("kind-summary")
        } else {
            match a.kind {
                ActivityKind::Task => tr!("kind-task"),
                ActivityKind::StartMilestone => tr!("kind-start-milestone"),
                ActivityKind::FinishMilestone => tr!("kind-finish-milestone"),
            }
        };
        let fact = |ui: &mut egui::Ui, name: String, value: String| {
            ui.horizontal(|ui| {
                ui.add_sized(egui::vec2(120.0, 18.0), egui::Label::new(egui::RichText::new(name).size(12.0).color(t.text_dim)));
                ui.label(egui::RichText::new(value).size(12.5).color(t.text));
            });
            ui.add_space(3.0);
        };
        egui::ScrollArea::vertical().id_salt("details").auto_shrink(false).show(ui, |ui| {
            ui.label(egui::RichText::new(&a.name).size(15.0).color(t.text).strong());
            ui.add_space(6.0);
            fact(ui, tr!("detail-id"), a.id.clone());
            fact(ui, tr!("detail-kind"), kind);
            if !summary && !a.kind.is_milestone() {
                fact(ui, tr!("detail-duration"), tr!("detail-days", n = a.working_days()));
            }
            fact(ui, tr!("detail-calendar"), a.calendar.clone().unwrap_or_else(|| project.calendar.name.clone()));
            if let Some(parent) = a.parent.as_deref() {
                fact(ui, tr!("detail-wbs"), parent.to_string());
            }
            for c in [a.constraint, a.secondary_constraint].into_iter().flatten() {
                fact(ui, tr!("detail-constraint"), constraint_name(c, &show_date));
            }
            if let Some(s) = scheduled {
                ui.add_space(6.0);
                fact(ui, tr!("col-early-start"), show_date(s.early_start));
                fact(ui, tr!("col-early-finish"), show_date(s.early_finish));
                fact(ui, tr!("col-late-start"), show_date(s.late_start));
                fact(ui, tr!("col-late-finish"), show_date(s.late_finish));
                fact(ui, tr!("col-total-float"), s.total_float.to_string());
                fact(ui, tr!("col-free-float"), s.free_float.to_string());
                if s.critical {
                    ui.label(egui::RichText::new(tr!("detail-critical")).size(12.5).color(t.red));
                }
            }
            let kind_code = |k: RelationshipType| k.code();
            let lag = |l: i64| if l == 0 { String::new() } else { format!(" {l:+}") };
            let predecessors: Vec<String> = project
                .relationships
                .iter()
                .filter(|r| r.successor == a.id)
                .map(|r| format!("{} {}{}", r.predecessor, kind_code(r.kind), lag(r.lag)))
                .collect();
            let successors: Vec<String> = project
                .relationships
                .iter()
                .filter(|r| r.predecessor == a.id)
                .map(|r| format!("{} {}{}", r.successor, kind_code(r.kind), lag(r.lag)))
                .collect();
            for (title, list) in [(tr!("detail-predecessors"), predecessors), (tr!("detail-successors"), successors)] {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(title).size(12.5).color(t.text).strong());
                if list.is_empty() {
                    ui.label(egui::RichText::new(tr!("detail-none")).size(12.0).color(t.text_dim));
                }
                for item in list {
                    ui.label(egui::RichText::new(item).size(12.5).color(t.text));
                }
            }
        });
    }

    /// The Filters pane at the right edge, folded into a strip when closed.
    fn filters_pane(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 0.0, t.ribbon);
        ui.painter().vline(rect.left(), rect.y_range(), egui::Stroke::new(1.0, t.border));
        let count = usize::from(self.options.critical_only) + usize::from(!self.options.search.trim().is_empty());
        if !self.filters_open {
            let name = tr!("filters-show");
            let response = ui.interact(rect, ui.id().with("filters-strip"), egui::Sense::click());
            response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
            let painter = ui.painter();
            if response.hovered() {
                painter.rect_filled(rect.shrink2(egui::vec2(1.0, 0.0)), 0.0, t.hover);
            }
            let x = rect.center().x;
            icons::paint(painter, egui::Rect::from_center_size(egui::pos2(x, rect.top() + 16.0), egui::vec2(14.0, 14.0)), Icon::ChevronLeft, &t);
            let galley = painter.layout_no_wrap(tr!("filters"), egui::FontId::proportional(13.0), t.text);
            let length = galley.size().x;
            let at = egui::pos2(x + galley.size().y / 2.0, rect.top() + 34.0);
            painter.add(egui::epaint::TextShape::new(at, galley, t.text).with_angle(std::f32::consts::FRAC_PI_2));
            let below = rect.top() + 34.0 + length + 16.0;
            icons::paint(painter, egui::Rect::from_center_size(egui::pos2(x, below), egui::vec2(12.0, 12.0)), Icon::Filter, &t);
            if count > 0 {
                painter.text(egui::pos2(x, below + 16.0), egui::Align2::CENTER_CENTER, count.to_string(), egui::FontId::proportional(12.0), t.accent);
            }
            if response.on_hover_text(name).clicked() {
                self.filters_open = true;
            }
            return;
        }
        let mut pane =
            ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(10.0, 8.0))).layout(egui::Layout::top_down(egui::Align::Min)));
        let ui = &mut pane;
        ui.spacing_mut().item_spacing.y = 6.0;
        if theme::pane_header(ui, Icon::Filter, &tr!("filters"), "", |ui| icon_button(ui, Icon::ChevronRight, &tr!("filters-hide"))) {
            self.filters_open = false;
        }
        ui.label(egui::RichText::new(tr!("filter-search")).size(12.0).color(t.text_dim));
        ui.add(egui::TextEdit::singleline(&mut self.options.search).hint_text(tr!("filter-search-hint")).desired_width(f32::INFINITY));
        ui.add_space(4.0);
        ui.checkbox(&mut self.options.critical_only, tr!("critical-only"));
        ui.checkbox(&mut self.options.late_bars, tr!("late-bars"));
        if count > 0 && ui.button(tr!("filters-clear")).clicked() {
            self.options.critical_only = false;
            self.options.search.clear();
        }
    }

    fn status_line(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), STATUS_HEIGHT), egui::Sense::hover());
        ui.painter().hline(rect.x_range(), rect.top(), egui::Stroke::new(1.0, t.border));
        let mut row = ui
            .new_child(egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(10.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
        let ready = tr!("status-ready");
        let (text, color) = match &self.status {
            Some(Status::Info(s)) => (s.as_str(), t.text_dim),
            Some(Status::Warning(s)) => (s.as_str(), t.warn),
            Some(Status::Error(s)) => (s.as_str(), t.danger),
            None => (ready.as_str(), t.text_dim),
        };
        let mut facts = Vec::new();
        match &self.session.schedule {
            Ok(s) => {
                facts.push(tr!("fact-activities", n = s.activities.len()));
                facts.push(tr!("fact-finish", date = show_date(s.finish).as_str()));
                facts.push(tr!("fact-critical", n = s.activities.iter().filter(|a| a.critical).count()));
                if !s.ignored.is_empty() {
                    facts.push(tr!("status-ignored", n = s.ignored.len()));
                }
            }
            Err(_) => facts.push(tr!("status-not-scheduled")),
        }
        let mut zoom = self.zoom;
        row.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let percent = ui.add(egui::Label::new(egui::RichText::new(format!("{zoom}%")).color(t.text_dim).size(12.0)).sense(egui::Sense::click()));
            if percent.on_hover_text(tr!("zoom-reset")).clicked() {
                zoom = 100;
            }
            if icon_button(ui, Icon::Plus, &tr!("zoom-in")) {
                zoom = (zoom / 10 * 10 + 10).min(200);
            }
            ui.spacing_mut().slider_width = 110.0;
            ui.add(egui::Slider::new(&mut zoom, 50..=200).show_value(false)).on_hover_text(tr!("zoom"));
            if icon_button(ui, Icon::Minus, &tr!("zoom-out")) {
                zoom = (zoom.div_ceil(10) * 10).saturating_sub(10).max(50);
            }
            ui.add_space(16.0);
            ui.label(egui::RichText::new(facts.join("   ·   ")).color(t.text_dim).size(12.0));
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add(egui::Label::new(egui::RichText::new(text).color(color).size(12.0)).truncate());
            });
        });
        self.zoom = zoom;
    }
}

/// A small icon button, for the status bar and the panes' headers.
fn icon_button(ui: &mut egui::Ui, icon: Icon, name: &str) -> bool {
    let t = Tokens::get(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    if response.hovered() {
        ui.painter().rect_filled(rect, 0.0, t.hover);
    }
    icons::paint(ui.painter(), rect.shrink(4.0), icon, &t);
    response.on_hover_text(name).clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_follow_the_wbs_depth_first() {
        let s = Session::default();
        let r = activities::rows(&s);
        assert_eq!(r.len(), s.project.activities.len());
        let id = |k: usize| s.project.activities[r[k].0].id.as_str();
        assert_eq!((id(0), r[0].1), ("W1", 0));
        assert_eq!((id(1), r[1].1), ("M0", 1), "children under their summary");
    }

    #[test]
    fn filters_keep_the_summaries_above_a_match() {
        let s = Session::default();
        let critical = activities::visible(&s, &Options { critical_only: true, ..Options::default() });
        assert!(critical.iter().all(|&(i, _)| {
            let summary = s.project.activities.iter().any(|c| c.parent.as_deref() == Some(s.project.activities[i].id.as_str()));
            summary || s.schedule.as_ref().unwrap().activities[i].critical
        }));
        let search = activities::visible(&s, &Options { search: "plaster".into(), ..Options::default() });
        let ids: Vec<&str> = search.iter().map(|&(i, _)| s.project.activities[i].id.as_str()).collect();
        assert_eq!(ids, ["W3", "C40"]);
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
