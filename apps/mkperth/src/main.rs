//! MK Perth desktop app.
//!
//! `mkperth [--no-splash] [project.json]` opens a Perth.jl project, or the sample project without
//! one, after the splash screen.
// A GUI program on Windows: no console window beside the app. Debug builds keep theirs.
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, wgpu};
use mkperth_ui_egui::splash::SplashScreen;
use mkperth_ui_egui::{PerthApp, Services};

struct App(PerthApp);

struct SplashWindow(SplashScreen);

impl eframe::App for SplashWindow {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
    }
}

/// A file in the platform's per-user configuration folder, under `mkperth`.
#[allow(clippy::disallowed_methods)] // the desktop app is native only
fn settings_file(name: &str) -> Option<std::path::PathBuf> {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(std::path::PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
    };
    base.map(|b| b.join("mkperth").join(name))
}

/// The recent projects kept by earlier runs, newest first.
fn load_recent() -> Vec<std::path::PathBuf> {
    settings_file("recent.txt")
        .and_then(|f| std::fs::read_to_string(f).ok())
        .map(|text| text.lines().filter(|l| !l.trim().is_empty()).take(mkperth_ui_egui::backstage::RECENT).map(std::path::PathBuf::from).collect())
        .unwrap_or_default()
}

/// Typeface families the systems have, with their faces' files (regular, bold, italic, bold
/// italic), looked for where each system keeps its fonts.
const FAMILIES: [(&str, [&str; 4]); 14] = [
    ("Segoe UI", ["segoeui.ttf", "segoeuib.ttf", "segoeuii.ttf", "segoeuiz.ttf"]),
    ("Arial", ["arial.ttf", "arialbd.ttf", "ariali.ttf", "arialbi.ttf"]),
    ("Calibri", ["calibri.ttf", "calibrib.ttf", "calibrii.ttf", "calibriz.ttf"]),
    ("Verdana", ["verdana.ttf", "verdanab.ttf", "verdanai.ttf", "verdanaz.ttf"]),
    ("Tahoma", ["tahoma.ttf", "tahomabd.ttf", "tahoma.ttf", "tahomabd.ttf"]),
    ("Trebuchet MS", ["trebuc.ttf", "trebucbd.ttf", "trebucit.ttf", "trebucbi.ttf"]),
    ("Georgia", ["georgia.ttf", "georgiab.ttf", "georgiai.ttf", "georgiaz.ttf"]),
    ("Times New Roman", ["times.ttf", "timesbd.ttf", "timesi.ttf", "timesbi.ttf"]),
    ("Consolas", ["consola.ttf", "consolab.ttf", "consolai.ttf", "consolaz.ttf"]),
    ("Arial (macOS)", ["Arial.ttf", "Arial Bold.ttf", "Arial Italic.ttf", "Arial Bold Italic.ttf"]),
    ("DejaVu Sans", ["DejaVuSans.ttf", "DejaVuSans-Bold.ttf", "DejaVuSans-Oblique.ttf", "DejaVuSans-BoldOblique.ttf"]),
    ("Liberation Sans", ["LiberationSans-Regular.ttf", "LiberationSans-Bold.ttf", "LiberationSans-Italic.ttf", "LiberationSans-BoldItalic.ttf"]),
    ("Noto Sans", ["NotoSans-Regular.ttf", "NotoSans-Bold.ttf", "NotoSans-Italic.ttf", "NotoSans-BoldItalic.ttf"]),
    ("Ubuntu", ["Ubuntu-R.ttf", "Ubuntu-B.ttf", "Ubuntu-RI.ttf", "Ubuntu-BI.ttf"]),
];

/// The folders the system keeps fonts in.
#[allow(clippy::disallowed_methods)] // the desktop app is native only
fn font_folders() -> Vec<std::path::PathBuf> {
    let mut folders = Vec::new();
    if let Some(windows) = std::env::var_os("WINDIR") {
        folders.push(std::path::PathBuf::from(windows).join("Fonts"));
    }
    for f in [
        "/System/Library/Fonts/Supplemental",
        "/Library/Fonts",
        "/usr/share/fonts/truetype/dejavu",
        "/usr/share/fonts/truetype/liberation",
        "/usr/share/fonts/truetype/noto",
        "/usr/share/fonts/truetype/ubuntu",
    ] {
        folders.push(std::path::PathBuf::from(f));
    }
    folders
}

/// Where a family's face is, if the system has it.
fn font_file(family: &str, bold: bool, italic: bool) -> Option<std::path::PathBuf> {
    let (_, files) = FAMILIES.iter().find(|(name, _)| *name == family)?;
    let wanted = files.get(usize::from(bold) + 2 * usize::from(italic)).copied().unwrap_or(files[0]);
    let folders = font_folders();
    [wanted, files[0]].into_iter().find_map(|file| folders.iter().map(|d| d.join(file)).find(|p| p.is_file()))
}

/// The interface's language: the system's.
#[allow(clippy::disallowed_methods)] // the desktop app is native only
fn language() -> mkperth_ui_egui::i18n::Lang {
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|v| std::env::var(v).ok().filter(|s| !s.is_empty())).unwrap_or_default();
    mkperth_ui_egui::i18n::Lang::from_locale(&locale)
}

