//! The top of the window, quiet as an online spreadsheet's: a header with the mark, the project's
//! name and whether it is saved, the menus (File, Edit, Project, View) under the name, and search,
//! theme and the Schedule button on the right; under it one slim toolbar of small icons in a
//! rounded band. Buttons only report what was asked for; the app acts through the session.

use crate::View;
use crate::icons::{self, Icon};
use crate::theme::Tokens;
use egui::{Key, KeyboardShortcut, Modifiers};
use mkperth_engine::core::{CriticalBy, Date, FloatKind, ProgressMode};
use mkperth_engine::i18n::{self, Lang, tr};

/// What was asked for in the header, a menu or the toolbar this frame.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Action {
    Open,
    Save,
    SaveAs,
    Sample,
    Schedule,
    Undo,
    Redo,
    AddActivity,
    DeleteActivity,
    CriticalOnly,
    LateBars,
    Table,
    Links,
    /// `project.options`' parameters.
    Options(serde_json::Value),
    /// Wider (1), narrower (-1) or fitted (0).
    TimeScale(i8),
    View(View),
    Theme(egui::ThemePreference),
    ToggleAutoSave,
    ToggleToolbar,
    Search,
    Language(Lang),
}

/// What the header, the menus and the toolbar show.
pub(crate) struct State<'a> {
    /// The project's name.
    pub name: &'a str,
    pub modified: bool,
    /// The project has a file to be saved to.
    pub has_file: bool,
    pub autosave: bool,
    pub critical_only: bool,
    pub late_bars: bool,
    pub links: bool,
    pub table_hidden: bool,
    pub float_kind: FloatKind,
    pub critical_by: CriticalBy,
    pub data_date: Option<Date>,
    pub progress_mode: ProgressMode,
    /// The time scale fits the whole project.
    pub fitted: bool,
    pub view: View,
    pub undo: Option<&'a str>,
    pub redo: Option<&'a str>,
    /// An activity is selected (Delete needs one).
    pub selected: bool,
    pub toolbar_hidden: bool,
}

const HEADER_HEIGHT: f32 = 60.0;
const TOOLBAR_HEIGHT: f32 = 36.0;
/// Corners of the hover tints on the menus and the toolbar's buttons.
const ROUND: f32 = 4.0;

pub(crate) const TOGGLE_TOOLBAR: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::F1);

/// A small square icon button with an accessible name and a tip; clicks count only when enabled.
/// `on` marks a toggle that is on.
fn tool(ui: &mut egui::Ui, icon: Icon, name: &str, tip: &str, enabled: bool, on: bool) -> bool {
    tool_response(ui, icon, name, tip, enabled, on).clicked() && enabled
}

fn tool_response(ui: &mut egui::Ui, icon: Icon, name: &str, tip: &str, enabled: bool, on: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(egui::vec2(30.0, 28.0), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, on, name));
    if on {
        ui.painter().rect_filled(rect, ROUND, t.accent_soft);
    } else if enabled && response.hovered() {
        ui.painter().rect_filled(rect, ROUND, if response.is_pointer_button_down_on() { t.pressed } else { t.hover });
    }
    let mut painter = ui.painter().clone();
    if !enabled {
        painter.set_opacity(0.35);
    }
    icons::paint(&painter, egui::Rect::from_center_size(rect.center(), egui::vec2(16.0, 16.0)), icon, &t);
    response.on_hover_text(tip)
}

