//! The top of the window, quiet as Pearl's: a slim title bar (AutoSave, undo and redo on the
//! left, the mark and the project's name in the middle, Schedule, presenting, theme and search on
//! the right), a row of menus (File, Edit, Project, View) and one row of small single-colour icons
//! in groups parted by hairlines, all on the window's surface. Buttons only report what was asked
//! for; the app acts through the session.

use crate::View;
use crate::icons::{self, Icon};
use crate::theme::Tokens;
use egui::emath::GuiRounding as _;
use egui::{Key, KeyboardShortcut, Modifiers};
use mkperth_engine::core::{CriticalBy, Date, FloatKind, ProgressMode};
use mkperth_engine::i18n::{self, Lang, tr};

/// What was asked for in the header, a menu or the toolbar this frame.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Action {
    /// The File backstage.
    Backstage,
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
    ToggleToolbar,
    Search,
    Calendars,
    /// The chosen activities tied one after another, or untied.
    Link,
    Unlink,
    /// The selected activity under the summary above it, or out of its own.
    Indent,
    Outdent,
    /// The chart scrolled to today, or to the selected activity.
    Today,
    Locate,
    /// Levelled now, with the project's options.
    Level,
    /// The Levelling pane.
    Levelling,
    /// The PERT pane.
    Pert,
    /// The Baselines pane.
    Baselines,
    /// The Costs pane.
    Costs,
    /// The Budget pane.
    Budget,
    /// The Earned value pane.
    EarnedValue,
    /// The Codes and fields pane.
    Codes,
    /// The Organisation pane.
    Organisation,
    /// The baseline's bars under the schedule's.
    BaselineBars,
    /// Every other row of the table tinted.
    Stripes,
    /// The selected activity's comments.
    Comment,
    /// The schedule alone, full screen.
    Present,
    /// The interface's typeface, size and style.
    Font(crate::fonts::FontChoice),
    Language(Lang),
    /// Every change saved to the project's file as it happens, or not.
    ToggleAutoSave,
}

/// What the header, the menus and the toolbar show.
pub(crate) struct State<'a> {
    /// The project's name.
    pub name: &'a str,
    pub modified: bool,
    /// The project has a file to be saved to.
    pub has_file: bool,
    pub autosave: bool,
    /// The File backstage is open.
    pub backstage: bool,
    pub font: &'a crate::fonts::FontChoice,
    /// The typeface families the system offers.
    pub font_families: &'a [String],
    pub critical_only: bool,
    pub late_bars: bool,
    pub baseline_bars: bool,
    pub striped: bool,
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
    /// How many activities are chosen (linking needs two).
    pub chosen: usize,
    /// The selected activity is under a summary.
    pub nested: bool,
    /// Today is known, to go to.
    pub today: bool,
    pub toolbar_hidden: bool,
}

/// The title bar, the menus' row and the toolbar's: together with the hairline under them, the
/// chrome is 110 points high.
const TITLE_HEIGHT: f32 = 36.0;
const MENU_HEIGHT: f32 = 30.0;
const TOOLBAR_HEIGHT: f32 = 43.0;
/// Corners of the hover tints on the menus and the toolbar's buttons.
const ROUND: f32 = crate::theme::RADIUS as f32;

/// Presenting: the schedule alone on the whole screen, as slides start.
pub(crate) const PRESENT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F5);
/// A comment on the selected activity, as spreadsheets have it.
pub(crate) const COMMENT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::ALT), Key::M);
pub(crate) const TOGGLE_TOOLBAR: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::F1);

/// A small square icon button with an accessible name and a tip; clicks count only when enabled.
/// `on` marks a toggle that is on.
fn tool(ui: &mut egui::Ui, icon: Icon, name: &str, tip: &str, enabled: bool, on: bool) -> bool {
    tool_response(ui, icon, name, tip, enabled, on, false).clicked() && enabled
}

/// A [`tool`] drawn in one quiet colour, as the title bar's are.
fn quiet_tool(ui: &mut egui::Ui, icon: Icon, name: &str, tip: &str, enabled: bool) -> bool {
    tool_response(ui, icon, name, tip, enabled, false, true).clicked() && enabled
}

