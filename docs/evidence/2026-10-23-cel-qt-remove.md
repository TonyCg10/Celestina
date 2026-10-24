# Evidence: CEL-QT-REMOVE — the Qt shell leaves the tree

- **Date:** 2026-10-23
- **Scope:** by the author's decision of 2026-10-23, `celestina-qt/` and
  `celestina-rs/crates/celestina-shell-core` are deleted from the tree with
  no tag, branch or copy kept. `docs/projects.toml` drops the `celestina-qt`
  entry and its `celestina-shell-core` commit scope and lists the shell under
  `[[retired_projects]]` (CEL-QT-RETIRE's rules); the workspace drops the
  crate and `Cargo.lock` drops only that package; `celestina-qt.yml` and its
  row in the workflows README go; the `celestina-qt` row of the radius
  ratchet and the `celestina-shell-core` rows of the language ratchet go;
  "Halted projects" in `AGENTS.md` becomes the generic rule plus a history
  note and the salvage record; the AGENTS.md of calcita, cuprita, hematita,
  magnetita, selenita, celestina and celestina-rs, `README.md`, `ROADMAP.md`,
  `STATUS.md`, `celestina/STATUS.md`, `Celestina.code-workspace` and the
  links in evidence, history and archived plans stop pointing at the removed
  paths; ADR 0002 of celestina is superseded by the new ADR 0023.
- **Environment:** CachyOS, worktree `suite-CEL-QT-REMOVE`, branch
  `unit/suite/CEL-QT-REMOVE`, stacked on `unit/suite/CEL-QT-RETIRE`
- **Artifact:** not built in the session. The landing builds and verifies
  every registered project, because the workspace manifests and the shared
  guards this unit touches are inputs of all of them, and deploys each
  deployable one

## Procedure

Run from the worktree root: the guards and script tests below, and from
`celestina-rs/` the workspace check and the tokens tests.

## Result

| Command | Exit | Output |
|---|---|---|
| `bash scripts/check-architecture-contract.sh` | 0 | Architecture contract: OK |
| `python3 scripts/check-language-contract.py` | 0 | Language contract: OK (128 legacy files ratcheted; three `celestina-shell-core` rows gone) |
| `bash scripts/check-documentation-contract.sh` | 0 | Documentation contract: OK |
| `python3 scripts/version_tool.py check` | 0 | version-contract: OK (11 owners; `celestina-qt` retired) |
| `python3 scripts/check-sealed-colours.py` | 0 | Sealed colour contract: OK (4 colour(s)) |
| `bash scripts/test-commit-scope.sh` | 0 | Commit scope: OK |
| `bash scripts/test-staged-units.sh` | 0 | Staged inventories: OK |
| `python3 scripts/test-version-contract.py` | 0 | OK |
| `bash scripts/test-architecture-scanners.sh` | 0 | Architecture fixtures: OK |
| `cargo check --workspace --offline` (celestina-rs) | 0 | clean; `Cargo.lock` loses only the `celestina-shell-core` package |
| `cargo test -p celestina-tokens --offline` | 0 | 8 passed |

## Limits

- `land-unit.py` was not run end to end.
- The RSH-1 plan's exclusion "Any edit of `celestina-qt/` or
  `celestina-rs/crates/celestina-shell-core`" still reads as written: a unit
  may change only its own row of that plan, and the exclusion is now moot
  because the trees are gone.
- The deployed `~/.local/lib/celestina/niri` and the generated
  `~/.local/share/celestina/generated/*` on the author's host are untouched;
  the portal `Settings` backend and the writer of `niri-colours.kdl` lived
  only in the removed trees and are not carried over.
- Nothing was stashed.

## Landing

- **Base revision:** `39bd1a89be09df8222e58f1bdb3801e99237a7b6`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check cuprita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check calcita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check selenita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:63dbea4579a43510db13aca15d4cb556af62f3e0685779498acdc8baad8e60be, verification_fingerprint sha256:ce4740a70d3e7a7efa60120e65470ce4c090c691e367fa22a4d70abfd1fc1c0c; celestina-style verify: verify-production.sh exit 0, manifest source_fingerprint sha256:0810aa272e5ba64ca25e014c71afd25636cd510bc5352bbb00e796d1e0244069, verification_fingerprint sha256:f7462d59294f4b59516c39991ae89811b66b2f421fe7ac93a36202e066e6f4c5; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:2c7dbcc4f9ae13c421be6bdb7d064188d22e5d321fe08da772db008a95e36bd2, verification_fingerprint sha256:80379e7255146de613e1bf1c11b605914c303b47534fb669232380edee97e993; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:73bf91970100343c7eacc1ea1e74e8d3dca0258aae007cba8496f320775a6ee2, verification_fingerprint sha256:df6b621ae12bd7df71d4532d7bd90539fa98ca17f32c257f9f559f40f1a3ac90; magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:94908a6b693a3e05b3ebb0a303facf73f494c1bfe248b0647883ca782390ea2b, verification_fingerprint sha256:2f166d969467d218b5c74d5d89fef93757b52fe2f1e7c72f8fb118403f7f48c2; magnetita-android build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:53e96e4711981db565e657df98a32afd1ffc764618dfadad0d7f0568b0b16de1, verification_fingerprint sha256:896efa7b25e51b3707a7480a630e2eb49a61904203b0b90fd2ce65c1ee731d50; grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:5f6e9f0f1724a2af64f2fff38863570f575008dbd21628657247db038a838f41, verification_fingerprint sha256:4ccab3ce71ac84bed119152192242c192ffe9dcb7e8337aa54c4b36c86173e77; fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:287974e8293c60f96a595b8d088caf2742fc3067c1e594db6af32a5add5f4897, verification_fingerprint sha256:82ccbb1ad260b07e85d3478aaec74b4822d92ac852b526fbccd80cd539018d9e; hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:5b6578ded7705edee7cac225a876a0074effec6ddc884c43edc7665d73bac1ef, verification_fingerprint sha256:c26ca6b61830100aa68ae607bbe972359fc53954c57899e80c4c07b8745a196e; cuprita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:a897e8974289d045cdfcdae958d6faaf6ab560b7a90103a9b3d3a188dce99dd4, verification_fingerprint sha256:cf0d547ec959d2b43088da2cc9da275e2e3b566527499d49fac6906a87b0efb3; calcita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:551310893bef95c27652cb580d730e53b492c31ecb09f4e3b1070fd06ac7fb8c, verification_fingerprint sha256:38cc96d600c9b5e52b468a211a64eca28d2f9c4e4923254bdfaf7e8ca0ce0f86; selenita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:c1df86cafc2bb2c3816914201273373f019c97ed58ca5b280a507ac8ec5647f7, verification_fingerprint sha256:6dd984c01d6db9fe4bdd9b13400a3beba2dbed81eceb8aeb68c19b3dc0117097
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh; siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh; cuprita: deploy-production.sh, status-production.sh; calcita: deploy-production.sh, status-production.sh; selenita: deploy-production.sh, status-production.sh
