//! MK Perth desktop app.
//!
//! `mkperth [project.json]` opens a Perth.jl project, or the sample project without one.
// A GUI program on Windows: no console window beside the app. Debug builds keep theirs.
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use mkperth_ui_egui::{PerthApp, Services};

struct App(PerthApp);

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
    }
}

/// The interface's language: the system's.
#[allow(clippy::disallowed_methods)] // the desktop app is native only
fn language() -> mkperth_ui_egui::i18n::Lang {
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|v| std::env::var(v).ok().filter(|s| !s.is_empty())).unwrap_or_default();
    mkperth_ui_egui::i18n::Lang::from_locale(&locale)
}

fn main() -> eframe::Result {
    mkperth_ui_egui::i18n::set(language());
    let services = Services {
        pick_open: Some(Box::new(|| {
            rfd::FileDialog::new().add_filter("Perth.jl project (.json)", &["json"]).add_filter("All files", &["*"]).pick_file()
        })),
    };
    let mut app = PerthApp::new(services);
    if let Some(path) = std::env::args().nth(1) {
        app.open(std::path::Path::new(&path));
    }
    let viewport = egui::ViewportBuilder::default()
        .with_title("MK Perth")
        .with_app_id("mkperth")
        .with_inner_size([1360.0, 820.0])
        .with_min_inner_size([800.0, 480.0]);
    let options = eframe::NativeOptions { viewport, centered: true, ..Default::default() };
    eframe::run_native("MK Perth", options, Box::new(|_cc| Ok(Box::new(App(app)))))
}
