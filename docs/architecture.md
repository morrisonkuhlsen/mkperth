# Architecture

A Cargo workspace; `cargo xtask layers` holds each crate to its layer.

| Layer | Crate | What it holds |
| --- | --- | --- |
| L0 | `mkperth-i18n` | message catalogs (`locales/*.ftl`, a subset of Fluent) and `tr!` |
| L0 | `mkperth-core` | `Date` (days from 1970, civil arithmetic), `Calendar`, `Project`, `Activity`, `Relationship` |
| L1 | `mkperth-sched` | `schedule(&Project) -> Result<Schedule, ScheduleError>`: the critical path method |
| L1 | `mkperth-p6xml` | Primavera P6's XML: `read` and `write` (calendars, WBS, activities, relationships) |
| L1 | `mkperth-perth` | Perth.jl's project JSON: `read` and `write`, lossless through `extra` |
| L2 | `mkperth-engine` | the session: the project, its schedule, `run(command, params)`; the sample project |
| L3 | `mkperth-ui-egui` | the Activities window: the table beside the Gantt chart |
| L3 | `mkperth-mcp` | *(next)* the MCP server |
| L4 | `apps/mkperth`, `apps/mkperth-cli`, `apps/mkperth-mcp` | the desktop app; the command line (schedule, run commands, explain); the MCP server for agents |

## The scheduler's time
Time runs in minutes from midnight on 1 January 1970 (`crates/sched/src/clock.rs`). Each
calendar has work periods for each day of the week (08:00–12:00 and 13:00–17:00 by default);
working time is those periods on its working days. An activity starts at a working minute, takes
its duration in working minutes (days at its calendar's hours a day, or minutes of its own) and
finishes at the end of its last; moments with no work between them are one moment for the
schedule. Lags count working minutes on the lag calendar. Floats are read off a running count of
each calendar's minutes per day over the schedule's span plus the part of a day. The
relationships' bounds are in `crates/sched/src/lib.rs`'s header.
