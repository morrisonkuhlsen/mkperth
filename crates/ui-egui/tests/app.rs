//! The window driven headless through its accessibility tree: no window, no file dialog.

use std::cell::Cell;
use std::rc::Rc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use mkperth_ui_egui::{PerthApp, Services};

fn harness(app: PerthApp) -> Harness<'static, PerthApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1360.0, 820.0)).build_ui_state(|ui, app: &mut PerthApp| app.ui(ui), app);
    h.run_steps(3);
    h
}

#[test]
fn opens_on_the_sample_scheduled() {
    let mut h = harness(PerthApp::default());
    let s = h.state().session.schedule.as_ref().unwrap();
    assert!(s.activities.iter().any(|a| a.critical));
    assert!(h.query_all_by_label("Schedule").next().is_some() && h.query_by_label("File").is_some());
    // Details and Filters wait folded in the strip; a click opens one beside it, another closes it.
    assert!(h.state().side.is_none());
    h.get_by_label("Details").click();
    h.run_steps(2);
    assert_eq!(h.state().side, Some(mkperth_ui_egui::Side::Details));
    assert!(h.query_by_label("No activity selected").is_some(), "the Details pane waits for a selection");
    h.get_by_label("Filters").click();
    h.run_steps(2);
    assert_eq!(h.state().side, Some(mkperth_ui_egui::Side::Filters));
    // The pane's header says Filters too; the strip's button is drawn after it.
    h.query_all_by_label("Filters").last().unwrap().click();
    h.run_steps(2);
    assert!(h.state().side.is_none());
}

