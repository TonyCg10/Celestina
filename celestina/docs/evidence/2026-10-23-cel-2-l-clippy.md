# Evidence: CEL-2-L-CLIPPY — the capture preview's chunking under Clippy 1.99

- **Date:** 2026-10-23
- **Scope:** one line of `celestina/src/capture/mod.rs` (`mean_luma_span`)
  and this unit's ledger row. No behaviour, dependency or version changed.
- **Environment:** CachyOS, worktree `celestina-CEL-2-L-CLIPPY` cut from
  `main` at `06da0899` (celestina 1.11.0); toolchains 1.97.1 (the project's
  `rust-toolchain.toml`) and 1.99 (the GitHub runner's stable). No shell was
  started; nothing touched the live session.
- **Artifact:** none sealed by this record; the landing builds its own.

## What was wrong

The `celestina` workflow went red on `main` after CEL-2-L landed
(`06da0899`): Clippy 1.99 denies `chunks_exact` with a constant chunk size
(`using chunks_exact with a constant chunk size`, help: `as_chunks::<4>().0`)
at `celestina/src/capture/mod.rs:186`, in the loop that averages the
luminance of a span of a preview's row. The project's pinned 1.97.1 does not
carry that lint, which is why the unit's own gates were green. `df534373`
had already converted `main`'s other constant chunkings to `as_chunks`; this
one arrived with CEL-2-L afterwards.

## Procedure

```sh
rg -n 'chunks_exact' celestina/src celestina/crates celestina-rs/crates/celestina-shell/src
cargo +1.99 clippy --all-targets -- -D warnings     # in celestina/
cargo clippy --all-targets -- -D warnings           # 1.97.1, the pinned one
cargo fmt --check
cargo test --lib capture
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

## Result

| Command | Exit | Output |
|---|---|---|
| `rg chunks_exact` | 0 | five sites; only `capture/mod.rs:186` has a constant size and came with CEL-2-L (`:182` chunks by the variable `row_len`; `painter.rs`, `nightlight.rs`, `notifications.rs` are other units' and variable-sized or already accepted by `df534373`'s run) |
| `cargo +1.99 clippy --all-targets -- -D warnings` | 1 before, 0 after | before: the one error at `:186`; after: clean |
| `cargo clippy --all-targets -- -D warnings` (1.97.1) | 0 | clean |
| `cargo fmt --check` | 0 | clean |
| `cargo test --lib capture` | 0 | 32 passed, 0 failed |
| architecture, language, documentation contracts | 0 | OK (the documentation guard also prints the known `SID-G7-D` erratum lines) |

The change: `for pixel in span.chunks_exact(4)` → `for pixel in
span.as_chunks::<4>().0`. `as_chunks::<4>()` returns the full `[u8; 4]`
chunks and the remainder; the loop takes the chunks, so the same pixels are
summed and the same partial tail (never present: `span` is a whole number of
pixels) is ignored. The `mean_luma`/`half_lumas` tests that read the result
are unchanged and pass.

## Limits

- The runner itself was not run from this worktree; the local 1.99 toolchain
  is the same stable the workflow installs, and the lint is the only error
  the red run reports for this crate.
- `cargo test --workspace` was not repeated for one line that changes no
  behaviour; the landing's `verify-production.sh` runs it.
- Nothing was stashed.

## Landing

- **Base revision:** `4b384fc6dc3c111b155b91882cb3c95d1759e42c`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:d24d7b6682243915df3c63eed23f968670772a7889a51bac585f4f54619d37d3, verification_fingerprint sha256:ca360e7217d088ad14554427bd162074f9e806fb400316d46a7a2ce41a59d1aa
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
