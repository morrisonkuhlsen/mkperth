//! Design tokens: the colours every part of the window is painted with, light and dark. The
//! layout is the house's quiet one, as Pearl has it: a slim title bar with the name in the
//! middle, a row of menus, one row of small single-colour icons, rails of icons flush with both
//! edges and a status bar, all on one warm surface, with the schedule on a lighter sheet. The
//! palette is Pearl's ivory in layers, accented with Morrison Kühlsen's blue until MK Perth's
//! brand is chosen (#18); the dark one is the house's ink in layers of the same blue-black.

use egui::{Color32, Stroke, Visuals};

/// The corners of menus, fields, buttons and dialogs: soft, as Pearl's. What runs to an edge of
/// the window (rails, bars, panes) stays square.
pub const RADIUS: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tokens {
    pub dark: bool,
    /// Behind everything: the title bar, the menus, the toolbar, the rails and the status bar.
    pub window: Color32,
    /// Raised a step from the window: cards, dialogs.
    pub ribbon: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub grid_bg: Color32,
    pub gridline: Color32,
    pub header_bg: Color32,
    pub header_text: Color32,
    pub header_line: Color32,
    /// Cells' text; dimmed for missing values.
    pub cell_text: Color32,
    pub input_bg: Color32,
    /// The panes' sheet beside the schedule.
    pub paper: Color32,
    /// Colours of the drawings beside the accent: data, analysis, charts, warnings.
    pub blue: Color32,
    pub green: Color32,
    pub orange: Color32,
    pub red: Color32,
    pub danger: Color32,
    pub warn: Color32,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

impl Tokens {
    pub fn light() -> Tokens {
        Tokens {
            dark: false,
            window: rgb(0xF4EFE7),
            ribbon: rgb(0xFBF8F2),
            border: rgb(0xE6DDD0),
            text: rgb(0x2A2622),
            text_dim: rgb(0x655D54),
            // Morrison Kühlsen's blue.
            accent: rgb(0x2540D8),
            accent_soft: rgb(0xE4E4F2),
            hover: rgb(0xEDE5D9),
            pressed: rgb(0xE3D9CA),
            grid_bg: rgb(0xFFFDF9),
            gridline: rgb(0xEDE6DB),
            header_bg: rgb(0xF8F4ED),
            header_text: rgb(0x5C544B),
            header_line: rgb(0xDDD3C5),
            cell_text: rgb(0x1F1B17),
            input_bg: rgb(0xFFFDF9),
            paper: rgb(0xFFFDF9),
            blue: rgb(0x1F6FC5),
            green: rgb(0x1E8A5A),
            orange: rgb(0xE07A1F),
            red: rgb(0xC93C3C),
            danger: rgb(0xC42B1C),
            warn: rgb(0xA36200),
        }
    }

    pub fn dark() -> Tokens {
        Tokens {
            dark: true,
            window: rgb(0x14161C),
            ribbon: rgb(0x1B1E25),
            border: rgb(0x262A33),
            text: rgb(0xE6E8EC),
            text_dim: rgb(0x9AA1AD),
            accent: rgb(0x8E9CFF),
            accent_soft: rgb(0x262C4E),
            hover: rgb(0x232731),
            pressed: rgb(0x2B303B),
            grid_bg: rgb(0x1B1E25),
            gridline: rgb(0x2A2E37),
            header_bg: rgb(0x181B21),
            header_text: rgb(0xB4BAC5),
            header_line: rgb(0x2E333D),
            cell_text: rgb(0xE6E8EC),
            input_bg: rgb(0x101217),
            paper: rgb(0x1B1E25),
            blue: rgb(0x6CAEF5),
            green: rgb(0x5CC58E),
            orange: rgb(0xF2A055),
            red: rgb(0xF07A7A),
            danger: rgb(0xFF9585),
            warn: rgb(0xF0C060),
        }
    }

    /// The palette of the theme in use.
    pub fn get(ctx: &egui::Context) -> Tokens {
        if ctx.theme() == egui::Theme::Dark { Tokens::dark() } else { Tokens::light() }
    }
}

/// Installs both palettes in egui's light and dark style slots and follows the system's choice
/// (light when the system doesn't say).
pub fn apply(ctx: &egui::Context) {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let t = if theme == egui::Theme::Dark { Tokens::dark() } else { Tokens::light() };
        let mut v = if t.dark { Visuals::dark() } else { Visuals::light() };
        v.panel_fill = t.window;
        v.window_fill = t.ribbon;
        v.window_stroke = Stroke::new(1.0, t.border);
        v.extreme_bg_color = t.input_bg;
        v.faint_bg_color = t.header_bg;
        v.selection.bg_fill = t.accent_soft;
        v.selection.stroke = Stroke::new(1.0, t.accent);
        v.hyperlink_color = t.accent;
        v.warn_fg_color = t.warn;
        v.error_fg_color = t.danger;
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, t.text);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, t.border);
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, t.text);
        v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        v.widgets.inactive.bg_fill = t.hover;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, t.border);
        v.widgets.hovered.weak_bg_fill = t.hover;
        v.widgets.hovered.bg_fill = t.hover;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, t.header_line);
        v.widgets.active.weak_bg_fill = t.pressed;
        v.widgets.active.bg_fill = t.pressed;
        for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
            w.corner_radius = egui::CornerRadius::same(RADIUS);
        }
        v.window_corner_radius = egui::CornerRadius::same(RADIUS + 2);
        v.menu_corner_radius = egui::CornerRadius::same(RADIUS);
        // A dialog's title bar is the open state's fill: the faint header tone, not a grey band.
        v.widgets.open.weak_bg_fill = t.header_bg;
        v.widgets.open.bg_fill = t.header_bg;
        v.widgets.open.bg_stroke = Stroke::new(1.0, t.border);
        // A soft, wide shadow lifts dialogs and menus without a heavy edge.
        v.popup_shadow =
            egui::epaint::Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(if t.dark { 110 } else { 28 }) };
        v.window_shadow = v.popup_shadow;
        v.striped = true;
        // A slider's handle a slim upright bar on a fine rail, as the house's status bars have it.
        v.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.45 };
        v.slider_trailing_fill = true;
        ctx.set_visuals_of(theme, v);
    }
    ctx.options_mut(|o| {
        o.fallback_theme = egui::Theme::Light;
        o.theme_preference = egui::ThemePreference::System;
    });
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(10.0, 4.0);
        s.spacing.interact_size.y = 24.0;
        s.spacing.window_margin = egui::Margin::same(14);
        s.spacing.menu_margin = egui::Margin::same(6);
        s.spacing.slider_rail_height = 3.0;
        // Scroll bars thin and floating over what they scroll, widening under the pointer.
        s.spacing.scroll = egui::style::ScrollStyle::thin();
        s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(13.0));
        s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
        s.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
        s.text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(18.0));
        s.text_styles.insert(egui::TextStyle::Monospace, egui::FontId::monospace(12.5));
    });
}

