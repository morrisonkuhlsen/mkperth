//! Renders the MK Perth window offscreen (wgpu, no window) to a PNG, to look at the UI without
//! touching the screen.
//!
//! `cargo run -p mkperth-ui-egui --example snapshot -- [--in project.json] [--dark] [--lang CODE]
//!  [--select N] [--resource N] [--group code:NAME|field:NAME] [--column code:NAME|field:NAME]… [--side details|filters|notes|calendars|levelling|pert|baselines|costs|budget|earned|codes] [--details general|status|relationships|notes|resources|comments] [--scale PT_PER_DAY] [--offset DAYS] [--toolbar-hidden] [--present] [--splash] [--backstage home|new|open|info|export|about] [--view activities|resources] [--cmd id=JSON]… [--click LABEL]… [--size 1360x820] out.png` (`--select` selects the activity at row N, from 1, `--resource` the resource;
//!  `--click` clicks what has that accessible name, in order: `--click View --click Theme` opens a submenu)

use egui_kittest::kittest::Queryable as _;
use mkperth_ui_egui::PerthApp;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut app = PerthApp::default();
    let mut size = (1360.0f32, 820.0f32);
    let mut out = "snapshot.png".to_string();
    let mut dark = false;
    let mut clicks = Vec::new();
    let mut splash = false;
    let mut i = 0;
    while let Some(arg) = args.get(i) {
        match arg.as_str() {
            "--in" => {
                i += 1;
                app.open(std::path::Path::new(args.get(i).ok_or("--in needs a path")?));
            }
            "--dark" => dark = true,
            "--toolbar-hidden" => app.toolbar_hidden = true,
            "--present" => app.presenting = true,
            "--splash" => splash = true,
            "--backstage" => {
                i += 1;
                use mkperth_ui_egui::backstage::Page;
                app.backstage = Some(match args.get(i).map(String::as_str) {
                    Some("home") => Page::Home,
                    Some("new") => Page::New,
                    Some("open") => Page::Open,
                    Some("info") => Page::Info,
                    Some("export") => Page::Export,
                    Some("about") => Page::About,
                    _ => return Err("--backstage home|new|open|info|export|about".into()),
                });
            }
            "--view" => {
                i += 1;
                app.view = match args.get(i).map(String::as_str) {
                    Some("activities") => mkperth_ui_egui::View::Activities,
                    Some("resources") => mkperth_ui_egui::View::Resources,
                    _ => return Err("--view activities|resources".into()),
                };
            }
            "--cmd" => {
                // A command run first: `--cmd 'resource.add={"name":"Ana"}'`.
                i += 1;
                let (id, params) = args.get(i).and_then(|c| c.split_once('=')).ok_or("--cmd id=JSON")?;
                app.run(id, serde_json::from_str(params).map_err(|e| e.to_string())?);
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
                    Some("notes") => Some(mkperth_ui_egui::Side::Notes),
                    Some("calendars") => Some(mkperth_ui_egui::Side::Calendars),
                    Some("levelling") => Some(mkperth_ui_egui::Side::Levelling),
                    Some("pert") => Some(mkperth_ui_egui::Side::Pert),
                    Some("baselines") => Some(mkperth_ui_egui::Side::Baselines),
                    Some("costs") => Some(mkperth_ui_egui::Side::Costs),
                    Some("budget") => Some(mkperth_ui_egui::Side::Budget),
                    Some("earned") => Some(mkperth_ui_egui::Side::EarnedValue),
                    Some("codes") => Some(mkperth_ui_egui::Side::Codes),
                    _ => return Err("--side details|filters|notes|calendars|levelling|pert|baselines|costs|budget|earned".into()),
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
                    Some("comments") => mkperth_ui_egui::DetailsTab::Comments,
                    Some("resources") => mkperth_ui_egui::DetailsTab::Resources,
                    _ => return Err("--details general|status|relationships|notes|resources|comments".into()),
                };
            }
            "--lang" => {
                i += 1;
                let code = args.get(i).ok_or("--lang needs a code")?;
                mkperth_ui_egui::i18n::set(mkperth_ui_egui::i18n::Lang::from_code(code).ok_or("--lang: en or pt-BR")?);
            }
            "--click" => {
                i += 1;
                clicks.push(args.get(i).ok_or("--click needs a label")?.clone());
            }
            "--group" | "--column" => {
                // A code's or a field's: `code:Phase`, `field:Risk`.
                let flag = arg.clone();
                i += 1;
                let custom = match args.get(i).and_then(|c| c.split_once(':')) {
                    Some(("code", n)) => mkperth_ui_egui::activities::Custom::Code(n.to_string()),
                    Some(("field", n)) => mkperth_ui_egui::activities::Custom::Field(n.to_string()),
                    _ => return Err(format!("{flag} code:NAME|field:NAME")),
                };
                if flag == "--group" {
                    app.options.group_by = Some(custom);
                } else {
                    app.options.custom_columns.push(custom);
                }
            }
            "--resource" => {
                i += 1;
                app.resource_selected = args.get(i).and_then(|n| n.parse::<usize>().ok()).and_then(|n| n.checked_sub(1));
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
    if splash {
        // The splash screen alone, at its own size, part way through.
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(mkperth_ui_egui::splash::SIZE[0], mkperth_ui_egui::splash::SIZE[1]))
            .with_pixels_per_point(1.0)
            .with_step_dt(0.5)
            .wgpu()
            .build_ui_state(|ui, s: &mut mkperth_ui_egui::splash::SplashScreen| s.ui(ui), Default::default());
        harness.run_steps(5);
        let image = harness.render().map_err(|e| e.to_string())?;
        image.save(&out).map_err(|e| e.to_string())?;
        println!("{out}");
        return Ok(());
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
    for label in &clicks {
        harness
            .query_by_label(label)
            .or_else(|| harness.query_by_label_contains(label))
            .ok_or(format!("--click: nothing is named {label:?}"))?
            .click();
        harness.run_steps(3);
    }
    let image = harness.render().map_err(|e| e.to_string())?;
    image.save(&out).map_err(|e| e.to_string())?;
    println!("{out}");
    Ok(())
}
