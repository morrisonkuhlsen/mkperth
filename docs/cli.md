# The command line's contract

`mkperth-cli` is run by scripts, CI and Perth.jl's bridge (`src/mkperth.jl` in Perth.jl:
`import_p6xml`, `export_p6xml`, `schedule_p6`). What this page lists is kept stable: a change
to it is a change to the bridge, made in both repositories together, with a line under
**Changed** in the CHANGELOG. `apps/mkperth-cli/tests/cli.rs` (`the_bridge_contract`) holds it.
Anything not listed here (the table's layout, the messages' wording) may change.

## Verbs

| Verb | Options | Prints |
|---|---|---|
| `schedule` | `[--in FILE] [--out FILE] [--format table\|json\|csv]` | the schedule |
| `run` | `[--in FILE] --cmd 'ID[=JSON]' [--cmd …] [--out FILE] [--format …]` | the schedule after the commands |
| `explain` | `[--in FILE] --activity ID [--tex FILE]` | the activity's calculation, step by step |

- `--in`: told by its content, else its extension: Perth.jl's JSON (`.json`), P6 XML or
  Microsoft Project XML (`.xml`), XER (`.xer`). Without it, the sample project.
- `--out`: written in the format of its extension (`.json` Perth.jl's, `.xml` P6 XML, `.xer`
  XER), or in the input's format when it is the input's own file; only when every command held
  and the project schedules: all or nothing.
- `--cmd`: any engine command (`mkperth_engine::COMMANDS`), its parameters a JSON object.
- The default format is `table`, for reading, in the person's language.

## `--format json`

One object, its keys in this order:

| Key | Type |
|---|---|
| `name` | string: the project's name |
| `finish` | string: the project's finish, `YYYY-MM-DD` |
| `activities` | array, one object per activity in the project's order |

Each activity:

| Key | Type |
|---|---|
| `id`, `name` | string |
| `early_start`, `early_finish`, `late_start`, `late_finish` | string, `YYYY-MM-DD` |
| `total_float`, `free_float` | number: working days on the activity's calendar, to two decimals |
| `critical`, `longest_path` | boolean |
| `status` | `not-started`, `in-progress` or `completed` |

Keys may be added at the end of an object; none is removed or renamed.

## `--format csv`

RFC 4180, CRLF line ends, the header fixed whatever the language:

```
id,name,early_start,early_finish,late_start,late_finish,total_float,free_float,critical,longest_path,status
```

## Output and exit codes

| Code | Meaning | stdout | stderr |
|---|---|---|---|
| 0 | done | the output asked for | a line per warning, `warning: ` first |
| 1 | an error (a file, a command, a project that doesn't schedule) | nothing | one line, `error: ` first |
| 2 | a usage mistake | nothing | the usage |

The prefixes are the C locale's; `LC_ALL`, `LC_MESSAGES` or `LANG` set the language of
everything on stderr (`aviso: `, `erro: `, `uso:` in Português). The bridge runs with the
environment it was given, so it reads stderr as lines, not by their prefix.
