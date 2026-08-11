# STYLE-G7-F prototype — external-backdrop glass material

- **Date:** 2026-08-11
- **Scope:** cumulative CelestinaStyle unit `STYLE-G7-F`; historical prototype
  label `STYLE-G7-H`
- **Artifact:** CelestinaStyle 1.3.0, canonical compiled QML module
- **Environment:** Linux, Qt 6.9 compiled-module floor, CMake Release build and
  offscreen Qt platform for automated construction
- **Plan:** [shared reading controls ledger](../plans/active/2026-08-04-shared-reading-controls.md)
- **Consumer validation:** Celestina's cumulative `PANEL-1-B` snapshot and
  `VAL-PANEL-1`

## Procedure

### What changed

`GlassSurface` keeps `InSceneCapture` as its compatible default and adds an
explicit `ExternalBackdrop` mode. The external mode never activates
`ShaderEffectSource` or `MultiEffect`; it renders the same tint, noise, outline
and lit edge above a backdrop supplied by the host compositor or its fallback.
The host remains the sole owner of compositor lifecycle and region geometry.

The focused QuickTest proves the default capture path, the no-capture external
path and explicit external degradation state. Celestina's menu regressions
prove that both the outer veil and denser content sections consume this mode
without publishing nested compositor regions.

## Result

### Automated evidence

- `bash scripts/check-architecture-contract.sh`: architecture, sealed colour,
  contrast and QML visual contracts passed.
- `celestina-style/scripts/build-production.sh`: built the canonical 1.3.0
  compiled module once.
- `celestina-style/scripts/verify-production.sh`: 29 production-common tests,
  style guards, compiled QML lint, CTest 1/1 and the eight-second gallery smoke
  passed against those bytes.
- `celestina-style/scripts/status-production.sh`: artifact current and verified.
- `python3 scripts/version_tool.py check`: six owner versions and append-only
  history passed.

The compiled QML lint retained pre-existing unqualified-access warnings in
`CelestinaLineGutter.qml`; verification completed successfully and this unit did
not change that file.

## Limits

Offscreen construction proves ownership, degradation and absence of QML
capture in external mode. Perceived tint, noise, outline and compositor blur
remain part of `VAL-STYLE-01` and Celestina's real-session `VAL-PANEL-1` review.
