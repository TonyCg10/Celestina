# Evidence: 2026-10-07 cards, photo arrows and the stream menu in Fluorita

- **Date:** 2026-10-07
- **Scope:** `FLU-H1-D` — `fluorita`
- **Environment:** the author's CachyOS, Qt 6.11.2, Niri with the blur rule for the org.celestina applications
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys the registered binary

## Procedure

Built and checked in the session; the GUI was not launched:

```sh
(cd fluorita && cargo build --release)
sh fluorita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/fluorita
bash scripts/qmllint-cxxqt.sh fluorita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

## Result

- **Exit:** the release build finished; the smoke, the qmllint ratchet, the
  architecture contract (radius baseline for `fluorita` stays 0), the language
  contract and the documentation contract exit 0.
- **Observed:** the gallery cards no longer draw the file name (the cell keeps
  its `Accessible.name`) and the thumbnail fills the card. Thumbnails in the
  grid, the filmstrip and the poster that grows on opening are masked with a
  `MultiEffect` rounded rectangle (`CelestinaTheme.opaqueMask`) at the card's
  radius, following the painted area. Left/Right step along the folder while a
  still is open; zoomed stills pan by dragging only, so the shortcuts need no
  zoom gate. The stream menu opens through `popupBeside(streamsButton, true)`.
  `ContentDock.qml` and `Main.qml` gained `pragma ComponentBehavior: Bound`,
  which lowers the `fluorita` qmllint row from 25 to 17.

## Limits

- Not looked at on screen: the corner rounding and the menu placement are
  judged from the code and the checks, not from a running window.

## Follow-up

- Automatic thumbnails for photos stay with the next unit.

## Landing

- **Base revision:** `cd017393f10dcd649b713907d1974a5513bdc9a7`
- **Check:** `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:33a66295d6891aade1640f9aabdb5dcdb765dac0858a08d9162594428468c57f, verification_fingerprint sha256:53ea7cde27f8a5dcd057348727a9ed965eaea84d727ded2ec2d22c45ef30e61d
- **Deploy:** after the push: fluorita: deploy-production.sh, status-production.sh
