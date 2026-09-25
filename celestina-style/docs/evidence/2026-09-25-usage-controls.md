# Evidence: 2026-09-25 the shared treemap and usage list

- **Date:** 2026-09-25
- **Scope:** `STYLE-G7-G` — `celestina-style` 1.9.0
- **Environment:** the author's Arch-derived Linux, Qt 6.11, offscreen QPA for
  tests
- **Artifact:** the registered `celestina-style` production artifact

## Procedure

The contract below was extracted from Hematita's local controls, then built,
tested and verified through the registered production scripts.

### Contract

`CelestinaTreemap` takes `tiles` (`{ id, x, y, w, h, name, kind, tone }`,
geometry 0..1 of the field, `id < 0` the merged remainder drawn disabled),
`toneColors`, `currentId`, `currentName`, and optional `markedIds` and
`dimmedIds` (`-1` dims the remainder). It emits `chosen(id)` on click or arrow
focus, `entered(id)` on double click or Enter, and `upRequested()` on
Backspace.

`CelestinaUsageList` takes `usageRows` (`{ id, name, kind, tone, share, size,
percent, detail }`, `detail` may be empty), `toneColors`, optional `markedIds`
and `emptyText`. It emits the same three signals and keeps the functions
`reset`, `keepViewport`, `follow`, `takeFocus` and `currentId`.

Neither control accepts Space or Delete: both bubble to the host, which owns
selection and every action.

### What changed from Hematita's originals

- No application import; the module is implicit.
- `selectedIds` became optional `markedIds`; the treemap's per-tile `matches`
  became the host's `dimmedIds`; the usage list's `filtered` flag became the
  host's `emptyText`.
- `toggled` and `trashRequested` and their Space/Delete handling are gone.
- Tiles carry `kind`, and tiles and rows are named with their kind for
  assistive technology; the row's `copies` count became a free `detail`
  string appended to the percentage.
- In the usage list, the ListView gives its current delegate active focus, so
  a focused `AbstractButton` would take Space as its own click and never let it
  reach the host. The row button now sits in a plain `Item` slot that receives
  that focus instead. Visuals and geometry are unchanged. The slot also carries the row's single accessible ListItem (the inner button is `Accessible.ignored`), so focus and the announced row sit on the same object; `VAL-STYLE-04` should confirm the announcement with a screen reader. Hematita's original
  has the same shape, so its own Space handler on the ListView cannot have
  received Space from a focused row; its migration will resolve that.
- Every visual value is the same `CelestinaTheme` token the originals used.

## Result

### Tests

`tests/tst_treemap.qml`: `test_arrows_walk_reading_order`,
`test_enter_enters_and_backspace_goes_up`,
`test_space_and_delete_reach_the_host`,
`test_remainder_is_named_and_disabled`, `test_tiles_are_named_by_kind`,
`test_dimmed_ids_fade`, `test_dimmed_remainder_fades`,
`test_left_and_up_step_backwards`.

`tests/tst_usagelist.qml`: `test_down_chooses_the_next_row`,
`test_return_enters_the_current_row`, `test_backspace_goes_up`,
`test_space_and_delete_reach_the_host`,
`test_empty_rows_show_the_empty_text`, `test_focused_row_is_announced`.

QuickTest totals: 91 passed, 0 failed.

### Verification

`bash celestina-style/scripts/build-production.sh`, then
`bash celestina-style/scripts/verify-production.sh`: production-common
fixtures OK, architecture, style, contrast and QML visual contracts OK,
`all_qmllint` with only the pre-existing `CelestinaLineGutter` warnings, CTest
1/1, the eight-second gallery smoke OK, and the manifest reported verified.
`python3 scripts/check-language-contract.py` OK.

## Limits

Real-session focus, pointer feel and AT-SPI announcements remain
`VAL-STYLE-04`; the consumers' migration and their own evidence are later
units.