/// A panel with a hairline border and soft corners: a card inside a pane.
pub fn card(t: &Tokens) -> egui::Frame {
    egui::Frame::new().fill(t.ribbon).stroke(Stroke::new(1.0, t.border)).corner_radius(RADIUS).inner_margin(egui::Margin::symmetric(12, 8))
}

/// A sheet flush with what is round it, edged by the hairlines its neighbours draw: the panes
/// and the calculation.
pub fn sheet(t: &Tokens) -> egui::Frame {
    egui::Frame::new().fill(t.paper).inner_margin(egui::Margin::symmetric(14, 8))
}

/// Text and icons on the accent colour: white on the light theme's deep blue, near-black on the
/// dark theme's pale one, so both read clearly.
pub fn on_accent(t: &Tokens) -> Color32 {
    if t.dark { t.window } else { Color32::WHITE }
}

/// Height of a pane's header row.
pub const HEADER_HEIGHT: f32 = 28.0;

/// A pane's header, the same in every pane: an icon and the title on the left, `actions` laid
/// out from the right, and a hairline under the row.
pub fn pane_header<R>(ui: &mut egui::Ui, icon: crate::icons::Icon, title: &str, tip: &str, actions: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let t = Tokens::get(ui.ctx());
    let inner = ui
        .horizontal(|ui| {
            ui.set_min_height(HEADER_HEIGHT);
            ui.spacing_mut().item_spacing.x = 6.0;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
            crate::icons::paint_mono(ui.painter(), rect, icon, t.text_dim);
            let label = ui.label(egui::RichText::new(title).size(13.5).color(t.text).strong());
            if !tip.is_empty() {
                label.on_hover_text(tip);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                actions(ui)
            })
            .inner
        })
        .inner;
    ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), Stroke::new(1.0, t.border));
    ui.add_space(8.0);
    inner
}

