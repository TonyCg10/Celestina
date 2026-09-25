# Evidence: 2026-09-25 SID-U1-B corrections after the folder-usage review

- **Date:** 2026-09-25
- **Scope:** unit `SID-U1-B`; plan
  [folder-usage-fixes](../plans/archive/2026-09-25-folder-usage-fixes.md)
- **Environment:** Arch Linux, `rustc 1.98.1`, `c++ (GCC) 16.2.1`,
  `cmake 4.4.3`, Qt 6 `qmltestrunner` offscreen
- **Artifact:** `siderita/target/release/siderita`; the sealed manifest
  records the framed digest sha256 b2b4832112867d2df9f4f6eb274eccf4bd69813bbff3cd2aed0c5a11907e375c,
  and the file and the installed `/home/toni/.local/bin/siderita` both hash
  92357c661d607bd4af9a30458068b3a2ad4a12bcc296d73b4249abfa2e77199e.
  The manifest records `git_revision = 74e981cbf43f3389fd818988154c2b93dcd12aa2`
  (the base, as the change was uncommitted) and `verified = true`
- **Sealed manifest:** `siderita/target/production-artifact.toml`

## What this fixes

The final review of the folder-usage work on the deployed 1.7.0 found:

1. «Abrir en Hematita» sent `usage.currentPath`, which `UsageSession::project`
   leaves empty until the tree lands, so while scanning or failed it always
   showed «No se pudo abrir Hematita». `FolderUsage.openInHematita` now falls
   back to `usage.root`, and the footer row is hidden while `failed` (the
   design's §4: a failure shows its cause and no controls).
2. Up on the first row, Down on the last (the list does not wrap) and
   Return/Enter on an empty list were left unaccepted and reached
   `QuickLookView`'s key handler, which stepped to another entry or closed
   and navigated. The row holding the list and the map now accepts Left,
   Right, Up, Down, Return and Enter in `Keys.onPressed`, which runs only
   after the focused list or map left the key unaccepted.
3. After the properties dialog took the hub over and closed, the quick look
   kept the plain glyph until its entry changed. `QuickLookView` now listens
   to `ownerChanged` and calls `syncUsage()` when the hub becomes free while
   it shows a folder.
4. The quick look's hint for a non-folder entry was a bare literal; it is now
   inside `qsTr()`.

## Procedure

```sh
bash siderita/scripts/qml-tests.sh
python3 scripts/version_tool.py bump siderita bug --unit SID-U1-B \
  --summary "Fix the Hematita hand-off during a scan and the keys that leaked out of the occupation section"
bash siderita/scripts/complete-production.sh
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
python3 scripts/version_tool.py check
```

## Result

- **Exit:** 0 for every command.
- **QML tests:** 165 passed, 0 failed, including three new
  `tst_folder_usage.qml` cases: `test_l` (Up on the first row, then Enter and
  Return on an empty list, reach no host handler and drill nothing),
  `test_m` (during a scan with an empty current path the button hands over
  the scanned root `/home`) and `test_n` (a failed analysis shows neither
  footer button).
- **Production:** `complete-production.sh` built, verified (Rust tests, the
  QML tests, the offscreen smoke «binario vivo 8 s, sin errores QML»), sealed
  the manifest, deployed to `/home/toni/.local` without rebuilding, and
  `status-production.sh` reported the artifact current and verified and the
  installed binary OK at 1.7.1.

## Deviations from the design

Accepted at the final review and recorded as amendments in the design's §10:

- Esc closes the quick look directly instead of first returning focus to the
  surface.
- There is no occupation heading nor indeterminate bar while scanning; the
  totals line says «Calculando…».
- Scan failures are shown as a line in the section, not as a toast.
- `gtk-launch` failures are asynchronous, so a missing Hematita is not
  detected.
- Space on a focused crumb or footer button presses it.

## Limits

Headless tests prove key routing against a stub hub and a stub host; they do
not prove the real quick look's focus chain, the takeover round trip with the
real `SideritaUsage`, Hematita actually opening on the root mid-scan, or what
a screen reader says. Those are the new steps of `VAL-SID-U1`, not run by
hand.
