//! MK Perth's window: a quiet header (the name, the menus) over one slim toolbar; a rail of
//! views on the left; the Activities view (the table beside the Gantt chart) with the
//! selected activity's calculation under it; its details on the right; a Filters strip at the
//! edge; a status bar with the zoom. The UI only reads the session and acts through
//! `Session::run`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod activities;
pub mod backstage;
mod baselines_ui;
mod calendars;
mod chrome;
mod comments_ui;
mod datepick;
pub mod fonts;
pub mod icons;
mod levelling_ui;
mod pert_ui;
mod resources_view;
pub mod splash;
pub mod theme;

use std::path::{Path, PathBuf};

use mkperth_engine::Session;
use mkperth_engine::core::{ActivityKind, RelationshipType};
use mkperth_engine::i18n::{Lang, current, tr};
use mkperth_engine::sched::constraint_name;
use serde_json::json;

pub use activities::{Column, Editing, Options};
pub use calendars::{CalendarForm, periods_text, read_periods};
pub use comments_ui::CommentForm;
pub use mkperth_engine::core::Date;
pub use mkperth_engine::i18n;
pub use resources_view::ResourceForm;
pub use theme::Tokens;

use icons::Icon;

/// Shows an open-file dialog; `None` when cancelled.
pub type PickOpen = Box<dyn Fn() -> Option<PathBuf>>;
/// Shows a save-file dialog suggesting a file name; `None` when cancelled.
pub type PickSave = Box<dyn Fn(&str) -> Option<PathBuf>>;
pub type Today = Box<dyn Fn() -> Option<Date>>;
/// The typeface families the system has; the bytes of one's face, bold and italic as asked
/// (the regular one when it has none such); keeping the choice for the next run.
pub type ListFonts = Box<dyn Fn() -> Vec<String>>;
pub type LoadFont = Box<dyn Fn(&str, bool, bool) -> Option<Vec<u8>>>;
pub type SaveFont = Box<dyn Fn(&fonts::FontChoice)>;
/// Who is using the app (signs comments), and the time now as comments show it.
pub type Who = Box<dyn Fn() -> String>;
/// Keeps the recent projects, newest first, for the next run.
pub type SaveRecent = Box<dyn Fn(&[PathBuf])>;

/// What the platform provides: dialogs come in from the app, so the UI stays testable.
#[derive(Default)]
pub struct Services {
    pub pick_open: Option<PickOpen>,
    pub pick_save: Option<PickSave>,
    /// Today where the person is, for the chart's line.
    pub today: Option<Today>,
    pub save_recent: Option<SaveRecent>,
    /// The person's name, for the comments they write.
    pub user: Option<Who>,
    /// The time now (`2026-10-23 14:05`), for the comments written.
    pub now: Option<Who>,
    pub fonts: Option<ListFonts>,
    pub load_font: Option<LoadFont>,
    pub save_font: Option<SaveFont>,
}

/// The views the rail switches between.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Activities,
    /// The resources, the roles they fill and their assignments.
    Resources,
}

/// The Details pane's tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DetailsTab {
    #[default]
    General,
    Status,
    Relationships,
    Notes,
    /// Threads of comments on the activity.
    Comments,
    /// The resources assigned to the activity.
    Resources,
}

/// The panes the strip at the right edge opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Details,
    Filters,
    /// The chart's notes: bands, markers, marked months.
    Notes,
    /// The calendars: weeks, work periods, holidays.
    Calendars,
    /// Resource levelling: what is over-allocated, the options, what was held.
    Levelling,
    /// PERT's analysis: the expected finish, its spread and its chances.
    Pert,
    /// The baselines: assigned, the one in use, deleted.
    Baselines,
}

/// The note the Notes pane is making: its kind (0 a band, 1 a marker, 2 a month), name, dates
/// and colour (`#rrggbb`, empty for the default).
#[derive(Debug, Clone, Default)]
pub struct NoteForm {
    pub kind: u8,
    pub name: String,
    pub from: Option<Date>,
    pub to: Option<Date>,
    pub color: String,
}

/// A band (`month: false`, from and to) or a marked month (`month: true`, its first day) being
/// added where a double click was, its name typed in a small window there.
#[derive(Debug, Clone, PartialEq)]
pub struct QuickNote {
    pub month: bool,
    pub from: Date,
    pub to: Date,
    pub name: String,
    pub at: egui::Pos2,
}

/// The colours a note can take besides its default: original picks, readable in both themes.
const NOTE_COLORS: [&str; 6] = ["#4a7fd4", "#3a9a5b", "#d08a2b", "#c4473f", "#8a5cc7", "#6b7280"];

/// What the last action said, for the status bar.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Info(String),
    Warning(String),
    Error(String),
}

const STATUS_HEIGHT: f32 = 24.0;
const RAIL_WIDTH: f32 = 44.0;
/// The height of the open views' tabs over the main area.
const VIEW_TABS: f32 = 30.0;
const SIDE_WIDTH: f32 = 320.0;
const SIDE_STRIP: f32 = 30.0;
const CALCULATION_HEIGHT: f32 = 230.0;

