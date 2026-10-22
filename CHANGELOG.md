# Changelog

Every release of MK Perth, newest first, in the spirit of
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow
[semantic versioning](https://semver.org/); before 1.0, a minor version may break projects and
commands.

**Changed results** lists the changes that give different dates or floats for the same project.
Read it before re-scheduling a plan with a new version.

## [Unreleased]

### Added

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
- Perth.jl as the parity oracle: 35 networks scheduled by Perth.jl 0.16.1 agree with MK Perth on
  every date and total float.
- Messages in English and Brazilian Portuguese.
