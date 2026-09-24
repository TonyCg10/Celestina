# Evidence: 2026-09-23 S1 delivered to the author's prefix

- **Date:** 2026-09-23
- **Scope:** checkpoint `S1`, units `S1-A` through `S1-Z`; plan
  [s1-storage](../plans/archive/2026-09-23-s1-storage.md)
- **Environment:** Arch Linux, `rustc 1.98.1`, `c++ (GCC) 16.2.1`,
  `cmake 4.4.3`; Qt reports `unavailable` in the toolchain probe, as it has on
  every earlier release
- **Artifact:** `hematita/target/release/hematita`,
  sha256 cdbc599075d9335935b4fe9d3c24e8a12e495e5b1a71f5132c58042f72abcf6b.
  The manifest records `git_revision = fe56832ef01388c1ff611e22209edc342e6cbbc1`
  and `verified = true`
- **Sealed manifest:** `hematita/target/production-artifact.toml`

## What this closes

`S1-A` through `S1-E` were written and their focused evidence recorded across
2026-09-23, but the checkpoint's own implementation exit had not yet run at
`1.0.0`. This record runs that exit: the version bump, the canonical build,
its verification, and the deployment to the author's prefix. It adds no
source change of its own beyond the version bump and its documents.

## Procedure

```sh
python3 scripts/version_tool.py bump hematita milestone --unit S1-Z \
  --summary "Add the storage analyzer"
python3 scripts/version_tool.py check
hematita/scripts/complete-production.sh
hematita/scripts/status-production.sh
```

## Result

- **Exit:** 0 for every command.
- **Observed:**

| Command | Result |
|---|---|
| `python3 scripts/version_tool.py bump …` | `hematita: 1.0.0 -> 1.1.0 (milestone)`, one appended history row |
| `python3 scripts/version_tool.py check` | `version-contract: OK (8 owners)` |
| `hematita/scripts/complete-production.sh` | release build finished; `cargo test -p hematita-core` 87 passed; `tests/captures.rs` 11 passed; `tests/usage_tree.rs` 19 passed; `qmllint-production` OK (0 non-fatal baseline warnings); smoke OK, binary alive 10 s, every section shown, the first row published the CPU contract, the Sensors page published chips, the Services page listed system units, the Storage page listed locations, no QML errors, no auto-bindings; manifest verified; deployed to `/home/toni/.local` without rebuilding |
| `hematita/scripts/status-production.sh` | `artifact: hematita current and verified`; `installed: OK /home/toni/.local/bin/hematita` |

## Installed state

- `~/.local/bin/hematita` — sha256 matches
  `hematita/target/release/hematita` byte for byte (plain `sha256sum` on
  both paths, both equal to
  `cdbc599075d9335935b4fe9d3c24e8a12e495e5b1a71f5132c58042f72abcf6b`).
- `~/.local/share/applications/org.celestina.Hematita.desktop` exists.
- `~/.local/share/icons/hicolor/*/apps/org.celestina.Hematita.{png,svg}`
  exist at every listed size plus the scalable SVG.

The installed binary was not launched interactively as part of this record:
per the controller's build-budget decision, the installed bytes were checked
by hash equality against the freshly verified release binary and by the
presence of the desktop entry and icon files, rather than by opening a
window on the live or nested session.

## Limits

- `worktree_dirty` is true in the manifest at seal time: the dirt is
  documentation and version-bump only — this record, the archived plan,
  `Cargo.toml`/`Cargo.lock`, the roadmap/status updates, `README.md`,
  `AGENTS.md` and `docs/version-history.tsv` in the same milestone unit. No
  source or QML file differs from what `S1-E` delivered.
- This proves delivery, not perception. Whether the Storage page reads well
  on the author's own compositor, whether a scan, a trash and a permanent
  deletion behave as intended on the real session, is `VAL-S1`, which stays
  pending and did not block this closure; `VAL-H2` and `VAL-H5` also stay
  pending from earlier checkpoints.
- No Qt version is recorded: the toolchain probe reports `qt = "unavailable"`,
  as it has for every earlier release in this suite.
- The manifest's recorded artifact digest is a keyed digest, not a plain
  file hash: `artifact_digest` in `scripts/production_artifact.py` feeds the
  artifact's logical path into the digest through `feed_path`, alongside the
  bytes, by design, so it never equals a plain `sha256sum` of the same file.
  The installed-bytes check above therefore rests on `status-production.sh`
  reporting the same keyed digest `OK` for the installed copy, and on the
  plain `sha256sum` equality of the release and installed copies recorded in
  "Installed state".
- No scan, no folder action and no permanent deletion were run against this
  session's real files during verification: the smoke's Storage assertion is
  headless and touches only the locations list.
- `delete_tree` lists a subtree, re-validates its root by device and inode,
  and then removes by path; a folder swapped for a symbolic link between the
  listing and its removal would be followed by the kernel. Acceptable on a
  single-user desktop and recorded here; `S2-A` names the `openat`-based fix.
- A `delete_tree` that fails or is cancelled midway does not report what it
  removed, so the tree keeps the old size until a rescan.
- Memory: about 200 MB resident for a scan of one million entries,
  unmeasured on this machine.

## Follow-up

`VAL-S1` remains pending in [VALIDATION.md](../../VALIDATION.md), alongside
the earlier `VAL-H2` and `VAL-H5`. `S1` is the storage checkpoint delivered on
top of version 1 (`1.0.0`); further work opens with a new checkpoint and the
author's word.
