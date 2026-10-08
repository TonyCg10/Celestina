# Evidence: 2026-10-08 pastel palette and livelier usage tiles

- **Date:** 2026-10-08
- **Scope:** `STYLE-G7-AA` — `celestina-style`
- **Environment:** the author's CachyOS, Qt 6.11.2; the QuickTest binary runs offscreen
- **Artifact:** not applicable in the session

## Procedure

```sh
bash celestina-style/scripts/check-style-contract.sh
cmake -S celestina-style -B celestina-style/build -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=ON
cmake --build celestina-style/build --parallel
QT_QPA_PLATFORM=offscreen ./celestina-style/build/celestina-style-modal-test
bash scripts/check-architecture-contract.sh
python3 scripts/check-sealed-colours.py
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
(cd siderita && cargo build --release)
bash scripts/qmllint-cxxqt.sh siderita
```

The author's decision of 2026-10-08 ("A · Pastel suave", "teselas 2") moves
the dark scheme to a calm pastel palette and makes the usage tiles livelier
under their light caption. A second ruling the same day keeps the sealed
accent (`#3e91ff`) and danger (`#ff746d`), which the halted shell's Niri
generator also carries, together with every token derived from them (accent
states, `accentSoft*`, `glyphDirectory`, `dangerFill`, `dangerBorder`,
`dangerFillInk`, `dangerInk`). The author picked the 55 % tile look; the
controller set `usageTileOpacity` to 0.48, visually indistinguishable, so the
light 11 px caption clears 4.5:1 on all six hues. Cards (`#0b0c10`), canvas
and glass are unchanged.

| Token | Old | New |
| --- | --- | --- |
| `ref.success` | `#59dc9e` | `#8ee0b4` |
| `ref.warning` | `#fc864c` | `#f7b27f` |
| `ref.favorite` | `#f2c55c` | `#f3cf8e` |
| `ref.codeComment` | `#8d9bab` | `#95a3b3` |
| `ref.codeString` | `#8fd3ac` | `#9ee0b8` |
| `ref.codeNumber` | `#f0b083` | `#f3cf8e` |
| `ref.codeKeyword` | `#b9a6f0` | `#c4b5fd` |
| `glyphAccentBlue` | `#6ea8ff` | `#8bb8ff` |
| `glyphAccentViolet` | `#a391e2` | `#c4b5fd` |
| `glyphAccentCyan` | `#68c3d4` | `#8dd8e3` |
| `glyphAccentGreen` | `#72cfa3` | `#9ee0b8` |
| `glyphAccentAmber` | `#dcb36a` | `#f3cf8e` |
| `glyphAccentCoral` | `#e88d82` | `#f5a8a0` |
| `glyphSymlink` | `#a391e2` | `#c4b5fd` |
| `glyphDevice` | `#68c3d4` | `#8dd8e3` |
| `glyphFile` | `#a9b5c5` | `#b4c1d3` |
| `glyphNavigation` | `#8fa3bb` | `#9fb3cc` |
| `successSoft` | `#1c59dc9e` | `#1c8ee0b4` |
| `usageTileOpacity` (new) | — (`accentSoftOpacity` 0.14) | 0.48 |

`ref.accent`, `ref.danger`, `accentInk` (`#050608`), `accentSoftOpacity` (0.14) and everything derived from the accent and danger are unchanged. The treemap
tile fill now reads `usageTileOpacity`; `CelestinaUsageList`'s bars already
paint their tone solid and are unchanged. The new case
`test_tiles_fill_at_the_usage_tile_opacity` checks a tile's fill colour and
opacity and that the remainder tile stays opaque.

## Result

- **Exit:** style contract (style and contrast guards) exit 0; CMake build
  exit 0; QuickTest exit 0 (175 passed, 0 failed); architecture contract
  exit 0; sealed-colour guard exit 0 (4 colours); language contract exit 0;
  documentation contract exit 0; Siderita release build exit 0; Siderita
  qmllint exit 0 (239 baseline warnings, unchanged).
- **Observed:** the contrast guard accepted every value without an
  adjustment. A light caption over a 0.48 tile on the `#0b0c10` card
  measures blue 6.13, violet 5.77, cyan 5.25, green 5.04, amber 4.97 and
  coral 5.93:1 (at 0.55 cyan, green and amber fell to 4.31, 4.13 and
  4.06:1).

## Limits

- No screenshot was taken in the session.
- The tile caption ratios are computed over the opaque card; no guard
  covers that pair, and the panel behind a tile may be a little lighter.
- The accent remains the saturated `#3e91ff` beside the pastel hues; moving
  it needs the halted shell's Niri generator to move with it.

## Landing

- **Base revision:** `2d68fc445ed88375b55d7f0015ad8c6c9a30a994`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:d4d20c58667730b28b60b6519c6c7e7031c142062a15fc9f9af0bd69f8fefdd4, verification_fingerprint sha256:eb89cfcd9be278512d33445283088594178b3adf0ba3bc6fb95f0a779542a016
