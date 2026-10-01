# Evidence: radius guard

- **Date:** 2026-09-30
- **Scope:** AUD-1-I — suite
- **Environment:** CachyOS, Python 3.14
- **Artifact:** not applicable

## Procedure

```sh
python3 scripts/test-radius-contract.py -v
```

```sh
python3 scripts/radius_contract.py --theme celestina-style/CelestinaTheme.qml \
    --baseline scripts/radius-baseline.tsv --write-baseline \
    $(python3 scripts/architecture_scanners.py registry-qml-projects docs/projects.toml \
        | awk -F'\t' '{print $2"="$4}') | tail -1 && cat scripts/radius-baseline.tsv
```

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/test-radius-contract.py
bash scripts/test-documentation-contract.sh
bash scripts/test-commit-scope.sh
```

Before writing the baseline, the scanner was changed (own commit) to treat a
full-bleed `Image` (`anchors.fill: parent`) as clipped artwork rather than a
glyph, so the two known findings in `celestina-style/gallery/Gallery.qml:418`
and `siderita/qml/dialogs/PhoneMediaDialog.qml:163` stopped being counted; an
`Image` with a partial anchor is still inspected. 24 unit tests cover the
scanner, including the two new ones for this exemption
(`test_full_bleed_image_is_clipped_artwork`,
`test_partially_anchored_image_is_still_a_glyph`).

The baseline was written for every role the registry lists —
`application`, `shell`, and `style` — including the halted shell `celestina`,
so its row is frozen at whatever the scanner finds while the shell stays
halted; no unit may lower it.

## Result

- Exit codes: all four commands above exited `0`.
- `python3 scripts/test-radius-contract.py -v`: `Ran 24 tests in 0.160s — OK`.
- Baseline written to `scripts/radius-baseline.tsv`:

  ```
  9	celestina
  0	celestina-style
  0	fluorita
  0	grafita
  0	hematita
  0	magnetita
  7	siderita
  ```

  `celestina-style` is `0` (no scanner debt to fix in this unit).
  `celestina`'s 9 findings are all `literal` (numeric margins/positions in
  `celestina/qml/OutputChooser.qml`) — frozen while the shell is halted.
  `siderita`'s 7 findings are pre-existing `literal` findings in
  `TopBar.qml`, `FolderCellDelegate.qml`, `FolderRowDelegate.qml`, and
  `PickerCellDelegate.qml` — this unit's scope is wiring the guard and its
  ratchet, not paying that debt down.

- `bash scripts/check-architecture-contract.sh`: printed the same 16
  `literal` findings (9 celestina, 7 siderita) followed by `Radius contract: OK`
  and ended with `Architecture contract: OK`, exit `0`.
- `python3 scripts/test-radius-contract.py`: `OK` (24/24), exit `0`.
- `bash scripts/test-documentation-contract.sh`: `Documentation contract and
  agent-context: OK`, exit `0`.
- `bash scripts/test-commit-scope.sh`: `Commit scope: OK`, exit `0` — the new
  `scripts/radius-baseline.tsv` shared-ratchet entry was accepted without
  changes to the test fixtures.

## Limits

- The scanner reads single-line token bindings and `background:` objects
  only; an inset computed through an expression, a `Layout.margins`, or a
  radius taken from a property alias is not inspected.
- A full-bleed `Image` (`anchors.fill`) is exempt from the inset rule as
  clipped artwork; any other anchor combination on an `Image` is still
  inspected as a glyph.
- The baseline counts findings, not files: a file with several defects
  contributes one row-count per defect, not one per file.
- The shell's `celestina` row is frozen while the shell stays halted per
  `AGENTS.md`; no unit may lower it until the halt is lifted.
- A duplicate binding written last in a JS block can shadow a real binding
  the scanner would otherwise read, since only the last-seen binding per
  object is kept.
- An alias-of-alias theme token (an alias pointing at another alias rather
  than directly at a numeric token) is not resolved.

## Follow-up

None

## Landing

- **Base revision:** `72c3b340ee89b3987c695ae42894581d3c49c058`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again
- **Build:** celestina-style verify: verify-production.sh exit 0, manifest source_fingerprint sha256:d4f9c09a41afe5fa60d8c3a09a5838ba9b5ed94c2d3157c76f04ba943c6c5666, verification_fingerprint sha256:d6d3703af81eea7eb5e574c95c8f8d468c4525c752ad5d76158d104ec5545042; celestina-rs verify: verify-production.sh exit 0, manifest source_fingerprint sha256:3b304f586f6100a053b1496a37e69ea0ece8f0783977303ad156831bb349d2b8, verification_fingerprint sha256:8569e147151ed9221d4d2c8a269c1a2b49ecc5dcb18c0f61d1f023b2765f2828; siderita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:50d43327de39cff92d2a7f6a22d069a8c013a28b6d692f542119cf93985ecd2e, verification_fingerprint sha256:4d4624ee5ab6ee98f42dd58f3f29e4c85d71981c3151e00c11e9860f15ed8388; magnetita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:92a656b40e303121a6640beaa86c05ebb7cb0baccc4c5a97a0dddf0522dc9114, verification_fingerprint sha256:da30e165475e0725975fa48c7a79eb9a2c4e7081d24047faf2d2e0c81edf7d5c; magnetita-android verify: verify-production.sh exit 0, manifest source_fingerprint sha256:5b55c4f62b23679f8584a626b5fece8b9b67c9f8a3e6778efeff4a4895fbb271, verification_fingerprint sha256:785d5a5eff545b3da13486674dd9ef703c0112747e004046f9deb43317610a1c; grafita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:dc706bafeceed3cab7df2ef2d5af87bfc61669778fcfa5d78579969c65bf5d0f, verification_fingerprint sha256:f6374e0c6f736a1a80e202e0727de112c72f18d29a0379d554df702aaacc50bc; fluorita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:cfe5c7329d896461e447f74ff6e00b3288ff4f3e82e9318327b88c861f45956d, verification_fingerprint sha256:cf89cf23d955090f74b1edd4ee962260ff853d05168c6767e5d038e2239fd179; hematita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:593eeabe6453eb6af82856638958f2e40d3a0c68e60d7f964362ea30bc124d7d, verification_fingerprint sha256:924dfb132e1348331e1a7593a5d12135c8b6e6efd8081cdff3fd0cd37f79dac7
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh
