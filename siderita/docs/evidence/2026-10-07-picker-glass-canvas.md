# Evidence: 2026-10-07 the glass canvas in Siderita's file picker

- **Date:** 2026-10-07
- **Scope:** `SID-H1-L` — `siderita`
- **Environment:** the author's CachyOS, Qt 6.11.2, Niri with the blur rule for the org.celestina applications
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys the registered binary

## Procedure

```sh
(cd siderita && cargo build --release)
sh siderita/scripts/qml-tests.sh
sh siderita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/siderita
bash scripts/qmllint-cxxqt.sh siderita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
```

The author opened the picker from another application (open and save),
focused.

## Result

- **Exit:** the release build, the smoke run, qmllint, the architecture
  contract and the language contract exited 0; the QML tests ended with
  171 passed, 0 failed.
- **Observed:** `PickerWindow.qml` sets `color: CelestinaTheme.clear` and
  paints `CelestinaBackdrop` as the first child of the chrome `Item`, under
  `PickerSidebar` and the listing, so the compositor blur shows through and
  the Haze canvas is the same as the main window's. The check
  `scan_window(PickerWindow.qml)` went from `['opaque', 'backdrop']` to `[]`.

## Limits

- The blur depends on the author's Niri rule.

## Follow-up

- spec §5.5 (`PickerWindow` follows the bar contract) stays owed.

## Landing

- **Base revision:** `a327003e975dc0c1ec706d949abeab590aa14d33`
- **Check:** `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:d5ee5b60914d5ba45164d90e00b2fa3ff5be8744b4a997adcea8e515a0bdda1e, verification_fingerprint sha256:4980674b06c28060326b1f3205c9a8c950ea78acc06ad1290abe0221ce87303f
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh
