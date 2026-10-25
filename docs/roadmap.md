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
  Float and critical options: total float as finish float, start float or the smaller; critical
  at a total-float threshold or on the longest path (the driving relationships back from the
  activities that finish the project), the longest path marked on every activity either way.
  Time in minutes: calendars with work periods per weekday (a short Friday), durations and lags
  in working minutes for part days and P6's hours, floats in minutes shown in days with decimals;
  every earlier test and oracle network keeps its dates.
  Progress: a data date before which nothing not started and no work left is scheduled; actual
  start and finish dates (a completed activity keeps them, with no float, never critical); the
  work left of an activity under way (told, or its duration less the work to the data date),
  after its predecessors with retained logic or from the data date with progress override; its
  start-to-start successors count from its actual start; start constraints drop once it starts.
  Read and written as P6 XML's `DataDate`, `ActualStartDate`, `ActualFinishDate` and
  `RemainingDuration`; told in Details and on the Project tab; drawn green on the Gantt chart.
  Constraint pairs: a secondary needs a dated primary and bounds the other end, neither
  mandatory nor as late as possible; other pairs are refused with a catalogued message. An
  expected finish sets the work, or the work left, from the start to that evening (P6 XML's
  `ExpectedFinishDate`).
  Every date with its calculation (#5): the bounds of each pass, the one that wins, the calendar's
  placing, the floats; held against the schedule for every hand network, and exported as a
  LaTeX document (ASCII, accents as TeX's commands) that pdflatex compiles in both languages
  (`activity.export-tex`, the Calculation pane's LaTeX button).
  An exception day works periods of its own (a short Christmas Eve, a Saturday morning), read
  and written as P6 XML's `HolidayOrException` work times.
  Calendars edited in the app: the Calendars pane (the strip, or Project > Calendars…) sets a
  calendar's working week, work periods, region and days off or worked, through `calendar.edit`.
- **Next:** calendars added, renamed and deleted; an activity's calendar chosen in Details.

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
  P6 XML (`mkperth-p6xml`): the first project's WBS (as summaries), activities, relationships of
  the four kinds, constraints and calendars (work weeks, holidays, worked exceptions) read and
  written; durations and lags in hours become working days by each calendar's hours per day,
  rounded up with a warning; a region's holidays written as ten years of dates; what isn't read is
  named once in the warnings; Open and Save choose the format by the extension. To check against
  Oracle's XSD: #31.
- **Next:** Microsoft Project XML (#9). XER is the user's decision (#10: no specification from
  Oracle).

## M2 — the engine and the command line
- *Started:* `mkperth-engine`: the session (the project, its schedule, what opening it said) and
  `Session::run` with the first commands, `project.open`, `project.sample` and
  `project.schedule`; editing (`activity.add`, `activity.edit`, `activity.delete`,
  `relationship.add`, `relationship.delete`) with undo and redo (`edit.undo`, `edit.redo`, two
  hundred deep), each change made on a copy and refused if it makes a loop or repeats an ID;
  every command fuzzed with empty and hostile parameters; `project.save`; `project.new` (an
  empty project from a given start; File > New, Ctrl+N); `calendar.edit` (a calendar's week,
  work periods, holidays, worked days and region, undoable; #12); a command that panics is
  caught and the session put back; the sample project built in code (a small building on Brazil's calendar,
  with a WBS, every kind of relationship, a lag and a lead, milestones, a constraint and a
  seven-day cure).
  `mkperth-cli` (#13): `schedule` prints the schedule as a table in the person's language, JSON
  or CSV; `run` applies commands (`--cmd 'id=JSON'`) and `--out` saves the project, all or
  nothing and only when every command held; `explain` says an activity's calculation and writes
  it as LaTeX; exit codes 0, 1 and 2; integration tests run the binary on files built in code.
  `mkperth-mcp` (#14): a Model Context Protocol server over stdio with six tools (open_project,
  get_schedule, run_command with every engine command, explain_activity, save_project,
  export_latex); JSON-RPC's error codes, batches, lines that aren't UTF-8 or JSON or are too
  long answered and survived; a test drives the binary end to end.
  The Perth.jl bridge (#15), in Perth.jl: `import_p6xml`, `export_p6xml` and `schedule_p6`
  run `mkperth-cli` (on PATH or `ENV["MKPERTH_CLI"]`), no FFI and no dependency; Perth.jl's
  tests hold MK Perth's dates and float against its own `schedule!` and `slack`.
- **Next:** the CLI's contract (verbs, `--format json` keys, exit codes) kept stable for the
  bridge; resources (M4).
- *Done (#22):* baselines: `baseline.assign` freezes each activity's dates and duration under
  a name (up to 50), `baseline.use` picks the one compared with, `baseline.delete`; the
  baseline's bars under the schedule's and the BL start, BL finish and start, finish and
  duration variance columns (P6's sign: baseline less schedule, in working days); in Perth's
  JSON (its own `baseline_start` and `baseline_duration` read as a baseline) and as P6 XML's
  `BaselineProject` with the project's current baseline.

## M3 — the Activities window
- *Started:* a quiet header as in an online spreadsheet (the mark, the project's name and
  whether it is saved, the File, Edit, Project and View menus under it, search, theme and the
  Schedule button) over one slim toolbar of small icons (undo and redo, add and delete, what the
  chart shows, the time scale, the data date), the rail of views (with the activity table's
  switch), a splash screen in MK Perth's lilac (#18), the File backstage (start, open, recent
  projects, info, save, export), comments on activities in threads (Details' Comments tab, a
  mark on commented rows), the Calculation pane under the activities (the
  selected activity's dates step by step, #5) with a Schedule button, the Details pane on the
  right (facts, constraints, dates and floats, predecessors and successors), the Filters strip
  (find by ID or name, critical only, late bars) and the status bar with the zoom; Save and Save
  As write Perth JSON atomically. The first window: the activity table (ID, name,
  duration, early and late start and finish, total and free float) beside the Gantt chart
  (months and weeks, non-working days shaded, early bars red on the critical path, late bars
  beneath, the float as a line, summaries as brackets, milestones as diamonds), in WBS order,
  scrolling together; Open (Ctrl+O), Sample, Schedule (F9); a status line; English and
  Brazilian Portuguese, light and dark; an offscreen snapshot example and kittest tests.
  The Activities window (#16): Details in tabs (General, Status, Relationships, Notes), notes per
  activity, the columns chosen in Filters, today and the data date on the chart, progress drawn
  green; the calculation exported as LaTeX. 10 000 activities scroll at about 3 ms a frame
  (`crates/ui-egui/tests/large.rs`, run in release).
  Chart notes, as Perth.jl draws them: bands shaded behind the bars with their names along their
  edge, markers as dashed lines with their names, months painted in the ruler; read and written
  in Perth.jl's JSON, made and deleted by command (`band.add`, `marker.add`, `month-mark.add`,
  and their `.delete`) with undo, and in the Chart notes pane.
  Clicking and dragging: rows chosen by a dragged rectangle, Ctrl and Shift; double clicks on
  the chart and ruler add bands and marked months where they land, or open a bar's details; a
  row's context menu.
- **Next:** the network view (#17); the brand (#18, the user's: the splash's lilac is chosen).

## M4 — resources
- *Started (#20):* resources (labor, nonlabor, material, with a unit, a calendar and units an
  hour), roles they fill, and assignments of a resource or an unstaffed role to an activity with
  budgeted, actual and remaining units; `resource.*`, `role.*` and `assignment.*` commands with
  undo, an assignment's default budget the activity's hours at the resource's units an hour.
  P6 XML reads and writes them; Perth's `assignee` is a resource assigned for the task's
  `effort`, written back byte for byte.
  The Resources view on the rail: the resources' table, the roles, a resource's details and
  assignments, each edit a command.
  A resource's work by week against its capacity (`mkperth_sched::usage`, `capacity`,
  `by_week`), as a histogram in the Resources view, over-allocated weeks in red.
  An activity's assignments in Details' Resources tab.
  Duration types: a duration, units and units an hour worked out from each other by the type.
  Resource-dependent activities: each resource on its own calendar, the last one finishing it.
  Levelling (#21): the parallel method day by day (Kelley 1963; Davis and Patterson 1975), the
  activities on an over-allocated resource kept by a chosen order of priorities, the others
  held to start after the first finishes; within float or not; every hold explained, one undo;
  the Levelling pane, `resource.level` and `resource.unlevel`.
  PERT as an analysis: three estimates per activity (Perth.jl's fields), the expected finish on
  a copy, its σ from the expected longest path, the chance of a day and the days for a chance;
  the PERT pane and the estimates in Details. Perth.jl's analysis as an oracle is still to type.
- **Next:** prices that change over time (#23); levelling against a resource's limit that
  changes over time, and with the activity codes of #26 as priorities.

## M5 — costs and earned value
- *Done (#23):* resources' prices over time, expenses with their accrual, cost accounts, a
  currency's symbol; each activity's budget, actual, remaining and at-completion costs (a price
  change mid-activity counted from its day), the project's, its spending by day; the cost
  variance against a baseline; P6 XML's rates, cost accounts and expenses; Perth's task cost.
- *Done (#24):* the project's and each WBS's budget: original, the log of changes (pending,
  approved, not approved) giving the current and the proposed, what the WBS under it take and
  leave undistributed, the spending plan by month and what it leaves unplanned, funding sources
  in a hierarchy and the funds with what they leave unfunded, the activities' budgeted cost
  beside it; in P6 XML and Perth's JSON; the Budget pane.
- *Done (#25):* earned value: each activity's physical percent complete; BAC, PV (over the
  baseline's working days), EV by six techniques (physical, duration, units, 0/100, 50/50, a
  custom share), AC; CV, SV, CPI, SPI, ETC by four techniques, EAC, VAC, TCPI; financial periods
  made by the week or the month and their closes stored; the S-curve. Perth.jl's `_scurve` is
  the oracle (planned and earned on calendar days agree); P6 XML's percent complete, techniques
  and financial periods; the Earned value pane.
- **Next:** each period's own actuals by kind of cost (labour, non-labour, material, expenses),
  for P6's `ActivityPeriodActual`; currencies and their exchange rates; WBS milestones as an
  earned value technique.

## Beyond P6 — billing (the user's to schedule)
P6 has no billing: measurements and invoices are left to other products. MK Perth could close
the loop: progress measurements per activity from percent complete and budgeted cost, the
measurement statement per period, retention and price adjustment, as construction contracts
measure work. A module of its own, after costs reach parity.

## Later
- Progress beyond dates: percent complete, suspend and resume; the progress
  mode in P6 XML.
- Earned value, S-curves; a cost variance once costs exist (#23).
- Activity codes, user-defined fields, EPS and OBS, multiple projects.
- The network (logic) view; reports; the web build of the whole app.
