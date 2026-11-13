# The README's screenshots

Every image in `docs/images/` is MK Perth itself, rendered offscreen by the snapshot example
(`crates/ui-egui/examples/snapshot.rs`) from the sample project: no window, nothing captured
from the desktop. After a visual change, render them again with the commands below, from the
repository's root, and look at each before committing.

## The sample, as is

```sh
# The desktop app writes in the system's interface face, its bold for emphasis; the screenshots
# stand Noto Sans for them.
FACES=(/usr/share/fonts/noto/NotoSans-Regular.ttf /usr/share/fonts/noto/NotoSans-Bold.ttf)
snap() { cargo run -q -p mkperth-ui-egui --example snapshot -- --faces "${FACES[@]}" "$@"; }

snap --select 9 docs/images/activities.png                        # B20's calculation
snap --dark --select 4 --side calendars docs/images/calendars-dark.png
snap --splash docs/images/splash.png
```

## The sample, under way

Resources, costs and earned value need a project with work in it: three resources with prices,
six assignments, progress up to a data date of 28 December 2026 (M0, A10 and A20 done, A30 nine
tenths through) and the months of November and December stored as financial periods. The
commands run first, through `--cmd`, in this order; `--lang` comes before them, so the status
bar speaks the language too.

```sh
UNDER_WAY=(
  --cmd 'project.options={"currency":"R$","data_date":"2026-12-28"}'
  --cmd 'resource.add={"id":"ANA","name":"Ana Souza","prices":[{"from":null,"price":120}]}'
  --cmd 'resource.add={"id":"CREW","name":"Concrete crew","prices":[{"from":null,"price":140}]}'
  --cmd 'resource.add={"id":"CRANE","name":"Crane","kind":"nonlabor","prices":[{"from":null,"price":450}]}'
  --cmd 'assignment.add={"activity":"A20","resource":"ANA"}'
  --cmd 'assignment.add={"activity":"A30","resource":"ANA"}'
  --cmd 'assignment.add={"activity":"B20","resource":"CREW"}'
  --cmd 'assignment.add={"activity":"B30","resource":"CREW"}'
  --cmd 'assignment.add={"activity":"B10","resource":"CRANE"}'
  --cmd 'assignment.add={"activity":"B30","resource":"CRANE"}'
  --cmd 'activity.edit={"id":"M0","actual_start":"2026-11-03","actual_finish":"2026-11-03"}'
  --cmd 'activity.edit={"id":"A10","actual_start":"2026-11-03","actual_finish":"2026-11-10"}'
  --cmd 'activity.edit={"id":"A20","actual_start":"2026-11-11","actual_finish":"2026-12-08"}'
  --cmd 'activity.edit={"id":"A30","actual_start":"2026-11-23","percent_complete":90}'
  --cmd 'assignment.edit={"activity":"A20","resource":"ANA","actual":136}'
  --cmd 'assignment.edit={"activity":"A30","resource":"ANA","actual":96}'
  --cmd 'period.generate={"from":"2026-11-01","to":"2026-12-31","every":"month"}'
  --cmd 'period.store={"name":"2026-11"}'
  --cmd 'period.store={"name":"2026-12"}'
)

snap "${UNDER_WAY[@]}" --view resources --resource 2 docs/images/resources.png   # CREW
snap --lang pt-BR "${UNDER_WAY[@]}" --side costs docs/images/costs-pt-BR.png
snap --dark "${UNDER_WAY[@]}" --side earned --size 1360x900 docs/images/earned-value-dark.png
```

The snapshot example's other options (`--side`, `--details`, `--view`, `--click`, `--size`…)
are listed at the top of its source.
