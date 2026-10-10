# FLU-H1 — Hardening after the monorepo audit

- **Opened:** 2026-09-26
- **Plan ID:** hardening
- **Closed:** 2026-10-10
- **Successor:** none; the author named no next Fluorita checkpoint. All 6 delivery rows (FLU-H1-A to FLU-H1-F) are done
- **Status:** done
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
| FLU-H1-A | `fluorita:` | done | [inventory](../../inventories/2026-09-26-hardening/FLU-H1-A.numstat.tsv) | 34 files, +2614/-316 | Replace writes a hidden sibling, fsyncs it, trashes the original through `siderita-ops`, then renames. Land media through the P-6 writer. Add a single `try_from_secs_f64` helper. Sanitise and cap probed tags, and keep a catalogue that fails to load. Create the cancel token in `submit`. Strip XMP GPS and MPF. Make solid fill the default redaction. (P-7: FLU-1, FLU-2, FLU-3, FLU-4, FLU-6, FLU-15, FLU-16) | [evidence](../../evidence/2026-09-26-replace-and-privacy.md) | `VAL-FLU-EDIT`, `VAL-FLU-METADATA` |
| FLU-H1-B | `fluorita:` | done | [inventory](../../inventories/2026-09-26-hardening/FLU-H1-B.numstat.tsv) | 57 files, +5603/-2694 | Cancellable resync. Project on a worker with `Arc`. Forget same-path stale records and probe touched audio. Turn directory events into resyncs. Track completeness per root. Fix the relative dotted-ancestor check. Close portal requests on cancel. Probe stills on a worker. Reopen copies from bounded recipes. Keyboard and accessible annotation selection. Shared render-state object in `fluorita-qt`. Extract the handshake state machine and session loop into `fluorita-core`/`fluorita-engine`, bumping the generation on every close. Real MPRIS rate. Remove the trailer encoder. Spec thumbnail chunks. Typed bridge structs. `Builder::spawn`. Adopt `file_uri`. Correct the docs. (P-14: FLU-5, FLU-7, FLU-8, FLU-9, FLU-10, FLU-11, FLU-13, FLU-14, FLU-17, FLU-18, FLU-19, FLU-20, FLU-24, FLU-25, FLU-26, FLU-27, FLU-28, FLU-29, FLU-30; FLU-23) | [evidence](../../evidence/2026-09-26-catalogue-and-player.md) | `VAL-FLU-EDIT`, `VAL-FLU-METADATA`, `VAL-FLU-TEARDOWN`, `VAL-FLU-IMMERSIVE` |
| FLU-H1-C | `fluorita:` | done | [inventory](../../inventories/2026-09-26-hardening/FLU-H1-C.numstat.tsv) | 8 files, +99/-5 | Give the library Siderita's glass canvas: transparent window over the existing `CelestinaBackdrop` (STYLE-G7-Q recipe), with an opaque `canvas` backing only while a picture is on screen so a video's letterbox stays black. Lowers the glass-canvas ratchet row to 0. Author request of 2026-10-07; no audit finding. | [evidence](../../evidence/2026-10-07-glass-canvas.md) | None |
| FLU-H1-D | `fluorita:` | done | [inventory](../../inventories/2026-09-26-hardening/FLU-H1-D.numstat.tsv) | 12 files, +229/-23 | Take the names off the gallery cards, round the thumbnails and posters to the card's corner, step along an open photo with Left/Right, and open the stream menu beside its button instead of over it. Author request of 2026-10-07; no audit finding. | [evidence](../../evidence/2026-10-07-cards-and-arrows.md) | None |
| FLU-H1-E | `fluorita:` | done | [inventory](../../inventories/2026-09-26-hardening/FLU-H1-E.numstat.tsv) | 26 files, +1692/-189 | Make thumbnails automatic: a shared freedesktop-thumbnail provider in `fluorita-qt` (Qt image reader, bounded pool, cancellation, owner-only cache writes, a hook seam for Siderita) serves every image as `image://thumb/<key>`, the video/audio poster pass runs in the background after each scan and watched change and goes past an item that does not answer, and the "Generar miniaturas" button with `Ctrl+G`/`Ctrl+Shift+G` goes away. Author decision of 2026-10-07; no audit finding. | [evidence](../../evidence/2026-10-07-automatic-thumbnails.md) | None |
| FLU-H1-F | `fluorita:` | done | [inventory](../../inventories/2026-09-26-hardening/FLU-H1-F.numstat.tsv) | 13 files, +129/-10 | Fix the stream menu so its four section headers stay above their rows and the menu opens clear of its button, and give the gallery, the music list and the folder sidebar the shared free wheel scrolling (`CelestinaWheelScroll`). Author decision of 2026-10-07; no audit finding. | [evidence](../../evidence/2026-10-07-menu-sections-and-wheel.md) | None |
| FLU-H1-G | `fluorita:` | done | [inventory](../../inventories/2026-09-26-hardening/FLU-H1-G.numstat.tsv) | 9 files, +173/-95 | Archive the delivered hardening plan through its own administrative unit and reconcile the Fluorita roadmap, status and plan index | [evidence](../../evidence/2026-10-10-hardening-archive.md) | None |

This plan records intent; it grants no authority.
