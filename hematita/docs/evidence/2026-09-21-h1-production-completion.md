# Evidence: 2026-09-21 H1 delivered to the author's prefix

- **Date:** 2026-09-21
- **Scope:** checkpoint `H1`, units `H1-A` through `H1-Z`; plan
  [h1-foundation](../plans/archive/2026-09-21-h1-foundation.md)
- **Environment:** Arch Linux, `rustc 1.98.1`, `c++ (GCC) 16.2.1`,
  `cmake 4.4.3`; Qt reports `unavailable` in the toolchain probe, as it has on
  every earlier release
- **Artifact:** `hematita/target/release/hematita`,
  sha256 0d731859f62d8632938569b02072972049325e45a02a6a47d8c103944bf0a327.
  The manifest records `git_revision = 623bde3f96d2baebac4d0e78d3fc451c45a43905`
  and `verified = true`
- **Sealed manifest:** `hematita/target/production-artifact.toml`

## What this closes

`H1-A` through `H1-D` were written and their focused evidence recorded across
2026-09-21, but the checkpoint's own implementation exit had not yet run at
`0.2.0`. This record runs that exit: the version bump, the canonical build,
its verification, and the deployment to the author's prefix. It adds no
source change of its own beyond the version bump.

## Procedure

```sh
python3 scripts/version_tool.py bump hematita milestone --unit H1-Z \
  --summary "Add the foundation and the Performance page with CPU and memory"
python3 scripts/version_tool.py check
hematita/scripts/complete-production.sh
hematita/scripts/status-production.sh
```

## Result

- **Exit:** 0 for every command.
- **Observed:**

| Command | Result |
|---|---|
| `python3 scripts/version_tool.py bump …` | `hematita: 0.1.0 -> 0.2.0 (milestone)`, one appended history row |
| `python3 scripts/version_tool.py check` | `version-contract: OK (8 owners)` |
| `hematita/scripts/build-production.sh` | release build finished |
| `hematita/scripts/verify-production.sh` | 29 crate-adjacent contract tests OK; `cargo test -p hematita-core` 17 passed; `tests/captures.rs` 3 passed; `hematita` unit test 1 passed; `qmllint-production` OK (0 non-fatal baseline warnings); smoke OK, binary alive 8 s, no QML errors, no auto-bindings |
| `hematita/scripts/deploy-production.sh` | `artifact: hematita current`; deployed to `/home/toni/.local` without rebuilding |
| `hematita/scripts/status-production.sh` | `artifact: hematita current and verified`; `installed: OK /home/toni/.local/bin/hematita` |

## Installed state

- `~/.local/bin/hematita` — sha256 matches
  `hematita/target/release/hematita` byte for byte (`sha256sum` on both
  paths).
- `~/.local/share/applications/org.celestina.Hematita.desktop` exists.
- `~/.local/share/icons/hicolor/*/apps/org.celestina.Hematita.{png,svg}`
  exist at every listed size plus the scalable SVG.

The installed binary was not launched interactively as part of this record:
per the controller's build-budget decision, the installed bytes were checked
by hash equality against the freshly verified release binary and by the
presence of the desktop entry and icon files, rather than by opening a window
on the live or nested session.

## Limits

- `worktree_dirty = true` in the manifest, unchanged from the H1-D
  verification: the dirt is documentation and version-bump only — this
  record, the archived plan, `Cargo.toml`/`Cargo.lock`, the roadmap/status
  updates and `docs/version-history.tsv` in the same administrative unit. No
  source or QML file differs from `623bde3`.
- This proves delivery, not perception. Whether the Performance page reads
  well on the author's own compositor, and whether the graphs read at a
  glance during real use, is `VAL-H1`, which stays pending and did not block
  this closure.
- No Qt version is recorded: the toolchain probe reports `qt = "unavailable"`,
  as it has for every earlier release in this suite.

## Follow-up

`VAL-H1` remains pending in [VALIDATION.md](../../VALIDATION.md). The next
checkpoint is `H2` (disks, network, GPU and swap; the per-core grid), not yet
opened.
