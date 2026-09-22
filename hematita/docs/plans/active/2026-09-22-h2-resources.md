# H2 — Every resource on the Performance page

- **Opened:** 2026-09-22
- **Plan ID:** h2-resources
- **Status:** active
- **Authorization:** the author asked to open the H2 plan on 2026-09-22
- **Scope:** hematita
- **Implementation checkpoint:** H2
- **Author-validation checkpoint:** `VAL-H2` in [VALIDATION.md](../../../VALIDATION.md)

## Hypothesis

The side list can become a live inventory of the machine — every whole disk,
every interface, the GPU — read from `/proc` and `/sys` once a second by the
same thread, each with its own minute of history and its own honest absence,
without the monitor's idle cost becoming visible.

## Tangible outcome

The installed 0.3.0 shows a Performance list with the processor (and a
per-core grid), memory with swap, the AMD GPU, each whole disk and each
network interface; a resource that cannot be read says so in its own row with
a Spanish sentence composed in QML from a typed reason.

## Scope

- `H2-A` — `hematita-core`: `rate`, `disk`, `network`, `gpu`, ring fractions,
  captures.
- `H2-B` — sampler sections and topology, `publish.rs`, `HematitaResources`
  as index-aligned lists, the page weaving rows, kind-driven detail, the
  per-core grid, per-resource failure state, and the follow-ups H1 booked
  (banner in the layout, `NavItem` checked state, graph token and motion,
  `NavStrip` name, environment before bus work, ring alignment, `generation`).
- `H2-Z` — implementation exit and 0.3.0.

## Exclusions

- Sensors (H4) — the GPU card shows busy, memory and clocks only; its
  temperature and power arrive with hwmon.
- Non-AMD GPUs; per-process anything (H3); history persistence.

## Build order

1. `H2-A`, then `H2-B`, then `H2-C`, then `H2-Z`.

## Implementation exit

`scripts/complete-production.sh` succeeds and the installed binary lists the
processor, memory, GPU, every whole disk and every interface with live
graphs.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| H2-A | `hematita:` | done | [inventory](../../inventories/2026-09-22-h2-resources/H2-A.numstat.tsv) | 16 files, +1011/-26 | Named-counter rates, diskstats, net/dev and amdgpu parsers, sysfs helpers, ring fractions, captures; H2 opened in the documents | [core](../../evidence/2026-09-22-h2-core.md) | `VAL-H2` |
| H2-B | `hematita:` | done | [inventory](../../inventories/2026-09-22-h2-resources/H2-B.numstat.tsv) | 16 files, +1399/-299 | Snapshot sections with typed reasons and topology; `publish.rs` tested; `HematitaResources` as lists with a revision; the page weaving rows, kind-driven detail, per-core grid, per-row failure state; the H1 follow-ups | [resources page](../../evidence/2026-09-22-h2-resources-page.md) | `VAL-H2` |
| H2-C | `hematita:` | done | [inventory](../../inventories/2026-09-22-h2-resources/H2-C.numstat.tsv) | 8 files, +202/-31 | Sort disks and interfaces by name; keep the list model stable across revisions; the GPU row's reason names the failing file; toggle and subtitle fixes | [list stability](../../evidence/2026-09-22-h2-list-stability.md) | `VAL-H2` |
| H2-Z | `hematita:` | planned | `hematita/`, `docs/version-history.tsv` | — | Implementation exit, 0.3.0, documents closed, plan archived | `scripts/complete-production.sh` | `VAL-H2` |
