# Scheduling parity

P6's scheduling behaviour, option by option, and where MK Perth stands. Two schedulers with the
same network give different dates when these differ, so this list is where parity lives.
Descriptions are MK Perth's own, from the scheduling literature and the public documentation's
ideas.

| Area | Behaviour | MK Perth |
| --- | --- | --- |
| Relationships | finish-to-start, start-to-start, finish-to-finish, start-to-finish | done |
| Lags | positive lags and negative leads, in working days | done (project calendar) |
| Lag calendar | the lag counted on the predecessor's, the successor's, a 24-hour or the project's calendar | next |
| Calendars | worked weekdays, holidays, exceptions | done (one per project) |
| Calendars | a calendar per activity; hours per day; work periods within the day | next |
| Durations | in hours, shown in days by hours per day | next (days for now) |
| Milestones | start and finish milestones | done |
| Float | total float as finish float (LF − EF) | done |
| Float | total float as start float or as the smaller of the two | next |
| Critical | total float ≤ 0 (or a chosen threshold) | done (≤ 0) |
| Critical | the longest path | next |
| Open ends | activities with no successor finish by the project's finish | done |
| Constraints | start/finish on, on or after, on or before; mandatory start/finish; as late as possible | done |
| Constraints | expected finish; a secondary constraint | next |
| Project | must finish by date | done |
| Progress | data date; actual start and finish; remaining duration | later |
| Progress | retained logic, progress override, actual dates | later |
| Resources | levelling with priorities | later |
