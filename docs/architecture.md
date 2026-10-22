# Architecture

A Cargo workspace; `cargo xtask layers` holds each crate to its layer.

| Layer | Crate | What it holds |
| --- | --- | --- |
| L0 | `mkperth-i18n` | message catalogs (`locales/*.ftl`, a subset of Fluent) and `tr!` |
| L0 | `mkperth-core` | `Date` (days from 1970, civil arithmetic), `Calendar`, `Project`, `Activity`, `Relationship` |
| L1 | `mkperth-sched` | `schedule(&Project) -> Result<Schedule, ScheduleError>`: the critical path method |
| L2 | `mkperth-engine` | *(next)* the session: the project, commands, undo |
| L3 | `mkperth-ui-egui`, `mkperth-mcp` | *(next)* the Activities window; the MCP server |
| L4 | `apps/*` | *(next)* the desktop app and the command line |

## The scheduler's time
Time runs in working days of the project's calendar from the morning of its first working day:
an activity of duration `d` starting at `t` works days `t … t + d − 1` and finishes the evening
of the last, at `t + d`. Dates are read off the calendar's working days once per schedule.
The relationships' bounds are in `crates/sched/src/lib.rs`'s header.
