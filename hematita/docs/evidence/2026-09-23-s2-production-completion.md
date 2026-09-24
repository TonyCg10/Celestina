# Evidence: 2026-09-23 S2 delivered to the author's prefix

- **Date:** 2026-09-23
- **Scope:** checkpoint `S2`, units `S2-A` through `S2-Z`; plan
  [s2-hardening](../plans/archive/2026-09-23-s2-hardening.md)
- **Environment:** Arch Linux, `rustc 1.98.1`, `c++ (GCC) 16.2.1`,
  `cmake 4.4.3`; Qt reports `unavailable` in the toolchain probe, as it has on
  every earlier release
- **Artifact:** `hematita/target/release/hematita`,
  sha256 26979451364a7c35b13201a6bf1c95e2498d26c947603f8509911dab4eccbe70.
  The manifest records `git_revision = 72aad454732b46e3355a2c0416870676ea40574e`
  and `verified = true`
- **Sealed manifest:** `hematita/target/production-artifact.toml`

## What this closes

`S2-A`, `S2-A2`, `S2-B` and `S2-B2` were written and their focused evidence
recorded on 2026-09-23; `S2-B` and `S2-B2` were built and verified but not
deployed, so the author's prefix still held `1.1.0`. This record runs the
checkpoint's implementation exit: the version bump, the canonical build, its
verification, and the deployment to the author's prefix. It adds no source
change of its own beyond the version bump and its documents.

## Procedure

```sh
python3 scripts/version_tool.py bump hematita bug --unit S2-Z \
  --summary "Fix the deletion window, the stale sizes after a stopped deletion and the analysis hub size"
python3 scripts/version_tool.py check
hematita/scripts/complete-production.sh
```

`complete-production.sh` ends by running `hematita/scripts/status-production.sh`.

## Result

- **Exit:** 0 for every command.
- **Observed:**

| Command | Result |
|---|---|
| `python3 scripts/version_tool.py bump …` | `hematita: 1.1.0 -> 1.1.1 (bug)`, one appended history row |
| `python3 scripts/version_tool.py check` | `version-contract: OK (8 owners)` |
| `hematita/scripts/complete-production.sh` | release build finished; architecture, colour, contrast and QML visual contracts OK; `cargo test -p hematita` 57 passed; `cargo test -p hematita-core` 88 passed; `tests/captures.rs` 11 passed; `tests/usage_tree.rs` 28 passed; `qmllint-production` OK (0 non-fatal baseline warnings); smoke OK, binary alive 10 s, every section shown, the first row published the CPU contract, the Sensors page published chips, the Services page listed system units, the Storage page listed locations, no QML errors, no auto-bindings; manifest verified; deployed to `/home/toni/.local` without rebuilding |
| `hematita/scripts/status-production.sh` | `artifact: hematita current and verified`; `installed: OK /home/toni/.local/bin/hematita` |

## Installed state

- `~/.local/bin/hematita` — sha256 matches
  `hematita/target/release/hematita` byte for byte (plain `sha256sum` on
  both paths, both equal to
  `26979451364a7c35b13201a6bf1c95e2498d26c947603f8509911dab4eccbe70`).

The installed binary was not launched interactively as part of this record:
the installed bytes were checked by `status-production.sh` and by hash
equality against the freshly verified release binary, rather than by opening
a window on the live or nested session.

## Limits

- `worktree_dirty` is true in the manifest at seal time: the dirt is
  documentation and version-bump only — this record, the archived plan and
  its READMEs, the plan links in the four `S2` evidence records,
  `Cargo.toml`/`Cargo.lock`, the roadmap/status updates, `AGENTS.md` and
  `docs/version-history.tsv` in the same unit. No source or QML file differs
  from what `S2-B2` delivered.
- This proves delivery, not perception. Whether a cancelled deletion names
  what it removed, whether the graft shows the true sizes and whether the
  dialog refuses a changed selection on the real session is `VAL-S2`, which
  stays pending and did not block this closure; `VAL-S1`, `VAL-H2` and
  `VAL-H5` also stay pending.
- The manifest's recorded artifact digest is a keyed digest, not a plain
  file hash (see the S1 completion record); the installed-bytes check rests
  on `status-production.sh` reporting `OK` and on the plain `sha256sum`
  equality above.
- No scan, no folder action and no permanent deletion were run against this
  session's real files during verification: the smoke's Storage assertion is
  headless and touches only the locations list.
- Residuals: a rename of the analysed root while a deletion runs; a mount
  created between the read-only pass and the removal pass; the `Weak<Tree>`
  mid-comparison copy; a prune or graft bumps the selection revision, so a
  graft landing under an open dialog refuses harmlessly; grafted duplicates
  need a rescan.

## Follow-up

`VAL-S2` remains pending in [VALIDATION.md](../../VALIDATION.md), alongside
`VAL-S1`, `VAL-H2` and `VAL-H5`. Further work opens with a new checkpoint and
the author's word.