/// A hairline between the toolbar's groups.
fn divider(ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(5.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(1.0, 20.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, t.header_line);
    ui.add_space(5.0);
}

/// A filled accent button with an icon and text, for a pane's one main action.
pub(crate) fn accent_button(ui: &mut egui::Ui, icon: Icon, text: &str) -> egui::Response {
    accent(ui, icon, text, 26.0, 0.0)
}

/// The accent button at `height` with corners of `round`.
fn accent(ui: &mut egui::Ui, icon: Icon, text: &str, height: f32, round: f32) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let ink = crate::theme::on_accent(&t);
    let galley = ui.painter().layout_no_wrap(text.to_string(), egui::FontId::proportional(13.0), ink);
    let pad = (height / 2.0).max(10.0);
    let size = egui::vec2(galley.size().x + 2.0 * pad + 16.0, height);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, text));
    let fill = if response.hovered() { t.accent.gamma_multiply(0.85) } else { t.accent };
    ui.painter().rect_filled(rect, round, fill);
    let mut on = t;
    on.accent = ink;
    let icon_center = egui::pos2(rect.left() + pad + 5.0, rect.center().y);
    icons::paint(ui.painter(), egui::Rect::from_center_size(icon_center, egui::vec2(12.0, 12.0)), icon, &on);
    ui.painter().galley(egui::pos2(rect.left() + pad + 16.0, rect.center().y - galley.size().y / 2.0), galley, ink);
    response
}

/// Hover tints without an edge, rounded: the look of the header's and the toolbar's controls.
fn quiet(ui: &mut egui::Ui) {
    let widgets = &mut ui.visuals_mut().widgets;
    for w in [&mut widgets.inactive, &mut widgets.hovered, &mut widgets.active, &mut widgets.open] {
        w.bg_stroke = egui::Stroke::NONE;
        w.corner_radius = egui::CornerRadius::same(ROUND as u8);
    }
    widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
    widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
}

/// The header: the mark, the name and its saved state over the menus, and on the right search,
/// theme and Schedule.
pub(crate) fn header(ui: &mut egui::Ui, state: &State) -> Option<Action> {
    let t = Tokens::get(ui.ctx());
    let mut action = None;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), HEADER_HEIGHT), egui::Sense::hover());
    let mut row =
        ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(14.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    quiet(&mut row);
    icons::show(&mut row, Icon::Mark, 30.0).on_hover_text("MK Perth");
    row.add_space(6.0);
    row.vertical(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 1.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            // In line with the menus' text, which sits inside their padding.
            ui.add_space(7.0);
            ui.label(egui::RichText::new(state.name).size(17.0).color(t.text));
            let status = if state.modified {
                Some(tr!("doc-unsaved"))
            } else if state.has_file {
                Some(if state.autosave { tr!("doc-autosaved") } else { tr!("doc-saved") })
            } else {
                None
            };
            if let Some(status) = status {
                ui.label(egui::RichText::new(status).size(12.0).color(t.text_dim));
            }
        });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.spacing_mut().button_padding = egui::vec2(7.0, 3.0);
            if let Some(a) = menus(ui, state) {
                action = Some(a);
            }
        });
    });
    row.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if accent(ui, Icon::Run, &tr!("schedule"), 32.0, 16.0).on_hover_text(tr!("schedule-tip")).clicked() {
            action = Some(Action::Schedule);
        }
        ui.add_space(6.0);
        let theme = tool_response(ui, Icon::Contrast, &tr!("theme"), &tr!("theme-tip"), true, false);
        if theme.clicked() {
            action = Some(Action::Theme(if t.dark { egui::ThemePreference::Light } else { egui::ThemePreference::Dark }));
        }
        theme.context_menu(|ui| {
            if let Some(p) = themes(ui) {
                action = Some(Action::Theme(p));
            }
        });
        if tool(ui, Icon::Search, &tr!("search"), &tr!("search-tip"), true, false) {
            action = Some(Action::Search);
        }
        // The hidden toolbar comes back from here, where its arrow was.
        if state.toolbar_hidden {
            let tip = format!("{} ({})", tr!("toolbar-show"), ui.ctx().format_shortcut(&TOGGLE_TOOLBAR));
            if tool(ui, Icon::ChevronDown, &tr!("toolbar-show"), &tip, true, false) {
                action = Some(Action::ToggleToolbar);
            }
        }
    });
    action
}

