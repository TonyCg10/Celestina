# Evidence: 2026-10-08 the volume's file system in the properties dialog

- **Date:** 2026-10-08
- **Scope:** `SID-H1-Q` — `siderita`
- **Environment:** the author's CachyOS, Qt 6.11.2, Niri with the blur rule for the org.celestina applications
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys the registered binary

## Procedure

```sh
(cd siderita && cargo fmt --all --check)
(cd siderita && cargo clippy --all-targets --locked -- -D warnings)
(cd siderita && cargo test)
(cd siderita && cargo build --release --locked)
sh siderita/scripts/qml-tests.sh
sh siderita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/siderita
bash scripts/qmllint-cxxqt.sh siderita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

`siderita/src/fsformat.rs` carries the unit tests of the mountinfo parsing
(escaped spaces, optional fields, byte-exact non-UTF-8 mount points, longest
prefix on component boundaries, stacked mounts) and of the name table.

## Result

- **Exit:** formatting, clippy, the unit tests (157 passed, 0 failed), the
  release build, the QML tests (174 passed, 0 failed), the smoke run, qmllint
  (239 baseline warnings, unchanged), the architecture contract, the language
  contract and the documentation contract exited 0.

## Limits

- The dialog itself was not opened in the session; the judgement of the row
  is the author's.

## Follow-up

- None.

## Landing

- **Base revision:** `2ae8e359c302fea1ec354add4b6a5841ceaacdea`
- **Check:** `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:9f908b2d61c3b14ccd29a8903b22374e8277741b5592d024e350b24a0027f548, verification_fingerprint sha256:4abba0361c9e8255a46dd788bf04792d13edc5251386c620e97d7ac6da098571
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh
