# Bottom chrome and the notice stack

- **Opened:** 2026-09-22
- **Closed:** 2026-09-23
- **Plan ID:** bottom-chrome-and-notices
- **Status:** done
- **Successor:** none
- **Authorization:** the author asked for the redesign on 2026-09-22 and
  approved the design in brainstorming
- **Scope:** siderita
- **Implementation checkpoint:** SID-B1
- **Author-validation checkpoint:** `VAL-SID-15` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

The bottom strip repeats what the ring above it already says because
`status_text` carries four unrelated kinds of message, and only one of them is
a running job. Split them by what they are — a ring for a job, a self-retiring
notice for everything else, the heading for what is simply true of the folder —
and the strip has nothing left to show. The bar it occupied then fits in one
capsule of three icons.

## Tangible outcome

A copy shows one ring and nothing else; unmounting a disk says so and stops
saying so when it is done; an error is a pill in the corner instead of a band
across the rows; and the bottom bar is a single capsule on the left with the
whole width to its right free.

## Scope

- One transient column anchored bottom right: rings, notices, danger notices.
- `ActivityNotice` with the 500 ms appearance threshold and the two tenses.
- The three-icon capsule and the merged view-and-sort menu.
- The dock's collapsed counted circle and expanded labelled list.
- `status_text` replaced by the notice queue; every running notice owned.
- The watch-degraded warning moves to the folder heading.
- The heading stops being a state machine with a detent per transition and
  becomes one continuous travel the same gesture moves.
- `SID-B1-C`: what the author found testing that heading on the real session.

## Exclusions

- The portal picker (`PickerWindow.qml`, `qml/components/picker/`) keeps the
  chrome it has; it runs no write operations. `HiddenTogglePill` survives for
  it. Aligning the two surfaces is a later decision.
- Per-notice history or a notification centre: a notice that has retired is
  gone.
- Any change to what a ring shows or to `OperationCallout`.

## Build order

1. `SID-B1-A`, then `SID-B1-Z`.

The heading's scroll was asked for after `SID-B1-A` was already written and
verified but still uncommitted, and its changes landed in the same files —
`FolderView.qml`, `FolderHeading.qml`, `build.rs` and both ratchets. An
inventory is exact per path, so the two could not be separated without
committing the first, and they are delivered as one unit.

## Implementation exit

`scripts/complete-production.sh` succeeds; the installed binary shows one
surface per running job, announces an unmount and stops announcing it, and its
bottom bar is one capsule.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| SID-B1-A | `siderita:` | done | [inventory](../../inventories/2026-09-22-bottom-chrome-and-notices/SID-B1-A.numstat.tsv) | 54 files, +3180/-1193 | The transient column, the notice and its threshold, the three-icon capsule, the merged menu, the dock's overflow, the notice queue replacing `status_text`, the watch warning moved to the heading, and the heading's three detents replaced by one continuous travel | [bottom chrome](../../evidence/2026-09-22-bottom-chrome-and-notices.md), [heading scroll](../../evidence/2026-09-22-heading-scroll.md) | `VAL-SID-15` |
| SID-B1-C | `siderita:` | done | [inventory](../../inventories/2026-09-22-bottom-chrome-and-notices/SID-B1-C.numstat.tsv) | 8 files, +372/-55 | The author's four findings on the delivered heading: the transitions were hard because the travel was not tweened, the detailed phase was shorter than one wheel notch, the title returned after a single notch from deep inside a folder, and the gesture died every frame while the heading animated | [heading corrections](../../evidence/2026-09-22-heading-corrections.md) | `VAL-SID-15` |
| SID-B1-Z | `siderita:` | done | [inventory](../../inventories/2026-09-22-bottom-chrome-and-notices/SID-B1-Z.numstat.tsv) | 13 files, +143/-90 | Implementation exit, `1.6.0`, documents closed, plan archived | [bottom chrome](../../evidence/2026-09-22-bottom-chrome-and-notices.md) | `VAL-SID-15` |

Like every plan in this repository, this one records intent and grants no
authority.
