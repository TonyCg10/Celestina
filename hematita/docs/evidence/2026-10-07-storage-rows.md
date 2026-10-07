# Evidence: 2026-10-07 the storage rows in Hematita

- **Date:** 2026-10-07
- **Scope:** `HEM-H1-D` — `hematita`
- **Environment:** the author's CachyOS, Qt 6.11.2, Niri with the blur rule for the org.celestina applications
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys the registered binary

## Procedure

Built and checked in the session worktree:

```sh
cd hematita && cargo build --release; echo "exit=$?"
sh hematita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/hematita; echo "exit=$?"
bash scripts/qmllint-cxxqt.sh hematita; echo "exit=$?"
bash scripts/check-architecture-contract.sh; echo "exit=$?"
python3 scripts/check-language-contract.py; echo "exit=$?"
bash scripts/check-documentation-contract.sh; echo "exit=$?"
```

## Result

- **Exit:** the release build finished; the smoke, qmllint (0 non-fatal baseline warnings), architecture, language and documentation contracts all exited 0.
- **Observed:** the storage location list rows now use `CelestinaTheme.rowHeightLg` instead of `CelestinaTheme.rowHeight`, giving each row the two-line height needed to fit the title, path, and usage bar without spilling to the next row.

## Limits

- The row height is judged by the author on the session.

## Follow-up

- None.

## Landing

- **Base revision:** `264db55e3748b3efcd9f54b95501a144fe183300`
- **Check:** `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:0719249167ebd7a2360cbad85b04c8ad2934c6d52ba22ac5e0ebe24ab05c25d1, verification_fingerprint sha256:dbd4492aad28864e25ef839809431476a9737e89760f2e264a6473a7cdbbd9ae
- **Deploy:** after the push: hematita: deploy-production.sh, status-production.sh
