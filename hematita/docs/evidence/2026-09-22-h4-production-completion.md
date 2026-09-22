# Evidence: 2026-09-22 H4 delivered to the author's prefix

- **Date:** 2026-09-22
- **Scope:** checkpoint `H4`, units `H4-A` through `H4-Z`; plan
  [h4-sensors](../plans/archive/2026-09-22-h4-sensors.md)
- **Environment:** Arch Linux, `rustc 1.98.1`, `c++ (GCC) 16.2.1`,
  `cmake 4.4.3`; Qt reports `unavailable` in the toolchain probe, as it has on
  every earlier release
- **Artifact:** `hematita/target/release/hematita`,
  sha256 75f5700df9bc5f13d7451026cae3ad6da1462235e87a0e4cd2c84a5aa72eb56f.
  The manifest records `git_revision = 92c452bd6d8e6bb969df960583a8d20fee750077`
  and `verified = true`
- **Sealed manifest:** `hematita/target/production-artifact.toml`

## What this closes

`H4-A` through `H4-C` were written and their focused evidence recorded
across 2026-09-22, but the checkpoint's own implementation exit had not yet
run at `0.5.0`. This record runs that exit: the version bump, the canonical
build, its verification, and the deployment to the author's prefix. It adds
no source change of its own beyond the version bump.

## Procedure

```sh
python3 scripts/version_tool.py bump hematita milestone --unit H4-Z \
  --summary "Add the Sensors page"
python3 scripts/version_tool.py check
hematita/scripts/complete-production.sh
hematita/scripts/status-production.sh
```

## Result

- **Exit:** 0 for every command.
- **Observed:**

| Command | Result |
|---|---|
| `python3 scripts/version_tool.py bump …` | `hematita: 0.4.1 -> 0.5.0 (milestone)`, one appended history row |
| `python3 scripts/version_tool.py check` | `version-contract: OK (8 owners)` |
| `hematita/scripts/complete-production.sh` | release build finished; `cargo test -p hematita-core` 59 passed; `tests/captures.rs` 11 passed; `qmllint-production` OK (0 non-fatal baseline warnings); smoke OK, binary alive 10 s, every section shown, the first row published the CPU contract, no QML errors, no auto-bindings; manifest verified; deployed to `/home/toni/.local` without rebuilding |
| `hematita/scripts/status-production.sh` | `artifact: hematita current and verified`; `installed: OK /home/toni/.local/bin/hematita` |

## Installed state

- `~/.local/bin/hematita` — sha256 matches
  `hematita/target/release/hematita` byte for byte (plain `sha256sum` on
  both paths, both equal to
  `75f5700df9bc5f13d7451026cae3ad6da1462235e87a0e4cd2c84a5aa72eb56f`).
- `~/.local/share/applications/org.celestina.Hematita.desktop` exists.
- `~/.local/share/icons/hicolor/*/apps/org.celestina.Hematita.{png,svg}`
  exist at every listed size plus the scalable SVG.

The installed binary was not launched interactively as part of this record:
per the controller's build-budget decision, the installed bytes were checked
by hash equality against the freshly verified release binary and by the
presence of the desktop entry and icon files, rather than by opening a
window on the live or nested session.

## Limits

- `worktree_dirty = true` in the manifest, unchanged from the H4-C
  verification: the dirt is documentation and version-bump only — this
  record, the archived plan, `Cargo.toml`/`Cargo.lock`, the roadmap/status
  updates and `docs/version-history.tsv` in the same administrative unit. No
  source or QML file differs from `92c452b`.
- This proves delivery, not perception. Whether the Sensors page reads well
  on the author's own compositor, whether every chip's card is legible, and
  whether the thermal load's colour reads as intended, is `VAL-H4`, which
  stays pending and did not block this closure.
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

## Follow-up

`VAL-H4` remains pending in [VALIDATION.md](../../VALIDATION.md). The next
checkpoint is `H5` (services and privileged actions), not yet opened. Its
first step is the author's written privilege decision: a foreign process is
signalled through `pkexec`, and a service is started, stopped or restarted
over the system bus through systemd rather than a setuid helper of its own.
