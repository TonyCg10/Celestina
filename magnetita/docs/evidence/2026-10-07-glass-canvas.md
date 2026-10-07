# Evidence: 2026-10-07 the glass canvas in Magnetita

- **Date:** 2026-10-07
- **Scope:** `MAG-D1-F` — `magnetita`
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
```

The author opened the devices, messages and settings pages, focused, and started the mirror.

## Result

- **Exit:** the glass-canvas guard failed with exit 1 (one `opaque` finding) once the baseline row was lowered to 0, then passed with exit 0 after the change. The release build, the smoke script, qmllint, the architecture contract and the language contract all exited 0.
- **Observed:** `Main.qml` now sets `color: CelestinaTheme.clear`, so the compositor blur shows through and the existing `CelestinaBackdrop` paints the Haze canvas (STYLE-G7-Q). `MirrorWindow.qml` is unchanged and keeps the opaque canvas.

## Limits

- The blur depends on the author's Niri rule; the mirror window keeps the opaque canvas.

## Follow-up

- Spec §5.4 (sidebar and top bar) stays owed.

## Landing

- **Base revision:** `de803faf55b1f0ea22496d3e56f13d5e3f961102`
- **Check:** `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:9f109f2e462163cc42dbcf1a8cd32b96343238535dbcc1a1e9a93439667b23c4, verification_fingerprint sha256:b087ac68774265ffd3bab9f83edd50495777ee86b4000007ad16c5c5053afb25
- **Deploy:** after the push: magnetita: deploy-production.sh, status-production.sh
