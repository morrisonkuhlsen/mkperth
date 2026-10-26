//! MK Perth's command line: schedules, changes and explains projects from scripts and CI.
//!
//! - `mkperth-cli schedule [--in project.json|.xml|.xer] [--out file] [--format table|json|csv]`
//! - `mkperth-cli run [--in …] --cmd 'activity.add={"name": "Survey"}' [--cmd …] [--out file] [--format …]`
//! - `mkperth-cli explain [--in …] --activity A100 [--tex calculation.tex]`
//!
//! Without `--in`, the sample project. Every command goes through the engine's `Session`; a file
//! is written only when everything before it succeeded, all or nothing. Exit codes: 0 done, 1 an
//! error (said on stderr), 2 a usage mistake. Messages follow the locale (`LC_ALL`,
//! `LC_MESSAGES`, `LANG`).
//!
//! The verbs, `--format json`'s keys, the CSV's header and the exit codes are a contract
//! (`docs/cli.md`): Perth.jl's bridge runs this binary. `tests/cli.rs` holds it.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::path::Path;
use std::process::ExitCode;

use mkperth_engine::Session;
use mkperth_engine::i18n::{self, tr};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Table,
    Json,
    Csv,
}

#[derive(Debug, Default)]
struct Args {
    verb: String,
    input: Option<String>,
    output: Option<String>,
    format: Option<Format>,
    commands: Vec<String>,
    activity: Option<String>,
    tex: Option<String>,
}

/// The arguments, or `None` for a usage mistake.
fn parse(args: &[String]) -> Option<Args> {
    let mut it = args.iter();
    let verb = it.next()?.clone();
    if !["schedule", "run", "explain"].contains(&verb.as_str()) {
        return None;
    }
    let mut a = Args { verb, ..Args::default() };
    while let Some(arg) = it.next() {
        let mut value = || it.next().cloned();
        match arg.as_str() {
            "--in" => a.input = Some(value()?),
            "--out" => a.output = Some(value()?),
            "--format" => {
                a.format = Some(match value()?.as_str() {
                    "table" => Format::Table,
                    "json" => Format::Json,
                    "csv" => Format::Csv,
                    _ => return None,
                })
            }
            "--cmd" => a.commands.push(value()?),
            "--activity" => a.activity = Some(value()?),
            "--tex" => a.tex = Some(value()?),
            _ => return None,
        }
    }
    // What each verb takes.
    let fits = match a.verb.as_str() {
        "schedule" => a.commands.is_empty() && a.activity.is_none() && a.tex.is_none(),
        "run" => !a.commands.is_empty() && a.activity.is_none() && a.tex.is_none(),
        _ => a.activity.is_some() && a.commands.is_empty() && a.output.is_none() && a.format.is_none(),
    };
    fits.then_some(a)
}

/// A `--cmd` as the command's id and its parameters: `id` alone, or `id={json}`.
fn command(text: &str) -> Result<(String, Value), String> {
    match text.split_once('=') {
        None => Ok((text.trim().to_string(), Value::Null)),
        Some((id, params)) => {
            let value = serde_json::from_str(params).map_err(|e| tr!("cli-bad-params", command = id.trim(), detail = e.to_string().as_str()))?;
            Ok((id.trim().to_string(), value))
        }
    }
}

fn run(a: &Args) -> Result<String, String> {
    let mut session = Session::default();
    if let Some(path) = &a.input {
        session.open(Path::new(path)).map_err(|e| e.to_string())?;
    }
    for w in &session.warnings {
        eprintln!("{}", tr!("cli-warning", text = w.as_str()));
    }
    for c in &a.commands {
        let (id, params) = command(c)?;
        session.run(&id, &params).map_err(|e| e.to_string())?;
    }
    if a.verb == "explain" {
        let id = a.activity.as_deref().unwrap_or_default();
        let explanation = session.explain(id).map_err(|e| e.to_string())?;
        if let Some(path) = &a.tex {
            session.export_latex(id, Path::new(path), &|d| d.to_string()).map_err(|e| e.to_string())?;
        }
        return Ok(explanation.lines(&|d| d.to_string()).iter().map(|l| format!("{l}\n")).collect());
    }
    // A project that doesn't schedule is said, and nothing is written.
    if let Err(e) = &session.schedule {
        return Err(e.to_string());
    }
    let text = match a.format.unwrap_or(Format::Table) {
        Format::Table => session.schedule_table(&|d| d.to_string()),
        Format::Json => session.schedule_json().map(|v| format!("{v:#}\n")),
        Format::Csv => session.schedule_csv(),
    }
    .map_err(|e| e.to_string())?;
    if let Some(path) = &a.output {
        session.save(Path::new(path)).map_err(|e| e.to_string())?;
        for w in &session.warnings {
            eprintln!("{}", tr!("cli-warning", text = w.as_str()));
        }
    }
    Ok(text)
}

fn main() -> ExitCode {
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|v| std::env::var(v).ok().filter(|s| !s.is_empty())).unwrap_or_default();
    i18n::set(i18n::Lang::from_locale(&locale));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(a) = parse(&args) else {
        eprintln!("{}", tr!("cli-usage"));
        return ExitCode::from(2);
    };
    match run(&a) {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{}", tr!("cli-error", text = e.as_str()));
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Option<Args> {
        parse(&list.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn each_verb_takes_its_own_options() {
        assert!(args(&["schedule", "--format", "csv"]).is_some());
        assert!(args(&["schedule", "--format", "xml"]).is_none());
        assert!(args(&["schedule", "--cmd", "edit.undo"]).is_none());
        assert!(args(&["run", "--in", "p.json"]).is_none(), "run needs a command");
        assert!(args(&["explain", "--in", "p.json"]).is_none(), "explain needs an activity");
        assert!(args(&["explain", "--activity", "A10", "--out", "x.json"]).is_none());
        assert!(args(&["schedule", "--in"]).is_none());
        assert!(args(&["frobnicate"]).is_none());
        assert!(args(&[]).is_none());
    }

    #[test]
    fn a_command_with_and_without_params() {
        assert_eq!(command("edit.undo"), Ok(("edit.undo".into(), Value::Null)));
        assert_eq!(command("activity.add={\"name\": \"a=b\"}").map(|c| c.1["name"].clone()), Ok(Value::from("a=b")));
        assert!(command("activity.add={nope").is_err());
    }
}