#[test]
fn f9_schedules_and_open_uses_the_dialog() {
    let dir = std::env::temp_dir().join(format!("mkperth-ui-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("p.json");
    std::fs::write(&path, r#"{"name": "Tiny", "tasks": [{"id": "a", "name": "A", "start": "2026-09-01", "duration": 3}], "calendar": ""}"#).unwrap();
    let asked = Rc::new(Cell::new(0));
    let (count, picked) = (asked.clone(), path.clone());
    let services = Services {
        pick_open: Some(Box::new(move || {
            count.set(count.get() + 1);
            Some(picked.clone())
        })),
        ..Services::default()
    };
    let mut h = harness(PerthApp::new(services));
    h.key_press(egui::Key::F9);
    h.run_steps(2);
    assert_eq!(h.state().status, Some(mkperth_ui_egui::Status::Info("Scheduled".into())));
    h.get_by_label("File").click();
    h.run_steps(2);
    h.get_by_label_contains("Open…").click();
    h.run_steps(3);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(asked.get(), 1);
    assert_eq!(h.state().session.project.name, "Tiny");
    assert_eq!(h.state().session.schedule.as_ref().map(|s| s.activities.len()), Ok(1));
}

#[test]
fn add_edit_in_place_undo_and_delete() {
    let mut h = harness(PerthApp::default());
    let count = h.state().session.project.activities.len();
    // A10 selected; a new activity goes after it, its name open for editing.
    h.state_mut().selected = Some(2);
    h.state_mut().add_activity();
    h.run_steps(3);
    assert_eq!(h.state().session.project.activities.len(), count + 1);
    let editing = h.state().editing.clone().expect("the new name is being edited");
    assert_eq!(editing.index, 3);
    // Typed and confirmed with Enter.
    if let Some(e) = h.state_mut().editing.as_mut() {
        e.text = "Soil tests".into();
    }
    h.run_steps(1);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert_eq!(h.state().session.project.activities[3].name, "Soil tests");
    // Its ID follows the highest (A40): A50.
    assert_eq!(h.state().session.project.activities[3].id, "A50");
    assert_eq!(h.state().session.undo_label(), Some("edit activity A50"));
    // Ctrl+Z undoes the rename; the activity stays.
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    h.run_steps(2);
    assert_eq!(h.state().session.project.activities[3].name, "New activity");
    // Delete removes the selected activity.
    h.state_mut().selected = Some(3);
    h.key_press(egui::Key::Delete);
    h.run_steps(2);
    assert_eq!(h.state().session.project.activities.len(), count);
    assert!(h.state().selected.is_none());
}

#[test]
fn progress_is_typed_in_details_and_the_toolbar() {
    use egui::accesskit::Role;
    let mut h = harness(PerthApp::default());
    // The data date in the toolbar, typed as it's shown.
    h.get_by_role_and_label(Role::TextInput, "Data date").click();
    h.run_steps(1);
    h.get_by_role_and_label(Role::TextInput, "Data date").type_text("10-Nov-26");
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert_eq!(h.state().session.project.data_date, mkperth_ui_egui::read_date("2026-11-10"));
    // A10's actual start in Details, on the Status tab: it's under way.
    h.state_mut().selected = Some(2);
    h.state_mut().side = Some(mkperth_ui_egui::Side::Details);
    h.run_steps(2);
    h.get_by_label("Status").click();
    h.run_steps(2);
    h.get_by_role_and_label(Role::TextInput, "Actual start").click();
    h.run_steps(1);
    h.get_by_role_and_label(Role::TextInput, "Actual start").type_text("2026-11-03");
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    let s = h.state().session.schedule.as_ref().unwrap();
    assert_eq!(s.activities[2].status, mkperth_engine::sched::Status::InProgress);
}

#[test]
fn dates_are_read_as_each_language_writes_them() {
    use mkperth_ui_egui::read_date;
    let d = read_date("2026-11-10");
    assert!(d.is_some());
    assert_eq!(read_date("10/11/2026"), d);
    assert_eq!(read_date(" 10-Nov-26 "), d);
    assert_eq!(read_date("10/11/26"), d);
    assert_eq!(read_date("31/02/2026"), None);
    assert_eq!(read_date("tomorrow"), None);
    assert_eq!(read_date("10/99/2026"), None);
}

#[test]
fn the_calculation_saves_as_latex() {
    let dir = std::env::temp_dir().join(format!("mkperth-ui-tex-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("calc.tex");
    let picked = target.clone();
    let services =
        Services { pick_save: Some(Box::new(move |suggested: &str| (suggested == "A10.tex").then(|| picked.clone()))), ..Services::default() };
    let mut h = harness(PerthApp::new(services));
    h.state_mut().selected = Some(2);
    h.run_steps(2);
    h.get_by_label("LaTeX").click();
    h.run_steps(2);
    let tex = std::fs::read_to_string(&target).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(tex.starts_with('%') && tex.contains("A10"), "{tex}");
    assert!(matches!(h.state().status, Some(mkperth_ui_egui::Status::Info(_))));
}

#[test]
fn notes_are_typed_on_their_tab() {
    use egui::accesskit::Role;
    let mut h = harness(PerthApp::default());
    h.state_mut().selected = Some(2);
    h.state_mut().side = Some(mkperth_ui_egui::Side::Details);
    h.run_steps(2);
    h.get_by_label("Notes").click();
    h.run_steps(2);
    assert_eq!(h.state().details_tab, mkperth_ui_egui::DetailsTab::Notes);
    h.get_by_role(Role::MultilineTextInput).click();
    h.run_steps(1);
    h.get_by_role(Role::MultilineTextInput).type_text("Call the surveyor");
    h.run_steps(1);
    // Leaving the box saves them.
    h.get_by_label("General").click();
    h.run_steps(3);
    assert_eq!(h.state().session.project.activities[2].notes, "Call the surveyor");
    assert_eq!(h.state().session.undo_label(), Some("edit activity A10"));
}

#[test]
fn columns_are_chosen_in_filters() {
    let mut h = harness(PerthApp::default());
    h.get_by_label("Filters").click();
    h.run_steps(2);
    // Late start and late finish hidden; the ID and the name can't be.
    h.get_by_label("Late start").click();
    h.get_by_label("Late finish").click();
    h.run_steps(2);
    let shown: Vec<&str> = h.state().options.columns().iter().map(|(_, id, _)| *id).collect();
    assert_eq!(shown, ["col-id", "col-name", "col-duration", "col-early-start", "col-early-finish", "col-total-float", "col-free-float"]);
}

#[test]
fn today_comes_from_the_platform() {
    let day = mkperth_ui_egui::Date::from_ymd(2026, 11, 20);
    let mut h = harness(PerthApp::new(Services { today: Some(Box::new(move || day)), ..Services::default() }));
    h.run_steps(1);
    assert_eq!(h.state().options.today, day);
    let h = harness(PerthApp::default());
    assert_eq!(h.state().options.today, None, "no clock, no line");
}

#[test]
fn a_band_is_added_and_deleted_in_chart_notes() {
    use egui::accesskit::Role;
    let mut h = harness(PerthApp::default());
    let before = h.state().session.project.bands.len();
    h.query_all_by_label("Chart notes").last().unwrap().click();
    h.run_steps(2);
    assert_eq!(h.state().side, Some(mkperth_ui_egui::Side::Notes));
    h.get_by_role_and_label(Role::TextInput, "Name").click();
    h.run_steps(1);
    h.get_by_role_and_label(Role::TextInput, "Name").type_text("Rainy season");
    h.run_steps(1);
    for (field, text) in [("From", "2026-12-01"), ("To", "2027-01-31")] {
        h.get_by_role_and_label(Role::TextInput, field).click();
        h.run_steps(1);
        h.get_by_role_and_label(Role::TextInput, field).type_text(text);
        h.key_press(egui::Key::Enter);
        h.run_steps(2);
    }
    h.get_by_label("Add").click();
    h.run_steps(3);
    let bands = &h.state().session.project.bands;
    assert_eq!(bands.len(), before + 1);
    assert!(bands.iter().any(|b| b.name == "Rainy season" && b.from.to_string() == "2026-12-01" && b.to.to_string() == "2027-01-31"));
    assert!(h.state().note_form.name.is_empty(), "the form is cleared for the next one");
    // Each listed note has its delete button; the first band goes.
    h.query_all_by_label("Delete this note").next().unwrap().click();
    h.run_steps(2);
    assert_eq!(h.state().session.project.bands.len(), before);
}

#[test]
fn the_toolbar_hides_and_comes_back() {
    let mut h = harness(PerthApp::default());
    h.get_by_label("Hide the toolbar").click();
    h.run_steps(2);
    assert!(h.state().toolbar_hidden);
    assert!(h.query_by_label("Data date").is_none(), "the toolbar is gone");
    // Its arrow waits in the header to bring it back.
    h.get_by_label("Show the toolbar").click();
    h.run_steps(2);
    assert!(!h.state().toolbar_hidden);
    assert!(h.query_by_label("Data date").is_some());
}

#[test]
fn the_rail_hides_and_shows_the_table() {
    let mut h = harness(PerthApp::default());
    assert!(!h.state().options.table_hidden, "the table shows at first");
    h.get_by_label("Activity table").click();
    h.run_steps(2);
    assert!(h.state().options.table_hidden);
    h.get_by_label("Activity table").click();
    h.run_steps(2);
    assert!(!h.state().options.table_hidden);
}
