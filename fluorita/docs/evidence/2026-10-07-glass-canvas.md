# Evidence: 2026-10-07 the glass canvas in Fluorita

- **Date:** 2026-10-07
- **Scope:** `FLU-H1-C` — `fluorita`
- **Environment:** the author's CachyOS, Qt 6.11.2, Niri with the blur rule for the org.celestina applications
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys the registered binary

## Procedure

Built on the author's session and judged by eye:

```sh
(cd fluorita && cargo build --release)
sh fluorita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/fluorita
bash scripts/qmllint-cxxqt.sh fluorita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
```

The author browsed the library, then played a video and opened a still, focused.

## Result

- **Exit:** the release build finished; the smoke, the qmllint ratchet, the
  architecture contract and the language contract exit 0.
- **Observed:** `Main.qml` sets `color: CelestinaTheme.clear`, so the
  compositor blur shows through and the existing `CelestinaBackdrop` paints
  the Haze canvas (STYLE-G7-Q). An opaque `CelestinaTheme.canvas` rectangle
  is visible only while a video or a still is on screen and the player
  frame covers the whole window (`window.pictureFillsWindow`: `expanded`, a
  picture, and the frame's animated geometry at the window's size), exactly
  when the backdrop hides, so letterboxing stays black while the library
  behind a growing or shrinking frame keeps the Haze canvas. The glass-canvas ratchet row for
  `fluorita` falls from 1 to 0.

## Limits

- The blur depends on the author's Niri rule.

## Follow-up

- Spec §5.3 (top bar) stays owed.

## Landing

- **Base revision:** `9b89a86196adbb98cd0b694a5d7fdcc2b83d631f`
- **Check:** `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:04c42514ea3799ced2f3bb0b7e0aa1db7c8e5db40b13bd0266226a9b39ca94f8, verification_fingerprint sha256:44635c77cd0da25aaa75633b88750e115b7e40d847e1b4ad9991fd90be26aa3f
- **Deploy:** after the push: fluorita: deploy-production.sh, status-production.sh
