//! The top of the window, as in MK Sigma: a title row (the mark, AutoSave, quick-access Save,
//! Undo and Redo, the project's name, the theme and search), a row of tabs and the ribbon under
//! them. Buttons only report what was asked for; the app acts through the session.

use crate::View;
use crate::icons::{self, Icon};
use crate::theme::Tokens;
use mkperth_engine::i18n::{self, Lang, tr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Home,
    Project,
    View,
}

const TABS: [Tab; 3] = [Tab::Home, Tab::Project, Tab::View];

impl Tab {
    fn name(self) -> String {
        match self {
            Tab::Home => tr!("tab-home"),
            Tab::Project => tr!("tab-project"),
            Tab::View => tr!("tab-view"),
        }
    }
}

/// What was asked for in the title row or the ribbon this frame.
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
    Links,
    /// `project.options`' parameters.
    Options(serde_json::Value),
    /// Wider (1), narrower (-1) or fitted (0).
    TimeScale(i8),
    View(View),
    Theme(egui::ThemePreference),
    ToggleAutoSave,
    Search,
    Language(Lang),
}

/// What the buttons show.
pub(crate) struct State<'a> {
    pub title: &'a str,
    pub autosave: bool,
    pub critical_only: bool,
    pub late_bars: bool,
    pub links: bool,
    pub float_kind: mkperth_engine::core::FloatKind,
    pub critical_by: mkperth_engine::core::CriticalBy,
    /// The time scale fits the whole project.
    pub fitted: bool,
    pub view: View,
    pub undo: Option<&'a str>,
    pub redo: Option<&'a str>,
    /// An activity is selected (Delete needs one).
    pub selected: bool,
}

pub(crate) const TITLE_HEIGHT: f32 = 36.0;
const RIBBON_HEIGHT: f32 = 78.0;
/// The height of a group's name under its buttons.
const GROUP_LABEL: f32 = 15.0;

/// A small square icon button with an accessible name; clicks count only when enabled.
fn icon_button(ui: &mut egui::Ui, icon: Icon, name: &str, enabled: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(egui::vec2(28.0, 26.0), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name));
    if enabled && response.hovered() {
        ui.painter().rect_filled(rect, 0.0, t.hover);
    }
    let icon_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(16.0, 16.0));
    let mut painter = ui.painter().clone();
    if !enabled {
        painter.set_opacity(0.35);
    }
    icons::paint(&painter, icon_rect, icon, &t);
    response.on_hover_text(name)
}

/// An on/off switch drawn as a pill with a knob.
fn switch(ui: &mut egui::Ui, on: bool, name: &str) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (r, response) = ui.allocate_exact_size(egui::vec2(30.0, 16.0), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, on, name));
    let painter = ui.painter();
    painter.rect_filled(r, 8.0, if on { t.accent } else { egui::Color32::TRANSPARENT });
    painter.rect_stroke(r, 8.0, egui::Stroke::new(1.0, if on { t.accent } else { t.text_dim }), egui::StrokeKind::Inside);
    let knob = if on { egui::pos2(r.right() - 8.0, r.center().y) } else { egui::pos2(r.left() + 8.0, r.center().y) };
    painter.circle_filled(knob, 5.0, if on { egui::Color32::WHITE } else { t.text_dim });
    response
}

/// A filled accent button with an icon and text, for a pane's one main action.
pub(crate) fn accent_button(ui: &mut egui::Ui, icon: Icon, text: &str) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let ink = crate::theme::on_accent(&t);
    let galley = ui.painter().layout_no_wrap(text.to_string(), egui::FontId::proportional(13.0), ink);
    let size = egui::vec2(galley.size().x + 36.0, 26.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
    let fill = if response.hovered() { t.accent.gamma_multiply(0.85) } else { t.accent };
    ui.painter().rect_filled(rect, 0.0, fill);
    let mut on = t;
    on.accent = ink;
    icons::paint(ui.painter(), egui::Rect::from_center_size(egui::pos2(rect.left() + 15.0, rect.center().y), egui::vec2(12.0, 12.0)), icon, &on);
    ui.painter().galley(egui::pos2(rect.left() + 26.0, rect.center().y - galley.size().y / 2.0), galley, ink);
    response
}

