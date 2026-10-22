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
    assert!(h.query_all_by_label("Schedule").next().is_some() && h.query_by_label("Open").is_some());
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
    h.get_by_label("Open").click();
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
