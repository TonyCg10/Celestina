# Evidence: 2026-10-07 the glass canvas in Grafita

- **Date:** 2026-10-07
- **Scope:** `GRA-H1-C` — `grafita`
- **Environment:** the author's CachyOS, Qt 6.11.2, Niri with the blur rule for the org.celestina applications
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys the registered binary

## Procedure

```sh
python3 scripts/glass_canvas_contract.py --baseline scripts/glass-canvas-baseline.tsv grafita=grafita/qml
(cd grafita && cargo build --release)
sh grafita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/grafita
bash scripts/qmllint-cxxqt.sh grafita
bash celestina-style/scripts/check-style-contract.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
```

The author opened Grafita with two documents and the find bar, focused, on the
session.

## Result

- **Exit:** with the ratchet row lowered to 0 the guard failed with an `opaque`
  and a `backdrop` finding before the change and passed after it. The release
  build finished, the smoke exited 0, `qmllint-production` stayed at its
  baseline (47 warnings) and every contract reported OK.
- **Observed:** `Main.qml` sets `color: CelestinaTheme.clear` and paints
  `CelestinaBackdrop` under the tab strip, the documents and the dialogs. The
  tab strip and the find bar take the Haze `pillFill` with no outline, and the
  document page is an opaque `card` box that keeps its input border.

## Limits

- The blur depends on the author's Niri rule; without it the window is a
  transparent canvas tinted at 0.70 over whatever the compositor paints.

## Follow-up

Spec §5.1 (the top bar) stays owed.

## Landing

- **Base revision:** `e008b7d754af2cb440df5bf41bc5d4359cae3888`
- **Check:** `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:494797ca5aa84d61818e0f1c5919c150a454743a0268b44173dac97aaabe9d6b, verification_fingerprint sha256:f179ed55224b8784ab892f8191d6bad3be00f729a6b55777eba4b299464e1dc0
- **Deploy:** after the push: grafita: deploy-production.sh, status-production.sh
