# Evidence: 2026-10-08 usage map with fewer tiles

- **Date:** 2026-10-08
- **Scope:** `STYLE-G7-AB` — `celestina-style`
- **Environment:** the author's CachyOS, Qt 6.11.2; the QuickTest binary runs offscreen
- **Artifact:** not applicable in the session

## Procedure

```sh
cmake -S celestina-style -B celestina-style/build -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=ON
cmake --build celestina-style/build --parallel
QT_QPA_PLATFORM=offscreen ./celestina-style/build/celestina-style-modal-test
bash celestina-style/scripts/check-style-contract.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
(cd siderita && cargo build --release)
bash scripts/qmllint-cxxqt.sh siderita
```

The author's Siderita log was full of `TypeError: Value is undefined and could
not be converted to an object` from `CelestinaTreemap.qml`. The map's
`Repeater` counts `tiles.length`, and when `tiles` is replaced by a shorter
array (entering a folder with fewer children) the surviving delegates beyond
the new length re-evaluate before the Repeater destroys them, so `tileData`
was undefined. Each tile now falls back to a new `emptyTile` when its index is
out of range, and a leftover tile is invisible and disabled. The new case
`test_shrinking_the_tiles_logs_no_errors` sets six tiles, then two, then none,
then three, under `failOnWarning(/TypeError/)`. `CelestinaUsageList` already
fell back to its `emptyRow`; the case `test_shrinking_the_rows_logs_no_errors`
pins that.

## Result

- **Exit:** before the fix the new treemap case failed on the `TypeError` at
  `CelestinaTreemap.qml` lines 104 to 153 (175 passed, 1 failed); after it,
  QuickTest exit 0 (177 passed, 0 failed); CMake build exit 0; style contract
  exit 0; architecture contract exit 0; language contract exit 0;
  documentation contract exit 0; Siderita release build exit 0; Siderita
  qmllint exit 0 (239 baseline warnings, unchanged).
- **Observed:** the usage list case passed without a code change.

## Limits

- No screenshot was taken in the session.
- The case drives the map directly; it does not enter folders in Siderita.

## Landing

- **Base revision:** `f54940067aad7cbbb82c16031e12b98c08e1f4d4`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:015e346ae3fb0bd76fbd62fb574fd802a2f78838705edea103a298ccc79d5c2f, verification_fingerprint sha256:c21a068c78248e1edb901a4036d87c8ce937bbad5f34d8a721f58d2671e6db49
