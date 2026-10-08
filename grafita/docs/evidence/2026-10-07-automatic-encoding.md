# Evidence: 2026-10-07 automatic encoding detection in Grafita

- **Date:** 2026-10-07
- **Scope:** `GRA-H1-E` — `grafita`, `grafita-core`
- **Environment:** the author's CachyOS, Qt 6.11.2, Niri with the blur rule for the org.celestina applications
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys the registered binary

## Procedure

Built and checked in the session worktree:

```sh
(cd celestina-rs && cargo test -p grafita-core)
(cd grafita && cargo test)
(cd grafita && cargo build --release)
sh grafita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/grafita
bash scripts/qmllint-cxxqt.sh grafita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

## Result

- **Exit:** the grafita-core tests, the grafita tests, the release build, the smoke script, qmllint (45 baseline warnings, down from 47), the architecture contract, the language contract and the documentation contract all exited 0.
- **Observed:** A file that is not UTF-8 is read through `Encoding::detect`, which asks the guesser the host supplies (the Grafita application passes `chardetng`; `grafita-core` carries no detector, so Siderita's editor, which passes none, keeps refusing such files) and keeps the guess only when decoding and re-encoding reproduce the bytes exactly. A Windows-1252 note, a KOI8 note and a Latin-1 note open as text and save back byte for byte; UTF-8 and BOM files are never guessed at; a NUL byte or an encoding with no reversible table (EUC-JP) is refused as before. The footer encoding button, `EncodingDialog`, `Ctrl + E` and the session properties behind them are gone.

## Limits

- The guess can pick a neighbouring language that writes the same bytes back; the guarantee is that no byte is lost, not that the guess matches the author's intent.
- A very short sample may be guessed wrongly or not at all; it then stays refused as before.
- The window was not driven by hand in the session; the controller launches the binary.

## Landing

- **Base revision:** `2f6004d9d580c5b7afe4bc0ee385abed7d055742`
- **Check:** `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:afcb84176e44650db2398eefd1133ed23bed7ffe4cf826cfad7fc9547d86e4c9, verification_fingerprint sha256:b87b5fbeb7f572bda338461eb6f70b0653c3eaea7260ae7c01cde0eab2921e88; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:68f0e64d172fbb571583ba165113b78147d5fbfcfc581d77ef55af01d96b4854, verification_fingerprint sha256:e1c2f810e0eb191bfbff02c3dd84030677ea1019215ed4e1cbce685daa1dfbe9; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:bafa0983f9ff8c16589caa74062b7df891a27b692e16658aad0cbd93aa1b33c0, verification_fingerprint sha256:a5548e3778becf871df1926e6b2636faa252ed530a14fb78fdb06ec6f8e4845f
- **Deploy:** after the push: grafita: deploy-production.sh, status-production.sh; siderita: deploy-production.sh, status-production.sh
