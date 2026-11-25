# CEL-TRACKS — The shell's standing design and improvement tracks

- **Opened:** 2026-11-25
- **Plan ID:** cel-tracks
- **Status:** active
- **Standing:** yes. Entries are added as the author proposes them, and the plan
  is not archived while the author keeps the tracks
- **Authorization:** the author approved
  [the standing-tracks design](../../../../docs/superpowers/specs/2026-11-24-celestina-standing-tracks-design.md)
  on 2026-11-24. The plan is opened by its first entry, `CEL-I-004`; its documents
  are committed by the suite plan
  [TRK-1](../../../../docs/plans/active/2026-11-25-cel-tracks-docs.md)
- **Scope:** celestina
- **Implementation checkpoint:** CEL-TRACKS
- **Author-validation checkpoint:** VAL-CEL-I-004 and each later entry's own row in [VALIDATION.md](../../../VALIDATION.md) (they do not block)

## Hypothesis

CEL-2 gave the shell every function the author needs, but its look and feel
are a base. Two tracks that never close can refine them one small unit at a
time:

- design (`CEL-D`): how the shell looks, moves and feels;
- improvements (`CEL-I`): small new functions of plugin size, and bugs.

Each unit is small enough for the author to try live the same day.

## Tangible outcome

Each entry lands as its own unit and, when it is visible or touchable, is
tried live by the author. A failed live check opens a corrective entry.

## Scope

- **Intake.** The author proposes an idea, a bug or a dislike in a sentence.
  Claude files it as an entry in the right track: design if it is how the
  shell looks or feels, improvements if it is something the shell does or
  fails to do.
- **Order.** Claude orders the entries by this rule:
  1. what breaks or blocks the author's live session;
  2. small and safe;
  3. large.

  Claude states the order in one line whenever it changes, and the author may
  correct it.
- **Units.** Each entry is one unit under the repository's rules. An entry that
  qualifies as a light unit (`docs/standards/verification.md`) uses the light
  lane ([ADR 0013](../../../../docs/decisions/0013-development-speed.md)).
- **Landings.** Claude asks the author to authorise the landings of the
  entries that are ready in batches, and each landing still needs that direct
  authorisation.
- **Documents.** Design notes and task plans live under `docs/superpowers/`
  and are committed by documents-only units of TRK-1.
- **Icons.** `celestina-style/icons/` is outside the `celestina:` commit roots,
  so a new icon lands in its own `celestina-style` unit.
- **Ids.** `CEL-D-NNN` and `CEL-I-NNN`. The kind is `milestone` for new
  behaviour, `maintenance` for polish without a behaviour change and `bug` for
  a fix.

## Exclusions

- New compositor features.
- Theming beyond the existing token schemes, in the first batch.
- Customize UI (a later entry).
- Any change that makes the bar or the Island slower to react, or regresses a
  CEL-2 live row.

## Build order

Current order (2026-11-25):

1. `CEL-I-004` (done);
2. `STYLE-ICONS-1`, then `CEL-I-006`, `CEL-D-001` and `CEL-D-006`;
3. `CEL-D-002` to `CEL-D-005`;
4. `CEL-I-003`.

`CEL-D-002` is the foundation of `CEL-D-003`, `CEL-D-005` and `CEL-I-003`.

## Implementation exit

