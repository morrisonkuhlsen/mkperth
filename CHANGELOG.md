# Changelog

Every release of MK Perth, newest first, in the spirit of
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow
[semantic versioning](https://semver.org/); before 1.0, a minor version may break projects and
commands.

**Changed results** lists the changes that give different dates or floats for the same project.
Read it before re-scheduling a plan with a new version.

## [Unreleased]

### Changed results

- **A resource-dependent activity** (P6 XML's "Resource Dependent", or the box in Details)
  schedules on its resources' calendars and finishes when the last of them does, instead of
  working its duration on its own calendar: a resource on a shorter week (or with more work)
  moves it, and its successors, later. Read from P6 XML it was a task before.
- **A P6 calendar exception with its own work times** works those times (four hours on a short
  day) instead of its weekday's whole day: activities across such days finish later.
- **A finish milestone with no work before it** (one with no predecessor, at the project's
  start) now falls at the first working minute, the morning of the project's first working day,
  instead of that day's evening; its total float counts that day too (one more day).

### Added

- Activity codes and user-defined fields (#26): codes with their values in a hierarchy
  (`code.add`, `.delete`, `code-value.add`, `.delete`), fields of a kind each (text, number,
  integer, cost, date or a red, yellow, green or blue indicator; `field.add`, `.delete`), and
  each activity's values, set or cleared on one activity or many at once (`activity.classify`).
  In P6 XML (`ActivityCodeType`, `ActivityCode`, `ActivityCodeAssignment`, `UDFType`,
  `UDFValue`; a field P6 keeps for something other than activities, or works out from a code,
  is left out with a warning) and in Perth's JSON (`mkperth_classification`, and each task's
  `mkperth_codes` and `mkperth_fields`).
- Grouping, filtering and sorting (#26): the rows grouped in bands by a code's or a field's
  values (Filters ▸ Group by), filtered to a code's value and those under it (Filters ▸ Code),
  sorted by a click on a column's title (upwards, downwards, then as they were) among each
  summary's children; codes and fields as columns; the Codes and fields pane (Project ▸ Codes
  and fields…) and Details' codes and fields.
- Earned value (#25): each activity's percent complete as judged (`activity.edit`
  `percent_complete`); the budget at completion, planned value (spread over the baseline's
  working days when one is in use), earned value and actual cost, by the technique the project
  chooses (physical, duration, units, 0/100, 50/50 or a custom share, `project.options`
  `ev_technique`); the cost and schedule variances and indices, the estimate to complete by
  what is left or by the budget still to earn at a factor of 1, 1/CPI or 1/(CPI × SPI)
  (`etc_technique`), the estimate and variance at completion and the TCPI. Financial periods
  (`period.add`, `period.generate` by the week or the month, `period.delete`) and their closes
  stored (`period.store`): the S-curve's points.
- Earned value in the files: Perth's task `progress` is the physical percent complete, read and
  written; the techniques, periods and stored closes travel in `mkperth_earned_value`. P6 XML
  reads and writes the physical percent complete (`PhysicalPercentComplete`,
  `PercentCompleteType`), the techniques (`EarnedValueComputeType`, `EarnedValueUserPercent`,
  `EarnedValueETCComputeType`) and the financial periods (`FinancialPeriod`); the stored closes
  are left out of it, with a warning.
- The Earned value pane (Project ▸ Earned value…): the techniques chosen, the measures at the
  data date (BAC, PV, EV, AC, CV, SV, CPI, SPI, ETC, EAC, VAC, TCPI), the S-curve (planned day
  by day, earned and actual at each stored close and at the data date), and the financial
  periods made by the month or the week and stored; Details' Status tab takes the percent
  complete.
- The budget (#24): the project's and each WBS's original budget (`budget.set`); a log of
  changes, pending, approved or not approved (`budget-change.add`, `.edit`, `.delete`), from
  which the current budget (the approved ones) and the proposed (the pending too); the spending
  plan month by month (`budget.plan`); funding sources in a hierarchy and what each gives
  (`funding-source.add`, `.delete`, `fund.set`). Each budget shows what its WBS take and leave
  undistributed, what its plan leaves unplanned, what the funds leave unfunded, and beside it
  the activities' budgeted cost.
- P6 XML reads and writes the budget: the project's and the WBS's original budgets
  (`OriginalBudget`), the change log (`ProjectBudgetChangeLog`), the spending plans
  (`ProjectSpendingPlan`, `WBSSpendingPlan`), funding sources (`FundingSource`) and the
  project's funds with their share (`ProjectFunding`). A budget on an activity that isn't a
  summary is left out, with a warning.
- Perth's project JSON keeps the budget in a field of its own (`mkperth_budget`), read back as it
  was written; Perth.jl passes it through untouched.
- The Budget pane (Project ▸ Budget…): the budget of the project or of a WBS, how it stands
  (original, current, proposed, undistributed, the activities' cost), its spending plan by month,
  its log of changes with their status, and the project's funding and funding sources.
- Resource levelling (#21): Project > Level resources…, or the Levelling pane on the right
  strip, delays activities so that no resource (but a material) is asked, on any day, for more
  units an hour than its max units. Day by day from the first over-allocation, the activities on
  the resource keep it in an order the project chooses (levelling priority, total float, early or
  late start, remaining duration; the ID last), the others waiting for the first to finish;
  begun activities and mandatory dates don't move. Within float only, the finish stays and what
  can't be resolved stays over. Each wait is a start no earlier than a day, shown in the
  calculation ("Levelling, so Ana stays within its limit"); one undo, Clear levelling takes it
  back. `resource.level` and `resource.unlevel` commands, `levelling_priority` on
  `activity.edit`; the priority in Details; P6 XML's LevelingPriority; Perth's JSON keeps the
  priority, the holds and the options in fields of MK Perth's own.
- Bars are dragged on the chart: the whole bar moves the activity (it starts on or after its
  new day, a finish milestone finishes on or after it), its finish edge stretches or shortens it
  (working days from its start on its calendar), its start edge moves its start and keeps its
  finish; a ghost bar with the new days follows the pointer, the cursor says move or resize,
  and the dates, floats and calculation follow on release, one undo each. Summaries and
  activities already begun don't drag.
- Costs in the window (#23): Details' Costs tab (the activity's budget, actual, remaining, at
  completion and variance; its expenses, the actual typed in place, a new one with its accrual
  and account); the Costs pane (Project > Costs…: the currency, the project's costs, the budget
  by month, the cost accounts in their hierarchy); a resource's prices over time in the
  Resources view; the Budget, Actual cost, At completion and Cost var. columns (in Filters).
  Costs travel in P6 XML (resource rates, cost accounts, activity expenses) and in Perth's JSON
  (its task `cost` read as an expense).
- Costs (#23, part 1): resources' prices over time (`resource.add` and `resource.edit` take
  `prices`), expenses on activities accrued at the start, the end or evenly (`expense.add`,
  `.edit`, `.delete`), cost accounts in a hierarchy (`cost-account.add`, `.delete`) and the
  currency's symbol (`project.options`' `currency`). Each activity's budget, actual,
  remaining and at-completion costs (a price change mid-activity counted from its day), the
  project's, and its spending day by day; a baseline keeps the budget, and its cost variance.
- Baselines (#22, Project > Baselines…): the schedule frozen under a name, one of them in use;
  its bars, thin and orange, under the schedule's (View > Baseline bars), and the columns BL
  start, BL finish and the start, finish and duration variances (in Filters; a negative variance
  is late, in working days). `baseline.assign`, `baseline.use` and `baseline.delete`; kept in
  Perth's JSON (Perth.jl's own baseline read as one) and in P6 XML as baseline projects.
- PERT as an analysis (Project > PERT analysis…): an activity's optimistic, most likely and
  pessimistic durations in Details (`activity.edit`'s `pert`; Perth's own fields in its JSON); the
  pane shows the finish planned and expected (each estimated activity at (O + 4M + P) / 6, on a
  copy), its σ, the chance of finishing by a day and the days for 50, 80, 90 and 95%, and the
  expected path. The schedule keeps its planned durations: no date changes.
- The open views' tabs along the bottom, as a spreadsheet's sheet tabs; every corner square,
  none rounded: buttons, menus, panes, bars and the splash.
- Striped rows (the toolbar's Show group, or View > Striped rows): every other row of the
  table and chart tinted, as spreadsheets band a table.
- More in the toolbar: the zoom as a drop-down beside undo; Link and Unlink (the chosen rows
  tied in their order, finish to start, or every tie between them taken away, one undo each:
  `relationship.add` and `relationship.delete` take `activities`); Indent and Outdent (under
  the activity above at its level, or out of its summary: `activity.edit` takes `parent`); a
  comment on the selected activity; Today and Go to the activity on the time scale; Level
  resources with the project's options. The groups' names show when the row has room for them.
- Dates picked in the activity table: a double click on a start or finish cell opens a month
  of days under it (arrows for other months, today in the accent) and a field to type one; a
  start picked moves the activity there (it starts on or after it), a finish stretches it to
  that day, as dragging its bar does, each one undo. Activities not yet begun only; Esc or a
  click elsewhere leaves it as it was.
- The font in the toolbar, as spreadsheets have it: the typeface as a drop-down ("Default" for
  MK Perth's own, Ubuntu Light, or one the system has), and the size in points between a minus
  and a plus (10 the usual; typed, or picked from the usual ones); View > Font also has bold
  and italic for a system typeface. Kept for the next run; a size kept in percent before reads
  as its tenth. A font file that doesn't read is left out, egui's own staying.
- Presentation mode (the screen icon in the header, View > Presentation mode, F5): the
  project's name over the activities and the chart, full screen, the menus, toolbar, rail and
  panes away; Esc, F5 or Leave the presentation ends it.
- Clicking and dragging on the activities: drag over rows to choose them (in the table every
  row spanned, on the chart the bars touched), Ctrl and a click to add or take one, Shift and a
  click for a run of them, Ctrl+A for all, Esc to keep only the selected; Delete takes every one
  chosen as one change. A double click on the chart away from a bar, or on the ruler's days,
  opens a small window there for a band from that day (its name, then Enter); on the ruler's
  months, one to mark the month; on a bar, its details. A right click on a row: new activity
  after it, delete, comment, details. The status bar counts those chosen.
- Duration types (#20): an activity's duration, its assignments' units and their units an hour
  hold together (units = units an hour × hours) by its type, chosen in Details (fixed duration
  and units/time, the default; fixed duration and units; fixed units/time; fixed units): a new
  duration, new units or a new rate work out the third as the type says. Assignments keep their
  units an hour (Units/h in Details' Resources tab and the Resources view; P6 XML's
  PlannedUnitsPerTime and DurationType read and written). Existing projects schedule as before.
- Details' Resources tab: the resources (and roles still to staff) on the selected activity,
  their budgeted, actual and remaining work typed in place, removed, and a resource assigned
  from a list.
- A resource's work by week (#20): its assignments' budgets spread over their activities'
  working days, as bars under its assignments in the Resources view, red in a week it has more
  work than it can do, what it can do as a dashed line (its units an hour over its calendar's
  working hours); a week's figures under the pointer.
- Comments on activities, as a spreadsheet keeps them: threads with replies, resolved and
  reopened, signed with the person's name and the time (`comment.add`, `.reply`, `.edit`,
  `.resolve`, `.delete`, with undo). In Details' Comments tab (Edit > Comment, Ctrl+Alt+M); an
  activity with an open thread is marked in its ID's corner. Saved in Perth JSON in a field of
  their own (`mkperth_comments`); P6 XML has no place for them and says so.
- The File backstage, as MK Sigma's and MK aeGIS's: File in the header opens a page over the
  window, a column in Morrison Kühlsen's ink (Home, New, Open; Info, Save, Save As, Export;
  About, Exit) beside the place's page: cards to start a new project, the sample or open one,
  the recent projects (kept between runs), the project's facts and AutoSave, and Export as P6
  XML, Perth.jl JSON or the selected activity's calculation in LaTeX. Esc or Back returns.
- `project.export`: a copy as P6 XML or Perth JSON by the path's extension; the open project
  keeps its file and its unsaved changes.
- The Resources view, second on the rail (and View > Resources): the resources in a table (type,
  unit, units an hour, calendar, roles, hours budgeted), added and deleted there, the roles they
  fill, and the selected resource's details and assignments (budgeted, actual and remaining
  work, edited in place; an activity assigned from a list).
- Resources, roles and assignments (#20): `resource.add`, `.edit`, `.delete`; `role.add`,
  `.delete`; `assignment.add`, `.edit`, `.delete`, with undo. An assignment's budget defaults
  to the activity's working hours at the resource's units an hour; renaming or deleting an
  activity or a resource carries or takes its assignments. P6 XML reads and writes them
  (Resource, Role, ResourceRole, UnitOfMeasure, ResourceAssignment); a Perth task's assignee is a
  resource assigned for its effort (else its working hours) and is written back as named.
- The Calendars pane (the strip at the right, or Project > Calendars…): a calendar's working
  week, its work periods, Brazil's national holidays, and its days off and worked days, edited
  in the app and undoable. Chart notes take a pin for their icon, leaving the calendar to it.
- `project.new`: an empty project from a given start (File > New, Ctrl+N, from today).
- `calendar.edit`: a calendar's working week, its work periods, holidays and worked days added
  or taken away, and its region's holidays, by command (the app, the CLI and MCP), undoable.
- A splash screen as MK Sigma's and MK aeGIS's, in MK Perth's lilac: the mark, the name, the
  version and the parts loading on Morrison Kühlsen's ink, beside artwork drawn in code (a
  schedule cascading down to a milestone); a click or Esc closes it, `--no-splash` skips it.
- The activity table hides from the rail of views (or View ▸ Activity table): the Gantt chart
  takes the width, with the names beside the bars.
- Progress: a data date, actual start and finish dates, the work left of an activity under way,
  out-of-sequence progress by retained logic or by progress override; read and written in P6
  XML; typed in Details and in the toolbar, drawn on the Gantt chart, explained in the
  calculation.
- `mkperth-mcp`: agents open a project, read its schedule, change it with any engine command,
  ask how a date came about, save it and export the calculation, over the Model Context Protocol.
- Chart notes from Perth.jl: bands (a stretch shaded behind the bars), markers (a named day as a
  dashed line) and marked months (painted in the ruler); in the Chart notes pane, by command,
  and in Perth.jl's JSON. P6 XML has no place for them and says so when written.
- `mkperth-cli`: `schedule` (a table, JSON or CSV), `run` (commands, then `--out` saves),
  `explain` (the calculation, `--tex` for LaTeX); exit codes 0, 1, 2; messages in the locale.
- The table's columns chosen in Filters (the ID and the name stay); today drawn on the chart.
- The Details pane in tabs: General, Status, Relationships and Notes; notes per activity, kept
  in Perth.jl's `notes` (P6 XML says they aren't written).
- Exception days with work periods of their own; P6 XML's exceptions keep their hours (they
  were read as whole working days on their weekday's periods).
- The calculation of an activity's dates as a LaTeX document: the Calculation pane's LaTeX
  button, the `activity.export-tex` command.
- An expected finish date that sets an activity's work (or its work left); secondary constraints
  checked against their primary, pairs that can't stand together refused.
- Time in hours: calendars with work periods for each day of the week, durations and lags of part
  days (P6's hours kept exact), floats shown in days with decimals; the calculation names a time
  of day when it isn't the start or end of the day's work.

- The scheduler: the critical path method with finish-to-start, start-to-start, finish-to-finish
  and start-to-finish relationships, lags and leads, the forward and backward pass, total and
  free float, critical activities, the project's finish. Loops of relationships are named.
- Work calendars: worked weekdays, holidays and extra working days; a calendar per activity; the
  calendar lags count on (the predecessor's, the successor's, 24 hours or the project's).
- Start and finish milestones.
- Constraints: start or finish on, on or after, on or before; mandatory start and finish; as late
  as possible; a secondary constraint beside the primary; the date a project must finish by.
- The WBS: summaries take their dates from their children; relationships that reach a summary
  are left out and listed.
- Perth.jl's project JSON, read and written back byte for byte, every field Perth keeps carried
  along.
- Perth.jl as the parity oracle: 43 networks scheduled by Perth.jl 0.16.1 agree with MK Perth on
  every date and total float.
- Scheduling options (Project tab): total float as finish float, start float or the smaller;
  critical activities by a total-float threshold or the longest path.
- Primavera P6 XML, read and written: WBS, activities, relationships, constraints and calendars;
  durations and lags in hours become working days; Open and Save choose by the extension.
- Brazil's national holidays, computed from their rules (Easter by the Gregorian computus), for
  Perth projects on the `Brazil` calendar and any calendar that names the region.
- The window, in the house's layout: title row, ribbon, the activity table beside the Gantt
  chart, the selected activity's calculation step by step, its details, filters (find, critical
  only), the status bar with the zoom; Open, Save, Save As, Sample, Schedule (F9).
- Editing: add, edit (ID, name, duration, type, calendar, constraint) and delete activities, add
  and remove relationships, each change scheduled at once, with undo and redo; in the window,
  edit a cell in place (double click), New activity (Insert), Delete, relationships in Details,
  Ctrl+Z and Ctrl+Y. Changes that would make a loop or repeat an ID are refused.
- The Gantt chart: rounded bars with an edge, red on the critical path; the total float as a light
  band to the late finish; each activity's name beside its bar; relationship lines with arrows,
  by kind (FS, SS, FF, SF), red between critical activities; thinner summary brackets; diamond
  milestones; weekends shaded only when a day is wide enough.
- The Gantt chart's time scale: zoom with Ctrl and the wheel (about the day under the pointer) or
  Zoom in time, Zoom out time and Fit project in View; scroll in time with Shift and the wheel, a
  sideways swipe, by dragging the dates or the scroll bar under the chart; days, weeks and months
  labelled as the scale allows, without overlapping.
- Every date with its calculation: the bounds of each pass, the winner, the calendar's placing,
  the floats.
- Messages in English and Brazilian Portuguese.

### Changed

- The baseline a Perth file's `baseline_start` and `baseline_duration` make now has the task's
  cost, as Perth.jl's S-curve weighs it: its cost variance was the whole cost at completion
  (against a baseline of nothing) and is now the cost's change since.
- The window's header works as its title bar, as in MK Sigma: a double click maximizes or
  restores the window, a drag moves it; the project's name sits in its middle, and an AutoSave
  switch at its top left saves every change to the file (asking for one first when there is none).
- A new mark, drawn in code: a hexagon seen as a solid, its band open at the lower left so it
  reads as a P, around a cube, in three shades of blue and violet; in the header, the splash
  screen and as the window's icon.
- The splash screen's foot reads "Powered by" and the Morrison Kühlsen wordmark, instead of the
  wordmark beside a line repeating its name.
- The panes keep their places: a wide pane (Calendars' working week) no longer pushes the strip
  of panes on the right out of the window, and the views' tabs keep their row above the status
  line instead of being covered.
- The side panes share one layout: sections headed in small capitals with space between them,
  form labels in one column with their fields lined up after them and filling the pane, lists
  with their delete buttons at the right edge, figures aligned right; nothing runs past a
  pane's edge any more (Details' duration type, Calendars' week). Filters lists the columns two
  to a row.
- The Resources view lines up: its table drawn as the activities' (a header band, fixed columns,
  banded rows), and the forms to add a resource and a role aligned under it.
- A column title wider than its column wraps onto a second line instead of being cut off
  ("ID da atividade").
- The top of the window is quieter, as in an online spreadsheet: the project's name with whether
  it is saved, the File, Edit, Project and View menus under it (each item with its shortcut),
  Schedule as one button on the right, and one slim toolbar of small icons in place of the
  ribbon's tabs and large buttons. The scheduling options are in the Project menu; the theme,
  the language and hiding the toolbar (Ctrl+F1) in the View menu; AutoSave in the File menu.
  The toolbar has no band or edge of its own; hidden, its arrow waits in the header. Schedule is
  an outlined button, quiet beside the menus.

### Fixed

- P6 XML as Oracle documents it (#31): the estimate to complete's techniques are written
  `Performance Factor = 1` (and `… / Cost Performance Index`, `… / (Cost Performance Index *
  Schedule Performance Index)`), not `PF = …`; a WBS's spending plan is a `ProjectSpendingPlan`
  naming its WBS, not a `WBSSpendingPlan`; a resource's role has no `ObjectId` of its own; every
  calendar says its `Type`. Files written before still read.
- A P6 XML file with resource rates, cost accounts or expenses no longer says they were left
  out: they have been read since costs came (#23).
- The app no longer crashes at start on Windows with some Intel graphics drivers (igvk64.dll,
  Vulkan): it tries DirectX 12, then OpenGL, as MK aeGIS does; `WGPU_BACKEND` still chooses.
- A project read from P6 XML was written to Perth.jl's JSON with its calendar's P6 name
  ("Standard", "Calendar"), which Perth.jl takes for an unknown BusinessDays calendar; it is now
  written as Perth.jl names it (`""`, `"WeekendsOnly"`, `"Brazil"`), with a warning when that is only
  the nearest. A calendar with no name is written to P6 XML with one.
- A month starting just after the chart's left edge kept its name hidden behind the edge's.
- A project of 10 000 activities scrolls at about 3 ms a frame (it took 300: which rows are
  summaries was worked out against every activity, for every activity, every frame).
