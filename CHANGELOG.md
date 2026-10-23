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

- The top of the window is quieter, as in an online spreadsheet: the project's name with whether
  it is saved, the File, Edit, Project and View menus under it (each item with its shortcut),
  Schedule as one button on the right, and one slim toolbar of small icons in place of the
  ribbon's tabs and large buttons. The scheduling options are in the Project menu; the theme,
  the language and hiding the toolbar (Ctrl+F1) in the View menu; AutoSave in the File menu.
  The toolbar has no band or edge of its own; hidden, its arrow waits in the header. Schedule is
  an outlined button, quiet beside the menus.

### Fixed

- The app no longer crashes at start on Windows with some Intel graphics drivers (igvk64.dll,
  Vulkan): it tries DirectX 12, then OpenGL, as MK aeGIS does; `WGPU_BACKEND` still chooses.
- A project read from P6 XML was written to Perth.jl's JSON with its calendar's P6 name
  ("Standard", "Calendar"), which Perth.jl takes for an unknown BusinessDays calendar; it is now
  written as Perth.jl names it (`""`, `"WeekendsOnly"`, `"Brazil"`), with a warning when that is only
  the nearest. A calendar with no name is written to P6 XML with one.
- A month starting just after the chart's left edge kept its name hidden behind the edge's.
- A project of 10 000 activities scrolls at about 3 ms a frame (it took 300: which rows are
  summaries was worked out against every activity, for every activity, every frame).
