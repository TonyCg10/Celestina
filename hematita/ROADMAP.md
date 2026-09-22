# Hematita implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** H4
- **Related author validation:** `VAL-H1` in [VALIDATION.md](VALIDATION.md)
  (does not block)

## Hypothesis and tangible outcome

A window that samples `/proc` once per second on its own thread can show CPU
and memory with sixty seconds of history, through parsers tested on text
alone, without the machine noticing the monitor.

## Scope

| Phase | Outcome |
|---|---|
| H1 | Registered project, skeleton, navigation strip, Performance page with CPU and memory |
| H2 | Disks, network, GPU and swap; the per-core grid |
| H3 | Processes and applications |
| H4 | Sensors (all of hwmon) |
| H5 | Services and privileged actions |

## Exclusions

- Per-process network; non-AMD GPUs; history persistence; alerts; any shell
  integration.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| H1-A | done | none | crate stub, application skeleton, strip, scripts, documents | `scripts/smoke.sh`, guards |
| H1-B | done | H1-A | `hematita-core`: ratio, cpu, memory, history with captures | `cargo test -p hematita-core` |
| H1-C | done | H1-B | sampler, `HematitaResources`, activation, Performance page | `scripts/verify-production.sh` |
| H1-D | done | H1-C | history properties without lint suppressions, row fixes | `scripts/verify-production.sh` |
| H1-Z | done | H1-D | implementation exit and 0.2.0 | `scripts/complete-production.sh` |
| H2-A | done | H1-Z | `hematita-core`: rate, disk, network, gpu, ring fractions, captures | `cargo test -p hematita-core` |
| H2-B | done | H2-A | sampler sections, `publish.rs`, list-publishing `HematitaResources`, page, per-core grid, failure path and H1 follow-ups | `scripts/verify-production.sh` |
| H2-C | done | H2-B | resource order, a list model stable across revisions, the GPU reason file, toggle and subtitle fixes | `scripts/verify-production.sh` |
| H2-Z | done | H2-C | implementation exit and 0.3.0 | `scripts/complete-production.sh` |
| H3-A | done | H2-Z | list operability, the smoke's shape gate and the H2 follow-ups | `scripts/verify-production.sh` |
| H3-B | done | H3-A | `hematita-core`: `process`, `passwd`, `process_view`, captures | `cargo test -p hematita-core` |
| H3-C | done | H3-B | sampler process section, `HematitaProcesses`, the table and both pages | `scripts/verify-production.sh` |
| H3-D | done | H3-C | the table's keyboard path and the four smaller H3-C review findings | `scripts/verify-production.sh` |
| H3-Z | done | H3-D | implementation exit and 0.4.0 | `scripts/complete-production.sh` |
| H3-E | done | H3-Z | the whole-branch review's fixes and 0.4.1 | `scripts/complete-production.sh` |
| H4-A | done | H3-E | `hematita-core`: `sensors`, captures, the deferred cgroup regression test | `cargo test -p hematita-core` |
| H4-B | planned | H4-A | sampler sensors section, `HematitaSensors`, the Sensors page | `scripts/verify-production.sh` |
| H4-Z | planned | H4-B | implementation exit and 0.5.0 | `scripts/complete-production.sh` |

## Implementation exit

`scripts/complete-production.sh` succeeds: the release build, its verification
(crate tests, clippy, fmt, qmllint ratchet, smoke) and the deployment to the
author's prefix; the installed binary shows live CPU and memory graphs.

## Closed evidence

- H1-A: [skeleton](docs/evidence/2026-09-21-h1-skeleton.md)
- H1-B: [core](docs/evidence/2026-09-21-h1-core.md)
- H1-C: [performance page](docs/evidence/2026-09-21-h1-performance-page.md)
- H1-D: [history type](docs/evidence/2026-09-21-h1-history-type.md)
- H1-Z: [production completion](docs/evidence/2026-09-21-h1-production-completion.md)
- H2-A: [core](docs/evidence/2026-09-22-h2-core.md)
- H2-B: [resources page](docs/evidence/2026-09-22-h2-resources-page.md)
- H2-C: [list stability](docs/evidence/2026-09-22-h2-list-stability.md)
- H2-Z: [production completion](docs/evidence/2026-09-22-h2-production-completion.md)
- H3-A: [list operability](docs/evidence/2026-09-22-h3-list-operability.md)
- H3-B: [core](docs/evidence/2026-09-22-h3-core.md)
- H3-C: [processes page](docs/evidence/2026-09-22-h3-processes-page.md)
- H3-D: [keyboard table](docs/evidence/2026-09-22-h3-keyboard-table.md)
- H3-Z: [production completion](docs/evidence/2026-09-22-h3-production-completion.md)
- H3-E: [table fixes](docs/evidence/2026-09-22-h3-table-fixes.md)
- H4-A: [core](docs/evidence/2026-09-22-h4-core.md)