fn tool_response(ui: &mut egui::Ui, icon: Icon, name: &str, tip: &str, enabled: bool, on: bool, mono: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(egui::vec2(28.0, 26.0), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, on, name));
    if on {
        ui.painter().rect_filled(rect, ROUND, t.accent_soft);
    } else if enabled && response.hovered() {
        ui.painter().rect_filled(rect, ROUND, if response.is_pointer_button_down_on() { t.pressed } else { t.hover });
    }
    let mut painter = ui.painter().clone();
    if !enabled {
        painter.set_opacity(0.4);
    }
    // In the text's colour, the action part in its family's ink, as the house's toolbars draw
    // them; snapped to whole pixels, so their one-pixel lines stay sharp.
    let square = egui::Rect::from_center_size(rect.center(), egui::vec2(16.0, 16.0)).round_to_pixels(ui.pixels_per_point());
    if mono {
        icons::paint_mono(&painter, square, icon, t.text_dim);
    } else {
        icons::paint_in(&painter, square, icon, &t, t.text);
    }
    response.on_hover_text(tip)
}

/// The zooms offered, in percent.
pub(crate) const ZOOMS: [u32; 9] = [50, 75, 90, 100, 110, 125, 150, 175, 200];

/// A hairline between the toolbar's groups.
fn divider(ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(6.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(1.0, 18.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, t.header_line);
    ui.add_space(6.0);
}

/// A filled accent button with an icon and text, for a pane's one main action.
pub(crate) fn accent_button(ui: &mut egui::Ui, icon: Icon, text: &str) -> egui::Response {
    accent(ui, icon, text, 26.0, ROUND)
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
    let icon_center = egui::pos2(rect.left() + pad + 5.0, rect.center().y);
    icons::paint_mono(ui.painter(), egui::Rect::from_center_size(icon_center, egui::vec2(12.0, 12.0)), icon, ink);
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

/// An on/off switch: its name, then a pill with a knob; a click on either turns it.
fn switch(ui: &mut egui::Ui, on: bool, name: &str) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let galley = ui.painter().layout_no_wrap(name.to_string(), egui::FontId::proportional(12.5), t.text_dim);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(galley.size().x + 8.0 + 30.0, 18.0), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, on, name));
    let painter = ui.painter();
    painter.galley(egui::pos2(rect.left(), rect.center().y - galley.size().y / 2.0), galley, t.text_dim);
    let r = egui::Rect::from_min_size(egui::pos2(rect.right() - 30.0, rect.center().y - 8.0), egui::vec2(30.0, 16.0));
    // A switch reads as a switch by its pill and round knob.
    painter.rect_filled(r, 8.0, if on { t.accent } else { egui::Color32::TRANSPARENT });
    painter.rect_stroke(r, 8.0, egui::Stroke::new(1.0, if on { t.accent } else { t.text_dim }), egui::StrokeKind::Inside);
    let knob = if on { egui::pos2(r.right() - 8.0, r.center().y) } else { egui::pos2(r.left() + 8.0, r.center().y) };
    painter.circle_filled(knob, 5.0, if on { egui::Color32::WHITE } else { t.text_dim });
    response
}

