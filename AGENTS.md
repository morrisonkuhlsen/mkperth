# MK Perth — instructions for agents

MK Perth is a clean-room, open-source, Rust-native project scheduler targeting Oracle Primavera P6 parity — the Activities window (activity table beside the Gantt chart), WBS, relationships of the four kinds with lags, constraints, calendars, resources and levelling, baselines, earned value, the XER and P6 XML files — and superiority: speed, openness, agent control, and every date with its calculation by hand (the forward and backward pass, step by step, down to the float). It runs natively on Linux, Windows and macOS; its scheduler also runs in the browser as WebAssembly. A Morrison Kühlsen project. Siblings with the same conventions: `../mksigma` (SPSS-class, the closest: same house, same layering) and `../mapcraft` (MK aeGIS, ArcGIS-class). `../julia/packages/Perth.jl` is the Julia package it grew from: its project format is one MK Perth reads and writes, and its scheduler is an oracle for the parity tests (see *Perth.jl*).

## Start every session here
1. Read [`docs/roadmap.md`](docs/roadmap.md): what is done, and the **Next:** items, which are the work list; then the relevant section of [`docs/architecture.md`](docs/architecture.md) and [`docs/scheduling-parity.md`](docs/scheduling-parity.md). Behaviour reference: Oracle's public P6 documentation (its ideas only), the scheduling literature, and open tools run as oracles (see *Clean-room*).
2. Work autonomously through the next item: build it, test it, look at it, commit it when the gates pass. Stop to ask only for what is the user's to decide: priorities and scope, brand and visual identity, anything published or shared (pushing, releases), and anything destructive.
3. **Issues.** The open GitHub issues (`gh issue list`) are work too. A problem found and not fixed in the same change becomes an issue at once (`gh issue create`), in English, following the templates in `.github/ISSUE_TEMPLATE/`: `[Bug Report] …` with *Summary*, *Steps to reproduce*, *Expected* and *Actual behavior*; `[Feature Request] …` with *Description*, *Proposal* and *Acceptance*; `[Decision] …` for what is the user's to decide; labelled by type (`bug`, `enhancement`) and priority (`priority: critical`, `high`, `medium`, `low`). Search the open issues first. A commit that fixes one says `Fixes #n`.

`plan/` is gitignored (local only): working notes.

