# Hematita implementation roadmap

- **Status:** idle
- **Active implementation checkpoint:** none
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
| H4-B | done | H4-A | sampler sensors section, `HematitaSensors`, the Sensors page, the parked process-table items | `scripts/verify-production.sh` |
| H4-C | done | H4-B | the review's corrections: the hwmon facts re-validated by chip name, stable sensor rows, a smoke that steps through every section | `scripts/verify-production.sh` |
| H4-Z | done | H4-C | implementation exit and 0.5.0 | `scripts/complete-production.sh` |
| H4-D | done | H4-Z | the whole-branch review's corrections: the Sensors page one Tab stop with arrows by card, a failed tick showing only its reason, a smoke gate on the page's chip count, and 0.5.1 | `scripts/complete-production.sh` |
| H5-A | done | H4-D | `hematita-core`: `services`, the unit projection, the privileged-action outcome mapping | `cargo test -p hematita-core` |
| H5-B | done | H5-A | sampler services section, `HematitaServices`, the Services page, the privileged actions | `scripts/verify-production.sh` |
| H5-C | done | H5-B | the review's correction: the foreign-process path reachable, the outcome races, the dead bus connection, the pending wording | `scripts/verify-production.sh` |
| H5-Z | done | H5-C | implementation exit and 0.6.0 | `scripts/complete-production.sh` |

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
- H4-B: [sensors page](docs/evidence/2026-09-22-h4-sensors-page.md)
- H4-C: [sensor gates](docs/evidence/2026-09-22-h4-sensor-gates.md)
- H4-Z: [production completion](docs/evidence/2026-09-22-h4-production-completion.md)
- H4-D: [sensors fixes](docs/evidence/2026-09-22-h4-sensors-fixes.md)
- H5-A: [core](docs/evidence/2026-09-22-h5-core.md)
- H5-B: [services page](docs/evidence/2026-09-22-h5-services-page.md)
- H5-C: [privilege fixes](docs/evidence/2026-09-22-h5-privilege-fixes.md)
- H5-Z: [production completion](docs/evidence/2026-09-22-h5-production-completion.md)

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

## H4 — closed 2026-09-22

Its falsifiable problem was whether every channel hwmon exposes could be
read as a typed value with its unit, its kernel limits and the session's
extremes, from files alone, and shown per chip without the monitor's idle
cost growing. The delivered result is the release binary built, verified and
deployed to the author's prefix at `0.5.0`, whose Sensors page lists every
hwmon chip as a card, one row per channel, with the value, the session's
minimum and maximum, the kernel's own limit and a thermal load coloured by
the chip's own `crit`.

`H4-A` delivered `hematita-core`'s hwmon discovery: `ChannelKind`, `Channel`,
`Chip`, `parse_channel_file`, `convert` and `discover`, captured from the
author's processor, GPU and board chips, plus the cgroup regression test
`H3-B` deferred — the [core evidence](docs/evidence/2026-09-22-h4-core.md).
`H4-B` wired it into the sampler and the window: the sensors section is read
every tick, `publish::thermal_load` grades a temperature by the chip's own
`crit`, `HematitaSensors` publishes chips and channels with the session's
extremes, the Sensors page shows each chip as a `ListSection` card, and the
six process-table items parked by `H3-E` are closed — the
[sensors page evidence](docs/evidence/2026-09-22-h4-sensors-page.md). `H4-C`
answered its review: the hwmon facts and the session's extremes are
re-validated and keyed by the chip's own name rather than its `hwmonN`
index, the sensor rows are stable across ticks so the keyboard can hold one,
the smoke steps through every section so no page goes unconstructed, and the
unreachable `channelStates` field is struck from the published contract —
the [sensor gates evidence](docs/evidence/2026-09-22-h4-sensor-gates.md).
Units `H4-A` through `H4-Z` are in the archived
[plan](docs/plans/archive/2026-09-22-h4-sensors.md). The checkpoint's
implementation exit ran on 2026-09-22: the
[completion evidence](docs/evidence/2026-09-22-h4-production-completion.md).
`H4-D` answered the whole-branch review: the Sensors page is a `ListView`
that is its own single Tab stop with arrows that scroll card by card and rows
that take no focus of their own, a tick that cannot read `/sys/class/hwmon`
publishes empty lists so only its reason shows, and the smoke now fails unless
the page publishes at least one chip and one channel — the
[sensors fixes evidence](docs/evidence/2026-09-22-h4-sensors-fixes.md),
deployed as `0.5.1`, in its own archived
[plan](docs/plans/archive/2026-09-22-h4-sensors-fixes.md).
`VAL-H4` stays pending in the author's lane and did not block this closure.