/// The title bar: AutoSave, undo and redo on the left; the mark and the project's name in the
/// middle, its saved state after it in a quieter grey; Schedule, presenting, theme and search on
/// the right. Under it, the menus' row. It is the window's title bar too: drag it to move the
/// window, double-click it to maximize or restore.
pub(crate) fn header(ui: &mut egui::Ui, state: &State) -> Option<Action> {
    let t = Tokens::get(ui.ctx());
    let mut action = None;
    let (rect, background) = ui.allocate_exact_size(egui::vec2(ui.available_width(), TITLE_HEIGHT), egui::Sense::click_and_drag());
    if background.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if background.double_clicked() {
        let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    }
    let mut row =
        ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(10.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    quiet(&mut row);
    row.spacing_mut().item_spacing.x = 2.0;
    let tip = if state.autosave { tr!("autosave-on-tip") } else { tr!("autosave-off-tip") };
    if switch(&mut row, state.autosave, &tr!("autosave")).on_hover_text(tip).clicked() {
        action = Some(Action::ToggleAutoSave);
    }
    row.add_space(8.0);
    let undo = state.undo.map_or_else(|| tr!("nothing-to-undo"), |l| tr!("undo-named", what = l));
    if quiet_tool(&mut row, Icon::Undo, &tr!("undo"), &undo, state.undo.is_some()) {
        action = Some(Action::Undo);
    }
    let redo = state.redo.map_or_else(|| tr!("nothing-to-redo"), |l| tr!("redo-named", what = l));
    if quiet_tool(&mut row, Icon::Redo, &tr!("redo"), &redo, state.redo.is_some()) {
        action = Some(Action::Redo);
    }
    let left_end = row.min_rect().right();
    let right_start = row
        .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if quiet_tool(ui, Icon::Search, &tr!("search"), &tr!("search-tip"), true) {
                action = Some(Action::Search);
            }
            let theme = tool_response(ui, Icon::Contrast, &tr!("theme"), &tr!("theme-tip"), true, false, true);
            if theme.clicked() {
                action = Some(Action::Theme(if t.dark { egui::ThemePreference::Light } else { egui::ThemePreference::Dark }));
            }
            theme.context_menu(|ui| {
                if let Some(p) = themes(ui) {
                    action = Some(Action::Theme(p));
                }
            });
            let tip = format!("{} ({})", tr!("present-tip"), ui.ctx().format_shortcut(&PRESENT));
            if quiet_tool(ui, Icon::Present, &tr!("present"), &tip, true) {
                action = Some(Action::Present);
            }
            let tip = format!("{} (F9)", tr!("schedule-tip"));
            if quiet_tool(ui, Icon::Run, &tr!("schedule"), &tip, true) {
                action = Some(Action::Schedule);
            }
            // The hidden toolbar comes back from here.
            if state.toolbar_hidden {
                let tip = format!("{} ({})", tr!("toolbar-show"), ui.ctx().format_shortcut(&TOGGLE_TOOLBAR));
                if quiet_tool(ui, Icon::ChevronDown, &tr!("toolbar-show"), &tip, true) {
                    action = Some(Action::ToggleToolbar);
                }
            }
            ui.min_rect().left()
        })
        .inner;
    // The name in the middle, cut with "…" so it never runs under the controls on either side;
    // the mark goes with it where there is room, the saved state after it.
    let status = if state.modified {
        Some(tr!("doc-unsaved"))
    } else if state.has_file {
        Some(if state.autosave { tr!("doc-autosaved") } else { tr!("doc-saved") })
    } else {
        None
    };
    const MARK: f32 = 18.0;
    let painter = ui.painter();
    let status = status.map(|s| painter.layout_no_wrap(s, egui::FontId::proportional(12.0), t.text_dim));
    let status_width = status.as_ref().map_or(0.0, |g| g.size().x + 10.0);
    let half = (rect.center().x - left_end).min(right_start - rect.center().x) - 16.0;
    let mut job = egui::text::LayoutJob::simple_singleline(state.name.to_string(), egui::FontId::proportional(13.5), t.text);
    job.wrap = egui::text::TextWrapping::truncate_at_width((2.0 * half - MARK - 8.0 - status_width).max(0.0));
    let name = painter.layout_job(job);
    let whole = MARK + 8.0 + name.size().x + status_width;
    let left = rect.center().x - whole / 2.0;
    if half > MARK {
        icons::paint(painter, egui::Rect::from_min_size(egui::pos2(left, rect.center().y - MARK / 2.0), egui::vec2(MARK, MARK)), Icon::Mark, &t);
    }
    let name_pos = egui::pos2(left + MARK + 8.0, rect.center().y - name.size().y / 2.0);
    let name_right = name_pos.x + name.size().x;
    // Strong by drawing twice, half a point apart: the built-in fonts have no bold face.
    painter.galley(name_pos, name.clone(), t.text);
    painter.galley(name_pos + egui::vec2(0.4, 0.0), name, t.text);
    if let Some(status) = status {
        painter.galley(egui::pos2(name_right + 10.0, rect.center().y - status.size().y / 2.0), status, t.text_dim);
    }
    // The menus' row.
    let (menu_rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), MENU_HEIGHT), egui::Sense::hover());
    let mut menus_row = ui
        .new_child(egui::UiBuilder::new().max_rect(menu_rect.shrink2(egui::vec2(8.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    quiet(&mut menus_row);
    menus_row.spacing_mut().item_spacing.x = 0.0;
    if let Some(a) = menus(&mut menus_row, state) {
        action = Some(a);
    }
    action
}

/// A name in the menus' row: its text, a soft tint under the pointer or while its menu is open.
fn top_label(ui: &mut egui::Ui, name: &str, color: egui::Color32, open: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let galley = ui.painter().layout_no_wrap(name.to_string(), egui::FontId::proportional(13.0), color);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(galley.size().x + 18.0, 24.0), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    if open || response.hovered() {
        ui.painter().rect_filled(rect, ROUND, if response.is_pointer_button_down_on() || open { t.pressed } else { t.hover });
    }
    let color = if response.hovered() && color == t.text_dim { t.text } else { color };
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    response
}

/// One menu's name in the menus' row, its items in a popup under it.
fn menu(ui: &mut egui::Ui, name: &str, add: impl FnOnce(&mut egui::Ui)) {
    let t = Tokens::get(ui.ctx());
    let response = top_label(ui, name, t.text_dim, false);
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
    // File opens the backstage, as office suites do; its items live there. It is in the accent,
    // as Pearl's.
    let t = Tokens::get(ui.ctx());
    if top_label(ui, &tr!("file"), t.accent, state.backstage).clicked() {
        set(Action::Backstage);
    }
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
        ui.separator();
        if item(ui, &tr!("comment-menu"), Some(COMMENT), state.selected) {
            set(Action::Comment);
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
        ui.separator();
        if item(ui, &tr!("calendars-menu"), None, true) {
            set(Action::Calendars);
        }
        if item(ui, &tr!("level-menu"), None, true) {
            set(Action::Levelling);
        }
        if item(ui, &tr!("pert-menu"), None, true) {
            set(Action::Pert);
        }
        if item(ui, &tr!("baselines-menu"), None, true) {
            set(Action::Baselines);
        }
        if item(ui, &tr!("costs-menu"), None, true) {
            set(Action::Costs);
        }
        if item(ui, &tr!("budget-menu"), None, true) {
            set(Action::Budget);
        }
        if item(ui, &tr!("earned-menu"), None, true) {
            set(Action::EarnedValue);
        }
        if item(ui, &tr!("codes-menu"), None, true) {
            set(Action::Codes);
        }
        if item(ui, &tr!("organisation-menu"), None, true) {
            set(Action::Organisation);
        }
    });
    menu(ui, &tr!("menu-view"), |ui| {
        if ui.radio(state.view == View::Activities, tr!("activities-view")).on_hover_text(tr!("activities-view-tip")).clicked() {
            set(Action::View(View::Activities));
        }
        if ui.radio(state.view == View::Resources, tr!("resources-view")).on_hover_text(tr!("resources-view-tip")).clicked() {
            set(Action::View(View::Resources));
        }
        if ui.radio(state.view == View::Network, tr!("network-view")).on_hover_text(tr!("network-view-tip")).clicked() {
            set(Action::View(View::Network));
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
        if check(ui, state.baseline_bars, &tr!("baseline-bars")) {
            set(Action::BaselineBars);
        }
        if check(ui, state.striped, &tr!("stripes")) {
            set(Action::Stripes);
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
        ui.menu_button(tr!("font"), |ui| {
            if let Some(f) = font_menu(ui, state) {
                set(Action::Font(f));
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
        if item(ui, &tr!("present"), Some(PRESENT), true) {
            set(Action::Present);
        }
        let toggle = if state.toolbar_hidden { tr!("toolbar-show") } else { tr!("toolbar-hide") };
        if item(ui, &toggle, Some(TOGGLE_TOOLBAR), true) {
            set(Action::ToggleToolbar);
        }
    });
    action
}

/// The Font submenu: the family (egui's own, or one the system has), the size, bold and italic
/// (for a family of the system's).
fn font_menu(ui: &mut egui::Ui, state: &State) -> Option<crate::fonts::FontChoice> {
    let mut chosen = font_families(ui, state);
    let now = state.font;
    ui.separator();
    chosen = chosen.or(font_sizes(ui, state));
    let mut with = |f: crate::fonts::FontChoice| chosen = Some(f);
    ui.separator();
    let own = now.family.is_some();
    let mut bold = now.bold;
    if ui.add_enabled(own, egui::Checkbox::new(&mut bold, tr!("font-bold"))).clicked() {
        with(crate::fonts::FontChoice { bold, ..now.clone() });
    }
    let mut italic = now.italic;
    if ui.add_enabled(own, egui::Checkbox::new(&mut italic, tr!("font-italic"))).clicked() {
        with(crate::fonts::FontChoice { italic, ..now.clone() });
    }
    chosen
}

/// egui's own face and the system's families, one to choose.
fn font_families(ui: &mut egui::Ui, state: &State) -> Option<crate::fonts::FontChoice> {
    let mut chosen = None;
    let now = state.font;
    if ui.radio(now.family.is_none(), tr!("font-default")).clicked() && now.family.is_some() {
        chosen = Some(crate::fonts::FontChoice { family: None, bold: false, italic: false, ..now.clone() });
    }
    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
        for f in state.font_families {
            let on = now.family.as_deref() == Some(f.as_str());
            if ui.radio(on, egui::RichText::new(f)).clicked() && !on {
                chosen = Some(crate::fonts::FontChoice { family: Some(f.clone()), ..now.clone() });
            }
        }
    });
    if state.font_families.is_empty() {
        ui.label(egui::RichText::new(tr!("font-none")).size(11.5).weak());
    }
    chosen
}

/// The sizes, one to choose.
fn font_sizes(ui: &mut egui::Ui, state: &State) -> Option<crate::fonts::FontChoice> {
    let mut chosen = None;
    let now = state.font;
    for size in crate::fonts::SIZES {
        if ui.radio(now.size == size, size.to_string()).clicked() && now.size != size {
            chosen = Some(crate::fonts::FontChoice { size, ..now.clone() });
        }
    }
    chosen
}

/// A text field in the toolbar, as tall as its drop-downs: the input's colour in a hairline
/// frame with soft corners.
fn field<R>(ui: &mut egui::Ui, t: &Tokens, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    // A place of the drop-downs' height: in a row centred on its height, a frame would stretch to it.
    let place = egui::vec2(ui.available_width(), 22.0);
    ui.allocate_ui_with_layout(place, egui::Layout::left_to_right(egui::Align::Center), |ui| field_frame(ui, t, add)).inner
}

fn field_frame<R>(ui: &mut egui::Ui, t: &Tokens, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(t.input_bg)
        .stroke(egui::Stroke::new(1.0, t.header_line))
        .corner_radius(ROUND)
        .inner_margin(egui::Margin::symmetric(4, 1))
        .show(ui, |ui| {
            ui.spacing_mut().interact_size.y = 20.0;
            let visuals = &mut ui.visuals_mut().widgets;
            for w in [&mut visuals.inactive, &mut visuals.hovered, &mut visuals.active] {
                w.bg_stroke = egui::Stroke::NONE;
                w.bg_fill = egui::Color32::TRANSPARENT;
                w.weak_bg_fill = egui::Color32::TRANSPARENT;
            }
            ui.visuals_mut().extreme_bg_color = egui::Color32::TRANSPARENT;
            ui.spacing_mut().button_padding = egui::vec2(0.0, 0.0);
            add(ui)
        })
        .inner
}

/// A field with a drop-down in the toolbar, as Pearl's: its value on the input's colour in a
/// hairline frame, a small chevron; a click opens `contents` under it.
pub(crate) fn drop_down<R>(
    ui: &mut egui::Ui,
    text: &str,
    name: &str,
    tip: &str,
    width: f32,
    contents: impl FnOnce(&mut egui::Ui) -> Option<R>,
) -> Option<R> {
    let t = Tokens::get(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 22.0), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, name));
    let fill = if response.is_pointer_button_down_on() {
        t.pressed
    } else if response.hovered() {
        t.hover
    } else {
        t.input_bg
    };
    ui.painter().rect(rect, ROUND, fill, egui::Stroke::new(1.0, t.header_line), egui::StrokeKind::Inside);
    let painter = ui.painter().with_clip_rect(rect.shrink2(egui::vec2(4.0, 0.0)).intersect(ui.clip_rect()));
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), egui::FontId::proportional(12.5), t.text);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width - 28.0);
    let galley = painter.layout_job(job);
    painter.galley(egui::pos2(rect.left() + 7.0, rect.center().y - galley.size().y / 2.0), galley, t.text);
    let chevron = egui::Rect::from_center_size(egui::pos2(rect.right() - 10.0, rect.center().y), egui::vec2(9.0, 9.0));
    icons::paint_mono(ui.painter(), chevron, Icon::ChevronDown, t.text_dim);
    let response = response.on_hover_text(tip);
    egui::Popup::menu(&response).show(contents).and_then(|r| r.inner)
}

