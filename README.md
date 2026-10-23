<h1 align="center">MK Perth</h1>

<p align="center">
  <b>Project scheduling, open and auditable: an open-source, clean-room project scheduler in pure Rust.</b>
</p>

<p align="center">
  Activities and WBS, the four kinds of relationship with lags, constraints, calendars,
  resources and levelling, baselines and earned value — the capabilities of enterprise
  schedulers such as Primavera P6, with every date shown with the pass that produced it. It runs
  natively on Linux, Windows and macOS, and its scheduler builds for WebAssembly.
</p>

<p align="center">
  <a href="https://github.com/morrisonkuhlsen/mkperth/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/morrisonkuhlsen/mkperth/ci.yml?branch=main&style=flat-square&label=CI"></a>
  <img alt="Written in Rust" src="https://img.shields.io/badge/written%20in-Rust-b7410e?style=flat-square&logo=rust&logoColor=white">
  <img alt="Runs on Linux, Windows, macOS and the web" src="https://img.shields.io/badge/runs%20on-Linux%20%C2%B7%20Windows%20%C2%B7%20macOS%20%C2%B7%20Web-14161c?style=flat-square">
  <img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-14161c?style=flat-square">
  <br>
  <img alt="Focus: project scheduling" src="https://img.shields.io/badge/focus-project%20scheduling-14161c?style=flat-square">
  <img alt="Dates checked against Perth.jl" src="https://img.shields.io/badge/checked%20against-Perth.jl-9558b2?style=flat-square">
  <img alt="Clean-room implementation" src="https://img.shields.io/badge/clean--room-no%20GPL%20code-14161c?style=flat-square">
  <img alt="Builds for WebAssembly" src="https://img.shields.io/badge/wasm-ready-654ff0?style=flat-square&logo=webassembly&logoColor=white">
  <img alt="No panics, no unsafe" src="https://img.shields.io/badge/never%20crash-no%20panics%20%C2%B7%20no%20unsafe-14161c?style=flat-square">
  <img alt="Status: early development, version 0.1.0" src="https://img.shields.io/badge/status-early%20%C2%B7%200.1.0-8a94a6?style=flat-square">
</p>

<br>

<p align="center">
  <a href="#why-mk-perth">Why MK Perth</a> ·
  <a href="#what-it-does-today">What it does today</a> ·
  <a href="#roadmap">Roadmap</a> ·
  <a href="#quick-start">Quick start</a> ·
  <a href="#perthjl">Perth.jl</a> ·
  <a href="#architecture">Architecture</a> ·
  <a href="#contributing">Contributing</a> ·
  <a href="#license-and-credits">License and credits</a>
</p>

## Why MK Perth

- **Every date with its calculation.** A schedule is only as good as the trust in its dates.
  MK Perth's scheduler is small, documented rule by rule, and each rule lands with networks
  scheduled by hand; the step-by-step explanation of any activity's dates is on the roadmap.
- **Parity measured, not claimed.** The scheduling options of enterprise tools are listed one by
  one in [`docs/scheduling-parity.md`](docs/scheduling-parity.md), each marked done or next.
  Where Perth.jl schedules the same network, its dates are the reference.
- **Clean-room and open.** Behaviour comes from the scheduling literature and public
  documentation, never from another product's code or files. Licensed MIT OR Apache-2.0.
- **Never crash.** No panics and no `unsafe` in non-test code. Loops of relationships, calendars
  with no working day, absurd durations and hostile files are catalogued errors.

## What it does today

- **The critical path method:** finish-to-start, start-to-start, finish-to-finish and
  start-to-finish relationships with lags and leads; the forward and backward pass; total and
  free float; critical activities.
- **Calendars:** worked weekdays, holidays and exceptions; a calendar per activity; the calendar
  lags count on.
- **Milestones, constraints and the WBS:** start and finish milestones; start or finish on, on or
  after, on or before, mandatory, as late as possible, a secondary constraint, a must-finish-by
  date; summaries that roll up their children.
