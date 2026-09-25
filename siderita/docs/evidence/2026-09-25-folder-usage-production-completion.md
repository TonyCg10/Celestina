# Evidence: 2026-09-25 SID-U1 delivered to the author's prefix

- **Date:** 2026-09-25
- **Scope:** checkpoint `SID-U1`, units `SID-U1-A` and `SID-U1-Z`; plan
  [folder-usage](../plans/archive/2026-09-25-folder-usage.md)
- **Environment:** Arch Linux, `rustc 1.98.1`, `c++ (GCC) 16.2.1`,
  `cmake 4.4.3`
- **Artifact:** `siderita/target/release/siderita`,
  sha256 ff9032abfd3c457f4b11b01162d13d167acf58d2f76ab62ecd20fba1493309a7.
  The manifest records `git_revision = a1eb9fb0e3ef31af639bf7cf34efc773db7c762b`
  and `verified = true`
- **Sealed manifest:** `siderita/target/production-artifact.toml`

## What this closes

`SID-U1-A` was written and its focused evidence recorded on 2026-09-25; it
was built and verified but not deployed, so the author's prefix still held
`1.6.1`. This record runs the checkpoint's implementation exit: the version
bump, the canonical build, its verification, and the deployment to the
author's prefix. It adds no source change of its own beyond the version bump
and its documents.

## Procedure

```sh
python3 scripts/version_tool.py bump siderita milestone --unit SID-U1-Z \
  --summary "Add the folder occupation to the properties dialog and the quick look"
bash siderita/scripts/complete-production.sh
python3 scripts/version_tool.py check
```

`complete-production.sh` ends by running `siderita/scripts/status-production.sh`.

## Result

- **Exit:** 0 for every command.
- **Observed:**

| Command | Result |
|---|---|
| `python3 scripts/version_tool.py bump …` | `siderita: 1.6.1 -> 1.7.0 (milestone)`, one appended history row |
| `bash siderita/scripts/complete-production.sh` | release build finished; every `cargo test` suite of the workspace passed with 0 failures (largest suites 120, 95, 39, 39, 31, 28 and 23 tests); `qmllint-production` OK (258 non-fatal baseline warnings); QML tests `Totals: 162 passed, 0 failed`; smoke OK, binary alive 8 s, no QML errors, no auto-bindings; manifest verified; deployed to `/home/toni/.local` without rebuilding |
| `siderita/scripts/status-production.sh` | `artifact: siderita current and verified`; `installed: OK /home/toni/.local/bin/siderita` |

## Installed state

- `~/.local/bin/siderita` — sha256 matches
  `siderita/target/release/siderita` byte for byte (plain `sha256sum` on
  both paths, both equal to
  `ff9032abfd3c457f4b11b01162d13d167acf58d2f76ab62ecd20fba1493309a7`).

The installed binary was not launched as part of this record: the installed
bytes were checked by `status-production.sh` and by hash equality against
the freshly verified release binary.

## Limits

- `worktree_dirty` is true in the manifest at seal time: the dirt is the
  version bump only (`Cargo.toml`, `Cargo.lock`,
  `docs/version-history.tsv`); the documents of this unit were written
  during and after the build. No source or QML file differs from what
  `SID-U1-A` delivered.
- This proves delivery, not perception. Whether the section reads and
  responds on the real session, whether closing mid-scan leaves no thread
  reading, and whether «Abrir en Hematita» reaches Hematita is `VAL-SID-U1`,
  which stays pending and did not block this closure.
- The smoke is headless and does not open either modal on a folder; no scan
  was run against this session's real files.
- Residuals: `bytesText` in `FolderUsage.qml` is a third copy of the byte
  formatter, pending a shared one; `gtk-launch` fails asynchronously, so a
  missing Hematita is not detected; after a properties takeover the quick
  look shows the glyph until its entry changes; Space on a focused crumb or
  footer button presses it rather than closing the quick look, by design.

## Follow-up

`VAL-SID-U1` remains pending in [VALIDATION.md](../../VALIDATION.md). Further
work opens with a new checkpoint and the author's word.
