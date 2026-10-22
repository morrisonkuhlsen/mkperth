# MK Perth

Project scheduling, open and auditable: every date with its calculation.

MK Perth is a clean-room, open-source project scheduler in pure Rust, aiming at the
capabilities of enterprise schedulers — activities and WBS, the four kinds of relationship
with lags, constraints, calendars, resources and levelling, baselines and earned value — with
every date shown with the pass that produced it. A Morrison Kühlsen project.

**Status:** early. The scheduler (critical path method with FS, SS, FF and SF relationships,
lags and leads, a work calendar with holidays, total and free float) is in place and tested;
the file formats, the engine's commands, the command line and the interface come next. See
[`docs/roadmap.md`](docs/roadmap.md).

```sh
cargo xtask ci     # every quality gate
cargo test         # the tests alone
```

Licensed under MIT or Apache-2.0, at your option.
