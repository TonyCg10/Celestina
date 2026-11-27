# STYLE-T1 — Icons for the shell's standing tracks

- **Opened:** 2026-11-25
- **Plan ID:** shell-track-icons
- **Status:** active
- **Standing:** yes. The shell's standing tracks (`CEL-TRACKS`) need new icons
  now and then; each batch is one unit here, and the plan is not archived
  while the author keeps the tracks
- **Authorization:** on 2026-11-25 the author approved the preview of the
  session menu, the icon tabs and the per-app mute button, which need four
  new glyphs, and the order that lands them first
- **Scope:** celestina-style
- **Implementation checkpoint:** STYLE-T1
- **Author-validation checkpoint:** none (the shell's entries carry the live rows)

## Hypothesis

The shell's track entries draw semantic icons from this catalogue. Adding the
glyphs they need here, as vendored Lucide like the rest, keeps one source of
icons for the whole suite.

## Tangible outcome

Each batch's glyphs in `icons/`, registered in `CMakeLists.txt`, `icons.qrc`
and `CelestinaIcons.qml`, and named in `tests/tst_iconcatalog.qml`.

## Scope

- `STYLE-ICONS-1`: `calendar`, `lock`, `log-out` and `moon` from Lucide
  1.27.0, with the catalogue's colour and stroke recipe (`#fff`, 2.5).
- Later rows: one per batch of glyphs a track entry needs.

## Exclusions

- Any change to existing glyphs, tokens or controls.

## Build order

1. `STYLE-ICONS-1`.
2. Later rows as the tracks need them.

## Implementation exit

Standing: each row closes on its own landing; the plan is archived when the
author ends the tracks.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| STYLE-ICONS-1 | `celestina-style:` | done | [inventory](../../inventories/2026-11-25-shell-track-icons/STYLE-ICONS-1.numstat.tsv) | 15 files, +231/-8 | Add the calendar, lock, log-out and moon glyphs for the shell's session menu and Island tabs | [evidence](../../evidence/2026-11-25-shell-track-icons.md) | None |
| STYLE-ICONS-2 | `celestina-style:` | done | [inventory](../../inventories/2026-11-25-shell-track-icons/STYLE-ICONS-2.numstat.tsv) | 14 files, +183/-4 | Add the rotate-ccw-clock, arrow-down-a-z, globe and calculator glyphs for the shell's launcher tabs and calculator | [evidence](../../evidence/2026-11-26-launcher-icons.md) | None |

This plan records intent; it grants no authority.
