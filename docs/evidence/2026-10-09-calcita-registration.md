# Evidence: Calcita registration

- **Date:** 2026-10-09
- **Scope:** `EXT-1-A` — suite (with the Calcita skeleton)
- **Environment:** CachyOS, Python 3, Rust 1.98.1, cxx-qt 0.9.1, Qt 6.12.0
- **Artifact:** `calcita` release binary in the shared Cargo target, not
  installed

## Procedure

Calcita (the suite's PDF viewer) is registered the way commit 720406e9
registered Cuprita, in one commit because the pre-commit hook runs the
documentation contract on the index: the registry entry (`versioned = false`;
the author records the 0.1.0 baseline by hand after the landing), the empty
`calcita-core` crate in the workspace, the ratchet rows, a README row, the app
icon, the `calcita/` skeleton (see
[the skeleton evidence](../../calcita/docs/evidence/2026-10-09-skeleton.md)),
`CALCITA` and `SELENITA` in `celestina_core::activation`'s names, Calcita in
the activation guard's real-tree test, and the root roadmap, status and
active-plans index opening `EXT-1`. The design spec and the plan are committed
with it.

Run from the worktree root unless noted; the exit code of each command:

| Command | Exit | Output |
|---|---|---|
| `cargo fmt --all --check` (in `calcita`) | 0 | no diff |
| `cargo clippy --all-targets --locked -- -D warnings` (in `calcita`) | 0 | no warnings |
| `cargo test --locked` (in `calcita`) | 0 | 2 passed |
| `cargo build --release --locked` (in `calcita`) | 0 | Finished |
| `cargo test --locked -p calcita-core -p celestina-core --features celestina-core/activation` (in `celestina-rs`) | 0 | 83 + 0 passed |
| `cargo fmt --all --check` (in `celestina-rs`) | 0 | no diff |
| `sh calcita/scripts/qml-tests.sh` | 0 | 5 passed, 0 failed |
| `sh calcita/scripts/smoke.sh --binary <release binary>` | 0 | alive 8 s, no QML errors, `empty=true` |
| `bash scripts/qmllint-cxxqt.sh calcita` | 0 | 1 non-fatal baseline warning |
| `bash scripts/check-architecture-contract.sh` | 0 | OK, activation contract OK |
| `python3 scripts/test-activation-contract.py` | 0 | Ran 13 tests, OK |
| `python3 scripts/check-language-contract.py` | 0 | OK |
| `bash scripts/check-documentation-contract.sh` | 0 | OK |
| `python3 scripts/test-production-artifacts.py` | 0 | Ran 49 tests, OK |
| `python3 scripts/test-land-unit.py` | 0 | Ran 85 tests, OK (run on the commit) |
| `bash scripts/test-production-common.sh` | 0 | production-common fixtures: OK |
| `python3 scripts/version_tool.py check` | 0 | version-contract: OK (9 owners) |

## Result

- **Exit:** every command above exits 0.
- **Observed:** `celestina-rs/Cargo.lock` gains one package entry for the
  empty `calcita-core`. The glass-canvas and radius rows for `calcita` are 0.
  The qmllint row is 1, not 0: Qt 6.12's qmllint reports
  `celestina-style/CelestinaIcons.qml` as "not declared as singleton in
  qmldir" although the generated qmldir declares it; the same warning is
  inside every application's row that links the file (measured in Cuprita
  and Grafita), and it appears for any singleton file that has no `for` loop.
  Calcita's own files have no warning.

## Limits

- No document opens and nothing is installed; live behaviour is for the
  author (`calcita/VALIDATION.md`, entries added by the CAL-1 units).
- `scripts/test-land-unit.py` copies untracked files with their symlinks
  followed, so before the commit it sees the style links as new text; it is
  run on the commit.

## Follow-up

`CAL-1-A`; after the landing, the author's baseline commit
(`version_source`, `version_mirrors`, `calcita 0.1.0 baseline EXT-1-A`).

## Landing

- **Base revision:** `47977b299aa6b6ce830858f1934a56cd36781b81`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check cuprita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check calcita --require-verified` exit 1: production-artifact: missing calcita/target/production-artifact.toml; run calcita/scripts/build-production.sh first
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:3badadb08016267725129dfe73b637cc31dec819cf7c5003d00d6b236a364f5b, verification_fingerprint sha256:ec43c91c3c9a14b56babf98652099b58b709f7020404a57cebc18802bb180d0f; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:50eeff6b32198da0e9fdf9970c1158262460d838974a17eb48e1ed34187fcafb, verification_fingerprint sha256:50d78c3bb5e519189018963d02e5c0e30420beaad04bf1d891948d93c80f7543; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:11414ad83e05db55a005e9292e3790ac0288e2c02c9b34e0c939b614abad88ab, verification_fingerprint sha256:4e4c9f4bbfe41cd41690fa242748446d59112ca070feb53af29f48f8dc373f81; magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:92b2ca70eedd4f5e68e7175d28188d846ac409673c2ad5c26a88853e347bb960, verification_fingerprint sha256:147871ad792a5b53ee0f7c665ce91ce25872de9ae2ac451bedc6bdde13f290e9; magnetita-android build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:10b70e890e8033493c0ad539157d63001b9d11c8f8e10f3a1d013995d7eed78b, verification_fingerprint sha256:ad5a9a7d84e572a6f89188ddfa0b1d5d3c6e1d3fb9d892a8930df0847d5ffcff; grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:30bd44af23eea2402c6f4b809dd88fbe2708a3374b3eacf23020f77244be4a02, verification_fingerprint sha256:a0ae233ecf3c7e8e31ae8a81422875600a4fc516e5f7cf743920aa144f202ad8; fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:2ef91ad755bda383a6ded48bfa797dfb9881838f934e54ae9032409344775e1e, verification_fingerprint sha256:bb4e7203c261ba3c77e61be97c966f04e13b0db0546428e91cc64671d6a8e1ea; hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:ea4f5a0952090bbd6c760149de1e6eaf0480ce9fce4ec30aa75aa5166f5b5622, verification_fingerprint sha256:daee35677d75e773025c27c23abffae482c9721a36f86ccaaab3fc9aaa047985; cuprita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:4e0bcc4f70c9634232b00ff3a3f177a2702a358449624b6cf5b9039bde85d5d5, verification_fingerprint sha256:b2cdbda340eec5b42466a3720d8925e7541a52675bd29e80cf4a2414dacd6606; calcita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:1cb902dbd222df6def87a0acb4305e1c677ee23ddf1f210511ab774333ce51ed, verification_fingerprint sha256:7ce8ab3c52f15da68ee666c7af4867aedbd457a5755bb965ce8ea47269cf0a44
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh; cuprita: deploy-production.sh, status-production.sh; calcita: deploy-production.sh, status-production.sh
