# The storage tiles all sharing one colour — S3-B

- **Date:** 2026-09-25
- **Scope:** `S3-B` of
  [`../plans/archive/2026-09-25-s3-rank-tones.md`](../plans/archive/2026-09-25-s3-rank-tones.md):
  the Storage page's tones
- **Environment:** the author's checkout; offscreen Qt platform, no session
  bus
- **Artifact:** `hematita/target/release/hematita` 1.2.1 built, verified and
  deployed by `hematita/scripts/complete-production.sh`

## Procedure

1. `qml/components/StoragePage.qml`: `toneColors` maps `p0`..`p5` to
   `CelestinaTheme.usagePalette[0..5]` and keeps `duplicate`, `empty`,
   `unreadable` and `other`; `dir` and `file` are gone (only `toneOf` produced
   them; `FolderList.qml` reads `kind`, not the tone). `toneOf(rank, ...)`
   still lets the filters impose `unreadable`, `duplicate` and `empty`, and
   otherwise returns `"p" + rank % 6`, where the rank is the row index of the
   published order, biggest first. Tiles take their row's tone as before and
   the remainder stays `other`.
2. `python3 scripts/version_tool.py bump hematita bug --unit S3-B` moved
   1.2.0 to 1.2.1.
3. `bash hematita/scripts/complete-production.sh`, once.
4. `bash scripts/check-architecture-contract.sh`,
   `bash scripts/check-documentation-contract.sh`,
   `python3 scripts/check-language-contract.py`,
   `python3 scripts/version_tool.py check`.

## Result

`complete-production.sh` exited 0: build, 188 tests, `qmllint-production`
(0 non-fatal baseline warnings) and the offscreen smoke (every section shown,
the storage page listed locations, a folder argument landed in storage, no
QML errors) passed; the verified artifact was deployed to `~/.local` and
`status-production.sh` reports it current and installed. The guards passed.

## Limits

The smoke does not scan a folder, so no automated check looks at a tile's
colour or the corner geometry; both are in `VAL-S3` in the author's lane.
