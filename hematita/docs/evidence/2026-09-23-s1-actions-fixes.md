# The storage actions' review corrections — S1-E

- **Date:** 2026-09-23
- **Scope:** `S1-E` of
  [`../plans/archive/2026-09-23-s1-storage.md`](../plans/archive/2026-09-23-s1-storage.md):
  the four findings and three minor points of the S1-D review
- **Environment:** the author's checkout; offscreen Qt only, no window on
  the live or nested session; no scan, selection or action ran during
  verification
- **Artifact:** `hematita/target/release/hematita` built by
  `scripts/build-production.sh`, verified by `scripts/verify-production.sh`;
  not deployed (S1-Z deploys 1.1.0)

## What changed

1. **Cancellable actions.** `cancelAction()` (new invokable) cancels the
   running worker's token through `WorkerHandle::cancel` and keeps the
   handle until the worker reports. `actionRunning` (new property) shows
   an `x` button (`Cancelar`) in the actions capsule while a trash or
   deletion runs.
2. **Partial successes kept.** A cancelled worker still reports the ids it
   removed, with the new outcome token `cancelled` (contract vocabulary:
   `"" | done | partial | failed | refused | cancelled`); the page says
   `cancelado tras N de M`. `cancelAction()` does not move the action
   epoch, so the report lands and the hub prunes the removed ids while the
   scan generation is current; only a rescan or leaving the analysis
   (`stop_analysis`) moves it.
3. **Entries re-validated.** `usage::walk` records `dev` and `ino` from
   `symlink_metadata` on every `Node` (root included). `usage::remove`
   gains `Refusal::Changed` and `check_identity(path, dev, ino)`, which
   answers `Missing` when nothing is there and `Changed` when another
   entry is. Both the trash and the delete worker call it per item before
   acting; a mismatch counts as refused. **Remaining window:** between
   this check and the operation's own syscalls (`rename` for the trash,
   the listing and `unlink`/`rmdir` of `delete_tree`) another process can
   still swap the entry; the check narrows the window, it cannot close it
   without descriptor-relative operations, which `siderita_ops::trash` and
   `delete_tree` do not use.
4. **Mount roots refused for trash too.** The actions worker reads the
   mountinfo boundary set on its own thread and refuses, for both trash
   and deletion, any outermost item whose path is a mount root; it counts
   as refused, so the outcome is `refused` when every item was.
5. **Minors.** The confirmation questions count the outermost selection
   (`selectedCount`, new property), not `selectedIds.length`. The hidden
   duplicate groups are kept (`Findings.hidden`) and pruned like the shown
   ones, so `hiddenGroupCount` is recomputed after a pruning. The workers
   queue `apply_action_progress` after each finished item, which updates
   `actionDone` (and `actionTotal` is set when the action starts).

## Procedure

```sh
cd celestina-rs && cargo test -p hematita-core \
  && cargo clippy -p hematita-core --all-targets -- -D warnings
cd hematita && cargo fmt --all
hematita/scripts/build-production.sh
cd hematita && cargo test --release --locked --all-targets \
  && cargo clippy --release --all-targets --locked -- -D warnings \
  && cargo fmt --all --check
hematita/scripts/verify-production.sh
QT_QPA_PLATFORM=offscreen QT_ASSUME_STDERR_HAS_CONSOLE=1 \
  HEMATITA_SMOKE_SHAPE=1 HEMATITA_SMOKE_SECTIONS=1 \
  timeout 14 hematita/target/release/hematita
bash scripts/check-architecture-contract.sh
```

## Result

- **Exit:** 0 for every command; the walk ended by `timeout` (124).
- **Tests:** `hematita-core` `87`, `11`, `19` passed (new: a scanned node
  carries the metadata's device and inode, and `check_identity` answers
  `Changed` for a replaced file and `Missing` for a removed one);
  `hematita` `47 passed` (the outcome test gained `cancelled`; the pruning
  test covers a hidden group left with one copy).
- **Clippy and fmt:** clean.
- **qmllint:** `org.celestina.hematita (0 non-fatal baseline warning(s))`.
- **Smoke:** `smoke: OK`.
- **Walk:** `hematita-shape cpu 3 60`, `hematita-sensors 9 44`,
  `hematita-services 187 true true`, `hematita-storage 7`; no QML error.
- **Builds:** one application build cycle.
- **Architecture contract:** OK.

## Limits

- No action ran: cancelling, the refusals and the progress count are for
  `VAL-S1` on the real session.

## Follow-up

`S1-Z`. `VAL-S1` stays pending.