## H1 — closed 2026-09-21

Its falsifiable problem was whether a window that samples `/proc` once a
second on its own thread could show CPU and memory with sixty seconds of
history through parsers tested on text alone. The delivered result is the
registered project, the release binary built, verified and deployed to the
author's prefix at `0.2.0`, whose Performance page shows live CPU and memory
with a one-minute graph and a pill navigation strip naming the later
sections.

Units `H1-A` through `H1-Z` are in the archived
[plan](docs/plans/archive/2026-09-21-h1-foundation.md). The checkpoint's
implementation exit ran on 2026-09-21: the
[delivery record](docs/evidence/2026-09-21-h1-production-completion.md).
`VAL-H1` stays pending in the author's lane and did not block this closure.
The next checkpoint is `H2`, not yet opened.

## H2 — closed 2026-09-22

Its falsifiable problem was whether the side list could become a live
inventory of the machine — every whole disk, every interface, the GPU — read
from `/proc` and `/sys` once a second by the same thread, each with its own
minute of history and its own honest absence, without the monitor's idle
cost becoming visible. The delivered result is the release binary built,
verified and deployed to the author's prefix at `0.3.0`, whose Performance
page lists the processor (with a per-core grid behind an icon toggle),
memory with swap, the AMD GPU, every whole disk and every network interface,
each with live graphs.

The follow-ups H1 booked as its first unit were delivered inside `H2-B`:
every one of them needed the build the list rewrite needed, so they shared
it. Units `H2-A` through `H2-Z` are in the archived
[plan](docs/plans/archive/2026-09-22-h2-resources.md). The checkpoint's
implementation exit ran on 2026-09-22: the
[completion evidence](docs/evidence/2026-09-22-h2-production-completion.md).
`VAL-H2` stays pending in the author's lane and did not block this closure.
The next checkpoint is `H3` (processes and applications), opened on
2026-09-22.

## H3 — closed 2026-09-22

Its falsifiable problem was whether every process the kernel lists could be
read, rated and grouped by the application that launched it from `/proc`
alone, on the sampler thread, so a person could search, sort, terminate and
kill their own processes from one table that keeps its place while the
numbers move. The delivered result is the release binary built, verified and
deployed to the author's prefix at `0.4.0`, whose Processes page lists every
process with live CPU, memory and IO, sortable columns, search, terminate and
kill behind a confirming dialog, whose Applications page groups the same rows
under their application's icon, and whose Performance list and process table
are both reachable by keyboard.

`H3-A` delivered the list operability and gates H2 booked. `H3-B` delivered
`hematita-core`'s process parsers, application scope decoding, per-PID CPU
sampler, passwd and the filter/sort/group projection, all under unit test and
captured from the author's machine — the
[core evidence](docs/evidence/2026-09-22-h3-core.md). `H3-C` wired it into the
binary: the sampler reads processes every second tick and publishes to both
hub objects from one thread, `HematitaProcesses` holds the table's state and
owns the only signal path, and the Processes and Applications pages are up
behind the strip — the
[processes page evidence](docs/evidence/2026-09-22-h3-processes-page.md).
`H3-D` answered its review: the sort header and the row cursor are reachable
from the keyboard, the last action's outcome is forgotten when it stops
being true, the IO counters are keyed by the process's start time, a
poisoned subscriber lock is reported, and the group row no longer overwrites
its own `checked` binding — the
[keyboard table evidence](docs/evidence/2026-09-22-h3-keyboard-table.md).
Units `H3-A` through `H3-Z` are in the archived
[plan](docs/plans/archive/2026-09-22-h3-processes.md). The checkpoint's
implementation exit ran on 2026-09-22: the
[completion evidence](docs/evidence/2026-09-22-h3-production-completion.md).
`VAL-H3` stays pending in the author's lane and did not block this closure.

`H3-E`, on 2026-09-22, is the whole-branch review's correction, delivered as
0.4.1 under its own archived
[plan](docs/plans/archive/2026-09-22-h3-table-fixes.md): the cursor is
re-anchored wherever the entries are rebuilt and a selection that left the
list is let go, the action outcome is cleared only by a new selection or a
new action, the signal path re-reads `/proc` and refuses a PID whose start
time or owner changed, the list is the table's one Tab stop, an application
folds from the keyboard, the user column sorts, the columns fit the minimum
window, the sampler hub can start again after a stop, the application row
reports itself checkable, and an unreadable `/proc` is announced before any
note about a row — the
[table fixes evidence](docs/evidence/2026-09-22-h3-table-fixes.md).

## H4 — opened 2026-09-22

`H4-A` delivers `hematita-core`'s hwmon discovery: the sensors work follows
in `H4-B`. Units and exit are in the
[active plan](docs/plans/active/2026-09-22-h4-sensors.md).
