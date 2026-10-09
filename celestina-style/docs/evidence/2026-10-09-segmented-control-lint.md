# Evidence: 2026-10-09 the segmented control under qmllint

- **Date:** 2026-10-09
- **Scope:** `STYLE-G7-AC` — `celestina-style`
- **Environment:** the author's CachyOS, Qt 6.12.0; the QuickTest binary runs offscreen
- **Artifact:** not applicable in the session; the landing builds and verifies the registered module and its consumers

## Procedure

`CelestinaSegmentedControl.qml` had no `pragma ComponentBehavior: Bound`, so
its `Repeater` delegate's references to the outer ids `control` and `row`
counted as unqualified accesses in every application that registers the
control. Cuprita met this in `CUP-1-I` (2026-10-09): registering the control
raised its qmllint ratchet from 9 to 14, and the unit wrote a local copy
(`cuprita/qml/components/ScaleChoice.qml`) with the pragma instead. The
delegate already declared `required property int index` and
`required property var modelData`, which the bound component needs; the unit
adds the pragma and nothing else. The control's API and tokens are unchanged.

```sh
cmake -S celestina-style -B celestina-style/build -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=ON
cmake --build celestina-style/build --parallel
QT_QPA_PLATFORM=offscreen ./celestina-style/build/celestina-style-modal-test
bash celestina-style/scripts/check-style-contract.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
(cd cuprita && cargo build --release)
ln -s ../../celestina-style/CelestinaSegmentedControl.qml cuprita/qml/CelestinaSegmentedControl.qml
bash scripts/qmllint-cxxqt.sh cuprita      # with and without the pragma
rm cuprita/qml/CelestinaSegmentedControl.qml
bash scripts/qmllint-cxxqt.sh cuprita
```

No application on `main` registers the control yet, so the consumer check
links the canonical file into Cuprita's QML root for the measurement only;
the link is not part of the unit. The ratchet script lints the whole root,
so the linked file is counted like a registered one.

## Result

- **Exit:** CMake build exit 0; QuickTest exit 0 (183 passed, 0 failed);
  style contract exit 0; architecture contract exit 0; language contract
  exit 0; documentation contract exit 0; Cuprita release build exit 0.
- **Observed:** with the control linked and the pragma stripped, the ratchet
  refused `warnings grew from 9 to 14`, the five being
  `CelestinaSegmentedControl.qml` lines 95, 98, 104, 122 and 170,
  `Unqualified access [unqualified]` (`control.currentIndex`, `row.height`,
  `control.iconOnly`, `control.activated(index)`, `control.iconOnly`). With
  the pragma, the same run reports `OK — org.celestina.cuprita (9 non-fatal
  baseline warning(s))`; without the link it reports the same 9.

## Limits

- The consumer measurement is Cuprita's root with a temporary link; the
  real registration happens in the Cuprita follow-up unit that replaces
  `ScaleChoice.qml` with the shared control.
- `qmllint -I celestina-style/build <file>` alone exits 255 without output
  for this file and for `GlassContextMenu.qml` alike (both bound); the
  module's own build and tests, and the ratchet script, are the checks.

## Follow-up

- A Cuprita unit replaces `cuprita/qml/components/ScaleChoice.qml` with the
  shared control (same `activated(index)` contract, Left/Right/Home/End, one
  Tab stop), keeping `tst_appearance_page.qml` green and the qmllint row at
  or below 9.

## Landing

- **Base revision:** `c2d71ddbf85e53e288126eddb920248fe1aaad8c`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:9475e3b9df004fbd54e9e8a8ee4101ba2ef45e3e885449989750712b9d851b28, verification_fingerprint sha256:ec82f2bb1ed32d77ca3b2aa966d2a456e9b4d6c97bac528bc4d5cab785eea153
