# Evidence: 2026-09-25 S3 delivered to the author's prefix

- **Date:** 2026-09-25
- **Scope:** checkpoint `S3`, units `S3-A` and `S3-Z`; plan
  [s3-shared-usage](../plans/archive/2026-09-25-s3-shared-usage.md)
- **Environment:** Arch Linux, `rustc 1.98.1`, `c++ (GCC) 16.2.1`,
  `cmake 4.4.3`
- **Artifact:** `hematita/target/release/hematita`,
  sha256 95bde32f83fb1975bd45911807b9003e7b8d00a93848af786fb77d6980251c6f.
  The manifest records `git_revision = 869ba6242be5e262403ed90f6b113daa56923f7a`
  and `verified = true`
- **Sealed manifest:** `hematita/target/production-artifact.toml`

## What this closes

`S3-A` was written and its focused evidence recorded on 2026-09-25; it was
built and verified but not deployed, so the author's prefix still held
`1.1.1`. This record runs the checkpoint's implementation exit: the version
bump, the canonical build, its verification, and the deployment to the
author's prefix. It adds no source change of its own beyond the version
bump and its documents.

## Procedure

```sh
python3 scripts/version_tool.py bump hematita milestone --unit S3-Z \
  --summary "Add the shared usage view and the folder argument"
python3 scripts/version_tool.py check
bash hematita/scripts/complete-production.sh
```

`complete-production.sh` ends by running `hematita/scripts/status-production.sh`.

## Result

- **Exit:** 0 for every command.
- **Observed:**

| Command | Result |
|---|---|
| `python3 scripts/version_tool.py bump …` | `hematita: 1.1.1 -> 1.2.0 (milestone)`, one appended history row |
| `python3 scripts/version_tool.py check` | `version-contract: OK (8 owners)` |
| `bash hematita/scripts/complete-production.sh` | release build finished; `cargo test -p hematita` 55 passed; `cargo test -p hematita-core` 94 passed; `tests/captures.rs` 11 passed; `tests/usage_tree.rs` 28 passed; `qmllint-production` OK (0 non-fatal baseline warnings); smoke OK, binary alive 10 s, every section shown, the first row published the CPU contract, the Sensors page published chips, the Services page listed system units, the Storage page listed locations, a folder argument landed in the storage section, no QML errors, no auto-bindings; manifest verified; deployed to `/home/toni/.local` without rebuilding |
| `hematita/scripts/status-production.sh` | `artifact: hematita current and verified`; `installed: OK /home/toni/.local/bin/hematita` |

## Installed state

- `~/.local/bin/hematita` — sha256 matches
  `hematita/target/release/hematita` byte for byte (plain `sha256sum` on
  both paths, both equal to
  `95bde32f83fb1975bd45911807b9003e7b8d00a93848af786fb77d6980251c6f`).

The installed binary was not launched as part of this record: the installed
bytes were checked by `status-production.sh` and by hash equality against
the freshly verified release binary.

## Limits

- `worktree_dirty` is true in the manifest at seal time: the dirt is the
  version bump only (`Cargo.toml`, `Cargo.lock`,
  `docs/version-history.tsv`); the documents of this unit were written
  after the seal. No source or QML file differs from what `S3-A` delivered.
- This proves delivery, not perception. Whether the shared treemap and list
  read and respond on the real session, whether Space and Delete act on the
  page, and whether a D-Bus `Open` reaches a running instance is `VAL-S3`,
  which stays pending and did not block this closure.
- The smoke's folder-argument assertion is headless and only checks that the
  storage section is shown; no scan, folder action or deletion was run
  against this session's real files.
- Residuals: the desktop entry declares no `MimeType=inode/directory`; an
  `Open` that arrives during a scan drops the analysis in progress.

## Follow-up

`VAL-S3` remains pending in [VALIDATION.md](../../VALIDATION.md). Further
work opens with a new checkpoint and the author's word.