pub(crate) fn title_bar(ui: &mut egui::Ui, state: &State) -> Option<Action> {
    let t = Tokens::get(ui.ctx());
    let mut action = None;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), TITLE_HEIGHT), egui::Sense::hover());
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, state.title, egui::FontId::proportional(13.5), t.text);
    // Bold by drawing twice, half a point apart: the built-in fonts have no bold face.
    ui.painter().text(rect.center() + egui::vec2(0.5, 0.0), egui::Align2::CENTER_CENTER, state.title, egui::FontId::proportional(13.5), t.text);
    let mut row =
        ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(10.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    row.spacing_mut().item_spacing.x = 2.0;
    icons::show(&mut row, Icon::Mark, 20.0).on_hover_text("MK Perth");
    row.add_space(10.0);
    row.label(egui::RichText::new(tr!("autosave")).size(12.5).color(t.text_dim));
    row.add_space(4.0);
    let tip = if state.autosave { tr!("autosave-on-tip") } else { tr!("autosave-off-tip") };
    if switch(&mut row, state.autosave, &tr!("autosave")).on_hover_text(tip).clicked() {
        action = Some(Action::ToggleAutoSave);
    }
    row.add_space(8.0);
    if icon_button(&mut row, Icon::Save, &tr!("save"), true).clicked() {
        action = Some(Action::Save);
    }
    let undo = state.undo.map_or_else(|| tr!("undo"), |l| tr!("undo-named", what = l));
    if icon_button(&mut row, Icon::Undo, &undo, state.undo.is_some()).clicked() && state.undo.is_some() {
        action = Some(Action::Undo);
    }
    let redo = state.redo.map_or_else(|| tr!("redo"), |l| tr!("redo-named", what = l));
    if icon_button(&mut row, Icon::Redo, &redo, state.redo.is_some()).clicked() && state.redo.is_some() {
        action = Some(Action::Redo);
    }
    row.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if icon_button(ui, Icon::Search, &tr!("search-tip"), true).clicked() {
            action = Some(Action::Search);
        }
        let theme = icon_button(ui, Icon::Contrast, &tr!("theme-tip"), true);
        if theme.clicked() {
            action = Some(Action::Theme(if t.dark { egui::ThemePreference::Light } else { egui::ThemePreference::Dark }));
        }
        theme.context_menu(|ui| {
            for (preference, name) in [
                (egui::ThemePreference::Light, tr!("theme-light")),
                (egui::ThemePreference::Dark, tr!("theme-dark")),
                (egui::ThemePreference::System, tr!("theme-system")),
            ] {
                if ui.button(name).clicked() {
                    action = Some(Action::Theme(preference));
                }
            }
        });
    });
    action
}

/// The tab row, with File as a menu at its start, and the ribbon of the current tab unless
/// `collapsed`.
pub(crate) fn ribbon(ui: &mut egui::Ui, tab: &mut Tab, collapsed: &mut bool, state: &State) -> Option<Action> {
    let t = Tokens::get(ui.ctx());
    let mut action = None;
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.spacing_mut().item_spacing.x = 16.0;
        ui.set_min_height(28.0);
        let widgets = &mut ui.visuals_mut().widgets;
        widgets.inactive.bg_stroke = egui::Stroke::NONE;
        widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
        let file = egui::RichText::new(tr!("file")).size(14.0).color(t.accent);
        let response = ui.add(egui::Button::new(file).frame(false).min_size(egui::vec2(0.0, 28.0))).on_hover_text(tr!("file-tip"));
        egui::Popup::menu(&response).show(|ui| {
            ui.set_min_width(180.0);
            for (name, then) in [
                (tr!("open-menu"), Action::Open),
                (tr!("save-menu"), Action::Save),
                (tr!("save-as-menu"), Action::SaveAs),
                (tr!("sample-menu"), Action::Sample),
            ] {
                if ui.button(name).clicked() {
                    action = Some(then);
                }
            }
        });
        ui.add_space(6.0);
        for which in TABS {
            let name = which.name();
            let on = *tab == which;
            let text = egui::RichText::new(name.as_str()).size(14.0).color(if on { t.text } else { t.text_dim });
            let response = ui.add(egui::Button::new(if on { text.strong() } else { text }).frame(false).min_size(egui::vec2(0.0, 28.0)));
            if on {
                let r = response.rect;
                let bar = egui::Rect::from_min_max(egui::pos2(r.left() + 4.0, r.bottom() - 3.0), egui::pos2(r.right() - 4.0, r.bottom()));
                ui.painter().rect_filled(bar, 0.0, t.accent);
            }
            if response.double_clicked() {
                *collapsed = !*collapsed;
            } else if response.clicked() {
                *tab = which;
                *collapsed = false;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(4.0);
            let (icon, name) = if *collapsed { (Icon::ChevronDown, tr!("ribbon-show")) } else { (Icon::ChevronUp, tr!("ribbon-hide")) };
            if icon_button(ui, icon, &name, true).clicked() {
                *collapsed = !*collapsed;
            }
        });
    });
    if *collapsed {
        return action;
    }
    egui::Frame::new().inner_margin(egui::Margin { left: 4, right: 4, top: 2, bottom: 6 }).show(ui, |ui| {
        ui.set_min_size(egui::vec2(ui.available_width(), RIBBON_HEIGHT + GROUP_LABEL));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if let Some(a) = buttons(ui, *tab, state) {
                action = Some(a);
            }
        });
    });
    action
}

