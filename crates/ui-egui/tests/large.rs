//! A project of 10 000 activities: how long a frame takes while the table scrolls. Run in
//! release: `cargo test --release -p mkperth-ui-egui --test large -- --ignored --nocapture`.

use egui_kittest::Harness;
use mkperth_engine::core::{Activity, Project, Relationship, RelationshipType};
use mkperth_ui_egui::PerthApp;

fn large(n: usize) -> Project {
    let mut p = mkperth_engine::Session::default().project;
    p.activities.clear();
    p.relationships.clear();
    // A hundred WBS summaries of a hundred activities each, every activity after the one before
    // and, every tenth, after one a hundred back.
    for w in 0..n / 100 {
        p.activities.push(Activity { id: format!("W{w}"), name: format!("Area {w}"), ..Activity::default() });
        for k in 0..99 {
            let id = format!("A{w}-{k}");
            p.activities.push(Activity {
                id: id.clone(),
                name: format!("Work {w}.{k}"),
                duration: 1 + (k as i64 % 5),
                parent: Some(format!("W{w}")),
                ..Activity::default()
            });
            if k > 0 {
                p.relationships.push(Relationship {
                    predecessor: format!("A{w}-{}", k - 1),
                    successor: id.clone(),
                    kind: RelationshipType::FinishToStart,
                    lag: 0,
                    lag_minutes: None,
                });
            }
            if w > 0 && k % 10 == 0 {
                p.relationships.push(Relationship {
                    predecessor: format!("A{}-{k}", w - 1),
                    successor: id,
                    kind: RelationshipType::FinishToStart,
                    lag: 0,
                    lag_minutes: None,
                });
            }
        }
    }
    p
}

#[test]
#[ignore = "a timing, for release builds"]
fn ten_thousand_activities_scroll() {
    let mut app = PerthApp::default();
    app.session.project = large(10_000);
    let t = std::time::Instant::now();
    app.session.reschedule();
    println!("schedule: {:?}", t.elapsed());
    assert!(app.session.schedule.is_ok());
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 900.0)).build_ui_state(|ui, app: &mut PerthApp| app.ui(ui), app);
    h.run_steps(3);
    let t = std::time::Instant::now();
    let frames = 60;
    for _ in 0..frames {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, -20.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        h.step();
    }
    let per = t.elapsed() / frames;
    println!("frame while scrolling: {per:?}");
}
