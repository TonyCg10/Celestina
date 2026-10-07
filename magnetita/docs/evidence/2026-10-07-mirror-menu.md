# Evidence: 2026-10-07 the mirror options in a context menu

- **Date:** 2026-10-07
- **Scope:** `MAG-D1-G` — `magnetita`
- **Environment:** the author's CachyOS, Qt 6.11.2, Niri with the blur rule for the org.celestina applications
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys the registered binary

## Procedure

Built and checked in the session worktree:

```sh
(cd magnetita && cargo build --release)
sh magnetita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/magnetita
bash scripts/qmllint-cxxqt.sh magnetita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

The author opens the devices page and the sliders button beside the mirror button.

## Result

- **Exit:** the release build, the smoke script, qmllint (4 baseline warnings, none new), the architecture contract, the language contract and the documentation contract all exited 0.
- **Observed:** the sliders button opens a `GlassContextMenu` holding the mirror status line, the sound choice (phone or PC), the screen-off toggle and the applies-later note while the mirror is open. The inline status line and `MirrorSettingsSheet` are gone, along with `MirrorChoiceRow`. The menu blurs the page surface (backdrop plus cards), passed down from `Main.qml`, which is not an ancestor of the overlay the menu lives in.

## Limits

- Choosing a row closes the menu (the `Menu` default); the menu is not kept open across toggles.

## Follow-up

- None.

## Landing

- **Base revision:** `b8396f3d411bdddf7465ea6fe9fd1b07c69c6dd8`
- **Check:** `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:9164c1b531be5d08b427f0e009560eb2928035a0f3bfde6f8e5304aee365de0b, verification_fingerprint sha256:b087ac68774265ffd3bab9f83edd50495777ee86b4000007ad16c5c5053afb25
- **Deploy:** after the push: magnetita: deploy-production.sh, status-production.sh
