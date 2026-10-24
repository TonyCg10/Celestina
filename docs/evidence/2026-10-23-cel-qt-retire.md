# Evidence: CEL-QT-RETIRE — the registry learns to retire a halted project

- **Date:** 2026-10-23
- **Scope:** the guard rules the removal of `celestina-qt/` needs ahead of it.
  `docs/projects.toml` may now carry `[[retired_projects]]`: a retired project
  keeps its `docs/version-history.tsv` rows as a closed chain with no version
  source, its old commit prefixes and roots keep resolving when the history is
  replayed, and its historical inventories may be deleted by the unit that
  retires it. Retiring is allowed only through `suite-maintenance`, for a
  project that is `halted` in HEAD, with the same `history_owner`, with no new
  rows and no other version bumps, and its `commit_prefixes` and
  `commit_roots` must repeat what the project had; every key of the entry is
  then permanent. The landing's `preflight` leaves a retired owner's deleted
  plans (active and archived) and inventories out of its deletion and
  immutable-inventory stops (the unit's inventory claims each deletion), and
  sets aside another owner's active plan whose only change is to stop
  linking the retired tree, strictly line by line and cell by cell. The architecture
  guard no longer requires a QML project of the shell role and no longer
  runs `cmake-qml-registration` against the fixed `celestina-qt` paths. The
  halted-project support stays as it is. No project is retired by this unit.
- **Environment:** CachyOS, worktree `suite-CEL-QT-RETIRE`, branch
  `unit/suite/CEL-QT-RETIRE`
- **Artifact:** not built; no production entry ran in the session

## Procedure

Run from the worktree root: the script tests and the guards below.

## Result

Run in series from the worktree root on the final tree (the commit plus the
amendment). Every command exited 0:

| Command | Exit | Output |
|---|---|---|
| `bash scripts/test-architecture-scanners.sh` | 0 | Architecture fixtures: OK |
| `bash scripts/test-commit-scope.sh` | 0 | Commit scope: OK (the `qt-shell` fixtures replaced by a `fixture-shell` project; one retired-project replay case added) |
| `bash scripts/test-documentation-contract.sh` | 0 | Documentation contract and agent-context: OK |
| `bash scripts/test-land-unit.sh` | 0 | Ran 96 tests, OK |
| `bash scripts/test-production-artifacts.sh` | 0 | production-common fixtures: OK |
| `bash scripts/test-production-common.sh` | 0 | production-common fixtures: OK |
| `bash scripts/test-qml-dev.sh` | 0 | all cases ok |
| `bash scripts/test-qmllint-target.sh` | 0 | all cases ok |
| `bash scripts/test-staged-units.sh` | 0 | Staged inventories: OK (three retirement cases added) |
| `bash scripts/test-worktree.sh` | 0 | all cases ok |
| `python3 scripts/test-activation-contract.py` | 0 | Ran 14 tests, OK |
| `python3 scripts/test-build-trees.py` | 0 | Ran 14 tests, OK |
| `python3 scripts/test-glass-canvas-contract.py` | 0 | Ran 9 tests, OK |
| `python3 scripts/test-land-unit.py` | 0 | Ran 96 tests, OK (three retirement function tests and one end-to-end retirement landing added) |
| `python3 scripts/test-language-contract.py` | 0 | Ran 19 tests, OK |
| `python3 scripts/test-production-artifacts.py` | 0 | Ran 49 tests, OK |
| `python3 scripts/test-radius-contract.py` | 0 | Ran 25 tests, OK |
| `python3 scripts/test-version-contract.py` | 0 | Ran 31 tests, OK (`RetireTests` and `RetiredRegistryTests` added; rejections of a later edit of `commit_roots`, `commit_prefixes` or `retired` and of a retirement whose prefixes or roots differ from HEAD's) |
| `bash scripts/check-architecture-contract.sh` | 0 | Activation contract: OK; Architecture contract: OK |
| `bash scripts/check-documentation-contract.sh` | 0 | Documentation contract: OK |
| `python3 scripts/check-language-contract.py` | 0 | Language contract: OK (131 legacy file(s) ratcheted) |
| `python3 scripts/check-sealed-colours.py` | 0 | Sealed colour contract: OK (4 colour(s)) |
| `python3 scripts/check-staged-units.py` | 0 | staged-unit: no staged unit closure |
| `python3 scripts/version_tool.py check` | 0 | version-contract: OK (12 owners) |

A landing simulation in a throwaway clone (a local bare origin at
`36f3a7eb`, the repository hooks enabled) drove `land-unit.py`'s steps from
`preflight` to `commit_and_push`, without build, deploy or prune: this unit
landed first, then the removal unit CEL-QT-REMOVE stacked on it (its branch
at `6bd136b2`) landed through the hooks with
`554 files changed, 1319 insertions(+), 111676 deletions(-)`; its inventory
claims 64 deleted inventories and 25 deleted plans of `celestina-qt/`, the
`celestina-style` plan that only stops linking the retired tree was set aside
as a settled plan, and `version_tool.py check` reported
`version-contract: OK (11 owners)` afterwards.

## Limits

- The rules are latent on this tree: `docs/projects.toml` declares no
  `[[retired_projects]]` yet, so every new branch is exercised by the tests
  only. The removal unit that follows is the first real use.
- `land-unit.py` was not run as a command on the real checkout; the
  simulation called its step functions in a clone and skipped the build,
  the deploy and the prune.
- Nothing was stashed.

## Landing

- **Base revision:** `36f3a7ebb3d6bae4e19981c67a823e26281b25e5`
- **Check:** `production_artifact.py check celestina --require-verified` exit 0: artifact: celestina current; `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: artifact digest or set does not match the recorded build; artifact is not verified yet; run verify-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check cuprita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check calcita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check selenita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:0810aa272e5ba64ca25e014c71afd25636cd510bc5352bbb00e796d1e0244069, verification_fingerprint sha256:c7994bc29be8c1f059d68cb63005e0ed8ea13ce090c4cd66d6ab7588d11da3db; celestina-rs verify: verify-production.sh exit 0, manifest source_fingerprint sha256:08062b784e51507ff4d56c03a1d6b0aff42cd282905857137ec083e1e4493cbc, verification_fingerprint sha256:80379e7255146de613e1bf1c11b605914c303b47534fb669232380edee97e993; siderita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:9368a00de1a29f587b1ea407afe25d5d37b86be9445afc16b7f835c3f01e3e9c, verification_fingerprint sha256:df6b621ae12bd7df71d4532d7bd90539fa98ca17f32c257f9f559f40f1a3ac90; magnetita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:57a25d52efee0ac17ded4404ac56b8ff27d65871b53f85d55c1b63346a5cb240, verification_fingerprint sha256:2f166d969467d218b5c74d5d89fef93757b52fe2f1e7c72f8fb118403f7f48c2; magnetita-android verify: verify-production.sh exit 0, manifest source_fingerprint sha256:feb113c9cf21e1197da4c66a6cdb2d907a1ffb4585b3bfcb8350762155ca5390, verification_fingerprint sha256:896efa7b25e51b3707a7480a630e2eb49a61904203b0b90fd2ce65c1ee731d50; grafita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:7a568afa3cb5623f75088990b530589618f8b367c7cd462ce07e8cae68a56409, verification_fingerprint sha256:4ccab3ce71ac84bed119152192242c192ffe9dcb7e8337aa54c4b36c86173e77; fluorita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:f75aafb8ed9ccfe6cf146a434298869cea9537b77796bdde43b116ac397edd7b, verification_fingerprint sha256:82ccbb1ad260b07e85d3478aaec74b4822d92ac852b526fbccd80cd539018d9e; hematita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:210781ffb83ffc5b1b5f86f605335ccf0a6207b61fb12a8ab303552de4aba98b, verification_fingerprint sha256:c26ca6b61830100aa68ae607bbe972359fc53954c57899e80c4c07b8745a196e; cuprita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:2a13434ffd389d9b394d28ff4535facda7b5a77598aba1a6612c6cef40d7064d, verification_fingerprint sha256:cf0d547ec959d2b43088da2cc9da275e2e3b566527499d49fac6906a87b0efb3; calcita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:cd1cdbf6261e33a1012a8d032f1e8976c2502325ba33d70c86c7c2779188e64c, verification_fingerprint sha256:38cc96d600c9b5e52b468a211a64eca28d2f9c4e4923254bdfaf7e8ca0ce0f86; selenita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:849761f414069ddd5aece4c6e8d06a72f2dcfc8fa7be08740146478a3bee4a86, verification_fingerprint sha256:6dd984c01d6db9fe4bdd9b13400a3beba2dbed81eceb8aeb68c19b3dc0117097
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh; siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh; cuprita: deploy-production.sh, status-production.sh; calcita: deploy-production.sh, status-production.sh; selenita: deploy-production.sh, status-production.sh
