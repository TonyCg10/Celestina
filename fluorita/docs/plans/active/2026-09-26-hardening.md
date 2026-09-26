# FLU-H1 — Hardening after the monorepo audit

- **Opened:** 2026-09-26
- **Plan ID:** hardening
- **Status:** active
- **Authorization:** after the 2026-09-26 monorepo audit the author asked
  for its whole program to be done; the findings and rulings are in
  [the audit evidence](../../../../docs/evidence/2026-09-26-monorepo-audit.md)
  and the Fluorita findings in
  [the Fluorita record](../../../../docs/evidence/2026-09-26-monorepo-audit-fluorita.md)
- **Scope:** fluorita
- **Implementation checkpoint:** FLU-H1
- **Author-validation checkpoint:** `VAL-FLU-EDIT`, `VAL-FLU-METADATA` and
  `VAL-FLU-TEARDOWN` in [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

Making Replace place the new bytes and then send the original to the Trash,
and bounding every value a file controls, restores ADR 0009's recoverable
original and removes the crashes a crafted file can cause, without adding an
encoder or changing what an ordinary edit produces.

## Tangible outcome

After any same-format Replace (edit, metadata write or batch) the original is
in the Trash and the result keeps the original's mode; a crafted duration or
tag cannot abort Fluorita or Siderita's player; location removal strips XMP
GPS and MPF images; redaction is a solid fill by default; and the library
stays true, off the GUI thread, after in-place edits and folder moves.

## Scope

- `FLU-H1-A` (P-7) — land edits safely and bound what files claim.
- `FLU-H1-B` (P-14) — keep the catalogue true and off the GUI thread, keep
  the editor's promises, one owner for the playback handshake, and threads
  that end; it may split into a library commit and a player and editor
  commit.

## Exclusions

- FLU-22 (the first-run seed guessing XDG folder names), unscheduled until a
  `celestina-core` owner for `user-dirs.dirs` is planned.
- Production builds, the `libmpv` tests and deployment, which run on the
  author's machine at landing.

## Build order

1. `FLU-H1-A` after `RS-H1-A` (P-6) landed.
2. `FLU-H1-B`, stacked on `FLU-H1-A`.

## Implementation exit

Each row's `Automated evidence` names its exit; every unit ends with
Fluorita's `scripts/complete-production.sh` at landing, and the landing also
rebuilds Siderita and Magnetita, which link the Fluorita crates. Under ruling
R-A2 only Fluorita's version moves.

## Change and commit ledger

Paths are repository-relative. Each row's `Intended change` ends with the
program id and the audit findings it closes.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| FLU-H1-A | `fluorita:` | planned | `celestina-rs/crates/fluorita-engine/src/edit.rs`; `celestina-rs/crates/fluorita-engine/src/metadata.rs`; `celestina-rs/crates/fluorita-engine/src/probe.rs`; `celestina-rs/crates/fluorita-engine/src/worker.rs`; `celestina-rs/crates/fluorita-engine/src/catalogue_store.rs`; a single duration helper; `fluorita/cpp/imagecanvas.cpp` (redaction default); the engine tests | — | Replace writes a hidden sibling, fsyncs it, trashes the original through `siderita-ops`, then renames. Land media through the P-6 writer. Add a single `try_from_secs_f64` helper. Sanitise and cap probed tags, and keep a catalogue that fails to load. Create the cancel token in `submit`. Strip XMP GPS and MPF. Make solid fill the default redaction. (P-7: FLU-1, FLU-2, FLU-3, FLU-4, FLU-6, FLU-15, FLU-16) | `cargo test -p fluorita-core -p fluorita-engine --offline` on the author's machine (needs libmpv): original found in the bin (rewrite the `edit.rs:718` test), 0600 preserved, copy refuses to clobber, `f64::MAX` duration, tag cap, cancel-right-after-submit, XMP and MPF fixtures. Fluorita `complete-production.sh`; the landing also rebuilds Siderita (and Magnetita after P-2). | `VAL-FLU-EDIT`, `VAL-FLU-METADATA` |
| FLU-H1-B | `fluorita:` | planned | `fluorita/src/library/work.rs`; `fluorita/src/library/project.rs`; `fluorita/src/library.rs`; `fluorita/src/player.rs`; `fluorita/src/editor.rs`; `fluorita/src/mpris.rs`; `fluorita/src/folders.rs`; `fluorita/src/activation.rs`; `fluorita/src/rasteriser.rs`; `celestina-rs/crates/fluorita-engine/src/` (`watch.rs`, `library.rs`, `trailer.rs`, `artwork.rs`, `edit_store.rs`, session loop); `celestina-rs/crates/fluorita-core/` (handshake state machine); `celestina-rs/crates/fluorita-qt/cpp/mpvvideoitem.cpp`; `fluorita/qml/components/EditObjectLayer.qml`; `fluorita/STATUS.md`; `fluorita/README.md` | — | Cancellable resync. Project on a worker with `Arc`. Forget same-path stale records and probe touched audio. Turn directory events into resyncs. Track completeness per root. Fix the relative dotted-ancestor check. Close portal requests on cancel. Probe stills on a worker. Reopen copies from bounded recipes. Keyboard and accessible annotation selection. Shared render-state object in `fluorita-qt`. Extract the handshake state machine and session loop into `fluorita-core`/`fluorita-engine`, bumping the generation on every close. Real MPRIS rate. Remove the trailer encoder. Spec thumbnail chunks. Typed bridge structs. `Builder::spawn`. Adopt `file_uri`. Correct the docs. (P-14: FLU-5, FLU-7, FLU-8, FLU-9, FLU-10, FLU-11, FLU-13, FLU-14, FLU-17, FLU-18, FLU-19, FLU-20, FLU-24, FLU-25, FLU-26, FLU-27, FLU-28, FLU-29, FLU-30; FLU-23) | `cargo test -p fluorita-core -p fluorita-engine` (handshake: close before publish, release before closing; per-root completeness; directory move → resync); the app tests; `rg -n libx264 celestina-rs` empty; Fluorita `complete-production.sh` (the landing also rebuilds Siderita and Magnetita) | `VAL-FLU-EDIT`, `VAL-FLU-METADATA`, `VAL-FLU-TEARDOWN` |

This plan records intent; it grants no authority.
