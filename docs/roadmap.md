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
- **Next:** a calendar per activity and the lag's calendar; durations in hours with hours per
  day.

## M1 — data in and out
- **Next:** Perth.jl's project JSON, read and written without loss (`extra` for what Perth
  lacks), and its scheduler as the parity oracle for the networks both express; P6 XML from
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

## Later
- Progress: data date, actuals, retained logic and progress override, out-of-sequence progress.
- Resources and roles, resource levelling, histograms.
- Baselines, earned value, S-curves.
- Activity codes, user-defined fields, EPS and OBS, multiple projects.
- The network (logic) view; reports; the web build of the whole app.