/// One tab's buttons, in named groups divided by thin lines.
fn buttons(ui: &mut egui::Ui, tab: Tab, state: &State) -> Option<Action> {
    let mut action = None;
    let mut chosen = None;
    let mut lang = None;
    let mut go = |ui: &mut egui::Ui, icon: Icon, label: &str, tip: &str, enabled: bool, on: bool, then: Action| {
        if large(ui, icon, label, tip, enabled, on) {
            action = Some(then.clone());
        }
    };
    match tab {
        Tab::Home => {
            group(ui, &tr!("group-file"), |ui| {
                go(ui, Icon::Open, &tr!("open"), &tr!("open-tip"), true, false, Action::Open);
                go(ui, Icon::Save, &tr!("save"), &tr!("save-tip"), true, false, Action::Save);
                go(ui, Icon::Page, &tr!("sample"), &tr!("sample-tip"), true, false, Action::Sample);
            });
            group(ui, &tr!("group-edit"), |ui| {
                go(ui, Icon::Undo, &tr!("undo"), state.undo.unwrap_or(&tr!("nothing-to-undo")), state.undo.is_some(), false, Action::Undo);
                go(ui, Icon::Redo, &tr!("redo"), state.redo.unwrap_or(&tr!("nothing-to-redo")), state.redo.is_some(), false, Action::Redo);
            });
            group(ui, &tr!("group-activities"), |ui| {
                go(ui, Icon::InsertCase, &tr!("add-activity"), &tr!("add-activity-tip"), true, false, Action::AddActivity);
                go(ui, Icon::Close, &tr!("delete-activity"), &tr!("delete-activity-tip"), state.selected, false, Action::DeleteActivity);
            });
            group(ui, &tr!("group-schedule"), |ui| {
                go(ui, Icon::Run, &tr!("schedule"), &tr!("schedule-tip"), true, false, Action::Schedule);
            });
        }
        Tab::Project => {
            group(ui, &tr!("group-schedule"), |ui| {
                go(ui, Icon::Run, &tr!("schedule"), &tr!("schedule-tip"), true, false, Action::Schedule);
            });
            group(ui, &tr!("group-activities"), |ui| {
                go(ui, Icon::InsertCase, &tr!("add-activity"), &tr!("add-activity-tip"), true, false, Action::AddActivity);
                go(ui, Icon::Close, &tr!("delete-activity"), &tr!("delete-activity-tip"), state.selected, false, Action::DeleteActivity);
            });
            group(ui, &tr!("group-show"), |ui| {
                go(ui, Icon::Critical, &tr!("critical-only"), &tr!("critical-only-tip"), true, state.critical_only, Action::CriticalOnly);
                go(ui, Icon::Gantt, &tr!("late-bars"), &tr!("late-bars-tip"), true, state.late_bars, Action::LateBars);
                go(ui, Icon::Network, &tr!("links"), &tr!("links-tip"), true, state.links, Action::Links);
            });
            group(ui, &tr!("group-options"), |ui| chosen = scheduling_options(ui, state));
        }
        Tab::View => {
            group(ui, &tr!("group-views"), |ui| {
                go(
                    ui,
                    Icon::Gantt,
                    &tr!("activities-view"),
                    &tr!("activities-view-tip"),
                    true,
                    state.view == View::Activities,
                    Action::View(View::Activities),
                );
            });
            group(ui, &tr!("group-time-scale"), |ui| {
                go(ui, Icon::Plus, &tr!("time-wider"), &tr!("time-wider-tip"), true, false, Action::TimeScale(1));
                go(ui, Icon::Minus, &tr!("time-narrower"), &tr!("time-narrower-tip"), true, false, Action::TimeScale(-1));
                go(ui, Icon::Grid, &tr!("time-fit"), &tr!("time-fit-tip"), true, state.fitted, Action::TimeScale(0));
            });
            group(ui, &tr!("language"), |ui| lang = language(ui));
        }
    }
    chosen.map(Action::Options).or(lang.map(Action::Language)).or(action)
}

/// A group of buttons with its name underneath and a thin line after it.
fn group(ui: &mut egui::Ui, name: &str, add: impl FnOnce(&mut egui::Ui)) {
    let t = Tokens::get(ui.ctx());
    let inner = ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let row = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.set_min_height(RIBBON_HEIGHT - 4.0);
            add(ui);
        });
        let galley = ui.painter().layout_no_wrap(name.to_string(), egui::FontId::proportional(11.0), t.text_dim);
        let width = row.response.rect.width().max(galley.size().x + 8.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, GROUP_LABEL), egui::Sense::hover());
        ui.painter().galley(egui::pos2(rect.center().x - galley.size().x / 2.0, rect.top()), galley, t.text_dim);
    });
    let r = inner.response.rect;
    ui.add_space(4.0);
    let x = ui.cursor().left();
    ui.painter().vline(x, (r.top() + 6.0)..=(r.bottom() - 4.0), egui::Stroke::new(1.0, t.border));
    ui.add_space(6.0);
}