pub struct PerthApp {
    pub session: Session,
    pub services: Services,
    /// The activity selected, by index in the project.
    pub selected: Option<usize>,
    pub status: Option<Status>,
    /// The toolbar under the menus is hidden (Ctrl+F1).
    pub toolbar_hidden: bool,
    /// Presenting: the activities and the chart alone, full screen (F5; Esc ends it).
    pub presenting: bool,
    /// The interface's typeface, size and style; the one last set in egui, and the families the
    /// system has (asked once, when the menu needs them).
    pub font: fonts::FontChoice,
    font_set: Option<fonts::FontChoice>,
    font_families: Option<Vec<String>>,
    /// The File backstage, when it is open, and the page it shows.
    pub backstage: Option<backstage::Page>,
    /// The projects opened or saved lately, newest first.
    pub recent: Vec<PathBuf>,
    pub view: View,
    /// The views open as tabs over the main area, in the order they were opened; the
    /// activities always among them.
    pub open_views: Vec<View>,
    pub options: Options,
    /// The pane open beside the strip at the right edge, if any.
    pub side: Option<Side>,
    /// Save after every change (only when the project has a file).
    pub autosave: bool,
    /// The interface's zoom, in percent.
    pub zoom: u32,
    /// The table cell being edited in place.
    pub editing: Option<Editing>,
    /// The Details pane's new predecessor: its ID, the kind (FS, SS, FF, SF) and the lag.
    link: (String, RelationshipType, i64),
    /// The Details pane's tab.
    pub details_tab: DetailsTab,
    /// The Notes pane's new note.
    pub note_form: NoteForm,
    /// The activities chosen together (dragged over, Ctrl- or Shift-clicked), by index; the
    /// selected one among them.
    pub marked: std::collections::BTreeSet<usize>,
    /// A band or a marked month being added from a double click on the chart, in a small window
    /// where it happened.
    pub quick_note: Option<QuickNote>,
    /// What the Calendars pane is editing.
    pub calendar_form: CalendarForm,
    /// The levelling options as the Levelling pane edits them, and the project's they began as.
    level_form: Option<(mkperth_engine::core::LevellingOptions, levelling_ui::LevelForm)>,
    /// PERT's analysis and the project it was worked out for; the day its chance is asked for.
    pert_cache: Option<(mkperth_engine::core::Project, Result<mkperth_engine::sched::PertAnalysis, String>)>,
    pert_target: Option<Date>,
    /// The name typed for the next baseline.
    baseline_name: String,
    /// The resource selected in the Resources view, by index, and what that view is typing.
    pub resource_selected: Option<usize>,
    pub resource_form: ResourceForm,
    /// What the Comments tab is typing.
    pub comment_form: CommentForm,
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

/// Working minutes in days of `day_minutes`, with the decimals they need, in the language's
/// decimal mark.
pub fn days_text(minutes: i64, day_minutes: i64) -> String {
    let d = day_minutes.max(1);
    let text =
        if minutes % d == 0 { (minutes / d).to_string() } else { format!("{:.2}", minutes as f64 / d as f64).trim_end_matches('0').to_string() };
    i18n::decimal(&text)
}

/// A date as the language writes it.
/// A date typed as it's shown (31/12/2026, 31-Dec-26) or as 2026-12-31.
pub fn read_date(text: &str) -> Option<Date> {
    let text = text.trim();
    if let Some(d) = Date::parse(text) {
        return Some(d);
    }
    let parts: Vec<&str> = text.split(['/', '-', '.', ' ']).filter(|p| !p.is_empty()).collect();
    let [day, month, year] = parts.as_slice() else { return None };
    let month = month.parse::<u32>().ok().or_else(|| {
        (1..=12u32).find(|m| {
            let name = tr!(&format!("month-{m}"));
            name.eq_ignore_ascii_case(month) || name.get(..3).zip(month.get(..3)).is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
        })
    })?;
    let year = year.parse::<i32>().ok().map(|y| if (0..100).contains(&y) && year.len() <= 2 { 2000 + y } else { y })?;
    Date::from_ymd(year, month, day.parse().ok()?)
}

/// A date that can be typed: shown as the language writes it, set when Enter is pressed or the
/// field is left. Returns the change: a date, or none when emptied. `label` names it.
pub(crate) fn date_field(ui: &mut egui::Ui, id: egui::Id, label: egui::Id, current: Option<Date>, width: f32) -> Option<Option<Date>> {
    let shown = current.map(show_date).unwrap_or_default();
    // The text typed so far, kept while the field has focus.
    let key = id.with("text");
    let mut text = ui.data_mut(|d| d.get_temp::<(String, String)>(key)).filter(|(was, _)| *was == shown).map_or(shown.clone(), |(_, t)| t);
    let response = ui.add(egui::TextEdit::singleline(&mut text).id(id).desired_width(width).hint_text(tr!("date-hint"))).labelled_by(label);
    let mut changed = None;
    if response.lost_focus() {
        let typed = text.trim();
        if typed.is_empty() {
            changed = current.is_some().then_some(None);
        } else if let Some(d) = read_date(typed) {
            changed = (Some(d) != current).then_some(Some(d));
        }
        ui.data_mut(|d| d.remove::<(String, String)>(key));
    } else {
        ui.data_mut(|d| d.insert_temp(key, (shown, text)));
    }
    changed
}

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
            toolbar_hidden: false,
            presenting: false,
            font: fonts::FontChoice::default(),
            font_set: None,
            font_families: None,
            backstage: None,
            recent: Vec::new(),
            view: View::default(),
            open_views: vec![View::Activities],
            options: Options { late_bars: true, links: true, baseline_bars: true, hidden: activities::HIDDEN, ..Options::default() },
            side: None,
            autosave: false,
            zoom: 100,
            editing: None,
            link: (String::new(), RelationshipType::FinishToStart, 0),
            details_tab: DetailsTab::default(),
            note_form: NoteForm::default(),
            marked: std::collections::BTreeSet::new(),
            quick_note: None,
            calendar_form: CalendarForm::default(),
            level_form: None,
            pert_cache: None,
            pert_target: None,
            baseline_name: String::new(),
            resource_selected: None,
            resource_form: ResourceForm::default(),
            comment_form: CommentForm::default(),
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
                "project.new" => Status::Info(tr!("status-new")),
                "project.save" => {
                    Status::Info(tr!("status-saved", path = self.session.path.as_deref().map(|p| p.display().to_string()).unwrap_or_default()))
                }
                "edit.undo" => Status::Info(tr!("status-undone")),
                "edit.redo" => Status::Info(tr!("status-redone")),
                "project.schedule" => Status::Info(tr!("status-scheduled")),
                _ if self.session.schedule.is_err() => Status::Warning(tr!("status-not-scheduled")),
                _ => Status::Info(tr!("status-changed")),
            },
            Err(e) => Status::Error(e.to_string()),
        });
        if matches!(id, "project.open" | "project.save") && matches!(self.status, Some(Status::Info(_) | Status::Warning(_))) {
            self.remember();
        }
        if matches!(id, "project.new" | "project.open" | "project.sample") {
            self.selected = None;
        }
        // A selection past the end (an activity deleted, an undo) goes.
        if self.selected.is_some_and(|i| i >= self.session.project.activities.len()) {
            self.selected = None;
        }
        self.calculation = None;
    }

    /// Puts the open project's file first among the recent ones, and keeps the list.
    fn remember(&mut self) {
        let Some(path) = self.session.path.clone() else { return };
        self.recent.retain(|p| *p != path);
        self.recent.insert(0, path);
        self.recent.truncate(backstage::RECENT);
        if let Some(save) = &self.services.save_recent {
            save(&self.recent);
        }
    }

    /// A bar dragged on the chart, as a change to its activity (one undo): moved, it starts on or
    /// after its new day; its finish dragged, its duration is the working days from its start
    /// to the new finish on its calendar; its start dragged, both, the finish kept.
    fn edit_bar(&mut self, edit: activities::BarEdit) {
        use mkperth_engine::core::Constraint;
        let p = &self.session.project;
        let Some(a) = p.activities.get(edit.index) else { return };
        let calendar = a.calendar.as_deref().and_then(|n| p.calendars.iter().find(|c| c.name == n)).unwrap_or(&p.calendar);
        let working = |from: Date, to: Date| -> i64 {
            let (mut d, mut n) = (from, 0i64);
            while d <= to && n < mkperth_engine::sched::MAX_SPAN {
                n += i64::from(calendar.is_working(d));
                match d.succ() {
                    Some(next) => d = next,
                    None => break,
                }
            }
            n.max(1)
        };
        // A finish milestone holds its finish; anything else its start.
        let (code, date, constraint) = match a.kind {
            ActivityKind::FinishMilestone => ("finish-on-or-after", edit.finish, Constraint::FinishOnOrAfter(edit.finish)),
            _ => ("start-on-or-after", edit.start, Constraint::StartOnOrAfter(edit.start)),
        };
        let mut params = json!({"id": a.id});
        if let Some(map) = params.as_object_mut() {
            if edit.mode != activities::BarMode::Finish {
                map.insert("constraint".into(), json!({"type": code, "date": date.to_string()}));
                // A secondary that wouldn't stand beside the new one goes.
                if a.secondary_constraint.is_some_and(|s| !Constraint::pairs(constraint, s)) {
                    map.insert("secondary_constraint".into(), serde_json::Value::Null);
                }
            }
            if edit.mode != activities::BarMode::Move {
                map.insert("duration".into(), json!(working(edit.start, edit.finish)));
            }
        }
        self.run("activity.edit", params);
        self.selected = Some(edit.index);
        self.marked = std::iter::once(edit.index).collect();
    }

    /// The small window a double click on the chart opens: a band's name and days, or a marked
    /// month's name; Enter or Add adds it, Esc or Cancel drops it.
    fn quick_note_window(&mut self, ctx: &egui::Context) {
        let Some(mut note) = self.quick_note.clone() else { return };
        let mut add = false;
        let mut cancel = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let title = if note.month {
            let (y, m, _) = note.from.ymd();
            tr!("quick-month", month = format!("{} {y}", tr!(&format!("month-{m}"))).as_str())
        } else {
            tr!("quick-band")
        };
        egui::Window::new(title)
            .id(egui::Id::unique("quick-note"))
            .collapsible(false)
            .resizable(false)
            .fixed_pos(note.at + egui::vec2(8.0, 8.0))
            .show(ctx, |ui| {
                let label = ui.label(tr!("notes-name")).id;
                let response =
                    ui.add(egui::TextEdit::singleline(&mut note.name).desired_width(220.0).hint_text(tr!("quick-name-hint"))).labelled_by(label);
                if !response.has_focus() && note.name.is_empty() {
                    response.request_focus();
                }
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    add = true;
                }
                if !note.month {
                    egui::Grid::new("quick-note-days").num_columns(2).show(ui, |ui| {
                        let from = ui.label(tr!("notes-from")).id;
                        if let Some(Some(d)) = date_field(ui, egui::Id::unique("quick-from"), from, Some(note.from), 110.0) {
                            note.from = d;
                            note.to = note.to.max(d);
                        }
                        ui.end_row();
                        let to = ui.label(tr!("notes-to")).id;
                        if let Some(Some(d)) = date_field(ui, egui::Id::unique("quick-to"), to, Some(note.to), 110.0) {
                            note.to = d.max(note.from);
                        }
                        ui.end_row();
                    });
                }
                ui.horizontal(|ui| {
                    add |= ui.add_enabled(!note.name.trim().is_empty(), egui::Button::new(tr!("notes-add"))).clicked();
                    cancel |= ui.button(tr!("comments-cancel")).clicked();
                });
            });
        if add && !note.name.trim().is_empty() {
            let (command, params) = if note.month {
                ("month-mark.add", json!({"month": note.from.to_string(), "name": note.name.trim()}))
            } else {
                ("band.add", json!({"name": note.name.trim(), "from": note.from.to_string(), "to": note.to.to_string()}))
            };
            self.run(command, params);
            self.quick_note = None;
        } else if cancel {
            self.quick_note = None;
        } else {
            self.quick_note = Some(note);
        }
    }

    /// A copy of the project as `extension` (`xml` for P6, `json` for Perth), where the person
    /// picks; the open project stays as it was.
    pub fn export(&mut self, extension: &str) {
        let suggested = format!("{}.{extension}", self.session.project.name.replace(['/', '\\'], "-"));
        if let Some(path) = self.services.pick_save.as_ref().and_then(|pick| pick(&suggested)) {
            self.run("project.export", json!({"path": path.display().to_string()}));
        }
    }

    /// What the File backstage asked for.
    fn backstage_act(&mut self, ctx: &egui::Context, action: backstage::Action) {
        use backstage::Action;
        let close = |app: &mut Self| {
            if matches!(app.status, Some(Status::Info(_) | Status::Warning(_))) {
                app.backstage = None;
            }
        };
        match action {
            Action::Back => self.backstage = None,
            Action::Page(page) => self.backstage = Some(page),
            Action::Blank => {
                self.new_project();
                close(self);
            }
            Action::Sample => {
                self.run("project.sample", json!(null));
                close(self);
            }
            Action::Open(path) => {
                self.open(&path);
                close(self);
            }
            Action::Browse => {
                let before = self.session.path.clone();
                self.pick_and_open();
                if self.session.path != before {
                    close(self);
                }
            }
            Action::Save => self.save(),
            Action::SaveAs => self.save_as(),
            Action::Export(extension) => self.export(extension),
            Action::Latex => self.export_latex(),
            Action::ToggleAutoSave => self.autosave = !self.autosave,
            Action::Exit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }

    /// A new, empty project from today (or, without a clock, from the open project's start).
    pub fn new_project(&mut self) {
        let start = self.services.today.as_ref().and_then(|today| today()).unwrap_or(self.session.project.start);
        self.run("project.new", json!({"start": start.to_string()}));
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

    fn act(&mut self, ctx: &egui::Context, action: chrome::Action) {
        use chrome::Action;
        match action {
            Action::Backstage => self.backstage = Some(backstage::Page::Home),
            Action::Schedule => self.run("project.schedule", json!(null)),
            Action::Undo => self.run("edit.undo", json!(null)),
            Action::Redo => self.run("edit.redo", json!(null)),
            Action::AddActivity => self.add_activity(),
            Action::DeleteActivity => self.delete_activity(),
            Action::CriticalOnly => self.options.critical_only = !self.options.critical_only,
            Action::LateBars => self.options.late_bars = !self.options.late_bars,
            Action::BaselineBars => self.options.baseline_bars = !self.options.baseline_bars,
            Action::Baselines => self.side = Some(Side::Baselines),
            Action::Links => self.options.links = !self.options.links,
            Action::Table => self.options.table_hidden = !self.options.table_hidden,
            Action::Options(params) => self.run("project.options", params),
            Action::TimeScale(step) => self.time_scale(step),
            Action::View(view) => self.open_view(view),
            Action::Theme(preference) => ctx.set_theme(preference),
            Action::ToggleToolbar => self.toolbar_hidden = !self.toolbar_hidden,
            Action::Search => self.side = Some(Side::Filters),
            Action::Calendars => self.side = Some(Side::Calendars),
            Action::Levelling => self.side = Some(Side::Levelling),
            Action::Pert => self.side = Some(Side::Pert),
            Action::Zoom(z) => self.zoom = z.clamp(50, 200),
            Action::Link | Action::Unlink => {
                let ids = self.chosen_ids();
                if ids.len() >= 2 {
                    let command = if matches!(action, Action::Link) { "relationship.add" } else { "relationship.delete" };
                    self.run(command, json!({ "activities": ids }));
                }
            }
            Action::Indent => self.indent(),
            Action::Outdent => {
                let p = &self.session.project;
                if let Some(a) = self.selected.and_then(|i| p.activities.get(i)) {
                    // Beside its summary: under that summary's own, or at the top.
                    let up = a.parent.as_deref().and_then(|s| p.activities.iter().find(|x| x.id == s)).and_then(|s| s.parent.clone());
                    let params = json!({"id": a.id, "parent": up.unwrap_or_default()});
                    self.run("activity.edit", params);
                }
            }
            Action::Today => self.options.goto = self.options.today,
            Action::Locate => {
                if let Some(i) = self.selected {
                    self.options.goto = self.session.schedule.as_ref().ok().and_then(|s| s.activities.get(i)).map(|s| s.early_start);
                    self.options.goto_row = Some(i);
                }
            }
            Action::Level => {
                self.run("resource.level", json!({}));
                self.level_status();
            }
            Action::Present => self.present(ctx, true),
            Action::Font(choice) => {
                if let Some(save) = &self.services.save_font {
                    save(&choice);
                }
                self.font = choice;
            }
            Action::Comment => {
                if self.selected.is_some() {
                    self.side = Some(Side::Details);
                    self.details_tab = DetailsTab::Comments;
                }
            }
            Action::Language(lang) => {
                i18n::set(lang);
                self.calculation = None;
            }
        }
    }

    /// The Gantt chart's time scale: wider (`1`), narrower (`-1`), or fitted to the project (`0`).
    pub fn time_scale(&mut self, step: i8) {
        self.options.scale = match (step, self.options.scale) {
            (0, _) => None,
            // From the fitted scale, the first step makes a day 6 points wide.
            (_, None) => Some(if step > 0 { 6.0 } else { 2.0 }),
            (s, Some(scale)) => Some((scale * if s > 0 { 1.5 } else { 1.0 / 1.5 }).clamp(activities::MIN_SCALE, activities::MAX_SCALE)),
        };
    }

    /// A new activity after the selected one (or at the end), selected and its name being
    /// edited.
    pub fn add_activity(&mut self) {
        let after = self.selected.and_then(|i| self.session.project.activities.get(i)).map(|a| a.id.clone());
        let count = self.session.project.activities.len();
        self.run("activity.add", json!({ "after": after }));
        if self.session.project.activities.len() > count {
            let index = after.and_then(|a| self.session.project.activities.iter().position(|x| x.id == a)).map_or(count, |k| k + 1);
            self.selected = Some(index);
            let name = self.session.project.activities.get(index).map(|a| a.name.clone()).unwrap_or_default();
            self.editing = Some(Editing { index, column: Column::Name, text: name });
        }
    }

    /// Deletes the chosen activities (the selected one, or all of those chosen together), as one
    /// change.
    pub fn delete_activity(&mut self) {
        let mut chosen: std::collections::BTreeSet<usize> = self.marked.clone();
        chosen.extend(self.selected);
        let ids: Vec<String> = chosen.iter().filter_map(|i| self.session.project.activities.get(*i)).map(|a| a.id.clone()).collect();
        if !ids.is_empty() {
            self.run("activity.delete", json!({ "ids": ids }));
            self.selected = None;
            self.marked.clear();
        }
    }

    /// The IDs of the activities chosen (the marked ones, and the selected), in the table's order.
    fn chosen_ids(&self) -> Vec<String> {
        let chosen = |i: usize| self.marked.contains(&i) || self.selected == Some(i);
        activities::rows(&self.session)
            .into_iter()
            .filter(|(i, _)| chosen(*i))
            .filter_map(|(i, _)| self.session.project.activities.get(i).map(|a| a.id.clone()))
            .collect()
    }

    /// The selected activity under the one above it at its level in the WBS (which becomes a
    /// summary, if it wasn't).
    fn indent(&mut self) {
        let Some(i) = self.selected else { return };
        let rows = activities::rows(&self.session);
        let Some(at) = rows.iter().position(|(r, _)| *r == i) else { return };
        let depth = rows.get(at).map_or(0, |(_, d)| *d);
        let above =
            rows.get(..at).and_then(|before| before.iter().rev().take_while(|(_, d)| *d >= depth).find(|(_, d)| *d == depth)).map(|(r, _)| *r);
        let p = &self.session.project;
        let (Some(a), Some(parent)) = (p.activities.get(i), above.and_then(|k| p.activities.get(k))) else {
            let id = p.activities.get(i).map_or_else(String::new, |a| a.id.clone());
            self.status = Some(Status::Error(tr!("err-nothing-above", id = id.as_str())));
            return;
        };
        let params = json!({"id": a.id, "parent": parent.id});
        self.run("activity.edit", params);
    }

    /// The minutes of a working day on the calendar of the activity at `index`.
    pub fn day_minutes_of(&self, index: usize) -> i64 {
        let p = &self.session.project;
        p.activities
            .get(index)
            .and_then(|a| a.calendar.as_deref())
            .and_then(|n| p.calendars.iter().find(|c| c.name == n))
            .map_or(p.calendar.day_minutes(), |c| c.day_minutes())
    }

    /// An edit made in a table cell.
    fn commit(&mut self, index: usize, column: Column, text: String) {
        let Some(a) = self.session.project.activities.get(index) else { return };
        let id = a.id.clone();
        let params = match column {
            Column::Id if text.trim() != id => json!({"id": id, "new_id": text.trim()}),
            Column::Name if a.name != text => json!({"id": id, "name": text}),
            // Days, with a decimal point or comma for part of a day.
            Column::Duration => match text.trim().replace(',', ".").parse::<f64>() {
                Ok(d) if d.is_finite() && d >= 0.0 && d.fract() == 0.0 => {
                    if a.duration == d as i64 && a.duration_minutes.is_none() {
                        return;
                    }
                    json!({"id": id, "duration": d as i64})
                }
                Ok(d) if d.is_finite() && d >= 0.0 => {
                    let day = self.day_minutes_of(index);
                    json!({"id": id, "duration_minutes": (d * day as f64).round() as i64})
                }
                _ => {
                    self.status = Some(Status::Error(tr!("err-not-a-duration", text = text.as_str())));
                    return;
                }
            },
            // A day picked or typed: the start moves the activity (it starts on or after it),
            // the finish stretches it, as dragging its bar does.
            Column::Start | Column::Finish => {
                let Some(d) = read_date(&text) else {
                    self.status = Some(Status::Error(tr!("err-not-a-date", text = text.as_str())));
                    return;
                };
                let Some(s) = self.session.schedule.as_ref().ok().and_then(|s| s.activities.get(index)) else { return };
                let stretches = column == Column::Finish && a.kind == ActivityKind::Task && a.working_days() > 0;
                let edit = if stretches {
                    activities::BarEdit { index, mode: activities::BarMode::Finish, start: s.early_start, finish: d.max(s.early_start) }
                } else {
                    activities::BarEdit { index, mode: activities::BarMode::Move, start: d, finish: d }
                };
                let now = if column == Column::Start { s.early_start } else { s.early_finish };
                if d != now {
                    self.edit_bar(edit);
                }
                return;
            }
            _ => return,
        };
        self.run("activity.edit", params);
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        // Undo, redo and the keys that act on the selection only when no text box has the
        // keyboard: there they edit the text.
        if ctx.memory(|m| m.focused().is_none()) {
            let shortcut = |modifiers, key| ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(modifiers, key)));
            if shortcut(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z) || shortcut(egui::Modifiers::COMMAND, egui::Key::Y) {
                self.run("edit.redo", json!(null));
            } else if shortcut(egui::Modifiers::COMMAND, egui::Key::Z) {
                self.run("edit.undo", json!(null));
            }
            if shortcut(egui::Modifiers::NONE, egui::Key::Insert) {
                self.add_activity();
            }
            if shortcut(egui::Modifiers::NONE, egui::Key::Delete) {
                self.delete_activity();
            }
            // Every activity chosen; Esc leaves the selected one alone.
            if shortcut(egui::Modifiers::COMMAND, egui::Key::A) {
                self.marked = (0..self.session.project.activities.len()).collect();
                self.selected = self.selected.or(self.marked.iter().next().copied());
            }
            // Read, not taken: the backstage and the quick window close on Esc too.
            if self.quick_note.is_none() && self.backstage.is_none() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.marked = self.selected.into_iter().collect();
            }
        }
        let shortcut = |modifiers, key| ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(modifiers, key)));
        if shortcut(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::S) {
            self.save_as();
        } else if shortcut(egui::Modifiers::COMMAND, egui::Key::S) {
            self.save();
        }
        if shortcut(egui::Modifiers::COMMAND, egui::Key::O) {
            self.pick_and_open();
        }
        if shortcut(egui::Modifiers::COMMAND, egui::Key::N) {
            self.new_project();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&chrome::PRESENT)) {
            self.present(ctx, !self.presenting);
        }
        if self.presenting && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.present(ctx, false);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&chrome::COMMENT)) && self.selected.is_some() {
            self.side = Some(Side::Details);
            self.details_tab = DetailsTab::Comments;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&chrome::TOGGLE_TOOLBAR)) {
            self.toolbar_hidden = !self.toolbar_hidden;
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
        if self.font_set.as_ref() != Some(&self.font) {
            let family = self.font.family.clone();
            let bytes = family.and_then(|f| self.services.load_font.as_ref().and_then(|load| load(&f, self.font.bold, self.font.italic)));
            ui.ctx().set_fonts(fonts::definitions(&self.font, bytes));
            self.font_set = Some(self.font.clone());
        }
        if self.font_families.is_none() {
            self.font_families = Some(self.services.fonts.as_ref().map(|list| list()).unwrap_or_default());
        }
        self.handle_input(ui.ctx());
        self.options.today = self.services.today.as_ref().and_then(|today| today());
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
        if self.presenting {
            self.presentation(ui);
            return;
        }
        // The chrome — header and toolbar — is one surface, painted once its height is known.
        let chrome = ui.painter().add(egui::Shape::Noop);
        ui.spacing_mut().item_spacing.y = 0.0;
        let state = chrome::State {
            name: &name,
            modified: self.session.modified,
            has_file: self.session.path.is_some(),
            autosave: self.autosave,
            backstage: self.backstage.is_some(),
            font: &self.font,
            font_families: self.font_families.as_deref().unwrap_or_default(),
            critical_only: self.options.critical_only,
            late_bars: self.options.late_bars,
            baseline_bars: self.options.baseline_bars,
            links: self.options.links,
            table_hidden: self.options.table_hidden,
            float_kind: self.session.project.float_kind,
            data_date: self.session.project.data_date,
            progress_mode: self.session.project.progress_mode,
            critical_by: self.session.project.critical_by,
            fitted: self.options.scale.is_none(),
            view: self.view,
            undo: self.session.undo_label(),
            redo: self.session.redo_label(),
            selected: self.selected.is_some(),
            chosen: self.chosen_ids().len(),
            nested: self.selected.and_then(|i| self.session.project.activities.get(i)).is_some_and(|a| a.parent.is_some()),
            today: self.options.today.is_some(),
            zoom: self.zoom,
            toolbar_hidden: self.toolbar_hidden,
        };
        let mut actions = Vec::new();
        actions.extend(chrome::header(ui, &state));
        // The File backstage covers everything under the header.
        if let Some(page) = self.backstage {
            let bottom = ui.cursor().top();
            ui.painter().set(chrome, egui::Shape::rect_filled(egui::Rect::from_min_max(whole.min, egui::pos2(whole.right(), bottom)), 0.0, t.ribbon));
            for action in actions {
                self.act(ui.ctx(), action);
            }
            let schedule = self.session.schedule.as_ref().ok();
            let finish = schedule.map(|s| show_date(s.finish));
            let selected = self.selected.and_then(|i| self.session.project.activities.get(i)).map(|a| a.id.clone());
            let p = &self.session.project;
            let view = backstage::View {
                page,
                recent: &self.recent,
                name: &name,
                file_path: self.session.path.as_deref(),
                modified: self.session.modified,
                autosave: self.autosave,
                activities: p.activities.len(),
                relationships: p.relationships.len(),
                resources: p.resources.len(),
                calendars: 1 + p.calendars.len(),
                finish,
                selected: selected.as_deref(),
            };
            if let Some(action) = backstage::show(ui, &view) {
                self.backstage_act(ui.ctx(), action);
            }
            if self.autosave && self.session.modified && self.session.path.is_some() {
                self.run("project.save", json!(null));
            }
            return;
        }
        actions.extend(chrome::toolbar(ui, &state));
        let bottom = ui.cursor().top();
        ui.painter().set(chrome, egui::Shape::rect_filled(egui::Rect::from_min_max(whole.min, egui::pos2(whole.right(), bottom)), 0.0, t.ribbon));
        for action in actions {
            self.act(ui.ctx(), action);
        }

        let body = (ui.available_height() - STATUS_HEIGHT - 8.0).max(100.0);
        ui.add_space(6.0);
        let gap = 6.0;
        // The activities take the width; a side pane, when open, sits beside the strip.
        let side_width = if self.side.is_some() { SIDE_WIDTH + gap } else { 0.0 };
        let left_width = (ui.available_width() - RAIL_WIDTH - SIDE_STRIP - side_width - 2.0 * gap).floor();
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(gap, 0.0);
            let top_down = egui::Layout::top_down(egui::Align::Min);
            self.view_rail(ui, body);
            ui.allocate_ui_with_layout(egui::vec2(left_width, body), top_down, |ui| {
                ui.set_width(left_width);
                ui.set_height(body);
                // The open views as tabs; the active one under them.
                self.view_tabs(ui);
                let below = (body - VIEW_TABS - 4.0).max(100.0);
                match self.view {
                    View::Activities => self.left_pane(ui, below),
                    View::Resources => self.resources_view(ui, below),
                }
            });
            if let Some(side) = self.side {
                ui.allocate_ui_with_layout(egui::vec2(SIDE_WIDTH, body), top_down, |ui| {
                    ui.set_width(SIDE_WIDTH);
                    ui.set_height(body);
                    theme::card(&t).fill(t.paper).show(ui, |ui| {
                        ui.set_min_size(ui.available_size());
                        match side {
                            Side::Details => self.details(ui),
                            Side::Filters => self.filters(ui),
                            Side::Notes => self.notes_pane(ui),
                            Side::Calendars => self.calendars_pane(ui),
                            Side::Levelling => self.levelling_pane(ui),
                            Side::Pert => self.pert_pane(ui),
                            Side::Baselines => self.baselines_pane(ui),
                        }
                    });
                });
            }
            ui.allocate_ui_with_layout(egui::vec2(SIDE_STRIP, body), top_down, |ui| {
                ui.set_width(SIDE_STRIP);
                ui.set_height(body);
                self.side_strip(ui);
            });
        });
        ui.add_space(2.0);
        self.status_line(ui);
        self.quick_note_window(ui.ctx());
        let zoom = self.zoom as f32 / 100.0;
        if (ui.ctx().zoom_factor() - zoom).abs() > 0.001 {
            ui.ctx().set_zoom_factor(zoom);
        }
        if self.autosave && self.session.modified && self.session.path.is_some() {
            self.run("project.save", json!(null));
        }
    }

    /// Starts presenting (full screen, the chrome away) or ends it.
    pub fn present(&mut self, ctx: &egui::Context, on: bool) {
        self.presenting = on;
        self.quick_note = None;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(on));
    }

    /// Presenting: the project's name over the activities and the chart on the whole window, a
    /// quiet way out in the corner.
    fn presentation(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        let mut leave = false;
        ui.horizontal(|ui| {
            ui.set_height(40.0);
            ui.add_space(12.0);
            icons::show(ui, Icon::Mark, 22.0);
            ui.add_space(6.0);
            let name = if self.session.project.name.is_empty() { tr!("untitled") } else { self.session.project.name.clone() };
            ui.label(egui::RichText::new(name).size(19.0).color(t.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                leave = theme::ghost(ui, &tr!("present-leave"), "Esc", true);
            });
        });
        let grid = egui::Frame::new().fill(t.grid_bg).stroke(egui::Stroke::new(1.0, t.border)).inner_margin(egui::Margin::same(1));
        let mut events = activities::Events::default();
        grid.show(ui, |ui| {
            let size = ui.available_size();
            ui.allocate_ui(size, |ui| {
                ui.set_min_size(size);
                if self.session.schedule.is_ok() {
                    events = activities::show(ui, &self.session, &mut self.options, self.selected, &self.marked, &mut self.editing);
                }
            });
        });
        // Choosing still works, to point at an activity; nothing opens over the picture.
        if let Some(i) = events.clicked {
            self.selected = Some(i);
            self.marked = std::iter::once(i).collect();
        }
        if leave {
            self.present(ui.ctx(), false);
        }
    }

    /// Opens `view` as a tab, or brings its tab forward.
    pub fn open_view(&mut self, view: View) {
        if !self.open_views.contains(&view) {
            self.open_views.push(view);
        }
        self.view = view;
    }

    /// The tabs of the open views: each with its icon and name, the active one underlined in
    /// the accent; every one but the activities closes.
    fn view_tabs(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        let mut choose = None;
        let mut close = None;
        ui.horizontal(|ui| {
            ui.set_height(VIEW_TABS);
            ui.spacing_mut().item_spacing.x = 2.0;
            for view in self.open_views.clone() {
                let (icon, name) = match view {
                    View::Activities => (Icon::Gantt, tr!("activities-view")),
                    View::Resources => (Icon::People, tr!("resources-view")),
                };
                let on = self.view == view;
                let galley = ui.painter().layout_no_wrap(name.clone(), egui::FontId::proportional(13.0), if on { t.text } else { t.text_dim });
                let closable = view != View::Activities;
                let width = 34.0 + galley.size().x + if closable { 22.0 } else { 10.0 };
                let (rect, response) = ui.allocate_exact_size(egui::vec2(width, VIEW_TABS), egui::Sense::click());
                response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, on, &name));
                if on {
                    ui.painter().rect_filled(rect, egui::CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, t.ribbon);
                    ui.painter().hline(rect.x_range(), rect.bottom() - 1.0, egui::Stroke::new(2.0, crate::splash::BRAND));
                } else if response.hovered() {
                    ui.painter().rect_filled(rect, egui::CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, t.hover);
                }
                icons::paint(
                    ui.painter(),
                    egui::Rect::from_center_size(egui::pos2(rect.left() + 18.0, rect.center().y), egui::vec2(14.0, 14.0)),
                    icon,
                    &t,
                );
                ui.painter().galley(egui::pos2(rect.left() + 30.0, rect.center().y - galley.size().y / 2.0), galley, t.text);
                if response.clicked() {
                    choose = Some(view);
                }
                if closable {
                    let x = egui::Rect::from_center_size(egui::pos2(rect.right() - 13.0, rect.center().y), egui::vec2(16.0, 16.0));
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(x));
                    if icon_button(&mut child, Icon::Close, &tr!("view-close", name = name.as_str())) {
                        close = Some(view);
                    }
                }
            }
        });
        if let Some(view) = choose {
            self.view = view;
        }
        if let Some(view) = close {
            self.open_views.retain(|v| *v != view);
            if self.view == view {
                self.view = self.open_views.last().copied().unwrap_or_default();
            }
        }
    }

    /// The rail of views down the left edge.
    fn view_rail(&mut self, ui: &mut egui::Ui, height: f32) {
        let t = Tokens::get(ui.ctx());
        let (rect, _) = ui.allocate_exact_size(egui::vec2(RAIL_WIDTH, height), egui::Sense::hover());
        ui.painter().rect_filled(rect, 0.0, t.ribbon);
        ui.painter().vline(rect.right(), rect.y_range(), egui::Stroke::new(1.0, t.border));
        let views = [
            (Icon::Gantt, tr!("activities-view"), tr!("activities-view-tip"), View::Activities),
            (Icon::People, tr!("resources-view"), tr!("resources-view-tip"), View::Resources),
        ];
        let views_len = views.len();
        for (k, (icon, name, tip, view)) in views.into_iter().enumerate() {
            let on = self.view == view;
            let slot = egui::Rect::from_min_size(rect.min + egui::vec2(0.0, 4.0 + 42.0 * k as f32), egui::vec2(RAIL_WIDTH - 1.0, 40.0));
            let response = ui.interact(slot, ui.scope_id().with(("rail", k)), egui::Sense::click());
            response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, &name));
            let painter = ui.painter();
            if on {
                painter.rect_filled(slot, 0.0, t.accent_soft);
                painter.rect_filled(egui::Rect::from_min_size(slot.min, egui::vec2(3.0, slot.height())), 0.0, t.accent);
            } else if response.hovered() {
                painter.rect_filled(slot, 0.0, t.hover);
            }
            icons::paint(painter, egui::Rect::from_center_size(slot.center(), egui::vec2(20.0, 20.0)), icon, &t);
            if response.on_hover_text(tip).clicked() {
                self.open_view(view);
            }
        }
        // Under the views, a hairline and the activity table's switch (in their view): off, the
        // chart takes the width.
        let top = rect.top() + 4.0 + 42.0 * views_len as f32;
        ui.painter().hline((rect.left() + 10.0)..=(rect.right() - 11.0), top + 2.0, egui::Stroke::new(1.0, t.border));
        let slot = egui::Rect::from_min_size(egui::pos2(rect.left(), top + 6.0), egui::vec2(RAIL_WIDTH - 1.0, 40.0));
        let shown = !self.options.table_hidden;
        let name = tr!("activity-table");
        let response = ui.interact(slot, ui.scope_id().with("rail-table"), egui::Sense::click());
        response.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, shown, &name));
        if shown {
            ui.painter().rect_filled(slot.shrink(4.0), 4.0, t.accent_soft);
        } else if response.hovered() {
            ui.painter().rect_filled(slot.shrink(4.0), 4.0, t.hover);
        }
        icons::paint(ui.painter(), egui::Rect::from_center_size(slot.center(), egui::vec2(18.0, 18.0)), Icon::Table, &t);
        let tip = if shown { tr!("activity-table-hide-tip") } else { tr!("activity-table-show-tip") };
        if response.on_hover_text(tip).clicked() {
            self.options.table_hidden = shown;
        }
    }

    /// The Activities view over the selected activity's calculation.
    fn left_pane(&mut self, ui: &mut egui::Ui, height: f32) {
        let t = Tokens::get(ui.ctx());
        let table_height = (height - CALCULATION_HEIGHT - 6.0).max(140.0);
        let grid = egui::Frame::new().fill(t.grid_bg).stroke(egui::Stroke::new(1.0, t.border)).inner_margin(egui::Margin::same(1));
        let mut events = activities::Events::default();
        grid.show(ui, |ui| {
            let size = egui::vec2(ui.available_width(), table_height - 2.0);
            ui.allocate_ui(size, |ui| {
                ui.set_min_size(size);
                match &self.session.schedule {
                    Ok(_) => events = activities::show(ui, &self.session, &mut self.options, self.selected, &self.marked, &mut self.editing),
                    Err(e) => {
                        ui.add_space(20.0);
                        ui.colored_label(t.danger, format!("  {e}"));
                    }
                }
            });
        });
        if events.clicked.is_some() && events.clicked != self.selected {
            self.selected = events.clicked;
            self.calculation = None;
        }
        // A plain click chooses the one row; Ctrl adds or takes one; a drag or Shift chooses many.
        if let Some(i) = events.clicked
            && events.marked.is_none()
        {
            self.marked = std::iter::once(i).collect();
        }
        if let Some(i) = events.toggled {
            // The selected one counts among those chosen; the row clicked joins them or leaves.
            self.marked.extend(self.selected);
            if !self.marked.remove(&i) {
                self.marked.insert(i);
            }
            self.selected = if self.marked.contains(&i) { Some(i) } else { self.marked.iter().next().copied() };
            self.calculation = None;
        }
        if let Some(rows) = events.marked {
            self.marked = rows.into_iter().collect();
            if events.clicked.is_none() {
                self.selected = self.marked.iter().next().copied();
            }
            self.calculation = None;
        }
        if let Some(i) = events.open {
            self.selected = Some(i);
            self.marked = std::iter::once(i).collect();
            self.side = Some(Side::Details);
            self.details_tab = DetailsTab::General;
        }
        if let Some((day, at)) = events.add_band {
            self.quick_note = Some(QuickNote { month: false, from: day, to: day, name: String::new(), at });
        }
        if let Some((month, at)) = events.add_month {
            self.quick_note = Some(QuickNote { month: true, from: month, to: month, name: String::new(), at });
        }
        if let Some(edit) = events.bar {
            self.edit_bar(edit);
        }
        if let Some((i, action)) = events.menu {
            self.selected = Some(i);
            match action {
                activities::RowAction::AddAfter => self.add_activity(),
                activities::RowAction::Delete => self.delete_activity(),
                activities::RowAction::Comment => {
                    self.side = Some(Side::Details);
                    self.details_tab = DetailsTab::Comments;
                }
                activities::RowAction::Details => {
                    self.side = Some(Side::Details);
                    self.details_tab = DetailsTab::General;
                }
            }
        }
        if let Some((index, column, text)) = events.commit {
            self.commit(index, column, text);
        }
        ui.add_space(6.0);
        let mut schedule = false;
        let mut export = false;
        theme::card(&t).show(ui, |ui| {
            ui.set_min_size(egui::vec2(ui.available_width(), ui.available_height()));
            schedule = theme::pane_header(ui, Icon::List, &tr!("pane-calculation"), &tr!("pane-calculation-tip"), |ui| {
                let run = chrome::accent_button(ui, Icon::Run, &tr!("schedule")).on_hover_text(tr!("schedule-tip")).clicked();
                if self.selected.is_some() {
                    ui.add_space(6.0);
                    export = ui.button(tr!("export-tex")).on_hover_text(tr!("export-tex-tip")).clicked();
                }
                run
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
        if export {
            self.export_latex();
        }
    }

    /// The selected activity's calculation as a LaTeX document, where the person picks.
    pub fn export_latex(&mut self) {
        let Some(id) = self.selected.and_then(|i| self.session.project.activities.get(i)).map(|a| a.id.clone()) else { return };
        let suggested = format!("{}.tex", id.replace(['/', '\\'], "-"));
        if let Some(path) = self.services.pick_save.as_ref().and_then(|pick| pick(&suggested)) {
            self.status = Some(match self.session.export_latex(&id, &path, &show_date) {
                Ok(()) => Status::Info(tr!("status-exported", name = path.display().to_string().as_str())),
                Err(e) => Status::Error(e.to_string()),
            });
        }
    }

    /// The selected activity's facts, relationships and dates.
    fn details(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        if theme::pane_header(ui, Icon::Info, &tr!("pane-details"), "", |ui| icon_button(ui, Icon::ChevronRight, &tr!("pane-close"))) {
            self.side = None;
        }
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
        let mut pending: Option<(&str, serde_json::Value)> = None;
        let day = self.day_minutes_of(i);
        let author = self.services.user.as_ref().map(|f| f()).unwrap_or_default();
        let at = self.services.now.as_ref().map(|f| f()).unwrap_or_default();
        ui.label(egui::RichText::new(&a.name).size(15.0).color(t.text).strong());
        ui.add_space(4.0);
        // General, Status, Relationships, Notes and Comments, one at a time.
        let mut tab = self.details_tab;
        // Wrapped: five tabs are wider than the pane in some languages.
        ui.horizontal_wrapped(|ui| {
            for (k, name) in [
                (DetailsTab::General, tr!("details-general")),
                (DetailsTab::Status, tr!("details-status")),
                (DetailsTab::Relationships, tr!("details-relationships")),
                (DetailsTab::Notes, tr!("details-notes")),
                (DetailsTab::Resources, tr!("details-resources")),
                (DetailsTab::Comments, tr!("details-comments")),
            ] {
                ui.selectable_value(&mut tab, k, name);
            }
        });
        ui.separator();
        // Notes typed and not yet saved when another tab took over.
        if tab != DetailsTab::Notes {
            let key = egui::Id::unique(("notes", i)).with("text");
            if let Some((was, text)) = ui.data_mut(|d| d.remove_temp::<(String, String)>(key))
                && was == a.notes
                && text != a.notes
            {
                pending = Some(("activity.edit", json!({"id": a.id, "notes": text})));
            }
        }
        egui::ScrollArea::vertical().id_salt("details").auto_shrink(false).show(ui, |ui| {
            match tab {
                DetailsTab::General => {
                    fact(ui, tr!("detail-id"), a.id.clone());
                    fact(ui, tr!("detail-kind"), kind);
                    if !summary && !a.kind.is_milestone() {
                        fact(ui, tr!("detail-duration"), tr!("detail-days", n = days_text(a.working_minutes(day), day).as_str()));
                        // What follows when its duration, its units or their units an hour change.
                        ui.horizontal(|ui| {
                            let label = ui.add_sized(
                                egui::vec2(120.0, 18.0),
                                egui::Label::new(egui::RichText::new(tr!("detail-duration-type")).size(12.0).color(t.text_dim)),
                            );
                            let name = |d: mkperth_engine::core::DurationType| tr!(&format!("duration-type-{}", d.code()));
                            let mut chosen = a.duration_type;
                            let combo = egui::ComboBox::from_id_salt("duration-type").selected_text(name(chosen)).width(190.0).show_ui(ui, |ui| {
                                for d in mkperth_engine::core::DurationType::ALL {
                                    ui.selectable_value(&mut chosen, d, name(d)).on_hover_text(tr!(&format!("duration-type-{}-tip", d.code())));
                                }
                            });
                            combo.response.labelled_by(label.id);
                            if chosen != a.duration_type {
                                pending = Some(("activity.edit", json!({"id": a.id, "duration_type": chosen.code()})));
                            }
                        });
                        ui.add_space(3.0);
                        // Its resources each on their own calendar, finishing with the last.
                        ui.horizontal(|ui| {
                            ui.add_space(124.0);
                            let mut on = a.resource_dependent;
                            if ui.checkbox(&mut on, tr!("detail-resource-dependent")).on_hover_text(tr!("detail-resource-dependent-tip")).clicked() {
                                pending = Some(("activity.edit", json!({"id": a.id, "resource_dependent": on})));
                            }
                        });
                        ui.add_space(3.0);
                        // Which of two activities keeps a resource both want when levelling.
                        ui.horizontal(|ui| {
                            let label = ui.add_sized(
                                egui::vec2(120.0, 18.0),
                                egui::Label::new(egui::RichText::new(tr!("detail-levelling-priority")).size(12.0).color(t.text_dim)),
                            );
                            let name = |l: mkperth_engine::core::LevellingPriority| tr!(&format!("levelling-priority-{}", l.code()));
                            let mut chosen = a.levelling_priority;
                            let combo =
                                egui::ComboBox::from_id_salt("levelling-priority").selected_text(name(chosen)).width(190.0).show_ui(ui, |ui| {
                                    for l in mkperth_engine::core::LevellingPriority::ALL {
                                        ui.selectable_value(&mut chosen, l, name(l));
                                    }
                                });
                            combo.response.labelled_by(label.id).on_hover_text(tr!("detail-levelling-priority-tip"));
                            if chosen != a.levelling_priority {
                                pending = Some(("activity.edit", json!({"id": a.id, "levelling_priority": chosen.code()})));
                            }
                        });
                        ui.add_space(3.0);
                        // PERT's three estimates, for the analysis only.
                        if let Some(v) = pert_ui::pert_fields(ui, a) {
                            pending = Some(("activity.edit", json!({"id": a.id, "pert": v})));
                        }
                    }
                    fact(ui, tr!("detail-calendar"), a.calendar.clone().unwrap_or_else(|| project.calendar.name.clone()));
                    if let Some(parent) = a.parent.as_deref() {
                        fact(ui, tr!("detail-wbs"), parent.to_string());
                    }
                    for (name, c) in [(tr!("detail-constraint"), a.constraint), (tr!("detail-secondary-constraint"), a.secondary_constraint)] {
                        if let Some(c) = c {
                            fact(ui, name, constraint_name(c, &show_date));
                        }
                    }
                }
                DetailsTab::Status => {
                    // Progress: where it stands, its actual dates and the work left.
                    if !summary {
                        if let Some(s) = scheduled {
                            let status = match s.status {
                                mkperth_engine::sched::Status::NotStarted => tr!("status-not-started"),
                                mkperth_engine::sched::Status::InProgress => tr!("status-in-progress"),
                                mkperth_engine::sched::Status::Completed => tr!("status-completed"),
                            };
                            fact(ui, tr!("detail-status"), status);
                        }
                        let mut fields = vec![
                            (tr!("detail-actual-start"), "actual_start", a.actual_start),
                            (tr!("detail-actual-finish"), "actual_finish", a.actual_finish),
                        ];
                        if !a.kind.is_milestone() && a.actual_finish.is_none() {
                            fields.push((tr!("detail-expected-finish"), "expected_finish", a.expected_finish));
                        }
                        for (name, key, value) in fields {
                            ui.horizontal(|ui| {
                                let label =
                                    ui.add_sized(egui::vec2(120.0, 18.0), egui::Label::new(egui::RichText::new(name).size(12.0).color(t.text_dim)));
                                if let Some(d) = date_field(ui, ui.scope_id().with((key, i)), label.id, value, 100.0) {
                                    pending = Some(("activity.edit", json!({"id": a.id, key: d.map(|d| d.to_string())})));
                                }
                            });
                            ui.add_space(3.0);
                        }
                        if a.actual_start.is_some() && a.actual_finish.is_none() {
                            let left = scheduled.map_or(0, |s| s.remaining_minutes);
                            ui.horizontal(|ui| {
                                ui.add_sized(
                                    egui::vec2(120.0, 18.0),
                                    egui::Label::new(egui::RichText::new(tr!("detail-remaining")).size(12.0).color(t.text_dim)),
                                );
                                let mut days = left as f64 / day.max(1) as f64;
                                let response = ui.add(
                                    egui::DragValue::new(&mut days)
                                        .range(0.0..=99_999.0)
                                        .speed(0.1)
                                        .max_decimals(2)
                                        .custom_formatter(|n, _| {
                                            mkperth_engine::i18n::decimal(format!("{n:.2}").trim_end_matches('0').trim_end_matches('.'))
                                        })
                                        .custom_parser(|text| text.trim().replace(',', ".").parse().ok()),
                                );
                                if response.changed() && !response.dragged() || response.drag_stopped() {
                                    let minutes = (days * day as f64).round() as i64;
                                    if minutes != left {
                                        pending = Some(("activity.edit", json!({"id": a.id, "remaining_minutes": minutes})));
                                    }
                                }
                            });
                            ui.add_space(3.0);
                        }
                    }
                    if let Some(s) = scheduled {
                        ui.add_space(6.0);
                        fact(ui, tr!("col-early-start"), show_date(s.early_start));
                        fact(ui, tr!("col-early-finish"), show_date(s.early_finish));
                        fact(ui, tr!("col-late-start"), show_date(s.late_start));
                        fact(ui, tr!("col-late-finish"), show_date(s.late_finish));
                        fact(ui, tr!("col-total-float"), days_text(s.total_float_minutes, day));
                        fact(ui, tr!("col-free-float"), days_text(s.free_float_minutes, day));
                        if s.critical {
                            ui.label(egui::RichText::new(tr!("detail-critical")).size(12.5).color(t.red));
                        }
                        if s.longest_path {
                            ui.label(egui::RichText::new(tr!("detail-longest-path")).size(12.5).color(t.text));
                        }
                    }
                }
                DetailsTab::Relationships => {
                    let lag = |l: i64| if l == 0 { String::new() } else { format!(" {l:+}") };
                    let mut remove = None;
                    for (title, predecessors) in [(tr!("detail-predecessors"), true), (tr!("detail-successors"), false)] {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(title).size(12.5).color(t.text).strong());
                        let list: Vec<&mkperth_engine::core::Relationship> =
                            project.relationships.iter().filter(|r| if predecessors { r.successor == a.id } else { r.predecessor == a.id }).collect();
                        if list.is_empty() {
                            ui.label(egui::RichText::new(tr!("detail-none")).size(12.0).color(t.text_dim));
                        }
                        for r in list {
                            let other = if predecessors { &r.predecessor } else { &r.successor };
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(format!("{other}  {}{}", r.kind.code(), lag(r.lag))).size(12.5).color(t.text));
                                if icon_button(ui, Icon::Close, &tr!("relationship-remove")) {
                                    remove = Some((r.predecessor.clone(), r.successor.clone()));
                                }
                            });
                        }
                    }
                    // A new predecessor: its ID, the kind and the lag.
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new(tr!("relationship-add-title")).size(12.0).color(t.text_dim));
                    let mut add = false;
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.link.0).hint_text(tr!("relationship-id-hint")).desired_width(90.0));
                        egui::ComboBox::from_id_salt("link-kind").selected_text(self.link.1.code()).width(48.0).show_ui(ui, |ui| {
                            for k in [
                                RelationshipType::FinishToStart,
                                RelationshipType::StartToStart,
                                RelationshipType::FinishToFinish,
                                RelationshipType::StartToFinish,
                            ] {
                                ui.selectable_value(&mut self.link.1, k, k.code());
                            }
                        });
                        ui.add(egui::DragValue::new(&mut self.link.2).range(-999..=999)).on_hover_text(tr!("relationship-lag-tip"));
                        add = ui.button(tr!("relationship-add")).clicked();
                    });
                    if add && !self.link.0.trim().is_empty() {
                        let params = json!({"predecessor": self.link.0.trim(), "successor": a.id, "kind": self.link.1.code(), "lag": self.link.2});
                        pending = Some(("relationship.add", params));
                    }
                    if let Some((p, s)) = remove {
                        pending = Some(("relationship.delete", json!({"predecessor": p, "successor": s})));
                    }
                }
                DetailsTab::Resources => {
                    let asked = resources_view::activity_assignments(ui, &t, project, &a.id, &mut self.resource_form);
                    if let Some(p) = asked.into_iter().next() {
                        pending = Some(p);
                    }
                }
                DetailsTab::Comments => {
                    if let Some(p) = comments_ui::comments_tab(ui, &t, project, &a.id, &mut self.comment_form, &author, &at) {
                        pending = Some(p);
                    }
                }
                DetailsTab::Notes => {
                    // Kept as typed, saved when the box is left or the tab changes.
                    let id = egui::Id::unique(("notes", i));
                    let key = id.with("text");
                    let mut text =
                        ui.data_mut(|d| d.get_temp::<(String, String)>(key)).filter(|(was, _)| *was == a.notes).map_or(a.notes.clone(), |(_, t)| t);
                    let response = ui.add(
                        egui::TextEdit::multiline(&mut text)
                            .id(id)
                            .desired_width(f32::INFINITY)
                            .desired_rows(12)
                            .hint_text(tr!("details-notes-hint")),
                    );
                    if response.lost_focus() {
                        if text != a.notes {
                            pending = Some(("activity.edit", json!({"id": a.id, "notes": text})));
                        }
                        ui.data_mut(|d| d.remove::<(String, String)>(key));
                    } else {
                        ui.data_mut(|d| d.insert_temp(key, (a.notes.clone(), text)));
                    }
                }
            }
        });
        self.details_tab = tab;
        if let Some((command, params)) = pending {
            let added = command == "relationship.add";
            self.run(command, params);
            let done = matches!(self.status, Some(Status::Info(_)));
            if added && done {
                self.link = (String::new(), RelationshipType::FinishToStart, 0);
            }
            // A comment sent clears its box; a reply closes its own.
            match command {
                "comment.add" if done => self.comment_form.text.clear(),
                "comment.reply" if done => {
                    self.comment_form.reply_to = None;
                    self.comment_form.reply.clear();
                }
                _ => {}
            }
        }
    }

    /// The strip at the right edge: Details and Filters, their names turned down the strip. A
    /// click opens one beside it; a click on the open one closes it.
    fn side_strip(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 0.0, t.ribbon);
        ui.painter().vline(rect.left(), rect.y_range(), egui::Stroke::new(1.0, t.border));
        let count = usize::from(self.options.critical_only) + usize::from(!self.options.search.trim().is_empty());
        let mut top = rect.top() + 4.0;
        for (k, (side, icon, name, badge)) in [
            (Side::Details, Icon::Info, tr!("pane-details"), 0),
            (Side::Filters, Icon::Filter, tr!("filters"), count),
            (Side::Notes, Icon::Pin, tr!("pane-notes"), 0),
            (Side::Calendars, Icon::Calendar, tr!("pane-calendars"), 0),
            (Side::Levelling, Icon::Level, tr!("pane-levelling"), 0),
        ]
        .into_iter()
        .enumerate()
        {
            let painter = ui.painter();
            let galley = painter.layout_no_wrap(name.clone(), egui::FontId::proportional(13.0), t.text);
            let height = 28.0 + galley.size().x + if badge > 0 { 18.0 } else { 6.0 };
            let slot = egui::Rect::from_min_size(egui::pos2(rect.left() + 1.0, top), egui::vec2(rect.width() - 1.0, height));
            top += height + 6.0;
            let response = ui.interact(slot, ui.scope_id().with(("side", k)), egui::Sense::click());
            let on = self.side == Some(side);
            response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, &name));
            let painter = ui.painter();
            if on {
                painter.rect_filled(slot, 0.0, t.accent_soft);
                painter.rect_filled(
                    egui::Rect::from_min_size(egui::pos2(slot.right() - 3.0, slot.top()), egui::vec2(3.0, slot.height())),
                    0.0,
                    t.accent,
                );
            } else if response.hovered() {
                painter.rect_filled(slot, 0.0, t.hover);
            }
            let x = slot.center().x;
            icons::paint(painter, egui::Rect::from_center_size(egui::pos2(x, slot.top() + 13.0), egui::vec2(14.0, 14.0)), icon, &t);
            // Turned a quarter clockwise, it reads downwards and its height lies to the left of `at`.
            let at = egui::pos2(x + galley.size().y / 2.0, slot.top() + 26.0);
            painter.add(egui::epaint::TextShape::new(at, galley, if on { t.accent } else { t.text }).with_angle(std::f32::consts::FRAC_PI_2));
            if badge > 0 {
                painter.text(
                    egui::pos2(x, slot.bottom() - 9.0),
                    egui::Align2::CENTER_CENTER,
                    badge.to_string(),
                    egui::FontId::proportional(12.0),
                    t.accent,
                );
            }
            if response.on_hover_text(name).clicked() {
                self.side = if on { None } else { Some(side) };
            }
        }
    }

    /// The Filters pane: find by ID or name, critical only, late bars.
    fn filters(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        if theme::pane_header(ui, Icon::Filter, &tr!("filters"), "", |ui| icon_button(ui, Icon::ChevronRight, &tr!("pane-close"))) {
            self.side = None;
        }
        ui.spacing_mut().item_spacing.y = 6.0;
        let count = usize::from(self.options.critical_only) + usize::from(!self.options.search.trim().is_empty());
        ui.label(egui::RichText::new(tr!("filter-search")).size(12.0).color(t.text_dim));
        ui.add(egui::TextEdit::singleline(&mut self.options.search).hint_text(tr!("filter-search-hint")).desired_width(f32::INFINITY));
        ui.add_space(4.0);
        ui.checkbox(&mut self.options.critical_only, tr!("critical-only"));
        ui.checkbox(&mut self.options.late_bars, tr!("late-bars"));
        ui.checkbox(&mut self.options.links, tr!("links"));
        if count > 0 && ui.button(tr!("filters-clear")).clicked() {
            self.options.critical_only = false;
            self.options.search.clear();
        }
        // The table's columns: the ID and the name stay.
        ui.add_space(8.0);
        ui.label(egui::RichText::new(tr!("filter-columns")).size(12.0).color(t.text_dim));
        for (k, (id, _)) in activities::COLUMNS.iter().enumerate().skip(2) {
            if let Some(hidden) = self.options.hidden.get_mut(k) {
                let mut shown = !*hidden;
                if ui.checkbox(&mut shown, tr!(id)).changed() {
                    *hidden = !shown;
                }
            }
        }
    }

    /// The chart's notes: each kind listed with a button to delete it, and a form for a new one.
    fn notes_pane(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        if theme::pane_header(ui, Icon::Pin, &tr!("pane-notes"), &tr!("pane-notes-tip"), |ui| icon_button(ui, Icon::ChevronRight, &tr!("pane-close")))
        {
            self.side = None;
        }
        let p = &self.session.project;
        let mut pending: Option<(&str, serde_json::Value)> = None;
        let swatch = |ui: &mut egui::Ui, color: egui::Color32| {
            let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
            ui.painter().rect_filled(r, 2.0, color);
        };
        let hex = |h: &str, fallback: egui::Color32| activities::note_color(h, fallback);
        egui::ScrollArea::vertical().id_salt("notes-pane").auto_shrink(false).show(ui, |ui| {
            let heading = |ui: &mut egui::Ui, text: String| {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(text).size(12.5).color(t.text).strong());
            };
            heading(ui, tr!("notes-bands"));
            if p.bands.is_empty() {
                ui.label(egui::RichText::new(tr!("detail-none")).size(12.0).color(t.text_dim));
            }
            for (k, b) in p.bands.iter().enumerate() {
                ui.horizontal(|ui| {
                    swatch(ui, hex(&b.color, t.blue));
                    ui.label(egui::RichText::new(format!("{}  {} – {}", b.name, show_date(b.from), show_date(b.to))).size(12.0).color(t.text));
                    if icon_button(ui, Icon::Close, &tr!("notes-delete")) {
                        pending = Some(("band.delete", json!({"index": k})));
                    }
                });
            }
            heading(ui, tr!("notes-markers"));
            if p.markers.is_empty() {
                ui.label(egui::RichText::new(tr!("detail-none")).size(12.0).color(t.text_dim));
            }
            for (k, m) in p.markers.iter().enumerate() {
                ui.horizontal(|ui| {
                    swatch(ui, hex(&m.color, t.orange));
                    ui.label(egui::RichText::new(format!("{}  {}", m.name, show_date(m.date))).size(12.0).color(t.text));
                    if icon_button(ui, Icon::Close, &tr!("notes-delete")) {
                        pending = Some(("marker.delete", json!({"index": k})));
                    }
                });
            }
            heading(ui, tr!("notes-months"));
            if p.month_marks.is_empty() {
                ui.label(egui::RichText::new(tr!("detail-none")).size(12.0).color(t.text_dim));
            }
            for (k, m) in p.month_marks.iter().enumerate() {
                let (y, month, _) = m.month.ymd();
                ui.horizontal(|ui| {
                    swatch(ui, hex(&m.color, t.red));
                    let name = if m.name.is_empty() { String::new() } else { format!(" · {}", m.name) };
                    ui.label(egui::RichText::new(format!("{} {y}{name}", tr!(&format!("month-{month}")))).size(12.0).color(t.text));
                    if icon_button(ui, Icon::Close, &tr!("notes-delete")) {
                        pending = Some(("month-mark.delete", json!({"index": k})));
                    }
                });
            }
            // A new note.
            ui.add_space(10.0);
            ui.separator();
            ui.label(egui::RichText::new(tr!("notes-new")).size(12.5).color(t.text).strong());
            let f = &mut self.note_form;
            ui.horizontal(|ui| {
                for (k, name) in [(0u8, tr!("notes-band")), (1, tr!("notes-marker")), (2, tr!("notes-month"))] {
                    ui.selectable_value(&mut f.kind, k, name);
                }
            });
            let row = |ui: &mut egui::Ui, name: String| {
                ui.add_sized(egui::vec2(70.0, 18.0), egui::Label::new(egui::RichText::new(name).size(12.0).color(t.text_dim))).id
            };
            ui.horizontal(|ui| {
                let label = row(ui, tr!("notes-name"));
                ui.add(egui::TextEdit::singleline(&mut f.name).desired_width(170.0)).labelled_by(label);
            });
            let first = match f.kind {
                0 => tr!("notes-from"),
                1 => tr!("notes-date"),
                _ => tr!("notes-month-of"),
            };
            ui.horizontal(|ui| {
                let label = row(ui, first);
                if let Some(d) = date_field(ui, egui::Id::unique("note-from"), label, f.from, 100.0) {
                    f.from = d;
                }
            });
            if f.kind == 0 {
                ui.horizontal(|ui| {
                    let label = row(ui, tr!("notes-to"));
                    if let Some(d) = date_field(ui, egui::Id::unique("note-to"), label, f.to, 100.0) {
                        f.to = d;
                    }
                });
            }
            ui.horizontal(|ui| {
                row(ui, tr!("notes-color"));
                let fallback = [t.blue, t.orange, t.red].get(usize::from(f.kind)).copied().unwrap_or(t.blue);
                for code in std::iter::once("").chain(NOTE_COLORS) {
                    let color = hex(code, fallback);
                    let (r, response) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
                    ui.painter().rect_filled(r.shrink(2.0), 3.0, color);
                    if f.color == code {
                        ui.painter().rect_stroke(r, 3.0, egui::Stroke::new(1.5, t.text), egui::StrokeKind::Inside);
                    }
                    let tip = if code.is_empty() { tr!("notes-color-default") } else { code.to_string() };
                    if response.on_hover_text(tip).clicked() {
                        f.color = code.to_string();
                    }
                }
            });
            ui.add_space(4.0);
            if ui.button(tr!("notes-add")).clicked() {
                let day = |d: Option<Date>| d.map(|d| d.to_string());
                pending = Some(match f.kind {
                    0 => ("band.add", json!({"name": f.name, "from": day(f.from), "to": day(f.to.or(f.from)), "color": f.color})),
                    1 => ("marker.add", json!({"name": f.name, "date": day(f.from), "color": f.color})),
                    _ => ("month-mark.add", json!({"month": day(f.from), "name": f.name, "color": f.color})),
                });
            }
        });
        if let Some((command, params)) = pending {
            let adding = command.ends_with(".add");
            self.run(command, params);
            if adding && matches!(self.status, Some(Status::Info(_))) {
                self.note_form = NoteForm { kind: self.note_form.kind, ..NoteForm::default() };
            }
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
                if self.marked.len() > 1 {
                    facts.push(tr!("fact-chosen", n = self.marked.len()));
                }
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
pub(crate) fn icon_button(ui: &mut egui::Ui, icon: Icon, name: &str) -> bool {
    let t = Tokens::get(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
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
    fn time_scale_steps() {
        let mut app = PerthApp::default();
        assert_eq!(app.options.scale, None, "fitted at first");
        app.time_scale(1);
        assert_eq!(app.options.scale, Some(6.0));
        app.time_scale(1);
        assert_eq!(app.options.scale, Some(9.0));
        for _ in 0..30 {
            app.time_scale(1);
        }
        assert_eq!(app.options.scale, Some(activities::MAX_SCALE));
        for _ in 0..60 {
            app.time_scale(-1);
        }
        assert_eq!(app.options.scale, Some(activities::MIN_SCALE));
        app.time_scale(0);
        assert_eq!(app.options.scale, None);
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
