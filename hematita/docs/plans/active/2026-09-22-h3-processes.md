# H3 — Processes and applications

- **Opened:** 2026-09-22
- **Plan ID:** h3-processes
- **Status:** active
- **Authorization:** the author asked to open the H3 plan on 2026-09-22
- **Scope:** hematita
- **Implementation checkpoint:** H3
- **Author-validation checkpoint:** `VAL-H3` in [VALIDATION.md](../../../VALIDATION.md)

## Hypothesis

Every process the kernel lists can be read, rated and grouped by the
application that launched it from `/proc` alone, on the sampler thread, and a
person can search, sort, terminate and kill their own processes from one
table that keeps its place while the numbers move.

## Tangible outcome

The installed 0.4.0 has a Processes page (search, sortable columns, terminate
and kill with a confirming dialog) and an Applications page (the same rows
under their application's icon and name), and the Performance list is
reachable by keyboard and screen reader.

## Scope

- `H3-A` — the list operability and gates booked by H2.
- `H3-B` — `hematita-core`: `process`, `passwd`, `process_view`, captures.
- `H3-C` — sampler process section, `HematitaProcesses`, the table, rows,
  dialog and both pages.
- `H3-Z` — implementation exit and 0.4.0.

## Exclusions

- Acting on other users' processes (H5, `pkexec`); services (H5); per-process
  network (never); process details beyond the columns (no tree, no threads,
  no open files).

## Build order

1. `H3-A`, then `H3-B`, `H3-C`, `H3-Z`.

## Implementation exit

`scripts/complete-production.sh` succeeds; the installed binary lists
processes with live CPU, memory and IO, sorts and filters them, groups them
by application, and terminates one of the user's own processes on request.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| H3-A | `hematita:` | done | [inventory](../../inventories/2026-09-22-h3-processes/H3-A.numstat.tsv) | 14 files, +401/-69 | Performance list reachable by keyboard and AT with the selection bound to `currentIndex`; smoke asserting a row's numbers against the kind contract; network subtitle only when ready; `no-rate` mapping narrowed; live core count; static sysfs facts cached by name; capture test asserting the disk set; dead arm removed; H3 opened | [list operability](../../evidence/2026-09-22-h3-list-operability.md) | `VAL-H2` |
| H3-B | `hematita:` | planned | `celestina-rs/crates/hematita-core/` | — | `/proc/PID` parsers, application scope decoding, per-PID CPU sampler, passwd, the filter/sort/group projection, captures | `cargo test -p hematita-core` | `VAL-H3` |
| H3-C | `hematita:` | planned | `hematita/src/`, `hematita/qml/`, `hematita/build.rs`, `hematita/Cargo.toml` | — | Process section in the snapshot with per-PID caches; `HematitaProcesses` lists, state and signals; `ProcessTable`, rows, `KillDialog`; Processes and Applications pages; style symlinks | `scripts/verify-production.sh` | `VAL-H3` |
| H3-Z | `hematita:` | planned | `hematita/`, `docs/version-history.tsv` | — | Implementation exit, 0.4.0, documents closed, plan archived | `scripts/complete-production.sh` | `VAL-H3` |
