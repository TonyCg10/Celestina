# Evidence: DEV-1-A — build cache and disk

- **Date:** 2026-11-24
- **Scope:** `scripts/build_trees.py`, `scripts/worktree.sh`,
  `scripts/land-unit.py` and their fixture tests; the `env:` block of
  `.github/workflows/celestina.yml` and `celestina-rs.yml`;
  `docs/contracts/landing.md` and `docs/contracts/production-artifacts.md`;
  the `DEV-1-A` row of `docs/plans/active/2026-11-24-dev-speed.md`; this
  record. No manifest, version or production input changed.
- **Environment:** CachyOS, worktree `suite-DEV-1-A`, branch
  `unit/suite/DEV-1-A`; `rustc 1.97.1 (8bab26f4f 2026-07-14)` (answered inside
  `celestina/`, so the pinned toolchain); `nproc` 8; no `RUSTC_WRAPPER` or
  `sccache` in the environment
- **Artifact:** not built; both measurement targets were deleted after
  measuring

## Procedure

Code and tests: the branch adds the trim, the `open` and `close` changes and
the auto-close, each with fixture tests (`scripts/test-build-trees.py`,
`scripts/test-worktree.sh`, `scripts/test-land-unit.py`), then the CI setting
and the contract text. The guards below ran on the final bytes.

Measurement, same commands for both columns, private target inside the
worktree, run from `celestina/` (`/usr/bin/time` and `bc` are not installed on
this machine, so the wall-clock seconds come from `date +%s.%N` around each
command, with Python doing the subtraction):

```sh
export CARGO_TARGET_DIR="$PWD/../target/baseline-before"   # baseline-after for After
cargo test --workspace --locked --no-run      # cold test build
touch src/lib.rs
cargo test --workspace --locked --no-run      # rebuild after touching src/lib.rs
du -sh "$CARGO_TARGET_DIR"
ls -l "$CARGO_TARGET_DIR"/debug/deps/celestina-* | sort -k5 -n | tail -1
readelf -p .comment <largest celestina-* test binary>
rm -rf "$CARGO_TARGET_DIR"
```

For After, the three profile lines `open` now writes (`[profile.dev]
debug = "line-tables-only"` and `[profile.dev.package."*"] debug = false`) were
appended to this worktree's own untracked `.cargo/config.toml`, which the old
`open` had written with the `[build]` table only. `cargo config get` needs
nightly, so the profile taking effect is shown by the size drop and by
`readelf -S` on the largest After binary: `.debug_info` is 0x6f959a bytes
(7.3 MB) in a 115 MB binary, line tables only, against a 392 MB binary before.

## Result

- **Exit:** 0 for every build of both columns.
- **Observed:** the linker line of the largest test binary reads
  `Linker: LLD 22.1.6` in both columns, so `rust-lld` is in use. `touch`
  changes only the mtime; `git status` showed no change in `celestina/`.
  Cargo's own `Finished` line of the After builds: 29.56 s cold, 3.23 s
  rebuild.

| Measure | Before | After |
|---|---|---|
| Cold test build (s) | 34.55 | 29.59 |
| Rebuild after touching `src/lib.rs` (s) | 4.23 | 3.27 |
| Target size | 8.6G | 3.6G |
| Largest test binary | 392573808 bytes, `debug/deps/celestina-73b39a2f9975faf3` | 115276560 bytes, `debug/deps/celestina-f77164b6af518c00` |
| Linker | LLD 22.1.6 | LLD 22.1.6 |

Guards run from the worktree root on the final bytes:

| Command | Exit |
|---|---|
| `bash scripts/check-documentation-contract.sh` | 0 |
| `python3 scripts/check-language-contract.py` | 0 |
| `bash scripts/check-architecture-contract.sh` | 0 |
| `python3 scripts/test-build-trees.py` | 0 |
| `bash scripts/test-worktree.sh` | 0 |
| `bash scripts/test-land-unit.sh` | 0 |

The final review (one round, then a check of its fixes) found two defects,
both fixed on the branch with a test each: the auto-close deleted a session
branch that had gained a commit during its own landing, and the trim removed
the `.cargo-lock` it held when it removed a profile whole. Three smaller
findings were fixed afterwards: `tree_size` counted a hard-linked file once
per link, `close_session` carried a guard that could not fire, and `close`
had no fixture case for a shared target over its cap or for a failing trim.

## Limits

- One run each, on a machine that may have other load; the figures are a
  baseline, not a benchmark. The After cold build ran twice: the first run's
  time was lost because `bc` is missing, and the figures above come from a
  second cold run on a new target. The first run produced the same 3.6G target
  and the same binary size.
- The cold build took 34.55 s before although the target was new, and 29.59 s
  after; nothing in the environment names a compiler cache, so the cause (a
  warm registry and fast machine, or a cache outside the environment) is not
  established. The time difference of about 5 s is within what one run each
  cannot separate from load; the size drop is not.
- `/usr/bin/time` is absent, so peak memory was not measured.
- The time of a whole landing was not measured here. The coordinating session
  takes it at this unit's landing.
- The trim and the auto-close were exercised by the fixture tests only, not
  against a real 40 GiB target or a real landing.
- `close` has no fixture case for a symbolic link named `target`: the
  fixture's and the repository's `**/target/` ignores directories only, so
  the status check refuses such a link before the removal is reached.
- Resuming a landing with `--continue` at `prune` was read in the code, not
  run in a test.
- Session worktrees opened before this unit keep the old profile and share
  `.cargo-target` with the new ones, so dependencies are compiled once per
  profile until those worktrees close.
- CI mirrors the workspace members' setting only: `debug = false` for
  dependencies has no single environment variable, so the workflows do not set
  it. No workflow run was observed.

## Follow-up

None.

## Landing

- **Base revision:** `12c228d8d9d6a1c61eb864037c49b764fcabfd83`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
