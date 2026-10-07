# Evidence: 2026-10-07 the stream menu sections and the wheel in Fluorita

- **Date:** 2026-10-07
- **Scope:** `FLU-H1-F` — `fluorita`
- **Environment:** the author's CachyOS, Qt 6.11.2, Niri with the blur rule for the org.celestina applications
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys the registered binary

## Procedure

Built and checked in the worktree; the menu and the wheel were measured offscreen on Qt 6.11.2 before the change:

```sh
(cd fluorita && cargo build --release)
sh fluorita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/fluorita
bash scripts/qmllint-cxxqt.sh fluorita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

## Result

- **Exit:** the release build, the smoke, the qmllint ratchet, the architecture
  contract, the language contract and the documentation contract exit 0.
- **Observed:** `StreamMenu` declares its four headers as `GlassMenuSection`
  items and inserts each section's rows with an `Instantiator` right after the
  section's anchor, so the headers keep their place above their rows. The
  gallery grid, the music list and the folder sidebar list carry
  `CelestinaWheelScroll`; the Ctrl+wheel zoom handlers are untouched. The
  qmllint baseline for `fluorita` stays at 17.

## Limits

- The author judges the menu placement and the wheel feel by eye on the session.

## Follow-up

- None.

## Landing

- **Base revision:** `679e41d6f49cd3536474ef813828c2765dd5919b`
- **Check:** `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:ec429a530b046a91b693c63918df9e0f3aaeb65603f6415a2dfef6982fd40b3f, verification_fingerprint sha256:29cb77423b778aa144c22a1394a95864e99d4628b8dcd88098401b31914e361d
- **Deploy:** after the push: fluorita: deploy-production.sh, status-production.sh
