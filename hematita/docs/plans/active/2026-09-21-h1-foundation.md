# H1 — Foundation and the Performance page

- **Opened:** 2026-09-21
- **Plan ID:** h1-foundation
- **Status:** active
- **Authorization:** the author approved the design and asked for the H1 plan
  on 2026-09-21
- **Scope:** hematita
- **Implementation checkpoint:** H1
- **Author-validation checkpoint:** `VAL-H1` in [VALIDATION.md](../../../VALIDATION.md)

## Hypothesis

A window that samples `/proc` once per second on its own thread can show CPU
and memory with sixty seconds of history, through parsers tested on text
alone, without the machine noticing the monitor.

## Tangible outcome

The registered project, a release binary installed under the author's prefix,
whose Performance page shows live CPU and memory with a one-minute graph and a
pill navigation strip naming the later sections.

## Scope

- `H1-A` — the skeleton: crate stub, application, strip, scripts, documents.
- `H1-B` — `hematita-core`: `ratio`, `cpu`, `memory`, `history`, captures.
- `H1-C` — sampler thread, `HematitaResources`, activation, Performance page.
- `H1-Z` — implementation exit and 0.2.0.

## Exclusions

- Everything in phases H2 to H5 of the
  [design](../../../../docs/superpowers/specs/2026-09-21-hematita-design.md).
- Any reference to the Celestina shell.

## Build order

1. `H1-A`, then `-B`, `-C`, `-Z`.

## Implementation exit

`scripts/complete-production.sh` succeeds and the installed binary shows live
CPU and memory graphs.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| H1-A | `hematita:` | done | [inventory](../../inventories/2026-09-21-h1-foundation/H1-A.numstat.tsv) | 42 files, +2260/-0 | Crate stub with the shared percentage, application skeleton opening the window with the pill navigation strip, production scripts, smoke, the document set | [skeleton](../../evidence/2026-09-21-h1-skeleton.md) | `VAL-H1` |
| H1-B | `hematita:` | planned | `celestina-rs/crates/hematita-core/` | — | `/proc/stat`, `/proc/meminfo`, frequency and model parsers; samplers; the ring; captures | `cargo test -p hematita-core` | `VAL-H1` |
| H1-C | `hematita:` | planned | `hematita/src/`, `hematita/qml/`, `hematita/build.rs` | — | Sampler thread, snapshot, `HematitaResources`, activation, Performance page with graph | `scripts/verify-production.sh` | `VAL-H1` |
| H1-Z | `hematita:` | planned | `hematita/`, `docs/version-history.tsv` | — | Implementation exit, 0.2.0, status and roadmap closed, plan archived | `scripts/complete-production.sh` | `VAL-H1` |
