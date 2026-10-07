# Evidence: 2026-10-07 the glass canvas in Hematita

- **Date:** 2026-10-07
- **Scope:** `HEM-H1-C` — `hematita`
- **Environment:** the author's CachyOS, Qt 6.11.2, Niri with the blur rule for the org.celestina applications
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys the registered binary

## Procedure

Built and checked in the session worktree:

```sh
(cd hematita && cargo build --release)
sh hematita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/hematita
bash scripts/qmllint-cxxqt.sh hematita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
```

The author walked the six sections, focused.

## Result

- **Exit:** the release build finished; the smoke, qmllint (0 non-fatal baseline warnings), architecture and language contracts all exited 0.
- **Observed:** `Main.qml` sets `color: CelestinaTheme.clear`, so the
  compositor blur shows through, and the linked `CelestinaBackdrop` paints the
  Haze canvas under the strip and every page (STYLE-G7-Q). The pages' panels
  are `CelestinaSurface` and keep painting `card`. The glass-canvas ratchet row
  for `hematita` falls from 2 to 0.

## Limits

- The blur depends on the author's Niri rule.

## Follow-up

- Spec §5.2 (top bar) stays owed.

## Landing

- **Base revision:** `b7cbf7af1f39d4a7d5d470fae3842f596a50a630`
- **Check:** `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:5325cb12510b809060113732414201fabc36196ea09215d7fa26595bd7963738, verification_fingerprint sha256:dbd4492aad28864e25ef839809431476a9737e89760f2e264a6473a7cdbbd9ae
- **Deploy:** after the push: hematita: deploy-production.sh, status-production.sh