- **Files:** Perth.jl's projects, read and written back byte for byte; Primavera P6 XML, read and
  written (WBS, activities, relationships, constraints, calendars).

- **A first window:** the activity table beside the Gantt chart, early and late dates, floats and
  the critical path, for the sample project or any Perth.jl project.

## Roadmap

| Milestone | What | Issues |
|-----------|------|--------|
| M0 the scheduler | durations in hours, float options, every date explained | #2, #3, #4, #5 |
| M1 data in and out | holidays of business calendars, P6 XML, Microsoft Project XML, XER (a decision) | #29, #8, #9, #10 |
| M2 the engine | commands and undo, the command line, the MCP server, the Perth.jl bridge | #12, #13, #14, #15 |
| M3 the interface | the Activities window (table and Gantt), the network view, the brand | #16, #17, #18 |
| M4 resources | resources and roles, levelling | #20, #21 |
| M5 costs | costs, budgets, earned value | #23, #24, #25 |
| Beyond | progress, baselines, codes and EPS, billing, the web app | #19, #22, #26, #27, #28 |

The plan and the progress log: [`docs/roadmap.md`](docs/roadmap.md).

## Quick start

```sh
cargo run --release -p mkperth                         # the window, with the sample project
cargo run --release -p mkperth -- project.json         # a Perth.jl project
cargo run -p mkperth-cli -- schedule --in project.json --format csv     # the schedule for scripts
cargo run -p mkperth-cli -- run --in p.xml --cmd 'activity.add={"name": "Survey"}' --out p.xml
cargo run -p mkperth-cli -- explain --in project.json --activity A100 --tex a100.tex
claude mcp add mkperth -- /path/to/mkperth-mcp        # agents open, schedule, explain and edit projects
cargo xtask ci                                         # fmt, clippy, tests, assets, layers, panics, wasm
```

Offscreen screenshots, with no window and nothing captured from the desktop:

```sh
cargo run -p mkperth-ui-egui --example snapshot -- [--in project.json] [--dark] [--lang pt-BR] out.png
```

The wasm gate needs the target once: `rustup target add wasm32-unknown-unknown`.

## Perth.jl

[Perth.jl](https://github.com/dantebertuzzi/Perth.jl) is the Julia package MK Perth grew from. It
is a sibling, never a dependency: MK Perth reads and writes its project JSON without loss, and its
scheduler is the oracle for the networks both express — 43 of them, ten drawn by hand and the
rest random, eight on the Brazil calendar, agree on every date and float (`crates/perth/tests/oracle.rs`).

## Architecture

An engine-first Cargo workspace with enforced layering (`cargo xtask layers`):

| Layer | Crates |
|-------|--------|
| L0    | `i18n` (message catalogs) · `core` (dates, calendars, the project) |
| L1    | `sched` (the critical path method) · `perth` (Perth.jl's JSON) |
| L2    | `engine` (the session and its commands) |
| L3    | `ui-egui` (the Activities window) · `mcp` *(next)* |
| L4    | `apps/mkperth` |

More in [`docs/architecture.md`](docs/architecture.md).

## Contributing

- **Start here:** [`CONTRIBUTING.md`](CONTRIBUTING.md); the full rules for people and agents
  (clean-room, asset policy, never-crash, quality gates) are in [`AGENTS.md`](AGENTS.md)
- **Where things stand:** [`docs/roadmap.md`](docs/roadmap.md); what changed: [`CHANGELOG.md`](CHANGELOG.md)
- **Issues:** bugs and feature requests on GitHub, with `priority:` labels; security problems
  privately, as [`SECURITY.md`](SECURITY.md) says

## License and credits

MK Perth is dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
All artwork is original or openly licensed (see [`ATTRIBUTION.md`](ATTRIBUTION.md)), as
[`NOTICE`](NOTICE) says. Primavera P6 is a trademark of Oracle; MK Perth is not affiliated with it.

A Morrison Kühlsen project.
