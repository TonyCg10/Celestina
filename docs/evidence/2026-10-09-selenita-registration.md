# Evidence: Selenita registration

- **Date:** 2026-10-09
- **Scope:** `EXT-1-B` — suite (with the Selenita skeleton)
- **Environment:** CachyOS, Python 3, Rust 1.98.1, cxx-qt 0.9.1, Qt 6.12.0
- **Artifact:** `selenita` release binary in the shared Cargo target, not
  installed

## Procedure

Selenita (the suite's screen capture and recording tool) is registered the way
EXT-1-A registered Calcita, in one commit because the pre-commit hook runs the
documentation contract on the index: the registry entry (`versioned = false`;
the author records the 0.1.0 baseline by hand after the landing), the empty
`selenita-core` crate in the workspace, the ratchet rows, a README row, the
app icon (a crescent moon inside four capture corners), the `selenita/`
skeleton (see
[the skeleton evidence](../../selenita/docs/evidence/2026-10-09-skeleton.md))
and Selenita in the activation guard's real-tree test. `SELENITA` was already
in `celestina_core::activation`'s names (added by EXT-1-A); nothing there
changes. The desktop entry has `Exec=selenita` with no field code, and both
the command line and an `Open` are ignored, so a file handed to Selenita never
triggers anything.

Run from the worktree root unless noted; the exit code of each command:

| Command | Exit | Output |
|---|---|---|
| `cargo fmt --all --check` (in `selenita`) | 0 | no diff |
| `cargo clippy --all-targets --locked -- -D warnings` (in `selenita`) | 0 | no warnings |
| `cargo test --locked` (in `selenita`) | 0 | 4 passed |
| `cargo build --release --locked` (in `selenita`) | 0 | Finished |
| `cargo test --locked -p selenita-core -p celestina-core --features celestina-core/activation` (in `celestina-rs`) | 0 | 83 + 0 passed |
| `cargo fmt --all --check` (in `celestina-rs`) | 0 | no diff |
| `sh selenita/scripts/qml-tests.sh` | 0 | 5 passed, 0 failed |
| `sh selenita/scripts/smoke.sh --binary <release binary>` | 0 | alive 8 s, no QML errors, `cards=3 fake=true` |
| `bash scripts/qmllint-cxxqt.sh selenita` | 0 | 1 non-fatal baseline warning |
| `bash scripts/check-architecture-contract.sh` | 0 | OK |
| `python3 scripts/test-activation-contract.py` | 0 | Ran 13 tests, OK |
| `python3 scripts/check-language-contract.py` | 0 | OK |
| `bash scripts/check-documentation-contract.sh` | 0 | OK |
| `python3 scripts/test-production-artifacts.py` | 0 | Ran 49 tests, OK |
| `python3 scripts/test-land-unit.py` | 0 | OK (run on the commit) |
| `bash scripts/test-production-common.sh` | 0 | production-common fixtures: OK |
| `python3 scripts/version_tool.py check` | 0 | version-contract: OK (10 owners) |

## Result

- **Exit:** every command above exits 0.
- **Observed:** `celestina-rs/Cargo.lock` gains one package entry for the
  empty `selenita-core`. The glass-canvas and radius rows for `selenita` are
  0. The qmllint row is 1, measured: the same Qt 6.12 report on the shared
  `celestina-style/CelestinaIcons.qml` ("not declared as singleton in
  qmldir") that Calcita's row records; Selenita's own files have no warning.

## Limits

- Nothing captures or records and nothing is installed; live behaviour is
  for the author (`selenita/VALIDATION.md`, entries added by the SEL-1
  units). The runtime tools (`grim`, `slurp`, `wl-clipboard`, niri,
  `gst-plugins-good`) are listed in the README and not needed yet.

## Follow-up

`SEL-1-A`; after the landing, the author's baseline commit
(`version_source`, `version_mirrors`, `selenita 0.1.0 baseline EXT-1-B`).

## Landing

- **Base revision:** `42595540b979ce2c7b985c1792ecdc761d9ab972`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check cuprita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check calcita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact is not verified yet; run verify-production.sh; `production_artifact.py check selenita --require-verified` exit 1: production-artifact: missing selenita/target/production-artifact.toml; run selenita/scripts/build-production.sh first
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:fbc4a293e5075a9c6a543bf7e2c224f6c3be52eb60a4594f14d5c54b99501906, verification_fingerprint sha256:a9c3b6c1e35ea2f7c432975303dcae78bfb1e66ad0a2c6cdf00db97ea55f14a7; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:b84b702a25bae96260d0d4fc7296010d4293889684e3772dd28557a138b2a224, verification_fingerprint sha256:f3e63d44eead06d3b38e276f9e0460e046333e90dd82421e06653df28f4e21a9; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:76cbea3262aa00212d603290c2f0a9b4146023ecac0447ecdc047d2851f4bb9f, verification_fingerprint sha256:4e4c9f4bbfe41cd41690fa242748446d59112ca070feb53af29f48f8dc373f81; magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:b9370cd2bf69f3f729bbb66a594ab9d5b7aa2bd6e1d44ae856149d7558ea5adf, verification_fingerprint sha256:147871ad792a5b53ee0f7c665ce91ce25872de9ae2ac451bedc6bdde13f290e9; magnetita-android build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:c7db33b61b1193f4a0b93de292e677f266df0f08d283d8935155cbde292129b5, verification_fingerprint sha256:ad5a9a7d84e572a6f89188ddfa0b1d5d3c6e1d3fb9d892a8930df0847d5ffcff; grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:dd35a2fd5df0f2e7e9d78fbc33a0f7f732aa05e7da4d725c206913779ed854a4, verification_fingerprint sha256:a0ae233ecf3c7e8e31ae8a81422875600a4fc516e5f7cf743920aa144f202ad8; fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:4721ca6a86e81516c94a5822621d6811c01f664dc002b11c1f82526f663fef6b, verification_fingerprint sha256:bb4e7203c261ba3c77e61be97c966f04e13b0db0546428e91cc64671d6a8e1ea; hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:06d830af961ca05b39340b5ba38ac2fe8d0038b4c097013e1a5fcfd15c8cce56, verification_fingerprint sha256:daee35677d75e773025c27c23abffae482c9721a36f86ccaaab3fc9aaa047985; cuprita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:88eda80fdb596a8d9e012023f8782875b8ef424eb7258edef54ca8ca7a437474, verification_fingerprint sha256:b2cdbda340eec5b42466a3720d8925e7541a52675bd29e80cf4a2414dacd6606; calcita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:5ee87c7549bdf61d3ace7ff1810b779e2af56d4a198339215598b169fdf11318, verification_fingerprint sha256:0f5538769d977295b37aab7ecd9a0aaf54febbe0d41a948e32589c2b5c53621e; selenita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:2a15914b4f2c02c3e09cb77fd865fc843c884c6b7b77875065e4e2f35d7445ac, verification_fingerprint sha256:d71000978130d432af2a7d8fb3ce42b27663d16ebb3b164bf0f07d4dd98af78c
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh; cuprita: deploy-production.sh, status-production.sh; calcita: deploy-production.sh, status-production.sh; selenita: deploy-production.sh, status-production.sh
