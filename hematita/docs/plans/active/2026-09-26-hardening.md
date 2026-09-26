# HEM-H1 — Hardening after the monorepo audit

- **Opened:** 2026-09-26
- **Plan ID:** hardening
- **Status:** active
- **Authorization:** after the 2026-09-26 monorepo audit the author asked
  for its whole program to be done; the findings and rulings are in
  [the audit evidence](../../../../docs/evidence/2026-09-26-monorepo-audit.md)
  and the Hematita findings in
  [the Hematita and Grafita record](../../../../docs/evidence/2026-09-26-monorepo-audit-hematita-grafita.md)
- **Scope:** hematita
- **Implementation checkpoint:** HEM-H1
- **Author-validation checkpoint:** none

## Hypothesis

Recognising mounts by identity instead of by lexical path, and bounding what
the storage scan and its publication hold, removes every way the storage
analyser can delete across a bind mount or exhaust memory, without changing
what the page shows for an ordinary folder.

## Tangible outcome

A permanent delete never crosses a mount reached through a symlinked root;
a scan of `/` stops at an entry ceiling with a typed error; duplicate
verdicts arrive coalesced and the page projects a bounded number of rows
through the core's `children_rows`; the action timeout is real; and the core
tests pass as root and as a user.

## Scope

- `HEM-H1-A` (P-9) — mount identity, an entry ceiling and bounded
  duplicate readers in `hematita-core`, with root-safe tests.
- `HEM-H1-B` (P-18) — bounded, coalesced storage publication and honest
  services, sampler and documents in the app.

## Exclusions

- HEM-16 (the single-instance hand-off copied between apps), unscheduled
  until a `celestina-core` owner is planned.
- Production builds and deployment, which run at the author's landing.

## Build order

1. `HEM-H1-A` on its own branch; it lands after the pipeline units of the
   suite plan (P-2).
2. `HEM-H1-B`, stacked on `HEM-H1-A`, after `RS-H1-A` (P-6) landed.

## Implementation exit

Each row's `Automated evidence` names its exit; every unit ends with
Hematita's `scripts/complete-production.sh` at landing, and `HEM-H1-A` also
redeploys Siderita, which links `hematita-core`.

## Change and commit ledger

Paths are repository-relative. Each row's `Intended change` ends with the
program id and the audit findings it closes.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| HEM-H1-A | `hematita:` | active | `celestina-rs/crates/hematita-core/src/usage/identity.rs`; `celestina-rs/crates/hematita-core/src/usage/mod.rs`; `celestina-rs/crates/hematita-core/src/usage/walk.rs`; `celestina-rs/crates/hematita-core/src/usage/remove.rs`; `celestina-rs/crates/hematita-core/src/usage/duplicates.rs`; `celestina-rs/crates/hematita-core/src/usage/tree.rs`; `celestina-rs/crates/hematita-core/tests/usage_tree.rs`; `hematita/src/usage_worker.rs`; `hematita/src/analysis_session.rs`; `hematita/src/analysis_view.rs`; `hematita/src/analysis.rs`; `hematita/src/actions.rs`; `hematita/STATUS.md`; `hematita/docs/evidence/2026-09-26-mount-and-delete-safety.md` | — | Compare statx mount ids in walk and delete, or canonicalise the root on the worker. Add an entry ceiling. Key duplicates by `st_size`. Open members `O_NOFOLLOW\|O_NONBLOCK` with a dev/ino check. Add `prune_many`. Make tests root-safe. (P-9: HEM-3, HEM-4, HEM-8, HEM-9, HEM-10, HEM-15) | [Mount and delete safety](../../evidence/2026-09-26-mount-and-delete-safety.md): `cargo test -p hematita-core --offline` green as root and as a user; new tests for a bind mount reached through a symlinked root, `TooManyEntries`, a FIFO member, `prune_many`. The landing redeploys Hematita and Siderita. | None |
| HEM-H1-B | `hematita:` | planned | `hematita/src/analysis_session.rs`; `hematita/src/analysis_workers.rs`; `hematita/src/services.rs`; `hematita/src/sampler.rs`; `hematita/src/processes.rs`; `hematita/qml/components/StoragePage.qml` and the other `bytesText` copies; `hematita/Cargo.toml` (zbus justification); `hematita/STATUS.md`; `hematita/README.md` | — | Project rows through `children_rows(MAX_ROWS)` with filters. Coalesce verdicts and mark incrementally. A real action-timeout watchdog. Correct zbus error typing. Live uid and name facts. Bounded cmdline read. A shutdown that cannot stall. One byte formatter. Adopt `desktop_entry::read`. STATUS and README truth. (P-18: HEM-1, HEM-2, HEM-5, HEM-6, HEM-7, HEM-11, HEM-12, HEM-13, HEM-14; RS-4 (hematita)) | Hematita tests (batching; row cap; error mapping; timeout); `qmllint-cxxqt.sh`; `check-documentation-contract.sh`; Hematita `complete-production.sh` | None |

This plan records intent; it grants no authority.
