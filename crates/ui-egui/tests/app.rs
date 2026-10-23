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
    // File opens the backstage; its Open a project card the dialog.
    h.get_by_label("File").click();
    h.run_steps(2);
    assert_eq!(h.state().backstage, Some(mkperth_ui_egui::backstage::Page::Home));
    h.get_by_label("Open a project").click();
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

/// File > New project: an empty project from the day the app is told is today.
#[test]
fn a_new_project_from_the_file_menu() {
    let services = Services { today: Some(Box::new(|| mkperth_ui_egui::Date::from_ymd(2026, 11, 6))), ..Services::default() };
    let mut h = harness(PerthApp::new(services));
    h.get_by_label("File").click();
    h.run_steps(2);
    h.get_by_label("New project").click();
    h.run_steps(3);
    assert!(h.state().session.project.activities.is_empty());
    assert_eq!(h.state().session.project.start.to_string(), "2026-11-06");
    assert_eq!(h.state().status, Some(mkperth_ui_egui::Status::Info("New project".into())));
}

/// The Calendars pane: Saturday worked, the region's holidays off, each undoable.
#[test]
fn calendars_are_edited_in_their_pane() {
    let mut h = harness(PerthApp::default());
    h.get_by_label("Calendars").click();
    h.run_steps(2);
    assert_eq!(h.state().side, Some(mkperth_ui_egui::Side::Calendars));
    assert!(!h.state().session.project.calendar.week[5]);
    h.get_by_label("Sat").click();
    h.run_steps(2);
    assert!(h.state().session.project.calendar.week[5], "Saturday is worked");
    h.get_by_label("Brazil's national holidays").click();
    h.run_steps(2);
    assert_eq!(h.state().session.project.calendar.region, None);
    h.state_mut().run("edit.undo", serde_json::Value::Null);
    h.run_steps(2);
    assert!(h.state().session.project.calendar.region.is_some());
}

/// The Resources view from the rail: a resource added through its form, picked, assigned.
#[test]
fn resources_are_added_and_assigned_in_their_view() {
    use egui::accesskit::Role;
    let mut h = harness(PerthApp::default());
    h.get_by_label("Resources").click();
    h.run_steps(2);
    assert_eq!(h.state().view, mkperth_ui_egui::View::Resources);
    h.get_by_role_and_label(Role::TextInput, "New resource").click();
    h.run_steps(1);
    h.get_by_role_and_label(Role::TextInput, "New resource").type_text("Ana");
    h.run_steps(1);
    h.get_by_label("Add resource").click();
    h.run_steps(3);
    assert_eq!(h.state().session.project.resources.len(), 1);
    assert_eq!(h.state().session.project.resources[0].name, "Ana");
    h.get_by_label("R1").click();
    h.run_steps(2);
    assert_eq!(h.state().resource_selected, Some(0));
    h.state_mut().resource_form.activity = "A10".into();
    h.run_steps(2);
    h.get_by_label("Assign").click();
    h.run_steps(3);
    let a = &h.state().session.project.assignments;
    assert_eq!(a.len(), 1);
    assert_eq!((a[0].activity.as_str(), a[0].planned), ("A10", 4000), "A10's 40 hours");
    // Her work by week shows under her assignments.
    h.run_steps(2);
    assert!(h.query_by_label("Work by week (hours)").is_some());
}

/// The backstage: its pages from the column, Esc back to the project; Back too.
#[test]
fn the_backstage_opens_its_pages_and_closes() {
    use mkperth_ui_egui::backstage::Page;
    let mut h = harness(PerthApp::default());
    h.get_by_label("File").click();
    h.run_steps(2);
    h.get_by_label("Info").click();
    h.run_steps(2);
    assert_eq!(h.state().backstage, Some(Page::Info));
    assert!(h.query_by_label("AutoSave").is_some(), "AutoSave lives on Info");
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert_eq!(h.state().backstage, None);
    h.get_by_label("File").click();
    h.run_steps(2);
    h.get_by_label("Back to the project").click();
    h.run_steps(2);
    assert_eq!(h.state().backstage, None);
}