/// A quiet action in a header or beside an Output item: text in the dim colour, a tint only
/// under the pointer. Disabled, it fades.
pub fn ghost(ui: &mut egui::Ui, text: &str, tip: &str, enabled: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let galley = ui.painter().layout_no_wrap(text.to_string(), egui::FontId::proportional(12.5), t.text_dim);
    let size = egui::vec2(galley.size().x + 14.0, 22.0);
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, text));
    let color = if !enabled {
        t.text_dim.gamma_multiply(0.45)
    } else if response.hovered() {
        ui.painter().rect_filled(rect, RADIUS, t.hover);
        t.text
    } else {
        t.text_dim
    };
    ui.painter().galley_with_override_text_color(rect.center() - galley.size() / 2.0, galley, color);
    let response = if tip.is_empty() { response } else { response.on_hover_text(tip) };
    response.clicked()
}

/// The width of a pane's form labels: one column, so the fields after them line up.
pub const LABEL_WIDTH: f32 = 118.0;

/// A section of a pane: space above, its title small and quiet in capitals, as panels in
/// design tools head theirs.
pub fn section(ui: &mut egui::Ui, title: &str) -> egui::Id {
    let t = Tokens::get(ui.ctx());
    ui.add_space(10.0);
    // Drawn in capitals; read out as written.
    let galley = ui.painter().layout(title.to_uppercase(), egui::FontId::proportional(11.0), t.text_dim, ui.available_width());
    let (rect, response) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, title));
    ui.painter().galley(rect.min, galley.clone(), t.text_dim);
    // Strong by drawing twice, half a point apart: the built-in fonts have no bold face.
    ui.painter().galley(rect.min + egui::vec2(0.4, 0.0), galley, t.text_dim);
    ui.add_space(2.0);
    response.id
}

/// Dim text under a section: what is empty, a line of help.
pub fn hint(ui: &mut egui::Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(text).size(12.0).color(t.text_dim));
}

/// A form's row: `label` in the label column, then what `add` puts after it, given the label's
/// id (for `labelled_by`) and filling the rest of the width.
pub fn row<R>(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui, egui::Id) -> R) -> R {
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.set_min_height(24.0);
        let id = ui
            .allocate_ui_with_layout(egui::vec2(LABEL_WIDTH, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_width(LABEL_WIDTH);
                ui.add(egui::Label::new(egui::RichText::new(label).size(12.5).color(t.text_dim)).wrap()).id
            })
            .inner;
        add(ui, id)
    })
    .inner
}

/// An item of a list in a pane: a colour swatch if it has one, its text, quieter detail after
/// it, and a button to delete it at the row's right edge. True when that button is clicked.
pub fn list_item(ui: &mut egui::Ui, swatch: Option<Color32>, text: &str, detail: &str, delete: &str) -> bool {
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.set_min_height(22.0);
        if let Some(color) = swatch {
            let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
            ui.painter().rect_filled(r, 0.0, color);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let deleted = crate::icon_button(ui, crate::icons::Icon::Close, delete);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add(egui::Label::new(egui::RichText::new(text).size(12.5).color(t.text)).truncate());
                if !detail.is_empty() {
                    ui.add(egui::Label::new(egui::RichText::new(detail).size(12.0).color(t.text_dim)).truncate());
                }
            });
            deleted
        })
        .inner
    })
    .inner
}

/// A fact in a pane: its name in the label column, its value at the row's right edge, as
/// figures line up in a table.
pub fn fact(ui: &mut egui::Ui, name: &str, value: &str, color: Color32) {
    row(ui, name, |ui, _| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value).size(12.5).color(color));
        });
    });
}

/// The space a button under a form's fields leaves on its left, so it starts where they do.
pub fn under_fields(ui: &mut egui::Ui) {
    ui.add_space(LABEL_WIDTH + ui.spacing().item_spacing.x);
}

/// The width a text field or a combo box is given to fill the rest of a row: their frames add
/// a margin on each side to the width asked for.
pub fn fill(ui: &egui::Ui) -> f32 {
    (ui.available_width() - 2.0 * ui.spacing().button_padding.x - 2.0).max(40.0)
}
