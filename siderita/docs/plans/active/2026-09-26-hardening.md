# SID-H1 — Hardening after the monorepo audit

- **Opened:** 2026-09-26
- **Plan ID:** hardening
- **Status:** active
- **Authorization:** after the 2026-09-26 monorepo audit the author asked
  for its whole program to be done; the findings and rulings are in
  [the audit evidence](../../../../docs/evidence/2026-09-26-monorepo-audit.md)
  and the Siderita findings in
  [the Siderita record](../../../../docs/evidence/2026-09-26-monorepo-audit-siderita.md)
- **Scope:** siderita
- **Implementation checkpoint:** SID-H1
- **Author-validation checkpoint:** none

## Hypothesis

Siderita's loss-free and contained-extraction promises fail only at their
edges (a symlink chain, a race, a missing `fsync`, a detached worker, a
blocking call on the Qt thread), so each can be restored inside the owning
crate or controller, with a test that reproduces the failure, without
redesigning the file domain.

## Tangible outcome

A crafted archive cannot write outside the folder the person chose; a copy,
move, trash or restore never deletes a file it did not create and never
removes a source before its copy is durable; quitting or closing a tab ends
every job cleanly; and undo, the Trash verbs, the device listings and the
remaining filesystem calls leave the Qt thread. Each failure the audit
reproduced has a regression test.

## Scope

- `SID-H1-A` (P-4) — contain archive extraction to its root and keep the
  archive tool's password off the command line.
- `SID-H1-B` (P-8) — make the `siderita-ops` verbs keep their loss-free
  promise.
- `SID-H1-C` (P-15) — deterministic job lifecycle, no stale asynchronous
  answers, no blocking work on the Qt thread, the shared owners from
  `RS-H1-A` and the handshake from `FLU-H1-B` adopted, and the documentation,
  dialog and motion fixes.

## Exclusions

- The unscheduled Minor findings SID-21 (three `unsafe localtime_r` copies),
  SID-22 (duplicated calendar and passwd recipes) and SID-23 (file-domain
  logic in the controller); each is taken when its file is next touched.
- Production builds, verification with Qt and deployment, which run at the
  author's landing.
- The author's own pass on a real session.

## Build order

1. `SID-H1-A` from `main`.
2. `SID-H1-B` on its own branch; it lands after the pipeline units of the
   suite plan (P-2).
3. `SID-H1-C`, stacked on `SID-H1-B`, after `RS-H1-A` (P-6) and `FLU-H1-B`
   (P-14) landed; if `FLU-H1-B` slips, land the two-line port of FLU-12 first.

## Implementation exit

Each row's `Automated evidence` names its exit; every unit ends with
Siderita's `scripts/complete-production.sh` at landing. The `controller.rs`
and `FolderView.qml` architecture ratchets do not grow.

## Change and commit ledger

Paths are repository-relative. Each row's `Intended change` ends with the
program id and the audit findings it closes.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| SID-H1-A | `siderita:` | planned | `celestina-rs/crates/siderita-archive/src/member.rs`; `celestina-rs/crates/siderita-archive/src/extract.rs`; `celestina-rs/crates/siderita-archive/src/tool.rs`; the crate's tests | — | Never write through a symlinked ancestor. Create links last. Resolve tool-path links canonically. Feed the 7z password on stdin. Bound tool output. (P-4: SID-1, SID-17) | `cargo test -p siderita-archive --offline` including the ported chained-link PoC tar and a tool-path escape test; Siderita `complete-production.sh` | None |
| SID-H1-B | `siderita:` | planned | `celestina-rs/crates/siderita-ops/src/` (`copy.rs`, `relocate.rs`, `restore.rs`, `trash.rs`, `volume.rs`, `purge.rs`); `siderita/src/controller/paste.rs` (Replace ordering) | — | Roll back only what this call created. fsync files and directories before removing a source, then remove only the copied entries. Use `rename_without_replacing` for trash and restore. Resolve relative `Path=`. Harden the `.Trash-$uid` choice and get the uid safely. Write local `DeletionDate`. Make Replace place-then-trash. Delete the dead `unwritable`. (P-8: SID-2, SID-3, SID-4, SID-14, SID-15, SID-16, SID-20, SID-31) | `cargo test -p siderita-ops --offline`: 200-round concurrent-create race with no `Ok` and missing file; relative-record restore; newcomer survives a forced copy-move; orphan `files/<name>` not replaced; symlinked `.Trash-$uid` refused; Replace-ordering test in the app crate. The landing redeploys Siderita, Fluorita and Hematita (and Magnetita after P-2). | None |
| SID-H1-C | `siderita:` | planned | `siderita/src/main.rs`; `siderita/src/controller/` (`jobs.rs`, `fileops.rs`, `archive.rs`, `scan.rs`, `find.rs`, `trash.rs`, `mounts.rs`, `paste.rs`, `selection.rs`, `marks.rs`, `shell.rs`, `glyphs.rs`); `siderita/src/devices.rs`; `siderita/src/dbus.rs`; `siderita/src/portal.rs`; `siderita/src/media.rs`; `siderita/src/apps.rs`; `siderita/src/ownicon.rs`; `siderita/cpp/thumbnailprovider.cpp`; `celestina-rs/crates/siderita-core/src/executor.rs`; `celestina-rs/crates/siderita-embedded/src/pe.rs`; `siderita/qml/` (`dialogs/NamePromptDialog.qml`, `CompressDialog.qml`, `OperationRing.qml`, `SidebarChevron.qml`, `FolderWheelHandler.qml`, `components/SizeRow.qml`); `siderita/STATUS.md`; `siderita/README.md` | — | Keep and join worker handles, asking or cancelling on quit. End jobs from the worker. Replay suppressed folder changes. Refresh must not cancel navigation. Add generations to search, Trash and Recientes. Run undo and the Trash verbs as jobs. Keep one process-wide device model on a worker. Move the remaining syscalls off-thread. Bound the PE reader. Use a bounded, cancellable thumbnail pool. Make the scan executor fallible. Adopt the P-14 handshake and delete Siderita's copy. Adopt `file_uri` and `desktop_entry::scan` (`open_with` and `ownicon` off-thread). Portal filter escaping. Dialog roles. Reduced motion. `CelestinaSlider` in `SizeRow`. Docs. (P-15: SID-5, SID-6, SID-7, SID-8, SID-9, SID-10, SID-11, SID-12, SID-13, SID-19, SID-24, SID-25, SID-26, SID-27, SID-28, SID-30; FLU-12; SID-18/RS-1, RS-4 (adoption); STY-4) | Siderita app tests and QML tests (Qt on the author's machine); `cargo test -p siderita-core -p siderita-embedded` (a sparse 4 GB `.exe` reads a bounded number of bytes); search-generation test; the `controller.rs` and `FolderView.qml` ratchets do not grow; Siderita `complete-production.sh` | None |

This plan records intent; it grants no authority.
