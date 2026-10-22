# Architecture

A Cargo workspace; `cargo xtask layers` holds each crate to its layer.

| Layer | Crate | What it holds |
| --- | --- | --- |
| L0 | `mkperth-i18n` | message catalogs (`locales/*.ftl`, a subset of Fluent) and `tr!` |
| L0 | `mkperth-core` | `Date` (days from 1970, civil arithmetic), `Calendar`, `Project`, `Activity`, `Relationship` |
| L1 | `mkperth-sched` | `schedule(&Project) -> Result<Schedule, ScheduleError>`: the critical path method |
| L1 | `mkperth-perth` | Perth.jl's project JSON: `read` and `write`, lossless through `extra` |
| L2 | `mkperth-engine` | the session: the project, its schedule, `run(command, params)`; the sample project |
| L3 | `mkperth-ui-egui` | the Activities window: the table beside the Gantt chart |
| L3 | `mkperth-mcp` | *(next)* the MCP server |
| L4 | `apps/mkperth` | the desktop app; the command line next |

## The scheduler's time
Time runs in calendar days from 1 January 1970 (`crates/sched/src/clock.rs`): a time `t` is the
morning of day `t` and the evening of day `t − 1`. Each activity works on its own calendar: it
starts on the morning of one of its working days, takes its duration in them, and finishes on
the evening of the last; its floats count them. Lags count working days of the lag calendar the
project chooses. Floats are read off a running count of each calendar's working days over the
schedule's span, built once per schedule. The relationships' bounds are in
`crates/sched/src/lib.rs`'s header.
