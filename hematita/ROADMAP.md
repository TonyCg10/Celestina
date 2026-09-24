# Hematita implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** S2
- **Related author validation:** `VAL-S2` in [VALIDATION.md](VALIDATION.md)
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
| S1 | Storage: mount points, browsing, a device-bounded scan, size list and treemap, duplicates and empty folders, trash and guarded deletion |
| S2 | Storage hardening: a descriptor-based deletion with partial results, grafted sub-scans, the hub split |

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
| H5-D | done | H5-Z | the whole-branch review's corrections: the polkit interaction flag, the action and listing timeouts, the outcome mapping, and 0.6.1 | `scripts/complete-production.sh` |
| VIS-1 | done | H5-D | the author's first real-session look: strip alignment, sparkline inset, colour per kind, the table card hierarchy, readable names, sentinel limits, the disk-rate dash, and 0.6.2 | `scripts/complete-production.sh` |
| VIS-2 | done | VIS-1 | the lists keep their viewport across a refresh, only the person's cursor scrolls them, applications open folded, and 0.6.3 | `scripts/complete-production.sh` |
| REL-1 | done | VIS-2 | record `VAL-VIS-2`, release 1.0.0, no code change | `scripts/complete-production.sh` |
| S1-A | done | REL-1 | `hematita-core::usage`: the walk, the tree, empty folders, duplicates, the treemap layout, the guarded deletion, mounts | `cargo test -p hematita-core` |
| S1-A2 | done | S1-A | the review's corrections: the deletion refuses a symlinked component, empty means no non-directory below, mount points from `mountinfo` bound the walk and the deletion | `cargo test -p hematita-core` |
| S1-B | done | S1-A2 | locations, browsing, the storage section and its smoke line | `scripts/verify-production.sh` |
| S1-C | done | S1-B | the scan with progress and cancel, size list and treemap, filters, duplicate confirmation | `scripts/verify-production.sh` |
| S1-D | done | S1-C | actions: open in Siderita, batch trash, guarded permanent deletion, selection | `scripts/verify-production.sh` |
| S1-E | done | S1-D | the review's corrections: actions cancellable keeping their partial successes, entries re-validated by device and inode, mount roots refused for trash too | `scripts/verify-production.sh` |
| S1-Z | done | S1-E | implementation exit and 1.1.0 | `scripts/complete-production.sh` |
| S2-A | done | S1-Z | `hematita-core::usage`: the deletion on directory descriptors with partial results, the hard-link set gated on `nlink > 1`, `scan_subtree`, `Tree::graft` and `Tree::is_live` | `cargo test -p hematita-core` |
| S2-A2 | done | S2-A | the review's corrections: the deletion refuses an inner mount before touching anything again, opened folders re-checked by `fstat`, an honest depth cap, missing components reported as missing | `cargo test -p hematita-core` |
| S2-B | planned | S2-A2 | the hub split, partial removals grafted, the `Arc` dropped before `make_mut`, dead ids rejected, the bound confirmation count | `scripts/verify-production.sh` |
| S2-Z | planned | S2-B | implementation exit and 1.1.1 | `scripts/complete-production.sh` |

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
- H5-D: [privilege fixes 2](docs/evidence/2026-09-22-h5-privilege-fixes-2.md)
- H5-Z: [production completion](docs/evidence/2026-09-22-h5-production-completion.md)
- VIS-1: [visual feedback](docs/evidence/2026-09-23-visual-feedback.md)
- VIS-2: [scroll and folds](docs/evidence/2026-09-23-scroll-and-folds.md)
- REL-1: [release](docs/evidence/2026-09-23-release-1.md)
- S1-A: [core](docs/evidence/2026-09-23-s1-core.md)
- S1-A2: [core fixes](docs/evidence/2026-09-23-s1-core-fixes.md)
- S1-B: [locations](docs/evidence/2026-09-23-s1-locations.md)
- S1-C: [analysis](docs/evidence/2026-09-23-s1-analysis.md)
- S1-D: [actions](docs/evidence/2026-09-23-s1-actions.md)
- S1-E: [actions fixes](docs/evidence/2026-09-23-s1-actions-fixes.md)
- S1-Z: [production completion](docs/evidence/2026-09-23-s1-production-completion.md)
- S2-A: [core](docs/evidence/2026-09-23-s2-core.md)
- S2-A2: [core fixes](docs/evidence/2026-09-23-s2-core-fixes.md)

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

`H5-D` answered the whole-branch review, and its critical finding is one no
run on this session could have caught: a unit action was sent without the
D-Bus `ALLOW_INTERACTIVE_AUTHORIZATION` flag, which systemd passes to polkit
as `AllowUserInteraction`, so polkit would never have consulted an agent and
would have refused every system-unit action with
`InteractiveAuthorizationRequired` — the very name the outcome mapping reads
as "no agent", which on a session that has none is indistinguishable from
the truth. The flag is now set, `NotAuthorized` reads as `denied` instead,
the action waits five minutes on its own connection and the listing two, the
hub refuses a unit its latest listing does not carry and publishes the acted
manager, and `AGENTS.md`, `VAL-H5` and the foreign kill question were
corrected — the [privilege fixes 2
evidence](docs/evidence/2026-09-22-h5-privilege-fixes-2.md), deployed as
`0.6.1`, in its own archived
[plan](docs/plans/archive/2026-09-22-h5-privilege-fixes.md). ADR 0010's
Consequences gained the sentence naming the flag.