/// How total float is measured and what is critical.
fn scheduling_options(ui: &mut egui::Ui, state: &State) -> Option<serde_json::Value> {
    use mkperth_engine::core::{CriticalBy, FloatKind};
    let t = Tokens::get(ui.ctx());
    let mut chosen = None;
    ui.vertical(|ui| {
        ui.set_width(170.0);
        ui.add_space(6.0);
        ui.label(egui::RichText::new(tr!("option-float")).size(12.0).color(t.text_dim));
        let name = |k: FloatKind| match k {
            FloatKind::Finish => tr!("float-finish"),
            FloatKind::Start => tr!("float-start"),
            FloatKind::Smallest => tr!("float-smallest"),
        };
        egui::ComboBox::from_id_salt("float-kind").selected_text(name(state.float_kind)).width(160.0).show_ui(ui, |ui| {
            for (k, code) in [(FloatKind::Finish, "finish"), (FloatKind::Start, "start"), (FloatKind::Smallest, "smallest")] {
                if ui.selectable_label(k == state.float_kind, name(k)).clicked() {
                    chosen = Some(serde_json::json!({ "float": code }));
                }
            }
        });
    });
    ui.vertical(|ui| {
        ui.set_width(190.0);
        ui.add_space(6.0);
        ui.label(egui::RichText::new(tr!("option-critical")).size(12.0).color(t.text_dim));
        let longest = state.critical_by == CriticalBy::LongestPath;
        ui.horizontal(|ui| {
            let selected = if longest { tr!("critical-longest-path") } else { tr!("critical-float") };
            egui::ComboBox::from_id_salt("critical-by").selected_text(selected).width(130.0).show_ui(ui, |ui| {
                if ui.selectable_label(!longest, tr!("critical-float")).clicked() && longest {
                    chosen = Some(serde_json::json!({ "critical": "float", "threshold": 0 }));
                }
                if ui.selectable_label(longest, tr!("critical-longest-path")).clicked() && !longest {
                    chosen = Some(serde_json::json!({ "critical": "longest-path" }));
                }
            });
            if let CriticalBy::TotalFloat(threshold) = state.critical_by {
                let mut value = threshold;
                let response = ui.add(egui::DragValue::new(&mut value).range(-999..=999).prefix("≤ ")).on_hover_text(tr!("critical-threshold-tip"));
                if response.changed() && value != threshold {
                    chosen = Some(serde_json::json!({ "critical": "float", "threshold": value }));
                }
            }
        });
    });
    chosen
}

/// The interface's language.
fn language(ui: &mut egui::Ui) -> Option<Lang> {
    let mut chosen = None;
    ui.vertical(|ui| {
        ui.set_width(150.0);
        ui.add_space(24.0);
        let current = i18n::current();
        egui::ComboBox::from_id_salt("language").selected_text(current.name()).width(140.0).show_ui(ui, |ui| {
            for lang in Lang::ALL {
                if ui.selectable_label(lang == current, lang.name()).clicked() {
                    chosen = Some(lang);
                }
            }
        });
    });
    chosen
}

/// A large ribbon button: a 32-point icon over its name. `on` marks a toggle that is on.
fn large(ui: &mut egui::Ui, icon: Icon, label: &str, tip: &str, enabled: bool, on: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let font = egui::FontId::proportional(12.0);
    let text = match label.split_once(' ') {
        Some((a, b)) if label.len() > 9 => format!("{a}\n{b}"),
        _ => label.to_string(),
    };
    let mut job = egui::text::LayoutJob::simple(text, font, if enabled { t.text } else { t.text_dim }, f32::INFINITY);
    job.halign = egui::Align::Center;
    let galley = ui.painter().layout_job(job);
    let width = (galley.size().x + 14.0).max(52.0);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, RIBBON_HEIGHT - 4.0), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, label));
    let painter = ui.painter();
    if on {
        painter.rect_filled(rect, 0.0, t.accent_soft);
    } else if enabled && response.hovered() {
        painter.rect_filled(rect, 0.0, if response.is_pointer_button_down_on() { t.pressed } else { t.hover });
    }
    let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 22.0), egui::vec2(30.0, 30.0));
    let mut icon_painter = painter.clone();
    if !enabled {
        icon_painter.set_opacity(0.35);
    }
    icons::paint(&icon_painter, icon_rect, icon, &t);
    painter.galley(egui::pos2(rect.center().x, rect.top() + 42.0), galley, t.text);
    response.on_hover_text(tip).clicked() && enabled
}
