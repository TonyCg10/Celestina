# Hematita implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** H3
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
| H3-B | planned | H3-A | `hematita-core`: `process`, `passwd`, `process_view`, captures | `cargo test -p hematita-core` |
| H3-C | planned | H3-B | sampler process section, `HematitaProcesses`, the table and both pages | `scripts/verify-production.sh` |
| H3-Z | planned | H3-C | implementation exit and 0.4.0 | `scripts/complete-production.sh` |

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

## H3 — opened 2026-09-22

`H3-A` delivers the list operability and gates H2 booked; the process work
follows in `H3-B` and `H3-C`. Units and exit are in the
[active plan](docs/plans/active/2026-09-22-h3-processes.md). `H3-B` delivers
`hematita-core`'s process parsers, application scope decoding, per-PID CPU
sampler, passwd and the filter/sort/group projection, all under unit test and
captured from the author's machine — the
[core evidence](docs/evidence/2026-09-22-h3-core.md). `H3-C` wires it into the
binary: the sampler reads processes every second tick and publishes to both
hub objects from one thread, `HematitaProcesses` holds the table's state and
owns the only signal path, and the Processes and Applications pages are up
behind the strip — the
[processes page evidence](docs/evidence/2026-09-22-h3-processes-page.md).
`H3-Z` closes the checkpoint.
