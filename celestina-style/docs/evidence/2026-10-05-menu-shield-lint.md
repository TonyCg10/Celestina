# Evidence: 2026-10-05 the menu shield under qmllint

- **Date:** 2026-10-05
- **Scope:** `STYLE-G7-R` — `celestina-style`
- **Environment:** the author's CachyOS, Qt 6.11.2, offscreen QPA for tests
- **Artifact:** not applicable in the session; the landing builds and verifies the registered module and its consumers

## Procedure

`STYLE-G7-Q` added a `MouseArea` shield component to `GlassContextMenu.qml`
whose handler calls `root.close()`. The style module's own lint does not
count consumer warnings, so the unit landed; Siderita's
`verify-production.sh` then refused the landing of `SID-H1-F` with
`qmllint-production: siderita: warnings grew from 257 to 258`, the one new
warning being `GlassContextMenu.qml:39: Unqualified access`.

```sh
qmllint -I celestina-style/build celestina-style/GlassContextMenu.qml celestina-style/CelestinaModalLayer.qml celestina-style/CelestinaBackdrop.qml
ctest --test-dir celestina-style/build --output-on-failure
bash scripts/check-architecture-contract.sh
```

## Result

- **Exit:** qmllint prints no warning for the three files; ctest `100% tests passed out of 1`; `Architecture contract: OK`.
- **Observed:** `pragma ComponentBehavior: Bound` at the top of
  `GlassContextMenu.qml` binds the shield component to the menu, so the
  outer id is a legal reference. No behaviour changes.

## Limits

- The consumers' qmllint ratchet is proven at the landing of `SID-H1-F`, which follows this unit.

## Follow-up

None.

## Landing

- **Base revision:** `70285b633f3b7fa26ad6adce8d31d6e925246a9b`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:100885c9768b477589386e432cf780b6d1f048c65bf74a159a19923e9cce8cb4, verification_fingerprint sha256:059e299969c40e316efbd1dedd2d5d68576c2062b1ab003d049ac3c63c504582
