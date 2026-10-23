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
- **Next:** calendars edited in the app.

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
  every command fuzzed with empty and hostile parameters; `project.save`; the sample project built in code (a small building on Brazil's calendar,
  with a WBS, every kind of relationship, a lag and a lead, milestones, a constraint and a
  seven-day cure).
- **Next:** `mkperth-engine`: the project as the session's state, every change a command (ids
  after P6's menus: `project.schedule`, `activity.add`, `relationship.add`, `baseline.assign`),
  undo; `mkperth-cli` (schedule a file, print or write the result); `mkperth-mcp`.

## M3 — the Activities window
- *Started:* the house's layout (MK Sigma's): a title row (the mark, AutoSave, Save, Undo and
  Redo, the project's name, theme and search), File and the Home, Project and View tabs with the
  ribbon's large buttons, the rail of views, the Calculation pane under the activities (the
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
- **Next:** calendars edited in the app; the network view (#17); the brand (#18, the user's).

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
- Progress beyond dates: percent complete, suspend and resume; the progress
  mode in P6 XML.
- Resources and roles, resource levelling, histograms.
- Baselines, earned value, S-curves.
- Activity codes, user-defined fields, EPS and OBS, multiple projects.
- The network (logic) view; reports; the web build of the whole app.
