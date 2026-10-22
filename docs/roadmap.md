# Roadmap

MK Perth's plan and progress log. Each section says what is done and its **Next:** items, the
work list. Parity reference: Oracle Primavera P6, from its public documentation (ideas only)
and the scheduling literature; see [`scheduling-parity.md`](scheduling-parity.md).

## M0 — the scheduler (in progress)
- *Done:* civil dates without dependencies (`mkperth-core`, every day of four centuries round
  trips); a work calendar (worked weekdays, holidays, extra working days; a calendar that works
  no day is an error, not a hang); the critical path method (`mkperth-sched`): FS, SS, FF and SF
  with lags and leads, the forward and backward pass, total and free float, critical when the
  total float is 0 or less, the project's finish; loops named, duplicate and unknown IDs,
  negative durations and spans past `MAX_SPAN` as catalogued errors; iterative throughout (a
  chain of 50 000 activities). Messages in English and Brazilian Portuguese.
  Start and finish milestones: no duration, a start milestone shown on the morning of its day,
  a finish milestone on the evening of the working day before.
  Constraints: start or finish on, on or after, on or before; mandatory start and finish (they
  override the relationships, so predecessors can get negative float); as late as possible (by
  the free float); the date a project must finish by.
  A calendar per activity, and the calendar lags count on (the predecessor's by default, the
  successor's, 24 hours or the project's): the scheduler's clock runs in calendar days and each
  activity snaps to its own working days.
- **Next:** durations in hours with hours per day (#2); float and critical options (#3); every
  date with its calculation (#5).

## M1 — data in and out
- *Done:* Perth.jl's project JSON (`mkperth-perth`), read and written back byte for byte: a
  task's start as a start on or after, its deadline as a secondary finish on or before, its
  dependency strings as relationships, milestones, the calendar (empty for calendar days, a
  business-day calendar's name); every other field, the task's and the project's, carried in
  `extra` and written back in the file's order. Hostile files are catalogued errors.
  Perth.jl as the parity oracle: 25 networks (ten drawn by hand, fifteen random from a fixed
  seed) with finish-to-start, start-to-start and finish-to-finish ties, lags and leads, own
  starts and deadlines, scheduled by Perth.jl 0.16.1 and re-typed as constants; MK Perth gives
  every early and late date and every total float the same (`crates/perth/tests/oracle.rs`).
  The WBS: each activity's parent; a summary (an activity with children) takes its dates from
  them, holds no constraint or duration of its own, and relationships that reach it are left
  out and listed; unknown parents, loops of parents and milestones with children are errors.
  Ten more oracle networks with summaries agree with Perth.jl's leaf view.
  Holidays by region, computed from their rules: Brazil's national holidays (the federal laws'
  fixed dates; Carnival, Good Friday and Corpus Christi from Easter by the Gregorian computus),
  the same weekdays as BusinessDays.jl's `Brazil` for 2020–2030; Perth projects on `Brazil` read
  with them, and eight oracle networks across those holidays agree with Perth.jl.
- **Next:** P6 XML from
  Oracle's published schema; Microsoft Project XML. XER is the user's decision (no specification
  from Oracle).

## M2 — the engine and the command line
- **Next:** `mkperth-engine`: the project as the session's state, every change a command (ids
  after P6's menus: `project.schedule`, `activity.add`, `relationship.add`, `baseline.assign`),
  undo; `mkperth-cli` (schedule a file, print or write the result); `mkperth-mcp`.

## M3 — the Activities window
- **Next:** the activity table beside the Gantt chart in egui, grouped by WBS; the details
  below (General, Status, Relationships); F9 schedules; every date's calculation by hand beside
  it; English and Brazilian Portuguese.

## M4 — resources
- **Next:** resources and roles with units per time and prices that change over time; assignments
  to activities; resource-dependent activities; levelling with priorities; histograms.

## M5 — costs and earned value
- **Next:** each activity's budgeted, actual, remaining and at-completion costs, from its
  resources' prices and from expenses (accrued at the start, the end or over the activity); cost
  accounts as a hierarchy; the project's and the WBS's budget (original and current, the log of
  its changes, the spending plan per period, funding sources); earned value (planned value,
  earned value, actual cost; CPI, SPI, EAC, ETC) with the usual percent-complete methods;
  financial periods that store each period's actuals; currencies. Perth.jl's task costs and
  S-curve are the oracle for what it covers.

## Beyond P6 — billing (the user's to schedule)
P6 has no billing: measurements and invoices are left to other products. MK Perth could close
the loop: progress measurements per activity from percent complete and budgeted cost, the
measurement statement per period, retention and price adjustment, as construction contracts
measure work. A module of its own, after costs reach parity.

## Later
- Progress: data date, actuals, retained logic and progress override, out-of-sequence progress.
- Resources and roles, resource levelling, histograms.
- Baselines, earned value, S-curves.
- Activity codes, user-defined fields, EPS and OBS, multiple projects.
- The network (logic) view; reports; the web build of the whole app.
