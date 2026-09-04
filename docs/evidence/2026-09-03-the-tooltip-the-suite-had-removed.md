# Evidence: 2026-09-03 the tooltip the suite had already removed

- **Date:** 2026-09-03
- **Scope:** `FEEDBACK-2` — `celestina-style` 1.8.5, `siderita` 1.5.6,
  `magnetita` 1.2.3, plus documentation corrections in `grafita` and
  `fluorita`. A correction of the same day's `FEEDBACK-1`
  ([record](2026-09-03-apps-feedback-and-icon-first.md))
- **Environment:** the author's Arch-derived Linux, Qt 6.11, `cargo` stable,
  offscreen QPA. No real session or pointer was driven
- **Artifact:** the registered production artifacts of the five owners

## The defect

The author reported it in one sentence: tooltips had been explicitly removed
everywhere, and the icon-first pass put them back.

The reading confirms it. `CelestinaButton` has carried
`ToolTip.visible: helpText.length > 0 && hovered` since the commit that
unified the suite's button, and two consumers had since grown a wrapper whose
entire body was the override that silenced it:

- `magnetita/qml/components/QuietIconButton.qml` — `ToolTip.visible: false`,
  delivered by the wireless-mirror milestone whose plan says "Icon-first, no
  tooltips".
- `celestina/qml/BackdropButton.qml` — `ToolTip.visible: false` and
  `ToolTip.text: ""`, with the comment that shell controls "never paint hover
  cards above the compact panel".

Two overrides of the same default in two of the six consumers is not a
preference; it is the default being wrong. `FEEDBACK-1` then made it worse in
two ways. It added a tooltip to `siderita/qml/components/chrome/FloatingButton.qml`,
which had never had one, so every floating pill in Siderita — the hidden-files
toggle, the size button, the trash, recents and search headers, the picker's
chrome — grew a hover card. And it converted a great many text buttons into
icon buttons carrying `helpText`, which is what turns the shared button's
tooltip on: in Siderita's bottom controls two silent `Accessible.name`
bindings were rewritten as `helpText` outright. The words the author had
removed came back on hover, over the very rows being acted on.

## What changed

**The shared button paints no tooltip.** `CelestinaButton` drops both
attached `ToolTip` bindings. `helpText` keeps its name and gains the meaning
the two wrappers had been asserting: it is the accessible name and nothing
else, now bound as `Accessible.name: helpText.length > 0 ? helpText : text`
so a labelled button cannot lose its name either. Every existing consumer
keeps the exact string it had; only the hover card is gone.

**`QuietIconButton` is deleted.** With the default corrected its whole body
was empty, which is precisely the boundary-free pass-through wrapper the root
contract forbids. Its thirteen uses across five Magnetita files now name
`CelestinaIconButton` directly, and `build.rs` no longer registers it.

**Siderita's floating pill loses the tooltip it had grown.** `FloatingButton`
keeps its `Accessible.name`, which already read `helpText` first.

**The design contract stops promising one.** `DESIGN.md` had a `Tooltip` row
in *specified, not exported* describing a delayed pointer label as an accepted
future shape, and listed tooltips among the L2 glass surfaces. Both are
corrected: a tooltip is not part of this language, and the component rows for
button and icon button no longer name one.

`celestina/qml/BackdropButton.qml` keeps its two override lines. They are now
redundant rather than load-bearing, but that file is the shell's, which is
outside this unit's scope, and it exists for its own reason — it overrides
`contentItem` and `background` for the shell's fixed light ink — so it is not
a pass-through and does not disappear with the default.

## Procedure

| Check | Result |
|---|---|
| `celestina-style`: build and verify production (73 Qt Quick tests, `all_qmllint`, style and contrast guards, gallery smoke) | pass |
| `siderita`: build and verify production (fmt, clippy, unit tests, qmllint at baseline, 102 QML tests, offscreen smoke) | pass |
| `magnetita`: build and verify production | pass |
| `grafita`, `fluorita`: build and verify production, so the installed bytes carry the corrected shared control | pass |
| `check-language-contract.py` | OK; `CelestinaButton.qml`'s header comment was translated to English and its baseline row removed in the same unit |
| Deployment to the author's prefix, four applications and the daemon | done |

## Result

No surface in the four applications paints a tooltip. `rg ToolTip` over
`celestina-style` and the four applications' QML returns nothing but the
shell's own override, which is out of scope. Every action that lost its
visible label in `FEEDBACK-1` still answers assistive technology with the same
Spanish string, now through `Accessible.name` alone.

## Limits

- That the glyphs are legible without any label is a perceptual claim and
  stays with the author: `VAL-SID-12`, `VAL-GRA-FEEDBACK`,
  `VAL-FLU-FEEDBACK`, `VAL-MAG-10` and `VAL-STYLE-05` already ask for it, and
  their pass conditions were corrected to say that no glyph may paint a hover
  card.
- The shell was not reviewed. If its `BackdropButton` is ever revisited, its
  two `ToolTip` lines are now dead weight.
