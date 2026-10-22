# Contributing to MK Perth

Thank you for helping. MK Perth is a project scheduler, and people trust it with their plans,
their dates and their contracts, so a few rules are strict. The full set, for people and AI
agents alike, is in [`AGENTS.md`](AGENTS.md); this page is the short version.

## Before you start

- **Pick from the roadmap.** [`docs/roadmap.md`](docs/roadmap.md) lists what is done and the
  **Next:** items, each with its GitHub issue; [`docs/scheduling-parity.md`](docs/scheduling-parity.md)
  lists the scheduling rules and where each stands. Comment on the issue to say you're on it.
- **Read the architecture.** [`docs/architecture.md`](docs/architecture.md): the crates in
  layers, and how the scheduler keeps time.

## The rules that are never bent

- **Clean-room.** Primavera P6 is not run to shape MK Perth, and nothing inside it is read. No
  GPL, AGPL or LGPL code (ProjectLibre, GanttProject, TaskJuggler, PyP6Xer…), and don't read
  their sources. Behaviour comes from public documentation (its ideas, in your own words), the
  scheduling literature and open tools run as oracles; say in the pull request which ones you
  used.
- **No one else's assets.** No vendor's icons, layouts, report templates, sample projects or help
  text. Every asset file needs a row in [`ATTRIBUTION.md`](ATTRIBUTION.md).
- **Never crash.** No `unwrap`, `expect`, `panic!`, `unreachable!`, `todo!` or `unsafe` outside
  tests. Files, networks and calendars are hostile: check them, cap them, return catalogued
  errors. A crash fix comes with a regression test.
- **Dates come with proof.** A scheduling rule lands with networks scheduled by hand in its
  tests, each date and float worked out in the comments, and with Perth.jl's results where it
  schedules the same network.

## Making the change

```sh
cargo xtask ci        # every quality gate; must pass
cargo test            # the tests alone
```

`cargo xtask ci` runs fmt, clippy with warnings as errors, the tests, and the asset, layer, panic
and wasm checks (`rustup target add wasm32-unknown-unknown` once). CI runs the same on every
pull request.

## Commits and pull requests

- One change per commit, its message saying what and why; `Fixes #n` when it closes an issue.
- Branch from `main` and open a pull request; CI must pass before it merges.
- Add a line to `CHANGELOG.md` under `## [Unreleased]` for anything a user would notice. A change
  that gives different dates or floats for the same project goes under **Changed results**,
  saying what was computed before and what is computed now.
- Update the docs your change affects (`docs/roadmap.md`, `docs/scheduling-parity.md`,
  `docs/architecture.md`) in the same pull request.

## Reporting

Bugs and feature requests: [GitHub issues](https://github.com/morrisonkuhlsen/mkperth/issues/new/choose).
Security problems: privately, as [`SECURITY.md`](SECURITY.md) says, never in a public issue.

By contributing, you agree that your contribution is licensed under MIT OR Apache-2.0, as the
rest of the project.