/// One menu's name in the header, its items in a popup under it.
fn menu(ui: &mut egui::Ui, name: &str, add: impl FnOnce(&mut egui::Ui)) {
    let t = Tokens::get(ui.ctx());
    let response = ui.add(egui::Button::new(egui::RichText::new(name).size(13.5).color(t.text)).frame_when_inactive(false));
    egui::Popup::menu(&response).show(|ui| {
        ui.set_min_width(230.0);
        add(ui);
    });
}

/// A menu item with its shortcut on the right.
fn item(ui: &mut egui::Ui, text: &str, shortcut: Option<KeyboardShortcut>, enabled: bool) -> bool {
    let mut button = egui::Button::new(text);
    if let Some(s) = shortcut {
        button = button.shortcut_text(ui.ctx().format_shortcut(&s));
    }
    ui.add_enabled(enabled, button).clicked()
}

/// A menu item that is on or off.
fn check(ui: &mut egui::Ui, on: bool, text: &str) -> bool {
    let mut value = on;
    ui.checkbox(&mut value, text).clicked()
}

fn menus(ui: &mut egui::Ui, state: &State) -> Option<Action> {
    let mut action = None;
    let mut set = |a: Action| action = Some(a);
    menu(ui, &tr!("file"), |ui| {
        if item(ui, &tr!("open-menu"), Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::O)), true) {
            set(Action::Open);
        }
        if item(ui, &tr!("save-menu"), Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::S)), true) {
            set(Action::Save);
        }
        if item(ui, &tr!("save-as-menu"), Some(KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::S)), true) {
            set(Action::SaveAs);
        }
        ui.separator();
        if item(ui, &tr!("sample-menu"), None, true) {
            set(Action::Sample);
        }
        ui.separator();
        let tip = if state.autosave { tr!("autosave-on-tip") } else { tr!("autosave-off-tip") };
        let mut on = state.autosave;
        if ui.checkbox(&mut on, tr!("autosave")).on_hover_text(tip).clicked() {
            set(Action::ToggleAutoSave);
        }
    });
    menu(ui, &tr!("menu-edit"), |ui| {
        let undo = state.undo.map_or_else(|| tr!("undo"), |l| tr!("undo-named", what = l));
        if item(ui, &undo, Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::Z)), state.undo.is_some()) {
            set(Action::Undo);
        }
        let redo = state.redo.map_or_else(|| tr!("redo"), |l| tr!("redo-named", what = l));
        if item(ui, &redo, Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::Y)), state.redo.is_some()) {
            set(Action::Redo);
        }
        ui.separator();
        if item(ui, &tr!("add-activity"), Some(KeyboardShortcut::new(Modifiers::NONE, Key::Insert)), true) {
            set(Action::AddActivity);
        }
        if item(ui, &tr!("delete-activity-menu"), Some(KeyboardShortcut::new(Modifiers::NONE, Key::Delete)), state.selected) {
            set(Action::DeleteActivity);
        }
    });
    menu(ui, &tr!("menu-project"), |ui| {
        if item(ui, &tr!("schedule"), Some(KeyboardShortcut::new(Modifiers::NONE, Key::F9)), true) {
            set(Action::Schedule);
        }
        ui.separator();
        if let Some(p) = scheduling_options(ui, state) {
            set(Action::Options(p));
        }
    });
    menu(ui, &tr!("menu-view"), |ui| {
        if ui.radio(state.view == View::Activities, tr!("activities-view")).on_hover_text(tr!("activities-view-tip")).clicked() {
            set(Action::View(View::Activities));
        }
        if check(ui, !state.table_hidden, &tr!("activity-table")) {
            set(Action::Table);
        }
        ui.separator();
        if check(ui, state.critical_only, &tr!("critical-only")) {
            set(Action::CriticalOnly);
        }
        if check(ui, state.late_bars, &tr!("late-bars")) {
            set(Action::LateBars);
        }
        if check(ui, state.links, &tr!("links")) {
            set(Action::Links);
        }
        ui.separator();
        if item(ui, &tr!("time-wider"), None, true) {
            set(Action::TimeScale(1));
        }
        if item(ui, &tr!("time-narrower"), None, true) {
            set(Action::TimeScale(-1));
        }
        if item(ui, &tr!("time-fit"), None, !state.fitted) {
            set(Action::TimeScale(0));
        }
        ui.separator();
        ui.menu_button(tr!("theme"), |ui| {
            if let Some(p) = themes(ui) {
                set(Action::Theme(p));
            }
        });
        ui.menu_button(tr!("language"), |ui| {
            let current = i18n::current();
            for lang in Lang::ALL {
                if ui.radio(lang == current, lang.name()).clicked() && lang != current {
                    set(Action::Language(lang));
                }
            }
        });
        ui.separator();
        let toggle = if state.toolbar_hidden { tr!("toolbar-show") } else { tr!("toolbar-hide") };
        if item(ui, &toggle, Some(TOGGLE_TOOLBAR), true) {
            set(Action::ToggleToolbar);
        }
    });
    action
}

