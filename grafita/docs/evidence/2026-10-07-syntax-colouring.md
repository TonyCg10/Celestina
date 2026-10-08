# Evidence: 2026-10-07 syntax colouring for every language KSyntaxHighlighting knows

- **Date:** 2026-10-07
- **Scope:** `GRA-H1-G` — `grafita`
- **Environment:** the author's CachyOS, Qt 6.11.2, KF6 KSyntaxHighlighting 6.30 (`syntax-highlighting` package), Niri with the blur rule for the org.celestina applications
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

- **Exit:** `cargo test -p grafita-core` (109, 4, 29, 44, 16 and 35 tests passed), `cargo test` in `grafita` (13 tests passed, 7 of them the new `syntax` tests), the release build, the smoke script (now also opening a `.kdl` file), qmllint (45 baseline warnings, unchanged), the architecture contract, the language contract and the documentation contract all exited 0.
- **Observed:** `grafita_definition_name` picks KDL for `config.kdl` and a full path to one, `.desktop` for a desktop entry, TOML for `Cargo.toml`, Markdown for `README.md` and Bash for an extension-less file starting `#!/bin/sh`, and nothing for `notas.xyz123`, `notas.txt` or an unnamed document. The bracket tests cover the bracket after the caret winning over the one before it, nesting by depth, pairs across lines, unmatched brackets, brackets inside a quoted range, and the 100 000-character bound. Typing `/*` above 5 000 lines of C re-colours every following line in the same pass and reaches the document as at most two edits (the highlighter drives KSyntaxHighlighting's `AbstractHighlighter` from a `QSyntaxHighlighter`, because the library's ready-made `SyntaxHighlighter` queued one edit per following line). The release binary links `libKF6SyntaxHighlighting.so.6`. The `grafita-core` session test that the colouring name follows a symlink to its file failed to compile before `syntax_file_name` existed and passes after; the `syntax` tests failed to build before the C++ functions existed and pass after.

## Limits

- The colours and the bracket boxes were not seen on the author's display in the session; the controller launches the binary. `VAL-GRA-SYNTAX` is the manual check.

## Landing

- **Base revision:** `edff0ef2d6128744c9a5da868a940fc79750a485`
- **Check:** `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:f582ad11c3df0b920b8c8466ca1b04dc8dd7c8d21e86fc3094dc0aace9fcfd28, verification_fingerprint sha256:46135472b86ee80bc8148824a12eba3075369cc656f66c11c9e69362cb30db3e; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:16db1ea3d96e80baf1b0f5b00ff6b5a79b97db5db9a45153c381dc8589eeb719, verification_fingerprint sha256:1805f1617d49be0892c3e2c3328331bfebb8b8ea3ef537528f504a9ccd5d0c28; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:f17b0bbb49dbd420c38fbc0752eddc36fa1f7154cbe47146a67c7131018de096, verification_fingerprint sha256:a5548e3778becf871df1926e6b2636faa252ed530a14fb78fdb06ec6f8e4845f
- **Deploy:** after the push: grafita: deploy-production.sh, status-production.sh; siderita: deploy-production.sh, status-production.sh
