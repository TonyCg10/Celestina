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
- `SID-H1-D` — SID-20, split out of `SID-H1-B` (ruling R-A17): the Trash's
  `DeletionDate` in local time, once `celestina-core` owns a local-time
  facility every `trash` consumer can pass.

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
4. `SID-H1-D` after a `celestina-core` local-time owner exists.

## Implementation exit

Each row's `Automated evidence` names its exit; every unit ends with
Siderita's `scripts/complete-production.sh` at landing. The `controller.rs`
and `FolderView.qml` architecture ratchets do not grow.

## Change and commit ledger

Paths are repository-relative. Each row's `Intended change` ends with the
program id and the audit findings it closes.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| SID-H1-A | `siderita:` | done | [inventory](../../inventories/2026-09-26-hardening/SID-H1-A.numstat.tsv) | 18 files, +3699/-295 | Never write through a symlinked ancestor. Create links last. Resolve tool-path links canonically. Feed the 7z and unrar password on stdin. Bound tool output. (P-4: SID-1, SID-17) | [evidence](../../evidence/2026-09-26-archive-extraction.md) | `VAL-SID-16` |
| SID-H1-B | `siderita:` | done | [inventory](../../inventories/2026-09-26-hardening/SID-H1-B.numstat.tsv) | 25 files, +2957/-280 | Roll back only what this call created. fsync files and directories before removing a source, then remove only the copied entries. Use `rename_without_replacing` for trash and restore. Resolve relative `Path=` and keep a volume's records on that volume. Harden the `.Trash-$uid` choice and get the uid safely. Refuse to trash a mount point. Make Replace place-then-trash. Delete the dead `unwritable`. SID-20 moved to `SID-H1-D` (R-A17). (P-8: SID-2, SID-3, SID-4, SID-14, SID-15, SID-16, SID-31) | [evidence](../../evidence/2026-09-26-copy-move-trash.md) | `VAL-SID-17` in [VALIDATION.md](../../../VALIDATION.md): «Reemplazar»; a cut to a USB stick while a file is added to the source; a restore from a Nautilus-written volume Trash |
| SID-H1-C | `siderita:` | done | [inventory](../../inventories/2026-09-26-hardening/SID-H1-C.numstat.tsv) | 54 files, +3551/-1018 | Keep and join worker handles, asking or cancelling on quit. End jobs from the worker. Replay suppressed folder changes. Refresh must not cancel navigation. Add generations to search, Trash and Recientes. Run undo and the Trash verbs as jobs. Keep one process-wide device model on a worker. Move the remaining syscalls off-thread. Bound the PE reader. Use a bounded, cancellable thumbnail pool. Make the scan executor fallible. Port FLU-12's close order and generation guard into Siderita's handshake copy (ruling R-A23; adopting the P-14 handshake and deleting the copy is a follow-up). Adopt `file_uri` and `desktop_entry::scan` (`open_with` and `ownicon` off-thread). Portal filter escaping. Dialog roles. Reduced motion. `CelestinaSlider` in `SizeRow`. Docs. (P-15: SID-5, SID-6, SID-7, SID-8, SID-9, SID-10, SID-11, SID-12, SID-13, SID-19, SID-24, SID-25, SID-26, SID-27, SID-28, SID-30; FLU-12; SID-18/RS-1, RS-4 (adoption); STY-4) | [evidence](../../evidence/2026-09-26-app-threading.md) | None |
| SID-H1-D | `siderita:` | planned | `celestina-rs/crates/siderita-ops/src/trash.rs`; `celestina-rs/crates/siderita-ops/src/trashinfo.rs`; the `trash` consumers in Siderita, Fluorita and Hematita | — | Write `DeletionDate` in local time, as the spec, GIO and KIO do, through a local-time facility owned by `celestina-core` (with `siderita-archive`'s `Zone` moved onto it), and order the Trash listing by instant rather than by a string that mixes zones. Depends on that `celestina-core` owner; a cross-prefix change, so it may land as a `suite:` unit. (SID-20; ruling R-A17) | `cargo test -p siderita-ops --offline`: a record written at a known instant and zone reads back local; mixed UTC and local records sort by instant | None |
| SID-H1-H | `siderita:` | done | [inventory](../../inventories/2026-09-26-hardening/SID-H1-H.numstat.tsv) | 9 files, +73/-5 | Paint the floating pills (bottom controls, operations dock) with the style's `pillFill`, the Haze tint, now that `controlFill` is the neutral control plate again (STYLE-G7-S); no other fill changes. | [evidence](../../evidence/2026-10-06-pill-fill.md) | None |
| SID-H1-I | `siderita:` | done | [inventory](../../inventories/2026-09-26-hardening/SID-H1-I.numstat.tsv) | 8 files, +177/-39 | Make a burst of wheel notches one continuous motion for the heading and the listing: both glides become a `Behavior` with a `SmoothedAnimation` (`Immediate` reversing) that re-aims and keeps its speed on every notch, instead of a 200 ms eased tween restarted per notch, which jumped and braked once per notch and left the heading's state changes out of step with the rows on a fast scroll. Author video of the 2026-10-06 session; no audit finding. | [evidence](../../evidence/2026-10-06-wheel-glide.md) | None |
| SID-H1-J | `siderita:` | done | [inventory](../../inventories/2026-09-26-hardening/SID-H1-J.numstat.tsv) | 8 files, +89/-21 | Make the wheel glide answer the wheel: both `SmoothedAnimation`s run on a fixed time (`wheelGlide`, `motionNormal`) instead of a fixed velocity, so whatever is left to cover is covered in that time and a burst of notches moves faster the more are pending; the fixed velocity of SID-H1-I made every burst crawl at one speed, like a belt. Author video of the 2026-10-06 session; no audit finding. | [evidence](../../evidence/2026-10-06-wheel-glide-time.md) | None |
| SID-H1-K | `siderita:` | done | [inventory](../../inventories/2026-09-26-hardening/SID-H1-K.numstat.tsv) | 13 files, +185/-373 | Remove the expanded heading: the metadata block that grew on a push up at the top is gone from `FolderHeading`, `HeadingScroll` (no `expandSpan`, no `compactProgress`, travel never below zero), the wheel handler (no expanding branch, no pinning of the listing to a moving origin) and the folder view (the content frame no longer moves with the heading). The author never reached for it, and its frame moving under the rows was where a fast wheel scroll jumped a frame back. The wheel glide reads its velocity at each re-aim from what is pending (`glideVelocity`), with `duration` unset so the Behavior carries the motion over. Author request on the 2026-10-06 session; no audit finding. | [evidence](../../evidence/2026-10-06-heading-without-expansion.md) | None |
| SID-H1-L | `siderita:` | done | [inventory](../../inventories/2026-09-26-hardening/SID-H1-L.numstat.tsv) | 7 files, +80/-3 | Give the file picker the main window's glass canvas: transparent window with `CelestinaBackdrop` under its sidebar and listing (STYLE-G7-Q recipe); it was the last Siderita window on the opaque canvas. Author request of 2026-10-07; no audit finding. | [evidence](../../evidence/2026-10-07-picker-glass-canvas.md) | None |
| SID-H1-N | `siderita:` | done | [inventory](../../inventories/2026-09-26-hardening/SID-H1-N.numstat.tsv) | 20 files, +256/-192 | Make the wheel answer on the next frame in every Siderita list: `FolderWheelHandler` becomes a `CelestinaWheelScroll` that carries the heading along, `HeadingScroll` follows its destination with the same per-frame exponential share (`wheelFollowMs`) instead of a `SmoothedAnimation` that started every notch from a standstill, and the sidebar, the picker and the scrolling dialogs take the same scroller instead of Qt's decelerating flick. Author report of 2026-10-07; no audit finding. | [evidence](../../evidence/2026-10-07-wheel.md) | None |
| SID-H1-M | `siderita:` | done | [inventory](../../inventories/2026-09-26-hardening/SID-H1-M.numstat.tsv) | 23 files, +339/-521 | Fix the bottom bar's view-and-sort menu and the picker's filter menu by opening them with the shared `GlassContextMenu.popupBeside` (headers as `GlassMenuSection`), and register the shared `fluorita-qt` thumbnail provider as "thumb" with Siderita's embedded-picture and own-icon hooks in place of its duplicated provider. Follow-up of FLU-H1-E and the style menu fix of 2026-10-07; no audit finding. | [evidence](../../evidence/2026-10-07-shared-menus-and-thumbnails.md) | None |
| SID-H1-O | `siderita:` | done | [inventory](../../inventories/2026-09-26-hardening/SID-H1-O.numstat.tsv) | 21 files, +2180/-164 | Add renaming in place and the UDisks2 format backend for removable drives in DISPOSITIVOS (`SetLabel`, `Block.Format`, whole-disk `dos`/`gpt` table plus `CreatePartitionAndFormat`, every action naming its volume by device node rather than by list position, FAT labels in capitals inside CP850 and FAT always FAT32, per-file-system label rules, a system-drive refusal over mount points, swap, LUKS/LVM backing and multi-device filesystems, one drive operation at a time on a worker thread with notices), with a rename entry in the device menu (and F2) and a format entry held back until the dialog of SID-H1-P; author request of 2026-10-07; no audit finding. | [evidence](../../evidence/2026-10-07-drive-tools.md) | None |
| SID-H1-P | `siderita:` | done | [inventory](../../inventories/2026-09-26-hardening/SID-H1-P.numstat.tsv) | 22 files, +1016/-48 | Add the format dialog for removable drives: the device menu opens a modal that shows the capacity, offers exFAT, FAT32, NTFS, ext4 and btrfs with one hint each, prefills the current name (in capitals for FAT32, validated live by the Rust label rules), offers quick format and a whole-disk switch, names the exact device and size it erases (the drive itself with the whole-disk switch), and hands the controller the device node and filesystem UUID it captured on opening, so a stick swapped or reformatted under the same node is refused in the controller and again on the worker; author request of 2026-10-07; no audit finding. | [evidence](../../evidence/2026-10-08-format-dialog.md) | None |
| SID-H1-G | `siderita:` | done | [inventory](../../inventories/2026-09-26-hardening/SID-H1-G.numstat.tsv) | 14 files, +291/-77 | Give the properties dialog the volume's free space (`statvfs` through safe `rustix`, exposed as free and total bytes with a thin usage bar) and reduce it to what a person opens it for: a heading with the kind and the path, then size, free space, modified and permissions with owner in one `Content` box at row height; MIME and access time leave the dialog and the controller. Author feedback on the 2026-10-06 session; no audit finding. | [evidence](../../evidence/2026-10-06-properties-dialog.md) | None |
| SID-H1-F | `siderita:` | done | [inventory](../../inventories/2026-09-26-hardening/SID-H1-F.numstat.tsv) | 9 files, +89/-8 | Give the window a transparent canvas so the compositor's blur and `CelestinaBackdrop`'s Haze material show through (STYLE-G7-Q), keep the floating pills glass at rest as the bar is, and make the folder-usage crumbs pointer-only so opening the properties dialog never lights a focus ring on them. Author feedback on the 2026-10-05 spike; no audit finding. | [evidence](../../evidence/2026-10-05-glass-canvas.md) | None |
| SID-H1-E | `siderita:` | done | [inventory](../../inventories/2026-09-26-hardening/SID-H1-E.numstat.tsv) | 10 files, +213/-3 | Give the operations dock's outside catcher the item it must cover instead of its parent: inside the transient column that parent is a positioner, an anchored child makes Qt refuse to lay it out ("Column will not function"), and so the rings moved on the first press, the error pill sat under them and a press over the folder did not close the callout. A defect of the 1.6 chrome the author found on 1.7.5, taken here as SID-H1-C took the dialog and motion fixes; no audit finding. | [evidence](../../evidence/2026-09-29-the-catcher-in-the-column.md) | None |

This plan records intent; it grants no authority.