/// Ctrl+Alt+M on an activity: its Comments tab; a comment signed by the app's user at its time,
/// then resolved.
#[test]
fn comments_are_written_and_resolved_in_details() {
    use egui::accesskit::Role;
    let services =
        Services { user: Some(Box::new(|| "Ana".to_string())), now: Some(Box::new(|| "2026-10-23 09:00".to_string())), ..Services::default() };
    let mut h = harness(PerthApp::new(services));
    h.state_mut().selected = Some(2);
    h.run_steps(1);
    h.key_press_modifiers(egui::Modifiers::COMMAND | egui::Modifiers::ALT, egui::Key::M);
    h.run_steps(2);
    assert_eq!(h.state().details_tab, mkperth_ui_egui::DetailsTab::Comments);
    h.get_by_role_and_label(Role::MultilineTextInput, "New comment").click();
    h.run_steps(1);
    h.get_by_role_and_label(Role::MultilineTextInput, "New comment").type_text("Booked?");
    h.run_steps(1);
    h.get_by_label("Comment").click();
    h.run_steps(3);
    let c = &h.state().session.project.comments;
    assert_eq!(c.len(), 1);
    assert_eq!((c[0].activity.as_str(), c[0].author.as_str(), c[0].at.as_str(), c[0].text.as_str()), ("A10", "Ana", "2026-10-23 09:00", "Booked?"));
    assert!(h.state().comment_form.text.is_empty(), "the box is cleared");
    h.get_by_label("Resolve").click();
    h.run_steps(3);
    assert!(h.state().session.project.comments[0].resolved);
}

/// Details' Resources tab: a resource assigned to the selected activity, its work typed in place.
#[test]
fn an_activity_takes_resources_in_details() {
    use egui::accesskit::Role;
    let mut h = harness(PerthApp::default());
    h.state_mut().run("resource.add", serde_json::json!({"name": "Ana"}));
    h.state_mut().selected = Some(2);
    h.state_mut().side = Some(mkperth_ui_egui::Side::Details);
    h.state_mut().details_tab = mkperth_ui_egui::DetailsTab::Resources;
    h.state_mut().resource_form.resource = "R1".into();
    h.run_steps(2);
    h.get_by_label("Assign").click();
    h.run_steps(3);
    assert_eq!(h.state().session.project.assignments.len(), 1);
    assert_eq!(h.state().session.project.assignments[0].activity, "A10");
    // Hours done, typed where they show: the third field on her row (budgeted, units/h, actual, left).
    h.get_all_by_role_and_label(Role::TextInput, "Ana (h)").nth(2).unwrap().click();
    h.run_steps(1);
    h.get_all_by_role_and_label(Role::TextInput, "Ana (h)").nth(2).unwrap().type_text("6");
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert_eq!(h.state().session.project.assignments[0].actual, 600);
}

/// A view from the rail opens as a tab beside the activities; the table's switch stays on the
/// rail; closing the tab goes back to the activities.
#[test]
fn views_open_as_tabs() {
    use mkperth_ui_egui::View;
    let mut h = harness(PerthApp::default());
    h.get_by_label("Resources").click();
    h.run_steps(2);
    assert_eq!((h.state().view, h.state().open_views.clone()), (View::Resources, vec![View::Activities, View::Resources]));
    assert!(h.query_by_label("Activity table").is_some(), "the table's switch stays");
    h.get_by_label("Close Resources").click();
    h.run_steps(2);
    assert_eq!((h.state().view, h.state().open_views.clone()), (View::Activities, vec![View::Activities]));
}

/// The pointer at `pos`, pressed (or let go) with `modifiers`.
fn button(h: &mut Harness<'static, PerthApp>, pos: egui::Pos2, pressed: bool, modifiers: egui::Modifiers) {
    // The keys held, as the window tells egui.
    h.event(egui::Event::ModifiersChanged(modifiers));
    h.event(egui::Event::PointerMoved(pos));
    h.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers });
    h.run_steps(1);
}

fn click(h: &mut Harness<'static, PerthApp>, pos: egui::Pos2, modifiers: egui::Modifiers) {
    button(h, pos, true, modifiers);
    button(h, pos, false, modifiers);
}

/// Two clicks close together (with a harness of short steps, `quick`).
fn double_click(h: &mut Harness<'static, PerthApp>, pos: egui::Pos2) {
    click(h, pos, egui::Modifiers::NONE);
    click(h, pos, egui::Modifiers::NONE);
    h.run_steps(2);
}

