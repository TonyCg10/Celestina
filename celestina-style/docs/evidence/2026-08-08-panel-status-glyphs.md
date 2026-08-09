# STYLE-G7-E — panel status glyphs

- **Date:** 2026-08-08
- **Scope:** CelestinaStyle unit `STYLE-G7-E`
- **Artifact:** CelestinaStyle 1.2.0, canonical compiled QML module
- **Environment:** Linux, Qt 6.9 compiled-module floor, CMake Release build and
  offscreen Qt platform for automated construction
- **Plan:** [shared reading controls ledger](../plans/active/2026-08-04-shared-reading-controls.md)
- **Consumer validation:** Celestina `VAL-PANEL-1`

## What changed

The canonical semantic UI catalogue gained the thirteen Lucide names required
by Celestina's accepted panel baseline: `wifi`, `bluetooth`, `cpu`,
`memory-stick`, `mic`, `mic-off`, `bell`, `bell-off`, `power`, `sun`, `gauge`,
`leaf` and `zap`. Both resource manifests carry the same SVG set, and the lookup
test proves every name resolves to its own embedded resource.

The module owns only the finite names and resources. Provider state, product
meaning, accessibility labels, placement and interaction remain in Celestina.

## Procedure

The module was bumped from 1.1.2 to 1.2.0 as a backward-compatible public
catalogue addition. The canonical production build produced the compiled
module once; verification reused those bytes for its guards, lint, CTest and
eight-second gallery smoke. Status then confirmed the verification seal was
current.

## Automated evidence

- `celestina-style/scripts/build-production.sh`
- `celestina-style/scripts/verify-production.sh`: 29 production-common tests,
  architecture, colour, contrast and QML visual guards, compiled QML lint,
  CTest 1/1 and the eight-second gallery smoke passed
- `celestina-style/scripts/status-production.sh`: artifact current and verified
- `python3 scripts/version_tool.py check`
- `python3 scripts/check-staged-units.py celestina-style/docs/inventories/2026-08-04-shared-reading-controls/STYLE-G7-E.numstat.tsv`
- `git diff --cached --check`

The compiled QML lint retained pre-existing unqualified-access warnings in
`CelestinaLineGutter.qml`; it completed successfully and this unit did not
change that file.

## Result

CelestinaStyle 1.2.0 contains one registered and tested source for every panel
status glyph in scope. The module is nondeployable, so no installed application
or live session changed during this unit.

## Limits

Automated verification proves catalogue and resource parity, not the glyphs'
perceived size, contrast or grouping over a live compositor. That belongs to
Celestina's `VAL-PANEL-1` review at scale 1 and scale 2.
