# Evidence: Cuprita registration

- **Date:** 2026-10-08
- **Scope:** AUD-1-P — suite (with `CUP-1-A`, the application skeleton)
- **Environment:** CachyOS, Python 3.14, Rust 1.97.1, cxx-qt 0.9.1, Qt 6.11.2
- **Artifact:** `cuprita` release binary in the shared Cargo target, not
  installed

## Procedure

Cuprita (control centre for network, Bluetooth and audio) is registered the
way commit 28d1b263 registered Hematita, in one commit because the guards now
require every registered path to exist: the registry entry with its Cargo
version source and mirror, the empty `cuprita-core` crate in the workspace,
ratchet rows at 0, a README row, the ledger unit, the app icon, the `cuprita/`
skeleton (see
[the skeleton evidence](../../cuprita/docs/evidence/2026-10-08-skeleton.md))
registered unversioned (the author records the 0.1.0 baseline by hand after the
landing, as `scripts/land-unit.py` requires). The design
spec and the foundation plan are committed with it.

Run from the worktree root unless noted; the exit code of each command:

| Command | Exit | Output |
|---|---|---|
| `bash scripts/check-documentation-contract.sh` | 0 | Documentation contract: OK |
| `bash scripts/check-architecture-contract.sh` | 0 | Architecture contract: OK |
| `python3 scripts/check-language-contract.py` | 0 | OK (141 legacy file(s) ratcheted) |
| `python3 scripts/test-production-artifacts.py` | 0 | Ran 49 tests, OK |
| `python3 scripts/test-land-unit.py` | 0 | Ran 85 tests, OK |
| `bash scripts/test-production-common.sh` | 0 | production-common fixtures: OK |
| `python3 scripts/version_tool.py check` | 0 | version-contract: OK (9 owners) |
| `sh cuprita/scripts/qml-tests.sh` | 0 | 5 passed, 0 failed |
| `sh cuprita/scripts/smoke.sh --binary <release binary>` | 0 | binary alive for 8 s, no QML errors |
| `bash scripts/qmllint-cxxqt.sh cuprita` | 0 | 0 non-fatal baseline warning(s) |
| `cargo check -p cuprita-core --locked` (in `celestina-rs`) | 0 | Finished |
| `cargo fmt --all --check` (in `cuprita`) | 0 | no diff |
| `cargo clippy --all-targets --locked -- -D warnings` (in `cuprita`) | 0 | no warnings |
| `cargo test` (in `cuprita`) | 0 | 0 tests, ok |
| `cargo build --release --locked` (in `cuprita`) | 0 | Finished |

## Result

- **Exit:** every command above exits 0.
- **Observed:** `Cargo.lock` of `celestina-rs` gains one package entry for the
  empty `cuprita_core`; the qmllint, glass-canvas and radius rows for
  `cuprita` are 0 and stay 0.

## Limits

- The pages are empty and nothing is installed; live behaviour is for the
  author (`cuprita/VALIDATION.md`).
- The architecture contract prints literal-spacing findings for
  `celestina/qml/OutputChooser.qml` and other files; they are identical on
  `origin/main` and do not fail the contract. The halted shell was not touched.

## Follow-up

`CUP-1-B`: the core models, the three traits and the fakes.

## Landing

- **Base revision:** `f9bca364ed1277679a60ed7394b1726be62c0151`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 0: artifact: celestina-style current; `production_artifact.py check celestina-rs --require-verified` exit 0: artifact: celestina-rs current; `production_artifact.py check siderita --require-verified` exit 0: artifact: siderita current; `production_artifact.py check magnetita --require-verified` exit 0: artifact: magnetita current; `production_artifact.py check magnetita-android --require-verified` exit 0: artifact: magnetita-android current; `production_artifact.py check grafita --require-verified` exit 0: artifact: grafita current; `production_artifact.py check fluorita --require-verified` exit 0: artifact: fluorita current; `production_artifact.py check hematita --require-verified` exit 0: artifact: hematita current; `production_artifact.py check cuprita --require-verified` exit 0: artifact: cuprita current
- **Build:** artifact current; no build
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh; cuprita: deploy-production.sh, status-production.sh
