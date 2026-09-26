# Evidence: 2026-09-26 swatch tokens, the literal-alpha rule and lock contrast

- **Date:** 2026-09-26
- **Scope:** `STYLE-G7-N` of the
  [shared reading controls plan](../plans/active/2026-08-04-shared-reading-controls.md),
  program unit P-16 of the
  [monorepo audit](../../../docs/evidence/2026-09-26-monorepo-audit.md). It
  closes STY-1, STY-2 and STY-3 and the token and contract half of SH-6 from
  the [shell and style audit record](../../../docs/evidence/2026-09-26-monorepo-audit-shell-style.md).
- **Environment:** session worktree `unit/celestina-style/STYLE-G7-N` on base
  `2a3c74f`, in a Linux container; Python 3.11.15, GNU bash 5.2.21, Git
  2.43.0. No Qt 6 SDK: no `qmltestrunner`, `qmllint`, `qmlformat`, CTest over
  the module or production build could run.
- **Artifact:** the landing builds and verifies the registered
  `celestina-style` artifact; this session built nothing.

## Procedure

The changes, per finding:

- **STY-1.** `CelestinaScrollBar` names itself
  `qsTr("Desplazamiento horizontal")` or `qsTr("Desplazamiento vertical")`
  instead of the English literals. `tst_scrollbar.qml` gains a horizontal bar
  and `test_the_accessible_names_are_product_copy`, which asserts the
  `ScrollBar` role and both names.
- **STY-2.** `CelestinaTheme` gains `compMenuSwatchSize` (14),
  `compMenuSwatchSlash` (11) and the scheme role `swatchOutline`, derived in
  the theme as `withAlpha(ref.textHi, swatchOutlineOpacity)` with
  `swatchOutlineOpacity` 0.24. `GlassMenuItem` consumes the three tokens, so
  the rendered values are unchanged. `tst_menuitem.qml` gains
  `test_the_swatch_anatomy_comes_from_tokens`, which finds the swatch by its
  new `objectName` and asserts its size, outline, automatic outline and slash.
  `check-style-contract.sh` refuses a `withAlpha`/`multiplyAlpha` call whose
  second argument starts with a number. The audit proposed the line pattern
  `(withAlpha|multiplyAlpha)\([^,]+,\s*[0-9.]`, but every call in the tree is
  wrapped over several lines, so that pattern found 0 hits even in the
  unfixed `GlassMenuItem.qml`. The rule therefore reads each call's second
  top-level argument across line breaks, skipping nested brackets and quoted
  strings. A token, or an expression that starts with one
  (`CelestinaTheme.decorationOpacitySoft / 3`), is accepted. The one other
  literal the rule found, `0.4` in `tst_icongradient.qml`'s
  `test_alpha_survives`, now uses `unavailableContentOpacity`, which is the
  same 0.4, and that test's comment is translated to English.
- **STY-3.** `tests/tst_switch.qml` (8 cases) and `tests/tst_textfield.qml`
  (10 cases) join the directory that `QUICK_TEST_SOURCE_DIR` scans, which is
  how every existing test is registered: `celestina-style-modal-test` runs
  each `tst_*.qml` there under CTest `celestina-style-modal-focus`. The switch
  cases cover checkable state, the consumer name, Tab and Backtab to the
  exterior ring (width `borderFocus`, colour `focusRing`), Space toggling with
  one `toggled` each, a pointer toggle that shows no ring, track and thumb
  state from tokens, the reduced-motion thumb landing at once against a
  thumb that travels without it, and a disabled switch that Tab skips and a
  click cannot toggle. The text field cases cover the consumer name and Tab
  reachability, the ring and lifted `inputFillFocus` for Tab, Backtab and
  Shortcut, none for a pointer or `Qt.OtherFocusReason`, the ring leaving
  with focus, typing and Backspace, the Standard and Search radii, and a
  disabled field that Tab skips. The Qt behaviour these cases assume was read
  in the Qt 6.9 sources (`qtdeclarative` 6.9 branch): `ButtonPressKeys`
  defaults to Space and Select, a checkable `QQuickAbstractButton` reports
  the CheckBox role, `QQuickItemPrivate::focusNextPrev` gives the Tab and
  Backtab reasons, a `TextField` press focuses with `Qt.MouseFocusReason`,
  and a zero-duration animation job completes when it starts.
- **SH-6, token and contract half.** `lockScrim` is `#d9000000`, a scheme
  role beside `scrim` with its flat `CelestinaTheme.lockScrim`.
  `check-contrast-contract.py` composites it over the black and white
  extremes and requires 4.5:1 for `text`, `textMuted` and `danger` on three
  lock surfaces: the wash itself, a `ContextualVeil` card over the wash
  (`glassHighlight` at `glassContextualVeilStrength`), and the card's
  `surfaceStrong` fallback. The veil tint is defined in `GlassSurface.qml`,
  not in the theme, so the contract checks that the `ContextualVeil` branch
  still selects `glassHighlight` and `glassContextualVeilStrength`, and
  fails with exit 2 if either changes. Celestina's `LockScreen.qml` still
  paints `scrim`. Adopting `lockScrim` there is `SURF-1-F`, which the plan
  boundary assigns to the lock's own unit.

Commands, from the worktree root:

```sh
python3 scripts/agent-context.py celestina-style
bash celestina-style/scripts/check-style-contract.sh
python3 celestina-style/scripts/check-contrast-contract.py
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
bash scripts/test-architecture-scanners.sh
git show HEAD:celestina-style/GlassMenuItem.qml \
  | grep -cE '(withAlpha|multiplyAlpha)\([^,]+,\s*[0-9.]'
```

