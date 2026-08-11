# STYLE-G7-F prototype — semantic shell glass roles

- **Date:** 2026-08-11
- **Scope:** cumulative CelestinaStyle unit `STYLE-G7-F`; historical prototype
  label `STYLE-G7-I`
- **Artifact:** CelestinaStyle 1.3.0, canonical compiled QML module
- **Environment:** Linux, Qt 6.9 compiled-module floor, CMake Release build and
  offscreen Qt platform for automated construction
- **Plan:** [shared reading controls ledger](../plans/active/2026-08-04-shared-reading-controls.md)
- **Consumer validation:** Celestina's cumulative `PANEL-1-B` snapshot and
  `VAL-PANEL-1`

## Procedure

### Reference and adaptation

Samsung's current One UI guidance describes blur and dim as restrained visual
depth tools whose strength must preserve hierarchy, and its layout guidance
uses grouped focus blocks to organize related content. The source material does
not publish numeric One UI 8.5 glass opacity values. Celestina therefore treats
the supplied crop as the measurable visual reference rather than presenting a
private Samsung implementation detail as fact.

- [Samsung visual depth](https://developer.samsung.com/one-ui/structure/visual-depth.html)
- [Samsung basic layout](https://developer.samsung.com/one-ui/layout/basic.html)
- [Samsung One UI 8.5](https://www.samsung.com/pe/one-ui/)

The crop's near-black card over its surrounding dark canvas yields an estimated
neutral dim strength around 0.63. Celestina rounds that adaptation to `0.64`
for the complete content material. The contextual veil uses `0.12` material
strength; its normal highlight tint is already translucent, so its ordinary
effective tint is approximately two percent.

### What changed

`GlassSurface.StandardMaterial` remains the compatible default.
`ContentSurface` applies the dense matte strength to tint, noise, outline and
lit edge together. `ContextualVeil` attenuates the same complete decorative
stack rather than allowing a consumer to rebuild a partial material. Neither
role changes capture mode, compositor ownership, geometry or elevation.

The shared renderer exposes stable internal object names only for focused
construction tests. Those tests require both roles to stay in
`ExternalBackdrop`, keep QML capture inactive, use zero elevation and preserve
the default role for every existing consumer.

## Result

### Automated evidence

- `bash scripts/check-architecture-contract.sh`: architecture, sealed colour,
  contrast and QML visual contracts passed.
- Focused `celestina-style-modal-focus`: 1/1 passed, including semantic role,
  material strength, no-capture and no-shadow assertions.
- `celestina-style/scripts/build-production.sh`: built the canonical 1.3.0
  compiled module once.
- `celestina-style/scripts/verify-production.sh`: 29 production-common tests,
  style guards, compiled QML lint, CTest 1/1 and the eight-second gallery smoke
  passed against those bytes.
- `celestina-style/scripts/status-production.sh`: artifact current and verified.
- `python3 scripts/version_tool.py check`: six owner versions and append-only
  history passed.
- Celestina's canonical 0.11.0 completion passed its complete consumer suite
  and deployed the verified Style bytes without activating the host session.

The compiled QML lint retained the pre-existing unqualified-access warnings in
`CelestinaLineGutter.qml`; verification completed successfully and this unit did
not change that file.

## Limits

Offscreen construction proves role ownership, compatibility and degradation;
it does not prove perceived blur or material balance. The nested real-session
comparison is recorded by Celestina's prototype evidence. Author acceptance
across all menus, output scales and hostile application backgrounds remains in
`VAL-PANEL-1`.
