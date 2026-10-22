# Architecture

A Cargo workspace; `cargo xtask layers` holds each crate to its layer.

| Layer | Crate | What it holds |
| --- | --- | --- |
| L0 | `mkperth-i18n` | message catalogs (`locales/*.ftl`, a subset of Fluent) and `tr!` |
| L0 | `mkperth-core` | `Date` (days from 1970, civil arithmetic), `Calendar`, `Project`, `Activity`, `Relationship` |
| L1 | `mkperth-sched` | `schedule(&Project) -> Result<Schedule, ScheduleError>`: the critical path method |
| L1 | `mkperth-perth` | Perth.jl's project JSON: `read` and `write`, lossless through `extra` |
| L2 | `mkperth-engine` | *(next)* the session: the project, commands, undo |
| L3 | `mkperth-ui-egui`, `mkperth-mcp` | *(next)* the Activities window; the MCP server |
| L4 | `apps/*` | *(next)* the desktop app and the command line |

## The scheduler's time
Time runs in calendar days from 1 January 1970 (`crates/sched/src/clock.rs`): a time `t` is the
morning of day `t` and the evening of day `t − 1`. Each activity works on its own calendar: it
starts on the morning of one of its working days, takes its duration in them, and finishes on
the evening of the last; its floats count them. Lags count working days of the lag calendar the
project chooses. Floats are read off a running count of each calendar's working days over the
schedule's span, built once per schedule. The relationships' bounds are in
`crates/sched/src/lib.rs`'s header.