/// Light, dark or the system's.
fn themes(ui: &mut egui::Ui) -> Option<egui::ThemePreference> {
    let current = ui.ctx().options(|o| o.theme_preference);
    let mut chosen = None;
    for (preference, name) in [
        (egui::ThemePreference::Light, tr!("theme-light")),
        (egui::ThemePreference::Dark, tr!("theme-dark")),
        (egui::ThemePreference::System, tr!("theme-system")),
    ] {
        if ui.radio(preference == current, name).clicked() {
            chosen = Some(preference);
        }
    }
    chosen
}

/// How total float is measured, what is critical and how progress out of sequence is scheduled:
/// the Project menu's submenus.
fn scheduling_options(ui: &mut egui::Ui, state: &State) -> Option<serde_json::Value> {
    let mut chosen = None;
    ui.menu_button(tr!("option-float"), |ui| {
        for (k, code, name) in [
            (FloatKind::Finish, "finish", tr!("float-finish")),
            (FloatKind::Start, "start", tr!("float-start")),
            (FloatKind::Smallest, "smallest", tr!("float-smallest")),
        ] {
            if ui.radio(k == state.float_kind, name).clicked() && k != state.float_kind {
                chosen = Some(serde_json::json!({ "float": code }));
            }
        }
    });
    ui.menu_button(tr!("option-critical"), |ui| {
        let longest = state.critical_by == CriticalBy::LongestPath;
        if ui.radio(!longest, tr!("critical-float")).clicked() && longest {
            chosen = Some(serde_json::json!({ "critical": "float", "threshold": 0 }));
        }
        if let CriticalBy::TotalFloat(threshold) = state.critical_by {
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                // A field framed as one, inside the menus' quiet style.
                let t = Tokens::get(ui.ctx());
                let inactive = &mut ui.visuals_mut().widgets.inactive;
                inactive.bg_fill = t.input_bg;
                inactive.bg_stroke = egui::Stroke::new(1.0, t.border);
                let mut value = threshold;
                let response = ui.add(egui::DragValue::new(&mut value).range(-999..=999).prefix("≤ ")).on_hover_text(tr!("critical-threshold-tip"));
                if response.changed() && value != threshold {
                    chosen = Some(serde_json::json!({ "critical": "float", "threshold": value }));
                }
            });
        }
        if ui.radio(longest, tr!("critical-longest-path")).clicked() && !longest {
            chosen = Some(serde_json::json!({ "critical": "longest-path" }));
        }
    });
    ui.menu_button(tr!("option-progress"), |ui| {
        for (m, code, name) in [
            (ProgressMode::RetainedLogic, "retained-logic", tr!("progress-retained-logic")),
            (ProgressMode::ProgressOverride, "progress-override", tr!("progress-override")),
        ] {
            let tip = tr!(&format!("progress-{code}-tip"));
            if ui.radio(m == state.progress_mode, name).on_hover_text(tip).clicked() && m != state.progress_mode {
                chosen = Some(serde_json::json!({ "progress": code }));
            }
        }
    });
    chosen
}

