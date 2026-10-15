# Evidence: 2026-10-15 smoke race log read from its own line

- **Date:** 2026-10-15
- **Scope:** `HEM-H1-G` — `hematita/scripts/smoke.sh`
- **Environment:** the author's CachyOS, `dbus-run-session` 1.16.2; the private bus activates `xdg-desktop-portal`, `xdg-desktop-portal-gtk` and, through it, `gvfsd`
- **Artifact:** the release binary of the canonical checkout, not rebuilt

## Procedure

Reproduction, observed by the controller on 2026-10-15 before the fix:

```sh
python3 scripts/production_artifact.py run-verification hematita
```

After the fix, from the unit's worktree and twice in a row:

```sh
sh hematita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA/hematita/target/release/hematita
```

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

## Result

- **Exit:** the reproduction exited 1 with "two launches on one bus did not end as one window (expected exit codes 124 and 0; got 'A connection to the bus can't be made')"; both smoke runs after the fix exited 0 and printed the final OK line, including "two launches on one bus ended as one window"; the three guards exited 0.
- **Observed:** `gvfsd`, activated through `xdg-desktop-portal-gtk`, printed "A connection to the bus can't be made" into `race.log` after the shell's `"$ra $rb"` line, once `dbus-run-session` killed the bus, so `tail -1` returned that sentence. The smoke now takes the one line shaped `N N` from the log and sets `GIO_USE_VFS=local` for the race so GLib does not activate gvfs. The failure path also called `tail -5` on two files, which GNU tail refuses; it now tails each file with `tail -n 5`.

## Limits

- The race still depends on the host's portal configuration; `GIO_USE_VFS=local` only prevents gvfs, other bus-activated services still write into the log, which the codes line read now tolerates.
- The binary was not rebuilt: the artifact tool refuses builds in a session worktree, so this proves the script, not a new build.

## Follow-up

None.

## Landing

- **Base revision:** `e79dffb8e6eaee52a7699a0f7360dde7700208e7`
- **Check:** `production_artifact.py check hematita --require-verified` exit 1: production-artifact: artifact is not verified yet; run verify-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** hematita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:228d7ffc22ed8ddabcb3ae3c14e7915f6bc0d24da3b99d8b43bd306d2b89ca84, verification_fingerprint sha256:662a16c16c1afb1edae157c4c9187c3d0e94a791b59747c86d65b8df1fbfb7c0
- **Deploy:** after the push: hematita: deploy-production.sh, status-production.sh
