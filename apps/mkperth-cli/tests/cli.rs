//! The binary run on files built here.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mkperth-cli-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A Perth.jl project: a(3) then b(2), from Tuesday 1 September 2026, every day worked.
fn project(at: &Path) -> PathBuf {
    let path = at.join("p.json");
    std::fs::write(
        &path,
        r#"{"name": "Tiny", "tasks": [{"id": "a", "name": "Dig, then pour", "start": "2026-09-01", "duration": 3}, {"id": "b", "name": "Build", "duration": 2, "dependencies": ["a"]}], "calendar": ""}"#,
    )
    .unwrap();
    path
}

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mkperth-cli")).args(args).env("LC_ALL", "C").env_remove("LANG").output().unwrap()
}

fn text(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn schedule_as_json_csv_and_a_table() {
    let d = dir("schedule");
    let p = project(&d);
    let p = p.to_str().unwrap();
    let o = cli(&["schedule", "--in", p, "--format", "json"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let v: serde_json::Value = serde_json::from_str(&text(&o)).unwrap();
    assert_eq!(v["finish"], "2026-09-05");
    assert_eq!(v["activities"][1]["early_start"], "2026-09-04");
    assert_eq!(v["activities"][1]["critical"], true);
    let csv = text(&cli(&["schedule", "--in", p, "--format", "csv"]));
    assert!(csv.starts_with("id,name,early_start"));
    assert!(csv.contains("a,\"Dig, then pour\",2026-09-01,2026-09-03"), "{csv}");
    let table = text(&cli(&["schedule", "--in", p]));
    assert!(table.contains("Early start") && table.contains("*b") && table.contains("Finishes 2026-09-05"), "{table}");
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn commands_change_the_project_and_out_saves_it() {
    let d = dir("run");
    let p = project(&d);
    let out = d.join("changed.xml");
    let o = cli(&[
        "run",
        "--in",
        p.to_str().unwrap(),
        "--cmd",
        r#"activity.add={"after": "b", "name": "Paint", "duration": 1}"#,
        "--cmd",
        r#"relationship.add={"predecessor": "b", "successor": "A10"}"#,
        "--out",
        out.to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let v: serde_json::Value = serde_json::from_str(&text(&o)).unwrap();
    assert_eq!(v["activities"][2]["name"], "Paint");
    assert_eq!(v["finish"], "2026-09-06");
    // Written as P6 XML, and it reads back.
    let back = cli(&["schedule", "--in", out.to_str().unwrap(), "--format", "csv"]);
    assert!(text(&back).contains("Paint"), "{}", String::from_utf8_lossy(&back.stderr));
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn a_failure_leaves_the_good_file_alone() {
    let d = dir("fail");
    let p = project(&d);
    let out = d.join("keep.json");
    std::fs::write(&out, "good").unwrap();
    // A loop: b already follows a.
    let o = cli(&[
        "run",
        "--in",
        p.to_str().unwrap(),
        "--cmd",
        r#"relationship.add={"predecessor": "b", "successor": "a"}"#,
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).starts_with("error: "));
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "good");
    // A file that isn't there; JSON that isn't.
    assert_eq!(cli(&["schedule", "--in", d.join("nope.json").to_str().unwrap()]).status.code(), Some(1));
    assert_eq!(cli(&["run", "--cmd", "activity.add={"]).status.code(), Some(1));
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn explain_says_the_calculation_and_writes_latex() {
    let d = dir("explain");
    let p = project(&d);
    let tex = d.join("b.tex");
    let o = cli(&["explain", "--in", p.to_str().unwrap(), "--activity", "b", "--tex", tex.to_str().unwrap()]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(text(&o).contains("Forward pass"), "{}", text(&o));
    assert!(std::fs::read_to_string(&tex).unwrap().contains("\\documentclass"));
    assert_eq!(cli(&["explain", "--in", p.to_str().unwrap(), "--activity", "zz"]).status.code(), Some(1));
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn usage_mistakes_exit_2_in_the_locale() {
    let o = cli(&["schedule", "--format", "xml"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).starts_with("usage:"));
    let pt = Command::new(env!("CARGO_BIN_EXE_mkperth-cli")).env("LC_ALL", "pt_BR.UTF-8").output().unwrap();
    assert!(String::from_utf8_lossy(&pt.stderr).starts_with("uso:"));
    // Without a file, the sample.
    assert!(cli(&["schedule", "--format", "csv"]).status.success());
}
