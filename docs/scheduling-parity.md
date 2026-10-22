# Scheduling parity

P6's scheduling behaviour, option by option, and where MK Perth stands. Two schedulers with the
same network give different dates when these differ, so this list is where parity lives.
Descriptions are MK Perth's own, from the scheduling literature and the public documentation's
ideas.

| Area | Behaviour | MK Perth |
| --- | --- | --- |
| Relationships | finish-to-start, start-to-start, finish-to-finish, start-to-finish | done |
| Lags | positive lags and negative leads, in working days | done |
| Lag calendar | the lag counted on the predecessor's, the successor's, a 24-hour or the project's calendar | done |
| Calendars | worked weekdays, holidays, exceptions | done |
| Calendars | a calendar per activity | done |
| Calendars | hours per day; work periods within the day | next (#2) |
| Durations | in hours, shown in days by hours per day | next (days for now) |
| Milestones | start and finish milestones | done |
| Float | total float as finish float (LF − EF) | done |
| Float | total float as start float or as the smaller of the two | next |
| Critical | total float ≤ 0 (or a chosen threshold) | done (≤ 0) |
| Critical | the longest path | next |
| Open ends | activities with no successor finish by the project's finish | done |
| Constraints | start/finish on, on or after, on or before; mandatory start/finish; as late as possible | done |
| Constraints | a secondary constraint beside the primary | done (both hold; the allowed pairs are #4) |
| Constraints | expected finish | next (#4) |
| Project | must finish by date | done |
| Progress | data date; actual start and finish; remaining duration | later |
| Progress | retained logic, progress override, actual dates | later |
| Resources | levelling with priorities | later |

## Perth.jl's conventions

On the networks both express, Perth.jl 0.16.1 and MK Perth agree on every date and total float
(`crates/perth/tests/oracle.rs`). Where Perth.jl and P6 differ, MK Perth follows P6 and
the difference is noted here.

- **Milestones.** Perth.jl's milestone takes its day (a successor starts the day after it); a P6
  start milestone takes no time (a successor starts the same morning). Perth's milestones read
  as start milestones.
- **Calendar days.** A Perth project without a calendar counts every day; MK Perth reads it as a
  seven-day calendar.
- **Summaries.** Perth.jl schedules only leaf tasks, ignores ties that reach a summary and derives
  summaries from their children; MK Perth does the same. In P6 relationships don't reach WBS
  nodes at all.