The five phases of the design are delivered; further work opens with a new
checkpoint and the author's word.

## VIS-1 — closed 2026-09-23

The author looked at 0.6.1 on the real session for the first time and
recorded `VAL-H1`, `VAL-H3` and `VAL-H4` as failed: the strip's items sat at
the top of their pills, the sparklines ran past their rounded corners, every
graph was the same colour, the Processes bar, actions and table read as one
flat surface with rows past the card's corners, path-named processes and
unit file names were unreadable, sensor labels were the kernel's
abbreviations with an NVMe limit of 65 261.8 °C, and every own process's
disk rate read "—". `VIS-1` corrected all of it without changing a contract
beyond the additive `processDisplayNames` list — the [visual feedback
evidence](docs/evidence/2026-09-23-visual-feedback.md), deployed as `0.6.2`,
in its own archived [plan](docs/plans/archive/2026-09-23-visual-feedback.md).
`VAL-VIS-1` asks the author to look at the same screens again.

## VIS-2 — closed 2026-09-23

Running `VAL-VIS-1` on 0.6.2, the author found that every refresh threw the
process table back to its top, and that the Aplicaciones page opened with
every application unfolded. `VIS-2` restores each list's viewport after its
integer model is reset, lets only the person's cursor move the view, and
inverts the fold map so an application is folded until opened — the [scroll
and folds evidence](docs/evidence/2026-09-23-scroll-and-folds.md), deployed as
`0.6.3`, in its own archived
[plan](docs/plans/archive/2026-09-23-scroll-and-folds.md). `VAL-VIS-2` asks the
author to check both.

## Version 1 — released 2026-09-23

The author ran `VAL-VIS-2` on the deployed `0.6.3` and reported that the
process table kept its scroll position across refreshes, applications opened
folded, and there was no flicker. On that validation the author asked to
close version 1. `REL-1` recorded `VAL-VIS-2` as passed, recorded `VAL-H1`,
`VAL-H3` and `VAL-H4` as passed through the same remediation, and released
`1.0.0`: the five design phases `H1` through `H5` (Performance with every
resource, Processes and Applications, Sensors, and Services with
polkit-mediated privileged actions) plus the two visual correction units
`VIS-1` and `VIS-2` are what the author accepts as version 1, per the
[release evidence](docs/evidence/2026-09-23-release-1.md), in its own archived
[plan](docs/plans/archive/2026-09-23-release-1.md).

## S1 — closed 2026-09-23

The author asked for a storage analyzer inside Hematita: Baobab's result is
unclear and leaves little to do with what it finds. Its falsifiable problem
was whether a device-bounded walk with progress can analyse the author's
430 GiB home while the window stays live, and whether a size list beside a
treemap, with verified duplicates and empty folders, lets the author act on
what is found without leaving Hematita. The delivered result is the release
binary built, verified and deployed to the author's prefix at `1.1.0`, whose
Almacenamiento section lists the mount points with their occupation, browses
any of them, scans the folder one is in with visible progress and
cancellation, shows the result as a size list beside a treemap, finds
duplicates verified by content and empty folders, and acts on a selection
through Siderita's trash or a confirmed permanent deletion, refusing the
scanned root, anything outside it, any mount root and any symlinked
component.

Units `S1-A` through `S1-Z` are in the archived
[plan](docs/plans/archive/2026-09-23-s1-storage.md). The checkpoint's
implementation exit ran on 2026-09-23: the
[completion evidence](docs/evidence/2026-09-23-s1-production-completion.md).
The design spec is
[here](../docs/superpowers/specs/2026-09-23-hematita-storage-design.md).
`VAL-S1` stays pending in the author's lane and did not block this closure.
Further work opens with a new checkpoint and the author's word.

## S2 — opened 2026-09-23

The S1 reviews left three things open: the permanent deletion re-resolved
paths between its listing and its removals, so a folder swapped for a link
in that window could redirect it; a deletion that failed or was cancelled
midway left the tree showing sizes that were no longer true; and the
analysis hub had grown past what one file should own. Its falsifiable
problem is whether a deletion that walks descriptors cannot be redirected,
and whether a tree that grafts what a stopped deletion left behind never
shows a size that is no longer true. The
[plan](docs/plans/active/2026-09-23-s2-hardening.md) orders `S2-A` (the
descriptor-based deletion and the graft in `hematita-core::usage`,
[core evidence](docs/evidence/2026-09-23-s2-core.md)), `S2-B` (the hub split
and the grafting in the application) and `S2-Z` (1.1.1); `VAL-S2` is the
author's check on the real session.
