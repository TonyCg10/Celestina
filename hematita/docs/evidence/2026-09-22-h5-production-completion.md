# Evidence: 2026-09-22 H5 delivered to the author's prefix

- **Date:** 2026-09-22
- **Scope:** checkpoint `H5`, units `H5-A` through `H5-Z`; plan
  [h5-services](../plans/archive/2026-09-22-h5-services.md)
- **Environment:** Arch Linux, `rustc 1.98.1`, `c++ (GCC) 16.2.1`,
  `cmake 4.4.3`; Qt reports `unavailable` in the toolchain probe, as it has on
  every earlier release
- **Artifact:** `hematita/target/release/hematita`,
  sha256 114a20a6b29949dc7652f4362719b31f8ebee94accc2a1ef3aecba0c88aeae06.
  The manifest records `git_revision = 8df8367b6c1b3735bf8eab65305b7c5d6bd9d39e`
  and `verified = true`
- **Sealed manifest:** `hematita/target/production-artifact.toml`

## What this closes

`H5-A` through `H5-C` were written and their focused evidence recorded
across 2026-09-22, but the checkpoint's own implementation exit had not yet
run at `0.6.0`. This record runs that exit: the version bump, the canonical
build, its verification, and the deployment to the author's prefix. It adds
no source change of its own beyond the version bump.

## Procedure

```sh
python3 scripts/version_tool.py bump hematita milestone --unit H5-Z \
  --summary "Add the Services page and polkit-mediated actions"
python3 scripts/version_tool.py check
hematita/scripts/complete-production.sh
hematita/scripts/status-production.sh
```

## Result

- **Exit:** 0 for every command.
- **Observed:**

| Command | Result |
|---|---|
| `python3 scripts/version_tool.py bump …` | `hematita: 0.5.1 -> 0.6.0 (milestone)`, one appended history row |
| `python3 scripts/version_tool.py check` | `version-contract: OK (8 owners)` |
| `hematita/scripts/complete-production.sh` | release build finished; `cargo test -p hematita-core` 67 passed; `tests/captures.rs` 11 passed; `qmllint-production` OK (0 non-fatal baseline warnings); smoke OK, binary alive 10 s, every section shown, the first row published the CPU contract, the Sensors page published chips, the Services page listed system units, no QML errors, no auto-bindings; manifest verified; deployed to `/home/toni/.local` without rebuilding |
| `hematita/scripts/status-production.sh` | `artifact: hematita current and verified`; `installed: OK /home/toni/.local/bin/hematita` |

## Installed state

- `~/.local/bin/hematita` — sha256 matches
  `hematita/target/release/hematita` byte for byte (plain `sha256sum` on
  both paths, both equal to
  `114a20a6b29949dc7652f4362719b31f8ebee94accc2a1ef3aecba0c88aeae06`).
- `~/.local/share/applications/org.celestina.Hematita.desktop` exists.
- `~/.local/share/icons/hicolor/*/apps/org.celestina.Hematita.{png,svg}`
  exist at every listed size plus the scalable SVG.

The installed binary was not launched interactively as part of this record:
per the controller's build-budget decision, the installed bytes were checked
by hash equality against the freshly verified release binary and by the
presence of the desktop entry and icon files, rather than by opening a
window on the live or nested session.

## Limits

- `worktree_dirty` may be true in the manifest at seal time: the dirt is
  documentation and version-bump only — this record, the archived plan,
  `Cargo.toml`/`Cargo.lock`, the roadmap/status updates and
  `docs/version-history.tsv` in the same administrative unit. No source or
  QML file differs from `8df8367`.
- This proves delivery, not perception. Whether the Services page reads well
  on the author's own compositor, whether starting or stopping a unit
  behaves as intended, and whether the polkit prompt appears when an agent
  is present, is `VAL-H5`, which stays pending and did not block this
  closure.
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
- This session has no authentication agent registered on either bus, so
  every privileged action this record's verification could exercise can
  only answer `no-agent`; no privileged action was performed during
  verification.

## Follow-up

`VAL-H5` remains pending in [VALIDATION.md](../../VALIDATION.md). The five
phases of the design (`H1` through `H5`) are delivered; further work opens
with a new checkpoint and the author's word.
