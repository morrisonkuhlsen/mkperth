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
| Calendars | a calendar per activity | done: moments with no work between them are one moment on a calendar (Friday 17:00 and Monday 08:00); backwards, a successor's late date reaches its predecessor as the latest such moment on the successor's calendar (and on the lag's, after a lead), and a start bound is kept strictly, so calendars of different hours give no float that isn't there, negative or positive (#62) |
| Calendars | a region's holidays computed from their rules (Brazil) | done |
| Calendars | work periods within each day of the week; hours per day from them | done |
| Calendars | work periods of their own on an exception day | next |
| Durations | in working minutes (from hours or part days), shown in days by the calendar's hours | done |
| Milestones | start and finish milestones | done |
| Float | total float as finish float (LF − EF) | done |
| Float | total float as start float or as the smaller of the two | done (they differ once progress exists, #19) |
| Critical | total float ≤ a chosen threshold | done |
| Critical | the longest path (driving relationships back from the project's finish) | done |
| Open ends | activities with no successor finish by the project's finish | done |
| Constraints | start/finish on, on or after, on or before; mandatory start/finish; as late as possible | done |
| Constraints | a secondary constraint beside the primary | done: both hold; a secondary needs a dated primary and bounds the other end (a start's with a finish's), never mandatory nor as late as possible — MK Perth's rule, as Oracle publishes no table of pairs (#4) |
| Constraints | expected finish | done: the work (left, once started) runs from the start to that evening (#4) |
| Project | must finish by date | done |
| Project | baselines: assigned, the one in use, variances | done (#22): each activity's early dates and duration frozen; start and finish variances in working days of its calendar and the duration's in working days, baseline less schedule (negative is late), as P6 counts them; P6 XML's baseline projects and current baseline; the cost variance waits for costs (#23) |
| Progress | data date; actual start and finish; remaining duration | done (#19) |
| Progress | retained logic, progress override, actual dates | done (#19); percent complete, suspend and resume later |
| Resources | duration types (fixed duration and units/time, fixed duration and units, fixed units/time, fixed units) | done: units = units an hour × the activity's working hours; a new duration changes the units (both "units/time" types) or the units an hour (both "units" types); new units change the units an hour (both "fixed duration" types) or the duration (the other two); a new rate changes the units, or the duration for fixed units; a duration from assignments is the longest they take. MK Perth's rule from P6's published idea of the four types (#20) |
| Resources | resource-dependent activities on their resources' calendars | done: each resource on it (not a material) works its units over its units an hour on its own calendar from the activity's start; the activity finishes when the last does, and backwards starts when the first must (its floats still measured on its own calendar); begun, with an expected finish or without resources it works its own duration. P6 XML's "Resource Dependent" type (#20) |
| Resources | levelling with priorities | done: the parallel method from the first over-allocated day (units an hour against max units, on each activity's working days); the activities that would start that day keep the resource by levelling priority, total float, early start, late start or remaining duration in a chosen order (the ID last), the others start after the first of those keeping it finishes; begun activities and mandatory dates stay; within float, an activity waits only to its late start. Each hold is a start no earlier than a day, explained, until levelled again or cleared. Materials aren't levelled; a limit that changes over time, levelling by codes and splitting activities are later. MK Perth's rule from the levelling literature (#21) |
| Costs | resource prices over time, expenses and their accrual, cost accounts | done (#23): a resource's work spread evenly over its activity's working days and priced each day at the price then; an expense at the start, the end or evenly; budget over the planned dates, actual from the actual start to the data date, remaining from the data date to the early finish, at completion their sum; summaries take their children's. Currencies are one symbol; exchange rates, financial periods and earned value are later (#25) |
| Costs | budgets: original, current and proposed, the change log, spending plans, funding | done (#24): the project's and each WBS's; current is the original and the approved changes, proposed adds the pending; a budget's undistributed part is what the WBS right under it don't take; the spending plan by month and what it leaves unplanned; funding sources in a hierarchy, the project's funds and their share; P6 XML's `OriginalBudget`, `ProjectBudgetChangeLog`, `ProjectSpendingPlan`, `WBSSpendingPlan`, `FundingSource`, `ProjectFunding`. Benefit plans are not kept |
| Costs | earned value: techniques, indices, estimates, financial periods | done (#25): BAC from the baseline in use (else the budget); PV spread over the planned working days; EV by physical, duration or units percent complete, 0/100, 50/50 or a custom share (P6's WBS milestones and resource curves aren't kept); AC to the data date; CV, SV, CPI, SPI; ETC as remaining cost or (BAC − EV) at PF 1, 1/CPI, 1/(CPI × SPI) (P6's custom PF isn't kept); EAC, VAC, TCPI; financial periods and stored closes: each period's own planned and earned value and actuals by kind of cost and units (#34) |
| Enterprise | activity codes, user-defined fields, EPS and OBS, several projects | done (#26): codes (project scope) with hierarchical values; fields of type text, double, integer, cost, date and indicator; grouping, filtering by a code value and its children, sorting; the EPS path, the OBS, the project's and each WBS's responsible; external relationships scheduled in one network with the other projects read from their files (late dates from the latest finish among them, a choice P6 offers too). Not yet: global and EPS-scope codes, code-type UDFs, P6's 'ignore relationships to and from other projects' option |
| Analysis | PERT: three estimates, the expected finish and its chances | done, beside the schedule and never in it (P6 leaves this to a separate risk product): tₑ = (O + 4M + P) / 6 and σ = (P − O) / 6 per activity; the project scheduled again on a copy at the expected durations (activities not begun); σ of the finish from the variances on that copy's longest path; the chance of a day Φ((W − T) / σ) on the project's calendar, and the days for 50, 80, 90 and 95%. Perth.jl's own estimates (`optimistic`, `most_likely`, `pessimistic`) are read and written |

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
