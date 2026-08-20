# Evidence: 2026-08-19 G7 delivered to both installed hosts

- **Date:** 2026-08-19
- **Scope:** checkpoint `G7`, units `G7-A` through `G7-D`; plan
  [g7-reading-comfort](../plans/archive/2026-08-04-g7-reading-comfort.md)
- **Environment:** Arch Linux, `rustc 1.97.1`, `c++ (GCC) 16.2.1`,
  `cmake 4.4.2`. Siderita was running as the author's session (PID 1612) and
  was not stopped; Celestina's shell was not built, verified or deployed
- **Artifact:** `grafita/target/release/grafita`,
  `sha256:5359cb9f13201f23a69bf78bfe1266398fad46a5ef084734a40ae0b27cb7ec6d`;
  `siderita/target/release/siderita`,
  `sha256:ce37ecb9f1fb9b1e392d1b08d1697ebc119e6afcde5b0cc7e4ce9a8f77896b1a`.
  Both manifests record `git_revision = e9ac330` and `verified = true`

## What this closes

`G7-A` through `G7-D` were written and their focused tests recorded across
2026-08-04 to 2026-08-06, but the checkpoint's own implementation exit was
never run: no canonical release existed and the author's installed binaries
still carried pre-G7 bytes. This record runs that exit. It adds no source
change of its own.

Because G7 changed `grafita-core`, both installed consumers were completed, as
the plan's exit and the local contract require.

## Procedure

```sh
cargo fmt --all --check                                          # in celestina-rs/
cargo test -p grafita-core
cargo clippy -p grafita-core --all-targets --locked -- -D warnings
cargo fmt --all --check                                          # in grafita/
cargo clippy --all-targets --locked -- -D warnings
cargo test --manifest-path grafita/Cargo.toml
bash scripts/qmllint-cxxqt.sh grafita
bash scripts/check-architecture-contract.sh
bash grafita/scripts/complete-production.sh
bash siderita/scripts/complete-production.sh
```

## Result

- **Exit:** 0 for every command.
- **Observed:**

| Command | Result |
|---|---|
| `cargo test -p grafita-core` | 28 passed, 0 failed |
| `cargo test --manifest-path grafita/Cargo.toml` | 7 passed, 0 failed |
| `cargo fmt --all --check` (crate and app) | clean |
| `cargo clippy … -D warnings` (crate and app) | no Rust diagnostic; the only output is the pre-existing `-Wmaybe-uninitialized` note from cxx-generated `syntax.cxx.cpp` |
| `scripts/qmllint-cxxqt.sh grafita` | OK, 62 non-fatal baseline warnings |
| `scripts/check-architecture-contract.sh` | sealed colour, contrast, QML visual and architecture contracts all OK |
| `grafita/scripts/complete-production.sh` | built once, `manifest: grafita (verified)`, smoke OK with the binary alive 8 s and no QML error, deployed to `/home/toni/.local/bin/grafita`, status `current and verified` |
| `siderita/scripts/complete-production.sh` | built once, verified, QML test runner 72 passed / 0 failed, smoke OK, deployed to `/home/toni/.local/bin/siderita`, status `current and verified` |

## Limits

- `worktree_dirty = true` in both manifests. The dirt is documentation only —
  this record, the archived plan and the roadmap/status updates in the same
  administrative unit. No source or QML file differed from `e9ac330` when the
  release was built.
- The Siderita process that was running during the deployment keeps executing
  its previous bytes until the author restarts it. Replacing the file does not
  change a live process, and nothing here stopped or restarted the author's
  session.
- This proves delivery, not perception. Whether the gutter tracks a wrapped
  line on the author's own compositor, whether the shortcuts arrive from the
  physical layout, and whether `Alt + Z` survives a compositor that claims it
  are `VAL-G7`, which stays pending and never blocked this checkpoint.
- Neither manifest records a Qt version: the toolchain probe reports
  `qt = "unavailable"`, as it did for every earlier release.

## Follow-up

`VAL-G7` and `VAL-GRA-SAVEAS` remain pending in
[VALIDATION.md](../../VALIDATION.md). The next checkpoint is `G8`, opened by
the author on 2026-08-19; its plan is
[g8-text-already-refused](../plans/archive/2026-08-19-g8-text-already-refused.md).
