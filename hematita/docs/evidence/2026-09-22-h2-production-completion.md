# Evidence: 2026-09-22 H2 delivered to the author's prefix

- **Date:** 2026-09-22
- **Scope:** checkpoint `H2`, units `H2-A` through `H2-Z`; plan
  [h2-resources](../plans/archive/2026-09-22-h2-resources.md)
- **Environment:** Arch Linux, `rustc 1.98.1`, `c++ (GCC) 16.2.1`,
  `cmake 4.4.3`; Qt reports `unavailable` in the toolchain probe, as it has on
  every earlier release
- **Artifact:** `hematita/target/release/hematita`,
  sha256 85f9f39f54a369e2701c0da74376ce9b5e6d6e1e2e44967bdcb5de573c6ce72c.
  The manifest records `git_revision = 66ec11a4ef756d02a29e7af7f2f8939f7cb2f75c`
  and `verified = true`
- **Sealed manifest:** `hematita/target/production-artifact.toml`

## What this closes

`H2-A` through `H2-C` were written and their focused evidence recorded across
2026-09-22, but the checkpoint's own implementation exit had not yet run at
`0.3.0`. This record runs that exit: the version bump, the canonical build,
its verification, and the deployment to the author's prefix. It adds no
source change of its own beyond the version bump.

## Procedure

```sh
python3 scripts/version_tool.py bump hematita milestone --unit H2-Z \
  --summary "Add every resource to the Performance page with the per-core grid"
python3 scripts/version_tool.py check
hematita/scripts/complete-production.sh
hematita/scripts/status-production.sh
```

## Result

- **Exit:** 0 for every command.
- **Observed:**

| Command | Result |
|---|---|
| `python3 scripts/version_tool.py bump …` | `hematita: 0.2.0 -> 0.3.0 (milestone)`, one appended history row |
| `python3 scripts/version_tool.py check` | `version-contract: OK (8 owners)` |
| `hematita/scripts/build-production.sh` | release build finished (relink) |
| `hematita/scripts/verify-production.sh` | 29 production-common fixtures OK; sealed colour, contrast, QML visual and architecture contracts OK; `cargo test -p hematita-core` 37 passed; `tests/captures.rs` 5 passed; `hematita` unit tests 4 passed; `qmllint-production` OK (0 non-fatal baseline warnings); smoke OK, binary alive 8 s, no QML errors, no auto-bindings |
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

- `worktree_dirty = true` in the manifest, unchanged from the H2-C
  verification: the dirt is documentation and version-bump only — this
  record, the archived plan, `Cargo.toml`/`Cargo.lock`, the roadmap/status
  updates and `docs/version-history.tsv` in the same administrative unit. No
  source or QML file differs from `66ec11a`.
- This proves delivery, not perception. Whether the full resource list reads
  well on the author's own compositor, and whether the per-core grid toggle
  reads at a glance during real use, is `VAL-H2`, which stays pending and did
  not block this closure.
- No Qt version is recorded: the toolchain probe reports `qt = "unavailable"`,
  as it has for every earlier release in this suite.

## Follow-up

`VAL-H2` remains pending in [VALIDATION.md](../../VALIDATION.md). The next
checkpoint is `H3` (processes and applications), not yet opened.
