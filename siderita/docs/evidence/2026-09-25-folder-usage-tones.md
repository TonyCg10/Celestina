# Evidence: 2026-09-25 SID-U1-C rank tones in the folder occupation

- **Date:** 2026-09-25
- **Scope:** unit `SID-U1-C`; plan
  [folder-usage-tones](../plans/archive/2026-09-25-folder-usage-tones.md)
- **Environment:** Arch Linux, `rustc 1.98.1`, `cmake 4.4.3`, Qt 6
  `qmltestrunner` offscreen
- **Artifact:** `siderita/target/release/siderita`; the file and the
  installed `/home/toni/.local/bin/siderita` both hash
  588a2059dbb1bb7c8814985641965307c5e3b033b112f4fdd04f3a650672631a. The
  sealed manifest `siderita/target/production-artifact.toml` records
  `git_revision = fa6fe5649a082b7b739d8f8ec2ec59d981988107` (the base, as the
  change was uncommitted) and `verified = true`

## What this fixes

Rows and tiles took their tone from the entry kind, so nearly every tile was
the same blue. `FolderUsage.toneColors` now maps `p0`..`p5` to
`CelestinaTheme.usagePalette`; `weave` gives each row `"p" + (index % 6)`
(rows are published biggest first), `unreadable` when it has unreadable
folders below and `other` for the merged remainder, and each tile inherits
its row's tone. The corner geometry comes from `celestina-style` 1.9.1
through the existing symlinks; nothing in Siderita changes for it.

## Procedure

```sh
bash siderita/scripts/qml-tests.sh
python3 scripts/version_tool.py bump siderita bug --unit SID-U1-C \
  --summary "Fix the occupation tiles all sharing one colour"
bash siderita/scripts/complete-production.sh
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
python3 scripts/version_tool.py check
```

## Result

- **Exit:** 0 for every command.
- **QML tests:** 167 passed, 0 failed, including two new
  `tst_folder_usage.qml` cases: `test_o` (three rows get `p0`, `p1`, `p2`,
  the tile of row 0 carries `p0`, and `p0` resolves to `usagePalette[0]`) and
  `test_p` (a row with unreadable folders below keeps `unreadable` and the
  merged remainder keeps `other`).
- **Production:** `complete-production.sh` built, verified (Rust tests, the
  QML tests, the offscreen smoke «binario vivo 8 s, sin errores QML»), sealed
  the manifest, deployed to `/home/toni/.local` without rebuilding, and
  `status-production.sh` reported the artifact current and verified and the
  installed binary OK.

## Limits

Headless tests prove the tone keys and their mapping, not the perceived
colours, their contrast on the real panel, or the corner rendering at the
session's scale. Those are the new step of `VAL-SID-U1`, not run by hand.