The RED runs temporarily restored the pre-fix bytes and then put the fixed
bytes back. `GlassMenuItem.qml` was restored from `HEAD`, and `lockScrim` was
set to `scrim`'s `#73000000`. For the veil-recipe probe, the first
`? CelestinaTheme.glassHighlight` in `GlassSurface.qml` was set to
`glassTint`. A temporary `celestina-style/tests/ProbeLiteralAlpha.qml` held
five calls: a single-line literal, a literal wrapped over lines, a literal
after a nested `Qt.binding(...)` argument, a token expression and an
identifier-led product. It was deleted after the run.

## Result

- **Exit:** style guard 0, contrast contract 0, architecture guard 0 (it
  runs the QML visual and contrast contracts), language guard 0 (148 legacy
  files ratcheted, unchanged), documentation guard 0 (with the pre-existing
  Siderita inventory errata). `test-architecture-scanners.sh` exited 1 with
  three identical lines, "`check-style-contract.sh`: an input list omits
  `hematita/qml`". It fails the same way with `HEAD`'s guard restored,
  because this is the audit's TOOL-1 and TOOL-2, which the suite plan owns.
- **RED, literal alpha.** With `HEAD`'s `GlassMenuItem.qml`, the style guard
  exited 1 and named `GlassMenuItem.qml:76` and `tst_icongradient.qml:118`.
  After the fix it exited 0. The audit's single-line pattern counted 0 hits in
  the unfixed file.
- **Negative probe.** The guard exited 1 on the three literal calls (lines 3,
  4 and 7) and accepted the token expression and the identifier-led product.
- **RED, lock contrast.** With `lockScrim` at `#73000000` the contract exited
  1 with six failures, all over white: wash `text` 3.17:1, `textMuted` 1.32:1
  and `danger` 1.28:1, which are the audit's ratios, and veil card 3.07:1,
  1.28:1 and 1.24:1. The fallback card and all black pairs passed.
- **RED, veil recipe.** With the veil tint changed, the contract exited 2:
  `ContextualVeil no longer uses the contractual recipe`.
- **GREEN, lock contrast** (`text` / `textMuted` / `danger`). Over white:
  wash 14.26 / 5.95 / 5.74, veil card 13.40 / 5.59 / 5.40, fallback card
  15.67 / 6.53 / 6.31. Over black: wash 19.79 / 8.25 / 7.97, veil card
  19.15 / 7.98 / 7.71, fallback card 16.06 / 6.70 / 6.47.
- **Observed.** At `#d9000000` the lock's current passphrase plate
  (`glassHighlight` at opacity `decorationOpacitySoft / 4` over the veil card)
  would hold its `textMuted` placeholder at 5.08:1 over white. That plate is
  Celestina's local recipe, so the style contract does not model it. At
  `#cc000000` (the `mediaScrim` value), `danger` on the veil card over white
  would be 4.49:1, which is why the lock wash is its own, denser role.
- **Qt Quick tests.** Not run: RED and GREEN for STY-1, STY-2 and STY-3 are
  argued from the source. Before the fix `bar.Accessible.name` was `"Vertical
  scroll"`, so the new scrollbar case fails. Before the fix the swatch had no
  `objectName` and no tokens to compare against, so the new menu item case
  cannot pass. The switch and text field cases add coverage and do not
  reproduce a defect.

## Limits

- No Qt Quick test, `qmllint`, `qmlformat --check` or build ran here. The new
  and changed QML was checked by reading against the Qt 6.9 sources named
  above. `celestina-style/scripts/verify-production.sh` must run on the
  author's machine, where the landing runs it: it builds `all_qmllint`, runs
  CTest (and so `tst_switch.qml`, `tst_textfield.qml` and the three changed
  tests) and the smoke. The landing also rebuilds Celestina, whose lint reads
  the changed theme.
- The accessible role and checked state of the switch and text field are set
  by Qt only while an assistive technology is active, so the tests assert the
  inputs Qt derives them from, not what an AT-SPI client hears. That remains
  `VAL-STYLE-03`.
- The contrast ratios are computed with the contract's own compositing over
  black and white extremes, not measured on a display over a real wallpaper.
  Blur, the capture pass and noise are not modelled: over uniform extremes
  they do not move the composite.
- The lock still paints `scrim` until Celestina adopts `lockScrim`
  (`SURF-1-F`, the lock half of SH-6). Until then SH-6 remains open on the
  real lock.
- The literal-alpha rule inspects only the start of the second argument, so
  `root.strength * 0.5` passes. It exists to stop a literal alpha, not to
  audit arithmetic.
- The row's plan names a new fixture in the root
  `scripts/test-architecture-scanners.sh`. That file is outside the
  `celestina-style/` commit root, so this unit could not change it. The
  temporary negative probe above stands in for that fixture.

## Follow-up

- TOOL-2 (the suite plan) adds `hematita/qml` to the three input lists of
  `check-style-contract.sh`. That is the file this unit also changes, so the
  later landing will rebase over this one.

- `SURF-1-F` (Celestina): paint the lock wash with `CelestinaTheme.lockScrim`.
- A root `suite:` unit may move the literal-alpha probe into
  `scripts/test-architecture-scanners.sh` as a permanent fixture.
- The audit also lists `CelestinaCapsule`, `CelestinaRowHighlight`,
  `GlassCard`, `GlassContextMenu`, `ListSection` and `CelestinaSectionLabel`
  as untested. The audit's fix asked for the switch and the text field "at
  minimum", and those two are what this unit adds.