## H5 — closed 2026-09-22

The delivered result is the release binary built, verified and deployed to
the author's prefix at `0.6.0`, whose Services page lists every user and
system unit from both buses, lets the person start, stop and restart their
own units freely, reaches a system unit or a foreign process only through
polkit, and reads the outcome of every privileged action as a typed
sentence, including a missing authentication agent, per [ADR
0010](../docs/decisions/0010-one-shot-privilege-through-polkit.md).

Its falsifiable problem was whether every systemd unit of the session and the
system can be listed on the sampler thread from both buses, the user's own
units started and stopped without authorisation, and every privileged
action reported truthfully, including the absence of an agent. The
author's written privilege decision is [ADR
0010](../docs/decisions/0010-one-shot-privilege-through-polkit.md): a
foreign process is signalled through `pkexec`, and a service is started,
stopped or restarted over the system bus through systemd rather than a
setuid helper of its own.

`H5-A` delivered `hematita-core`'s unit projection and outcome mapping:
`Scope`, `UnitKind` (read from a unit name's suffix), `Unit`, `is_actionable`
(a service or a socket only), `project` (the scope toggles, the
`services_only` kind filter and a name/description search, sorted by scope
then active state then name — `process_view`'s shape ported to units), and
`Outcome` with `outcome_of_dbus_error` and `outcome_of_pkexec`, which turn a
system-bus error name or a `pkexec` exit status into the typed result ADR
0010 asks every privileged action to report — the [core
evidence](docs/evidence/2026-09-22-h5-core.md). None of it is wired into the
sampler thread or a page yet — that is `H5-B`.

`H5-B` wired it to the machine: the sampler asks both managers for
`ListUnits` every fifth tick (and on the first, so the page fills at once),
each bus its own section, so the system bus answering while the session bus
does not is a page with half its rows and one sentence rather than a failure;
`HematitaServices` publishes the projection and is the only place a unit is
started, stopped or restarted, each action one call on a named worker thread
whose typed outcome is queued back to Qt; `privilege.rs` is the one place
`pkexec` runs, around `/usr/bin/kill` with a fixed argument shape, which is
how a process that is not the person's own is ended; and the Services page
lists every unit in one Tab stop with the search, the scope toggles and the
three actions, saying `pending`, `no-agent`, `denied`, `failed` or `refused`
in words. This session has no authentication agent, so the privileged paths
can only answer `no-agent` today and no action was performed during
verification — the [services page
evidence](docs/evidence/2026-09-22-h5-services-page.md). The three items
`H4-D` deferred closed with it: the sensor facts are re-enumerated every
thirtieth tick, a channel keeps its session extremes while its chip is
listed, and a power reading is graded against the chip's own cap.

`H5-C` answered its review: both process actions are offered on any
selected row — a foreign one asks for authorisation instead of being
disabled — an answer about the selected row outranks the note about who
owns it, each hub drops the outcome of an action superseded while its
prompt stood open, a bus connection that proves dead is reopened on the
next service tick, `pending` reads as an authorisation wait only for a
system unit, and both pages assign their rows under `anchoring` — the
[privilege fixes evidence](docs/evidence/2026-09-22-h5-privilege-fixes.md).
Units `H5-A` through `H5-Z` are in the archived
[plan](docs/plans/archive/2026-09-22-h5-services.md). The checkpoint's
implementation exit ran on 2026-09-22: the [completion
evidence](docs/evidence/2026-09-22-h5-production-completion.md). `VAL-H5`
stays pending in the author's lane and did not block this closure.

The five phases of the design are delivered; further work opens with a new
checkpoint and the author's word.
