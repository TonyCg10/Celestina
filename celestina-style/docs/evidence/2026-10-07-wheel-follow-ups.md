# Evidence: 2026-10-07 wheel scroller follow-ups

- **Date:** 2026-10-07
- **Scope:** `STYLE-G7-Z` — `celestina-style`
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
(cd hematita && cargo build --release)
bash scripts/qmllint-cxxqt.sh siderita
bash scripts/qmllint-cxxqt.sh hematita
sh siderita/scripts/smoke.sh --binary <release>/siderita
sh hematita/scripts/smoke.sh --binary <release>/hematita
```

`test_h_idle_retarget_leaves_a_running_flick_alone` flicks a plain
`Flickable` with a scroller (`flick(0, -2000)`), changes its `contentHeight`
by 1 px, calls `retarget()` and checks that `flicking` is still true.

## Result

- **Exit:** before the change the new case failed ("an in-bounds idle
  retarget ended the flick": `flicking` false), 173 passed and 1 failed. After
  it all 174 cases pass, three runs in a row; the style, architecture,
  language and documentation contracts exit 0; the Siderita and Hematita
  release builds exit 0; the qmllint ratchets are unchanged (Siderita 239,
  Hematita 0) and exit 0; both smoke runs exit 0 with the usage list now
  carrying `CelestinaWheelScroll`.
- **Observed:** `retarget()` writes `contentY` when idle only if clamping
  moves it by more than 0.01 px. A horizontal-dominant pixel delta is passed
  on only when `contentWidth > width`; otherwise it is handled as vertical
  (`stepped` is emitted and the y part applied). QtTest's `mouseWheel` sends
  no pixel deltas, so that rule has no automated case; it is documented in
  the component's comment.

## Limits

- Only the two applications that link `CelestinaUsageList.qml` (Siderita and
  Hematita) exist as consumers; both link `CelestinaWheelScroll.qml`.
- The touchpad rule is not exercised by any automated case.

## Landing

- **Base revision:** `c09f4ad1b766ff9a09531f26579c9936ca6993d1`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:59f1f9427913b58015a970192e5169c936dae1949dc0437cdc559f820376230a, verification_fingerprint sha256:e4fcbf321e06b18b38a8a7c47b1d2606e77e87c726a3cf4319107b9dd9aa1cbd