Standing: the checkpoint stays open while the author keeps the tracks. Each
row closes on its own landing with its inventory and evidence.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| CEL-I-004 | `celestina:` | done | [inventory](../../inventories/2026-11-25-cel-tracks/CEL-I-004.numstat.tsv) | 28 files, +4409/-147 | Add the output and input device selector and per-application volume and mute to the volume popup ([design](../../../../docs/superpowers/specs/2026-11-25-celestina-cel-i-004-audio-design.md), [plan](../../../../docs/superpowers/plans/2026-11-25-celestina-cel-i-004-audio-plan.md)); needs `CUP-WPCTL-STATUS` of Cuprita; absorbs `CEL-I-005` | [evidence](../../evidence/2026-11-25-cel-i-004-audio.md) | VAL-CEL-I-004 |
| CEL-I-006 | `celestina:` | done | [inventory](../../inventories/2026-11-25-cel-tracks/CEL-I-006.numstat.tsv) | 27 files, +1703/-57 | Add a Session piece at the end of the bar whose menu locks, suspends (lock and suspend), logs out, reboots and powers off, the last three asking first in an inline danger card (preview approved 2026-11-25, variant A) | [evidence](../../evidence/2026-11-25-cel-i-006-session-menu.md) | VAL-CEL-I-006 |
| CEL-D-001 | `celestina:` | done | [inventory](../../inventories/2026-11-25-cel-tracks/CEL-D-001.numstat.tsv) | 21 files, +305/-174 | Show each Island tab as its page's icon instead of its title: nine equal pills, the open one in the accent fill with its icon in accent ink (preview approved 2026-11-25, variant A); needs `STYLE-ICONS-1` for `calendar` | [evidence](../../evidence/2026-11-25-cel-d-001-tab-icons.md) | VAL-CEL-D-001 |
| CEL-D-002 | `celestina:` | planned | `crates/celestina-ui/`, `src/island/`, `src/bar/` | pending | Add anchored panels inside the bar surface and the new hover and press language (design §6.2) | pending | VAL-CEL-D-002 |
| CEL-D-003 | `celestina:` | planned | `src/pieces/`, `src/osd/popup.rs`, `src/island/` | pending | Open every piece's menu as an anchored panel; tray menus stay `xdg_popup` (design §6.3) | pending | VAL-CEL-D-003 |
| CEL-D-004 | `celestina:` | planned | `src/island/`, `src/pieces/` | pending | Merge clock and date into one centre capsule that is the Island (design §6.4) | pending | VAL-CEL-D-004 |
| CEL-D-005 | `celestina:` | planned | `src/pieces/`, `src/overview/` | pending | Open a small workspaces panel with each workspace's app icons; the wheel still switches (design §6.5) | pending | VAL-CEL-D-005 |
| CEL-I-003 | `celestina:` | done | [inventory](../../inventories/2026-11-25-cel-tracks/CEL-I-003.numstat.tsv) | 18 files, +1527/-63 | Add the brightness of every monitor to the brightness popup when there are two or more: an All heading with the mean and a slider that sets every monitor with a reading, a divider, and one compact row per monitor with its model, slider and percent, the bar's own first (design §6.6; preview approved 2026-11-25, variant C) | [evidence](../../evidence/2026-11-25-cel-i-003-brightness.md) | VAL-CEL-I-003 |
| CEL-D-006 | `celestina:` | done | [inventory](../../inventories/2026-11-25-cel-tracks/CEL-D-006.numstat.tsv) | 8 files, +209/-19 | Mute each application of the volume popup with a speaker button, struck through while muted, instead of an unlabelled switch, and dim the muted application's name (preview approved 2026-11-25, variant A) | [evidence](../../evidence/2026-11-25-cel-d-006-app-mute-button.md) | VAL-CEL-D-006 |
| CEL-I-007 | `celestina:` | done | [inventory](../../inventories/2026-11-25-cel-tracks/CEL-I-007.numstat.tsv) | 9 files, +233/-27 | Fix the brightness rows jumping back after a drag: each row keeps the level the popup asked of it until its monitor reads that level back or one DDC settle passes, so the read-backs of the drag's earlier writes no longer show (the author's recording of 2026-11-25) | [evidence](../../evidence/2026-11-25-cel-i-007-brightness-hold.md) | VAL-CEL-I-007 |

## Backlog

Entries that are not yet ledger rows, and entries recorded outside this
ledger.

- **Design (`CEL-D`):** none beyond the ledger.
- **Improvements (`CEL-I`):**
  - `CEL-I-001`: the lock and polkit default flip, landed as
    `CEL-2-M-LOCK-POLKIT` (1.15.0) in the
    [CEL-2 ledger](../archive/2026-10-16-cel-2-parity.md).
  - `CEL-I-002`: the smoke and CI hardening, landed as `SMOKE-REST`,
    `SMOKE-LOCK`, `OVERVIEW-SLOT-FLAKY`, `SMOKE-PHASES` and `SMOKE-ORDER` in
    the same ledger.
  - `CEL-I-005`: the device selector, merged into `CEL-I-004`.
  - `CEL-I-006`: the author asked for the session menu on 2026-11-25; it is a
    ledger row now.

This plan records intent; it grants no authority.
