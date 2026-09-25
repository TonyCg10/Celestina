# Evidence: 2026-09-25 the corners of the shared usage controls

- **Date:** 2026-09-25
- **Scope:** `STYLE-G7-H` — `celestina-style` 1.9.1
- **Environment:** the author's Arch-derived Linux, Qt 6.11, offscreen QPA for
  tests
- **Artifact:** the registered `celestina-style` production artifact

## Procedure

The author saw `CelestinaTreemap` and `CelestinaUsageList` in Siderita and
reported radii that did not match, text and elements poking through the
panel's rounded corners or sitting against its edge, and every tile in the
same colour. The fix, built and verified through the registered scripts:

- **Concentric corners.** Both panels set `radiusOverride:
  CelestinaTheme.radiusMd` and inset their content by `spaceSm` on every side;
  rows and tiles keep `radiusSm`, so 12 + 8 = 20. The list's `ListView`
  margins and the treemap's `padding` are that inset, and the list clips. The
  row plate and its focus border are drawn inside the row. The treemap's 2 px
  exterior focus ring may enter the 8 px inset and never crosses the panel
  edge.
- **Tiles.** The layout spans the field plus one gap and each tile gives the
  gap back on its right and bottom, so outer tiles meet the field edges
  exactly (keeping the corners concentric) and neighbours stand `spaceSm`
  apart. Labels have `spaceSm` side padding and show only on tiles wider than
  `fontCaption * 4` and taller than `fontCaption * 2`.
- **Rows.** The name elides right. The numbers column has one width, from a
  `TextMetrics` probe of "8.888,8 GiB" in `fontRowSecondary` with tabular
  figures, so every row's name column and bar have the same width; both
  numbers lines are right-aligned; the size never elides (a thousands
  separator must not cost its leading digits), the detail line elides right. The row's inner
  margins stay `spaceMd`.
- **Palette.** `CelestinaTheme.usagePalette` is `[glyphAccentBlue,
  glyphAccentViolet, glyphAccentCyan, glyphAccentGreen, glyphAccentAmber,
  glyphAccentCoral]`, documented in `DESIGN.md` §4 as the rank palette for
  share visualizations. The controls' contract is unchanged: `tone` is still
  looked up in the consumer's `toneColors`. No consumer maps ranks
  `p0..p5` to it yet; that is an obligation of Siderita's and Hematita's own
  units.
- `DESIGN.md` §5.1 states the concentric rule once; §6.1 describes the new
  anatomy of both controls.

## Result

### Tests

New: `tst_treemap.qml` `test_field_is_inset_by_space_sm`,
`test_outer_tiles_meet_the_field_edges`,
`test_usage_palette_has_six_entries`; `tst_usagelist.qml`
`test_list_is_inset_by_space_sm`, `test_a_separated_size_is_shown_whole`
("1.234,5 GiB" untruncated inside its column). The palette test asserts all
six entries in the contracted order. No existing assertion changed.

`QT_QPA_PLATFORM=offscreen build/celestina-style-qml-test -input tests`:
96 passed, 0 failed.

### Verification

`bash celestina-style/scripts/build-production.sh`, then
`bash celestina-style/scripts/verify-production.sh`: production-common
fixtures, architecture, style and QML contracts OK, `all_qmllint` with only
the pre-existing `CelestinaLineGutter` warnings, CTest 1/1, gallery smoke OK,
manifest verified. The suite guards (`check-architecture-contract.sh`,
`check-documentation-contract.sh`, `check-language-contract.py`,
`version_tool.py check`) ran before staging.

## Limits

No consumer is changed here: Siderita and Hematita still supply their own
`toneColors` and must map ranks to `usagePalette` in their own units. How the
corners, gaps and palette read on the real display, and AT-SPI, remain
`VAL-STYLE-04`.
