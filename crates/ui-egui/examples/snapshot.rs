//! Renders the MK Perth window offscreen (wgpu, no window) to a PNG, to look at the UI without
//! touching the screen.
//!
//! `cargo run -p mkperth-ui-egui --example snapshot -- [--in project.json] [--dark] [--lang CODE]
//!  [--select N] [--side details|filters] [--details general|status|relationships|notes] [--scale PT_PER_DAY] [--offset DAYS] [--tab home|project|view] [--size 1360x820] out.png` (`--select` selects the activity at row N, from 1)

use mkperth_ui_egui::PerthApp;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut app = PerthApp::default();
    let mut size = (1360.0f32, 820.0f32);
    let mut out = "snapshot.png".to_string();
    let mut dark = false;
    let mut i = 0;
    while let Some(arg) = args.get(i) {
        match arg.as_str() {
            "--in" => {
                i += 1;
                app.open(std::path::Path::new(args.get(i).ok_or("--in needs a path")?));
            }
            "--dark" => dark = true,
            "--tab" => {
                i += 1;
                app.tab = match args.get(i).map(String::as_str) {
                    Some("home") => mkperth_ui_egui::Tab::Home,
                    Some("project") => mkperth_ui_egui::Tab::Project,
                    Some("view") => mkperth_ui_egui::Tab::View,
                    _ => return Err("--tab home|project|view".into()),
                };
            }
            "--scale" => {
                i += 1;
                app.options.scale = Some(args.get(i).and_then(|n| n.parse().ok()).ok_or("--scale needs points per day")?);
            }
            "--offset" => {
                i += 1;
                app.options.offset = args.get(i).and_then(|n| n.parse().ok()).ok_or("--offset needs days")?;
            }
            "--side" => {
                i += 1;
                app.side = match args.get(i).map(String::as_str) {
                    Some("details") => Some(mkperth_ui_egui::Side::Details),
                    Some("filters") => Some(mkperth_ui_egui::Side::Filters),
                    _ => return Err("--side details|filters".into()),
                };
            }
            "--details" => {
                i += 1;
                app.side = Some(mkperth_ui_egui::Side::Details);
                app.details_tab = match args.get(i).map(String::as_str) {
                    Some("general") => mkperth_ui_egui::DetailsTab::General,
                    Some("status") => mkperth_ui_egui::DetailsTab::Status,
                    Some("relationships") => mkperth_ui_egui::DetailsTab::Relationships,
                    Some("notes") => mkperth_ui_egui::DetailsTab::Notes,
                    _ => return Err("--details general|status|relationships|notes".into()),
                };
            }
            "--lang" => {
                i += 1;
                let code = args.get(i).ok_or("--lang needs a code")?;
                mkperth_ui_egui::i18n::set(mkperth_ui_egui::i18n::Lang::from_code(code).ok_or("--lang: en or pt-BR")?);
            }
            "--select" => {
                i += 1;
                app.selected = args.get(i).and_then(|n| n.parse::<usize>().ok()).and_then(|n| n.checked_sub(1));
            }
            "--size" => {
                i += 1;
                let (w, h) = args.get(i).and_then(|s| s.split_once('x')).ok_or("--size WxH")?;
                size = (w.parse().map_err(|_| "--size WxH")?, h.parse().map_err(|_| "--size WxH")?);
            }
            other => out = other.to_string(),
        }
        i += 1;
    }
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(size.0, size.1))
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_ui_state(|ui, app: &mut PerthApp| app.ui(ui), app);
    if dark {
        harness.ctx.set_theme(egui::Theme::Dark);
    }
    harness.run_steps(4);
    let image = harness.render().map_err(|e| e.to_string())?;
    image.save(&out).map_err(|e| e.to_string())?;
    println!("{out}");
    Ok(())
}