/// A harness whose steps are a hundredth of a second, so two clicks make a double click.
fn quick(app: PerthApp) -> Harness<'static, PerthApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1360.0, 820.0)).with_step_dt(0.01).build_ui_state(|ui, app: &mut PerthApp| app.ui(ui), app);
    h.run_steps(3);
    h
}

// Where the sample's rows and ruler are in the 1360 × 820 window: W1, M0, A10, A20, A30 down
// the table; the ruler's months over its days; the chart past the table.
const TABLE_X: f32 = 200.0;
const ROW_A10: f32 = 253.0;
const ROW_A20: f32 = 277.0;
const ROW_A30: f32 = 301.0;

/// Dragging over rows of the table chooses them all; Ctrl and a click adds one; Delete takes
/// every one chosen, in one change.
#[test]
fn rows_are_chosen_by_dragging_and_ctrl_clicks() {
    let mut h = harness(PerthApp::default());
    let n = h.state().session.project.activities.len();
    button(&mut h, egui::pos2(TABLE_X, ROW_A10), true, egui::Modifiers::NONE);
    for y in [260.0, 280.0, ROW_A30] {
        h.event(egui::Event::PointerMoved(egui::pos2(TABLE_X + 10.0, y)));
        h.run_steps(1);
    }
    button(&mut h, egui::pos2(TABLE_X + 10.0, ROW_A30), false, egui::Modifiers::NONE);
    h.run_steps(2);
    assert_eq!(h.state().marked.iter().copied().collect::<Vec<_>>(), [2, 3, 4], "A10, A20 and A30");
    // A plain click chooses one; Ctrl and a click adds another.
    click(&mut h, egui::pos2(TABLE_X, ROW_A10), egui::Modifiers::NONE);
    h.run_steps(1);
    assert_eq!(h.state().marked.iter().copied().collect::<Vec<_>>(), [2]);
    click(&mut h, egui::pos2(TABLE_X, ROW_A30), egui::Modifiers::COMMAND);
    h.run_steps(1);
    assert_eq!(h.state().marked.iter().copied().collect::<Vec<_>>(), [2, 4]);
    h.key_press(egui::Key::Delete);
    h.run_steps(2);
    assert_eq!(h.state().session.project.activities.len(), n - 2);
    assert_eq!(h.state().session.undo_label(), Some("delete 2 activities"));
    // Ctrl+A chooses every one.
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run_steps(1);
    assert_eq!(h.state().marked.len(), n - 2);
}

/// A double click on the chart away from a bar opens a small window for a band from that day;
/// on the ruler's months, one to mark the month; on a bar, its details.
#[test]
fn double_clicks_on_the_chart_add_notes_and_open_details() {
    let mut h = quick(PerthApp::default());
    double_click(&mut h, egui::pos2(1250.0, ROW_A10));
    let note = h.state().quick_note.clone().expect("a band's window");
    assert!(!note.month && note.from == note.to);
    h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Name").type_text("Rain");
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    let bands = &h.state().session.project.bands;
    assert!(bands.iter().any(|b| b.name == "Rain" && b.from == note.from), "{bands:?}");
    assert!(h.state().quick_note.is_none());
    // January 2027 on the ruler's months (a fresh window: the name just typed keeps the focus).
    let mut h = quick(PerthApp::default());
    double_click(&mut h, egui::pos2(1090.0, 161.0));
    let note = h.state().quick_note.clone().expect("a month's window");
    assert_eq!((note.month, note.from.to_string()), (true, "2027-01-01".to_string()));
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert!(h.state().quick_note.is_none(), "Esc drops it");
    // A20's bar.
    let mut h = quick(PerthApp::default());
    double_click(&mut h, egui::pos2(960.0, ROW_A20));
    assert_eq!((h.state().selected, h.state().side), (Some(3), Some(mkperth_ui_egui::Side::Details)));
}

/// F5 presents: the header and the panes away, the activities alone; Esc ends it.
#[test]
fn presenting_shows_the_schedule_alone() {
    let mut h = harness(PerthApp::default());
    h.key_press(egui::Key::F5);
    h.run_steps(2);
    assert!(h.state().presenting);
    assert!(h.query_by_label("File").is_none() && h.query_by_label("Details").is_none(), "the chrome is away");
    assert!(h.query_by_label("Leave the presentation").is_some());
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert!(!h.state().presenting && h.query_by_label("File").is_some());
}
