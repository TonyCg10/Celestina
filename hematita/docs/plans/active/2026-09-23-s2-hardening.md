# S2 — Hardening the storage analyzer

- **Opened:** 2026-09-23
- **Plan ID:** s2-hardening
- **Status:** active
- **Authorization:** the author asked on 2026-09-23 to close what the S1
  reviews left open, planned in
  [the S2 plan](../../../../docs/superpowers/plans/2026-09-23-hematita-s2-hardening.md)
- **Scope:** hematita
- **Implementation checkpoint:** S2
- **Author-validation checkpoint:** `VAL-S2` in [VALIDATION.md](../../../VALIDATION.md)

## Hypothesis

A deletion that walks descriptors cannot be redirected, and a tree that
grafts what a stopped deletion left behind never shows a size that is no
longer true.

## Tangible outcome

The installed 1.1.1 deletes permanently through directory descriptors, so a
folder swapped for a symbolic link while the deletion runs cannot send it
elsewhere; a deletion that fails or is cancelled midway leaves a tree whose
sizes match the disk; the analysis hub is split so its session state has one
owner; and the confirmation dialog names exactly what will be acted on.

## Scope

- `S2-A` — `hematita-core::usage`: the permanent deletion rewritten on
  directory descriptors (`rustix` `openat`/`statat`/`unlinkat`/`fstat`/`Dir`)
  returning what it removed when it stops midway, the walk's hard-link set
  gated on `nlink > 1`, `scan_subtree`, `Tree::graft` and `Tree::is_live`;
  S2 opened in the documents.
- `S2-A2` — the review's corrections of `S2-A`: a read-only pre-pass that
  refuses an inner mount before anything is removed, every opened folder
  re-checked by `fstat`, the depth cap at 256, a missing folder on the way
  reported as missing.
- `S2-B` — the hub split (`analysis_session.rs`), partial removals grafted
  from a fresh sub-scan, the confirm worker's `Arc` dropped before
  `make_mut`, dead ids rejected, the dialog's confirmed count bound.
- `S2-B2` — the review's corrections of `S2-B`: a cancel inside the last
  item reads as cancelled, the confirmation bound to a selection revision
  instead of its count, the refusal naming the live count, the graft's
  unreadable counts aligned before extending, and the workers going
  through session methods.
- `S2-Z` — implementation exit and 1.1.1.

## Exclusions

- Persisting scan results; scanning across mounts; any privilege; any shell
  integration (the shell is in standby); new storage features.

## Build order

1. `S2-A`, `S2-A2`, then `S2-B`, `S2-B2`, then `S2-Z`.

## Implementation exit

`scripts/complete-production.sh` succeeds; the installed binary deletes
through descriptors, grafts a fresh sub-scan in place of what a stopped
deletion left, and its confirmation dialog's count equals what is acted on.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| S2-A | `hematita:` | done | [inventory](../../inventories/2026-09-23-s2-hardening/S2-A.numstat.tsv) | 14 files, +1054/-161 | `usage::remove` on directory descriptors with `Failure { error, removed }` and the scanned identity checked in `delete_tree`; `walk` hard-link set gated on `nlink > 1`, `hard_link_names`, `scan_subtree`; `Tree::graft`, `Tree::is_live`; S2 opened | [core](../../evidence/2026-09-23-s2-core.md) | `VAL-S2` |
| S2-A2 | `hematita:` | done | [inventory](../../inventories/2026-09-23-s2-hardening/S2-A2.numstat.tsv) | 10 files, +350/-38 | Deletion refuses an inner mount before touching anything again; opened folders re-checked by fstat; honest depth cap; missing components reported as missing | [core fixes](../../evidence/2026-09-23-s2-core-fixes.md) | `VAL-S2` |
| S2-B | `hematita:` | done | [inventory](../../inventories/2026-09-23-s2-hardening/S2-B.numstat.tsv) | 16 files, +1528/-863 | hub split into `analysis_session.rs` (session state and rules) and `analysis_workers.rs` (worker glue) under `analysis.rs` (bridge, navigation, publication); partial removals grafted from a fresh sub-scan; the confirm worker holds a `Weak<Tree>`; live-id check; bound confirmation count | [hub](../../evidence/2026-09-23-s2-hub.md) | `VAL-S2` |
| S2-B2 | `hematita:` | done | [inventory](../../inventories/2026-09-23-s2-hardening/S2-B2.numstat.tsv) | 11 files, +373/-68 | a cancel inside the last item is `cancelled` (`Step::Partial { gone, cancelled }`); confirmation bound to `selectionRevision`; refusal names the live count; `graft` resizes `unreadable` first; workers call session methods | [hub fixes](../../evidence/2026-09-23-s2-hub-fixes.md) | `VAL-S2` |
| S2-Z | `hematita:` | planned | documents, version | — | implementation exit and 1.1.1 | `scripts/complete-production.sh` | `VAL-S2` |