/// The toolbar's font, as spreadsheets have it: the typeface as a drop-down, then the size
/// between a minus and a plus, a box that takes a size typed or opens the usual ones.
fn font_tools(ui: &mut egui::Ui, state: &State) -> Option<crate::fonts::FontChoice> {
    let t = Tokens::get(ui.ctx());
    let now = state.font;
    let shown = now.family.clone().unwrap_or_else(|| tr!("font-default-short"));
    let name = tr!("font-family-named", name = shown.as_str());
    let mut chosen = drop_down(ui, &shown, &name, &tr!("font-family-tip"), 120.0, |ui| font_families(ui, state));
    ui.add_space(6.0);
    let with = |size: u8| (size != now.size).then(|| crate::fonts::FontChoice { size: crate::fonts::clamp_size(size), ..now.clone() });
    if tool(ui, Icon::Minus, &tr!("font-smaller"), &tr!("font-smaller"), now.size > crate::fonts::MIN_SIZE, false) {
        chosen = with(now.size.saturating_sub(1));
    }
    // The size: typed and kept on Enter, or picked from the usual ones under it.
    let key = egui::Id::unique("toolbar-font-size");
    let mut text = ui.data(|d| d.get_temp::<(u8, String)>(key)).filter(|(was, _)| *was == now.size).map_or(now.size.to_string(), |(_, t)| t);
    let label = tr!("font-size");
    let response = field(ui, &t, |ui| {
        ui.add(egui::TextEdit::singleline(&mut text).desired_width(24.0).align(egui::Align2::CENTER_CENTER).frame(egui::Frame::NONE))
    });
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, &label));
    if response.lost_focus() {
        if let Ok(n) = text.trim().parse::<u8>() {
            chosen = with(n);
        }
        ui.data_mut(|d| d.remove::<(u8, String)>(key));
    } else if response.changed() {
        let typed = text.clone();
        ui.data_mut(|d| d.insert_temp(key, (now.size, typed)));
    }
    let picked = egui::Popup::menu(&response).width(48.0).show(|ui| {
        let mut picked = None;
        for size in crate::fonts::SIZES {
            if ui.selectable_label(size == now.size, size.to_string()).clicked() {
                picked = Some(size);
            }
        }
        picked
    });
    if let Some(size) = picked.and_then(|r| r.inner) {
        chosen = with(size);
    }
    if tool(ui, Icon::Plus, &tr!("font-larger"), &tr!("font-larger"), now.size < crate::fonts::MAX_SIZE, false) {
        chosen = with(now.size.saturating_add(1));
    }
    chosen
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