## Assets and iconography — absolute rule
**No Oracle, Microsoft, Deltek, Asta, Bentley (or any other vendor's) iconography, images, artwork, fonts, templates, layouts, themes or sample projects — ever.** This is the most important rule in this repository; breaking it is a serious failure.
- Every asset is original work by MK Perth's contributors, or public domain / CC0, or licensed for redistribution, or one of Morrison Kühlsen's brand assets.
- Every asset file has a row in [`ATTRIBUTION.md`](ATTRIBUTION.md). `cargo xtask assets` enforces it.
- Prefer art generated in code: icons, the Gantt and network drawings, themes and the sample projects are drawn or defined in code.
- Never copy P6's icons, toolbar artwork, layouts (`.plf`), report templates, global data, sample projects (EPS, the demo databases) or wording beyond feature names. Feature *names* ("Activities window", "Total Float", "Retained Logic", "Level Resources", "XER") are fine.
- Screenshots of any vendor's software are never committed.

## Never crash
People trust MK Perth with their plans; a crash loses their work. **This outranks feature work.**
- **No panics in non-test code:** no `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!`, `unimplemented!`; no `unsafe` (`unsafe_code = "forbid"`).
- **Errors are `Result<T, E>`** through each crate's error type and `?`; their messages come from the catalogs (see Languages).
- **Input-derived numbers are hostile** (XER and XML files, Perth JSON, commands, MCP params): `get()` instead of indexing, checked/saturating arithmetic, caps on input-sized allocations (`MAX_ACTIVITIES`, `MAX_SPAN`).
- **Scheduling's own hostile inputs** become a catalogued error, never a hang: a loop of relationships (named), a calendar with no working day, a date past the calendar's range, a duration or lag beyond `MAX_SPAN`.
- **No recursion over the network**: topological order and both passes are iterative (a chain of 50 000 activities is a test).
- **Prove it:** every crash fix lands with a regression test that feeds the input that crashed.
- Crates carry `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]`; `cargo xtask panics` enforces it.

## Non-negotiables
- **Clean-room.** Oracle Primavera P6 is not run to shape MK Perth: observing it under a licence is the user's decision, recorded with the licence used and legal sign-off before it happens. Behaviour comes from public documentation — its ideas only, never its text, tables, examples or screenshots — the scheduling literature (the precedence diagramming method, resource levelling, earned value: textbooks, papers, PMI's standards), published worked examples, and open tools run as oracles (Perth.jl; others named in the commit). Never read, disassemble or copy anything inside a vendor's installation, never commit files a vendor's product produced (XER or XML exports from P6, its demo projects), never copy GPL/AGPL/LGPL code (ProjectLibre, GanttProject, TaskJuggler, PyP6Xer…): reading their documentation is allowed, their code is not. A commit names the sources it was built from.
- **File formats** come from public descriptions: P6 XML from the schema Oracle publishes; XER has no specification from Oracle, so reading it is the user's decision, like any format known only through third parties; Perth JSON from Perth.jl (the user's own); Microsoft Project XML from Microsoft's published schema.
- **Dependencies** are permissively licensed (MIT, Apache-2.0, BSD, Zlib or similar) and, below the UI layer, build for wasm.
- **Numerical parity is a feature.** Every scheduling rule lands with networks scheduled by hand in its tests (each date and float worked out in the comments) and, where Perth.jl schedules the same network, its results re-typed as constants.
- **Every date has its calculation.** A scheduled activity's dates and floats can be shown step by step (the Σ of MK Sigma): the relationships that drive it, the pass, the calendar's days.
- **Everything goes through the engine.** User-visible behaviour is a command run by the engine; menus, dialogs, the CLI and the MCP server reach the same commands. Programmatic calls never open dialogs.
- **Layering** is enforced by `cargo xtask layers`: `i18n`/`core` (L0) → `sched` and the formats (L1) → `engine` (L2) → `ui-egui`/`mcp` (L3) → `apps/*` (L4). Nothing below L3 depends on egui/eframe/winit/rfd. **The UI crate is swappable.**
- **Languages.** Every user-visible text comes from the catalogs in `crates/i18n/locales/*.ftl` through `tr!`, in English and Português (Brasil); their tests fail on a missing message or placeholder.
- **Rust only. Never break wasm** (`cargo xtask wasm`).
- **CHANGELOG.** Anything a user would notice gets a line in [`CHANGELOG.md`](CHANGELOG.md) under `## [Unreleased]`; a change that gives different dates or floats for the same project goes under **Changed results**, with what was computed before and what is now.
- **Quality gates** before every commit: `cargo xtask ci` (fmt, clippy -D warnings, tests, assets, layers, panics, wasm). Run it as `cargo xtask ci && git commit …`: a pipe after it hides its failure. One change per commit, its message saying what and why. Push only when the user asks.

## Perth.jl
The Julia package (`../julia/packages/Perth.jl`, MIT, the user's own) is a sibling, never a dependency: no FFI, no Julia at run time.
- **Format:** its project JSON is a first-class format here, read and written without loss; what only MK Perth's model has travels in a preserved `extra` field.
- **Oracle:** networks both can express (finish-to-start, start-to-start and finish-to-finish with lags, business-day calendars, PERT) are scheduled by Perth.jl and its dates re-typed as test constants; a disagreement is an issue in one repository or the other.
- **Bridge:** Perth.jl reaches MK Perth through `mkperth-cli` (plain functions in Perth.jl's `src/mkperth.jl` running a subprocess, no dependency): `import_p6xml`, `export_p6xml`, `schedule_p6`. The CLI's verbs, its `--format json` keys and its exit codes are a contract: change them only with the bridge.

## Roadmap
[`docs/roadmap.md`](docs/roadmap.md) is the plan and the progress log; [`docs/scheduling-parity.md`](docs/scheduling-parity.md) lists P6's scheduling options and where each stands. Update them whenever work lands.

## Credits
- MK Perth is a Morrison Kühlsen project.
- Commits made by an agent end with the attribution lines its session gives.
- Never add, edit, guess or copy anyone's name or credit.