/// The toolbar: undo and redo, the activities, what the chart shows, the time scale and the data
/// date, as small icons in one row on the header's surface, with no band or edge of its own.
/// Nothing when hidden.
pub(crate) fn toolbar(ui: &mut egui::Ui, state: &State) -> Option<Action> {
    if state.toolbar_hidden {
        return None;
    }
    let t = Tokens::get(ui.ctx());
    let mut action = None;
    egui::Frame::new().inner_margin(egui::Margin { left: 10, right: 10, top: 0, bottom: 8 }).show(ui, |ui| {
        let (band, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), TOOLBAR_HEIGHT), egui::Sense::hover());
        let mut row = ui
            .new_child(egui::UiBuilder::new().max_rect(band.shrink2(egui::vec2(12.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
        quiet(&mut row);
        row.spacing_mut().item_spacing.x = 1.0;
        let mut go = |ui: &mut egui::Ui, icon: Icon, name: &str, tip: &str, enabled: bool, on: bool, then: Action| {
            if tool(ui, icon, name, tip, enabled, on) {
                action = Some(then);
            }
        };
        let undo = state.undo.map_or_else(|| tr!("nothing-to-undo"), |l| tr!("undo-named", what = l));
        go(&mut row, Icon::Undo, &tr!("undo"), &undo, state.undo.is_some(), false, Action::Undo);
        let redo = state.redo.map_or_else(|| tr!("nothing-to-redo"), |l| tr!("redo-named", what = l));
        go(&mut row, Icon::Redo, &tr!("redo"), &redo, state.redo.is_some(), false, Action::Redo);
        divider(&mut row);
        go(&mut row, Icon::InsertCase, &tr!("add-activity"), &tr!("add-activity-tip"), true, false, Action::AddActivity);
        go(&mut row, Icon::Close, &tr!("delete-activity"), &tr!("delete-activity-tip"), state.selected, false, Action::DeleteActivity);
        divider(&mut row);
        go(&mut row, Icon::Critical, &tr!("critical-only"), &tr!("critical-only-tip"), true, state.critical_only, Action::CriticalOnly);
        go(&mut row, Icon::Gantt, &tr!("late-bars"), &tr!("late-bars-tip"), true, state.late_bars, Action::LateBars);
        go(&mut row, Icon::Network, &tr!("links"), &tr!("links-tip"), true, state.links, Action::Links);
        divider(&mut row);
        go(&mut row, Icon::Minus, &tr!("time-narrower"), &tr!("time-narrower-tip"), true, false, Action::TimeScale(-1));
        go(&mut row, Icon::Grid, &tr!("time-fit"), &tr!("time-fit-tip"), true, state.fitted, Action::TimeScale(0));
        go(&mut row, Icon::Plus, &tr!("time-wider"), &tr!("time-wider-tip"), true, false, Action::TimeScale(1));
        divider(&mut row);
        row.add_space(4.0);
        let label = row.label(egui::RichText::new(tr!("option-data-date")).size(12.5).color(t.text_dim));
        row.add_space(6.0);
        if let Some(d) = crate::date_field(&mut row, egui::Id::unique("data-date"), label.id, state.data_date, 96.0) {
            action = Some(Action::Options(serde_json::json!({ "data_date": d.map(|d| d.to_string()) })));
        }
        row.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let tip = format!("{} ({})", tr!("toolbar-hide"), ui.ctx().format_shortcut(&TOGGLE_TOOLBAR));
            if tool(ui, Icon::ChevronUp, &tr!("toolbar-hide"), &tip, true, false) {
                action = Some(Action::ToggleToolbar);
            }
        });
    });
    action
}