/// The graphics backends to try, best first; `None` is wgpu's own choice. On Windows that choice
/// is Vulkan, and some Intel Vulkan drivers crash the app at start (igvk64.dll), so it is
/// DirectX 12, then OpenGL, as in MK aeGIS. `WGPU_BACKEND` overrides it everywhere.
fn backends() -> Vec<Option<wgpu::Backends>> {
    if cfg!(target_os = "windows") && wgpu::Backends::from_env().is_none() {
        vec![Some(wgpu::Backends::DX12), Some(wgpu::Backends::GL)]
    } else {
        vec![None]
    }
}

/// A window on the given graphics backends.
fn options(viewport: egui::ViewportBuilder, backends: Option<wgpu::Backends>) -> eframe::NativeOptions {
    let mut wgpu_options = WgpuConfiguration::default();
    if let (Some(backends), WgpuSetup::CreateNew(setup)) = (backends, &mut wgpu_options.wgpu_setup) {
        setup.instance_descriptor.backends = backends;
    }
    eframe::NativeOptions { viewport, wgpu_options, centered: true, ..Default::default() }
}

/// The splash screen in a small window of its own, centred and undecorated, until it closes.
fn splash(backends: Option<wgpu::Backends>) -> eframe::Result {
    let viewport = egui::ViewportBuilder::default()
        .with_title("MK Perth")
        .with_app_id("mkperth")
        .with_inner_size(mkperth_ui_egui::splash::SIZE)
        .with_decorations(false)
        .with_resizable(false)
        .with_taskbar(false);
    eframe::run_native("MK Perth splash", options(viewport, backends), Box::new(|_cc| Ok(Box::new(SplashWindow(SplashScreen::default())))))
}

fn main() -> eframe::Result {
    mkperth_ui_egui::i18n::set(language());
    let services = Services {
        pick_open: Some(Box::new(|| {
            rfd::FileDialog::new()
                .add_filter("Projects (.json, .xml)", &["json", "xml"])
                .add_filter("Perth.jl project (.json)", &["json"])
                .add_filter("Primavera P6 XML (.xml)", &["xml"])
                .add_filter("All files", &["*"])
                .pick_file()
        })),
        pick_save: Some(Box::new(|suggested: &str| {
            let dialog = if suggested.ends_with(".tex") {
                rfd::FileDialog::new().add_filter("LaTeX (.tex)", &["tex"])
            } else {
                rfd::FileDialog::new().add_filter("Perth.jl project (.json)", &["json"]).add_filter("Primavera P6 XML (.xml)", &["xml"])
            };
            dialog.set_file_name(suggested).save_file()
        })),
        save_recent: Some(Box::new(|files: &[std::path::PathBuf]| {
            if let Some(file) = settings_file("recent.txt") {
                let text: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
                let _ = file.parent().map(std::fs::create_dir_all);
                let _ = std::fs::write(
                    file,
                    text.join(
                        "
",
                    ),
                );
            }
        })),
        user: Some(Box::new(|| ["USERNAME", "USER"].iter().find_map(|v| std::env::var(v).ok().filter(|s| !s.is_empty())).unwrap_or_default())),
        now: Some(Box::new(|| chrono::Local::now().format("%Y-%m-%d %H:%M").to_string())),
        fonts: Some(Box::new(|| {
            FAMILIES.iter().map(|(name, _)| *name).filter(|name| font_file(name, false, false).is_some()).map(str::to_string).collect()
        })),
        load_font: Some(Box::new(|family: &str, bold: bool, italic: bool| font_file(family, bold, italic).and_then(|p| std::fs::read(p).ok()))),
        save_font: Some(Box::new(|choice: &mkperth_ui_egui::fonts::FontChoice| {
            if let Some(file) = settings_file("font.txt") {
                let _ = file.parent().map(std::fs::create_dir_all);
                let _ = std::fs::write(file, choice.to_line());
            }
        })),
        today: Some(Box::new(|| {
            use chrono::Datelike as _;
            let d = chrono::Local::now().date_naive();
            mkperth_ui_egui::Date::from_ymd(d.year(), d.month(), d.day())
        })),
    };
    let mut app = PerthApp::new(services);
    app.recent = load_recent();
    if let Some(line) = settings_file("font.txt").and_then(|f| std::fs::read_to_string(f).ok()) {
        app.font = mkperth_ui_egui::fonts::FontChoice::from_line(&line);
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(path) = args.iter().find(|a| !a.starts_with("--")) {
        app.open(std::path::Path::new(path));
    }
    // The splash also finds a graphics backend that starts, for the main window to use; without
    // it (--no-splash) the program starts on the first one.
    let candidates = backends();
    let first = candidates.first().copied().flatten();
    let backends = if args.iter().any(|a| a == "--no-splash") {
        first
    } else {
        candidates.iter().copied().find(|b| !matches!(splash(*b), Err(eframe::Error::Wgpu(_)))).unwrap_or(first)
    };
    let viewport = egui::ViewportBuilder::default()
        .with_title("MK Perth")
        .with_app_id("mkperth")
        .with_inner_size([1360.0, 820.0])
        .with_min_inner_size([800.0, 480.0]);
    eframe::run_native("MK Perth", options(viewport, backends), Box::new(|_cc| Ok(Box::new(App(app)))))
}
