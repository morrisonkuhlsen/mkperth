# Changelog

Every release of MK Perth, newest first, in the spirit of
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow
[semantic versioning](https://semver.org/); before 1.0, a minor version may break projects and
commands.

**Changed results** lists the changes that give different dates or floats for the same project.
Read it before re-scheduling a plan with a new version.

## [Unreleased]

### Changed results

- **A P6 calendar exception with its own work times** works those times (four hours on a short
  day) instead of its weekday's whole day: activities across such days finish later.
- **A finish milestone with no work before it** (one with no predecessor, at the project's
  start) now falls at the first working minute, the morning of the project's first working day,
  instead of that day's evening; its total float counts that day too (one more day).

### Added

- Progress: a data date, actual start and finish dates, the work left of an activity under way,
  out-of-sequence progress by retained logic or by progress override; read and written in P6
  XML; typed in Details and on the Project tab, drawn on the Gantt chart, explained in the
  calculation.
- The ribbon's groups carry their names underneath.
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

### Fixed

- A month starting just after the chart's left edge kept its name hidden behind the edge's.
- A project of 10 000 activities scrolls at about 3 ms a frame (it took 300: which rows are
  summaries was worked out against every activity, for every activity, every frame).
