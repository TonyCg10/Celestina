# Evidence: CEL-QT-RESCUE — what only the halted shell holds, copied out

- **Date:** 2026-10-23
- **Scope:** two salvage copies ahead of the removal of `celestina-qt/`: the
  Niri compositor patch with its README and build script into
  `celestina/packaging/niri/`, and the Niri colour generator into
  `celestina-rs/crates/celestina-tokens/src/niri_colours.rs`; the sealed-colours
  guard repointed to the new path; the salvage entry in `AGENTS.md` "Halted
  projects"; this unit's row and scope bullet in the RSH-1 ledger. Nothing of
  `celestina-qt/` or `celestina-shell-core` is edited or deleted.
- **Environment:** CachyOS, worktree `suite-CEL-QT-RESCUE`, branch
  `unit/suite/CEL-QT-RESCUE`
- **Artifact:** not built; no production entry ran in the session

## What was copied

| Source (commit) | Destination | Change |
|---|---|---|
| `celestina-qt/packaging/niri/per-layer-blur-strength.patch` (`c7fb0b5d`) | `celestina/packaging/niri/per-layer-blur-strength.patch` | none, byte for byte |
| `celestina-qt/packaging/niri/README.md` (`c7fb0b5d`) | `celestina/packaging/niri/README.md` | provenance header; the script path |
| `celestina-qt/scripts/build-patched-niri.sh` (`c7fb0b5d`) | `celestina/packaging/niri/build-patched-niri.sh` | provenance header; `patch_file` now `$here/per-layer-blur-strength.patch`; messages name the new paths; mode 0755 kept |
| `celestina-rs/crates/celestina-shell-core/src/niri_colours.rs` (`8e65ea16`) | `celestina-rs/crates/celestina-tokens/src/niri_colours.rs` | provenance header only; the module depends on nothing but `std` |

## Procedure

Run from the worktree root: the guards below and `cargo test -p celestina-tokens`
from `celestina-rs/`.

## Result

| Command | Exit | Output |
|---|---|---|
| `python3 scripts/check-sealed-colours.py` | 0 | Sealed colour contract: OK (4 colour(s)) |
| `bash scripts/check-architecture-contract.sh` | 0 | Architecture contract: OK |
| `python3 scripts/check-language-contract.py` | 0 | OK (131 legacy files ratcheted) |
| `bash scripts/check-documentation-contract.sh` | 0 | OK (only the existing SID-G7-D erratum notes) |
| `python3 scripts/version_tool.py check` | 0 | OK (12 owners) |
| `cargo test -p celestina-tokens` | 0 | 8 passed (five of them the copied `niri_colours` tests) |
| `cargo clippy -p celestina-tokens --all-targets -- -D warnings` | 0 | clean |

## Limits

- `build-patched-niri.sh` was checked with `sh -n` only: it clones niri and
  builds it, which a session does not run.
- `celestina-shell-core` still carries its own `niri_colours` and
  `appearance.rs` still uses it; it is halted and unedited.
- Nothing was stashed.

## Landing

- **Base revision:** `06da0899dc6afefef0d67daef6b3160ca59e3997`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: artifact is not verified yet; run verify-production.sh; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina verify: verify-production.sh exit 0, manifest source_fingerprint sha256:60ebdcb7d016b9fe4a07eb6539b394d9d113cb8cd06fd458d2cbfe39b714cc42, verification_fingerprint sha256:ca360e7217d088ad14554427bd162074f9e806fb400316d46a7a2ce41a59d1aa; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:08062b784e51507ff4d56c03a1d6b0aff42cd282905857137ec083e1e4493cbc, verification_fingerprint sha256:64e4b9d0f90fec44a8255f1aa55cad18b4a7b789b8298831cba0141816eb17e4
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
