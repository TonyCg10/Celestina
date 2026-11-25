# Evidence: ungating the pure wpctl status parser (CUP-WPCTL-STATUS)

- **Date:** 2026-11-25
- **Scope:** `CUP-2-A`: `Card`, `Status`, `parse_status`, `parse_volume` and their private helpers moved from `cuprita-core`'s `wpctl.rs` into `wpctl_status.rs`, declared without a feature gate; `wpctl.rs` re-exports the same names; no behaviour change to existing fields; additive `Status::playing` (ids of streams with an `[active]` port line, either direction), tested by `playing_streams_come_from_active_port_lines`
- **Environment:** worktree `cuprita-CUP-WPCTL-STATUS` on branch `unit/cuprita/CUP-WPCTL-STATUS`, base `b77fb722`; Rust workspace in `celestina-rs/`, Cuprita in `cuprita/` (Qt toolchain present)
- **Artifact:** not applicable (no production input changes)

## Procedure

Run from `celestina-rs/`:

```sh
cargo fmt --all -- --check
cargo test -p cuprita-core
cargo test -p cuprita-core --no-default-features --features nm,bluez
cargo clippy -p cuprita-core --all-targets -- -D warnings
cargo clippy -p cuprita-core --all-targets --no-default-features --features nm,bluez -- -D warnings
```

Run from the worktree root:

```sh
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
```

Run from `cuprita/`:

```sh
cargo check
```

## Result

- `cargo fmt --all -- --check`: exit 0 (after one `cargo fmt --all` that reflowed the moved code)
- `cargo test -p cuprita-core` (default features): exit 0; 70 tests passed, 0 failed across the test binaries; includes `wpctl_status::tests::card_names_lose_only_an_api_tag`, `wpctl_status::tests::volume_lines` and `wpctl_status::tests::playing_streams_come_from_active_port_lines`
- `cargo test -p cuprita-core --no-default-features --features nm,bluez`: exit 0; 52 tests passed, 0 failed; the `wpctl_status` tests run here, so the parser is built without the `wpctl` feature
- `cargo test -p cuprita-core --no-default-features`: exit 0 (earlier run; the crate's no-feature build, 2 + 6 + 7 + 4 + 2 tests in its binaries, all passed)
- `cargo clippy -p cuprita-core --all-targets -- -D warnings`: exit 0
- `cargo clippy -p cuprita-core --all-targets --no-default-features --features nm,bluez -- -D warnings`: exit 0
- `check-architecture-contract.sh`: exit 0
- `check-documentation-contract.sh`: exit 0 (the output carries many `erratum` lines for sealed inventories; they are informational, not errors)
- `check-language-contract.py`: exit 0 (128 legacy files ratcheted)
- `cargo check` in `cuprita/` (default features of `cuprita-core`): exit 0

## Observed facts

- The unit test `volume_lines` was split. Its `parse_volume` assertions moved with it to `wpctl_status`. Its `volume_arg` assertions stay in `wpctl`, under the new name `volume_arg_rounds`, because `volume_arg` is process-side.
- The new test was run first against the unchanged parser and failed to compile (`no field playing on type Status`, `cannot find type HashSet`), then passed after the field and the port-state check were added.
- `tests/wpctl.rs` (feature-gated integration test) imports `parse_status` through `cuprita_core::wpctl`; the re-export keeps it compiling unchanged.

## Limits

- No live `wpctl` or PipeWire check was run; the parser is tested over captured fixtures and the existing integration suite only.
- The `celestina-shell` crate was not built or tested: it does not consume this parser yet.

## Follow-up

- The shell's first use of `cuprita_core::wpctl_status::parse_status` belongs to the CEL-I-004 unit, not here.

## Landing

- **Base revision:** `b77fb722654d94325b4608fbed5ba39285d153e1`
- **Check:** `production_artifact.py check cuprita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; the toolchain changed since the build; run build-production.sh; `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** cuprita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:524b5ef958cb3d11df7311885c85b62b59dc1f8ea4fc1fa75d5ed29ea6483282, verification_fingerprint sha256:cf0d547ec959d2b43088da2cc9da275e2e3b566527499d49fac6906a87b0efb3; celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:86bfbc689b90fe60e48ed14770abd91edd01ce9181a24bde33e797d2260cc5e4, verification_fingerprint sha256:f0c17c671689126894dc28d424e02a4048eb41e95600c966b9b1ad1c0d5d44dd; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:31328c7c94ebcbff2e74aa7ba398ca519bcf0513695bf4467964b7a4f25276a4, verification_fingerprint sha256:bdf1f01d055fc0616141444c2e06baaf7d79efae197885f3f16168729c9be91b
- **Deploy:** after the push: cuprita: deploy-production.sh, status-production.sh; celestina: deploy-production.sh, status-production.sh