/// The toolbar: the font, the activities, what the chart shows, the time scale, levelling and
/// the data date, as small single-colour icons in one row on the window's surface, groups parted
/// by hairlines, with no band or edge of its own. Nothing when hidden.
pub(crate) fn toolbar(ui: &mut egui::Ui, state: &State) -> Option<Action> {
    if state.toolbar_hidden {
        return None;
    }
    let t = Tokens::get(ui.ctx());
    let mut action = None;
    let (band, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), TOOLBAR_HEIGHT), egui::Sense::hover());
    let inner = egui::Rect::from_min_max(egui::pos2(band.left() + 10.0, band.top()), egui::pos2(band.right() - 10.0, band.bottom() - 9.0));
    let mut row = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    quiet(&mut row);
    row.spacing_mut().item_spacing.x = 1.0;
    let mut go = |ui: &mut egui::Ui, icon: Icon, name: &str, tip: &str, enabled: bool, on: bool, then: Action| {
        if tool(ui, icon, name, tip, enabled, on) {
            action = Some(then);
        }
    };
    let font = font_tools(&mut row, state);
    divider(&mut row);
    go(&mut row, Icon::InsertCase, &tr!("add-activity"), &tr!("add-activity-tip"), true, false, Action::AddActivity);
    go(&mut row, Icon::Delete, &tr!("delete-activity"), &tr!("delete-activity-tip"), state.selected, false, Action::DeleteActivity);
    go(&mut row, Icon::Link, &tr!("link-activities"), &tr!("link-activities-tip"), state.chosen >= 2, false, Action::Link);
    go(&mut row, Icon::Unlink, &tr!("unlink-activities"), &tr!("unlink-activities-tip"), state.chosen >= 2, false, Action::Unlink);
    go(&mut row, Icon::Outdent, &tr!("outdent"), &tr!("outdent-tip"), state.nested, false, Action::Outdent);
    go(&mut row, Icon::Indent, &tr!("indent"), &tr!("indent-tip"), state.selected, false, Action::Indent);
    let tip = format!("{} ({})", tr!("comment-tool-tip"), ui.ctx().format_shortcut(&COMMENT));
    go(&mut row, Icon::Comment, &tr!("comment-tool"), &tip, state.selected, false, Action::Comment);
    divider(&mut row);
    go(&mut row, Icon::Critical, &tr!("critical-only"), &tr!("critical-only-tip"), true, state.critical_only, Action::CriticalOnly);
    go(&mut row, Icon::Gantt, &tr!("late-bars"), &tr!("late-bars-tip"), true, state.late_bars, Action::LateBars);
    go(&mut row, Icon::Network, &tr!("links"), &tr!("links-tip"), true, state.links, Action::Links);
    go(&mut row, Icon::Stripes, &tr!("stripes"), &tr!("stripes-tip"), true, state.striped, Action::Stripes);
    divider(&mut row);
    go(&mut row, Icon::Minus, &tr!("time-narrower"), &tr!("time-narrower-tip"), true, false, Action::TimeScale(-1));
    go(&mut row, Icon::Fit, &tr!("time-fit"), &tr!("time-fit-tip"), true, state.fitted, Action::TimeScale(0));
    go(&mut row, Icon::Plus, &tr!("time-wider"), &tr!("time-wider-tip"), true, false, Action::TimeScale(1));
    go(&mut row, Icon::Today, &tr!("go-today"), &tr!("go-today-tip"), state.today, false, Action::Today);
    go(&mut row, Icon::Locate, &tr!("go-selected"), &tr!("go-selected-tip"), state.selected, false, Action::Locate);
    divider(&mut row);
    go(&mut row, Icon::Level, &tr!("level-tool"), &tr!("level-tool-tip"), true, false, Action::Level);
    divider(&mut row);
    if let Some(f) = font {
        action = Some(Action::Font(f));
    }
    row.add_space(4.0);
    let label = row.label(egui::RichText::new(tr!("option-data-date")).size(12.5).color(t.text_dim));
    row.add_space(6.0);
    if let Some(d) = field(&mut row, &t, |ui| crate::date_field(ui, egui::Id::unique("data-date"), label.id, state.data_date, 96.0)) {
        action = Some(Action::Options(serde_json::json!({ "data_date": d.map(|d| d.to_string()) })));
    }
    row.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let tip = format!("{} ({})", tr!("toolbar-hide"), ui.ctx().format_shortcut(&TOGGLE_TOOLBAR));
        if tool(ui, Icon::ChevronUp, &tr!("toolbar-hide"), &tip, true, false) {
            action = Some(Action::ToggleToolbar);
        }
    });
    action
}
