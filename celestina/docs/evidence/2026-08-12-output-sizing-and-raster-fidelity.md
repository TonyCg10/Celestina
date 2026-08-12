# PANEL-1-L — one physical size per output, and rasters that survive it

- **Date:** 2026-08-12
- **Scope:** Celestina unit `PANEL-1-L`
- **Artifact:** Celestina 0.13.0 with CelestinaStyle 1.5.0
- **Environment:** Linux release production workflow with offscreen Qt
  verification and normal test-prefix deployment; the author's own three
  outputs read live from the running compositor for the measurements below
- **Plan:** [panel glass redesign ledger](../plans/active/2026-08-08-panel-glass-redesign.md)
- **Validation:** `VAL-PANEL-1`

## Procedure

### What was wrong

Three defects, found while the author checked the shell on a second monitor.

**A logical pixel is not a length.** The shell's tokens are logical pixels, so
the same 40-token bar is a different physical size on every output. Read from
the author's running session:

| Output | Diagonal | Density | Bar height |
|---|---|---|---|
| HP M27h (HDMI-A-1), 1080p @ 1 | 27.2" | 81.3 dpi | 12.50 mm |
| LG UltraGear (DP-2), 1080p @ 1 | 24.0" | 92.0 dpi | 11.04 mm |
| LG UltraGear+ (DP-1), 4K @ 1.5 | 31.5" | 92.9 dpi | 10.94 mm |

The bar is 13 % smaller on the 32" monitor than on the 27" one — smaller on
the larger screen, which is also the one viewed from further away. That is
exactly what the author reported — the shell looking smaller and slightly
uncomfortable there — and it is arithmetic rather than taste.

**Rasters were generated smaller than they were drawn.** `CelestinaIcon` asked
for its vendored SVG at the item's logical size, so on any scaled output the
compositor received a pixmap smaller than the area it filled. Six further
paths — the panel tray, the inventory grid, menu rows, the workspace map,
album art, wallpaper thumbnails — did the same for their sources. Worse,
`traywatcher` rasterized every foreign tray icon once, at 18 pixels, in the
host: that is the only rasterization in the pipeline, and no consumer can
recover detail that was never generated. The inventory grid draws the same
icon larger still, which is where the author saw it worst.

**Three menus rebuilt themselves once a second.** `PerformanceMenu` built a
fresh entry array from every provider reading, so its `Instantiator` destroyed
and recreated all four rows on each tick. The card was then measured against a
menu in the middle of being replaced and stayed short for as long as it was
open — the clipped `Rendimiento` the author recorded twice. `NetworkMenu` and
`BluetoothMenu` had the same shape, tearing down every network and device row
on each tick of an aggregate that publishes for unrelated reasons.

### What was done

#### One bounded scale per output

`shellscale.{h,cpp}` turns an output's density into one factor.
`QScreen::physicalDotsPerInch` divides the output's logical width by its real
width, so the compositor's own scale is already accounted for; the factor is
that number over the density the tokens were drawn against — the author's 27"
panel, which they describe as correctly sized and which therefore does not
move at all. Both LG panels resolve to 1.15.

The factor is stepped to 0.05 so two similar monitors do not end up fractions
of a pixel apart in every derived metric, and bounded to 0.85..1.75. An output
whose published physical size is missing or absurd — televisions and virtual
outputs routinely report zero, and some report a diagonal of millimetres —
returns 1.0: the shell keeps the size it has rather than resizing itself from
a number it cannot believe.

It is applied as a scene scale on the panel window. That is the decision worth
recording: the alternative was making every size token per-output, and
`CelestinaTheme` is a singleton referenced 925 times while `ensurePanel`
creates one panel per screen, so a singleton cannot carry per-output sizes at
all. Scaling the scene keeps the layout in the shell's own logical pixels — the
40-pixel bar, the capsules at y=5 with height 30, the seam a contextual
surface attaches to — and differs only in the last step to real pixels. No
contract changed, which is why the whole suite passes untouched.

#### Rasters asked for at the density they are drawn at

`CelestinaIcon` multiplies its `sourceSize` by its screen's device pixel
ratio, and so do the six raster consumers. The tray host now rasterizes at 64
pixels instead of 18: a themed SVG is rendered at that size and an
application's own pixmaps are chosen against it, at a cost of 16 KiB per item.

#### Weight without size

The author asked for thicker text and glyphs at unchanged dimensions, so the
vendored catalogue's stroke width rises from 2 to 2.5 across all 96 icons and
the bar's own readings — clock, phone battery, unread count, now-playing —
take the demi-bold weight. No size token changed.

#### Row lists that carry identity, not values

The three provider-driven menus now build a list of what rows exist, keyed by
identity alone, and every moving value is read live by the row that shows it.
A structural signature is recomputed on each tick and is almost always
identical, so the rows are rebuilt only when a network or device really
appears, disappears, the adapter is switched, or a failure has to be reported.
`Component.onCompleted` is deliberately not used to seed the first list: it
would silently replace the one `AnchoredCard` uses to raise `ready`, which is
what opens the menu.

`PerformanceMenu` also lost its tools section at the author's request; its
readings are now the way into the system monitor.

## Result

```sh
cmake --build celestina/build --parallel 4
ctest --test-dir celestina/build --output-on-failure
bash celestina/scripts/complete-production.sh
```

The complete CTest suite passes 18/18, including the new
`celestina-shell-scale` case, and the offscreen QuickTest runner passes with
no failures, skips or blacklisted cases.

`shellscale_test.cpp` states the decision rather than the implementation: the
author's own three densities produce factors that bring every bar within a
third of a millimetre of the reference output; the reference output does not
move; unbelievable readings change nothing; plausible extremes are bounded
rather than obeyed; and every factor lands on the step.

The three menus are covered by row identity — that after seven reading ticks
the rows are the same objects rather than equal-looking replacements — plus
the card's measured height and the values having actually moved. Those cases
fail against the previous code.

## Limits

An application that publishes only a small pixmap and has no themed icon
remains soft when magnified. That is the best source available and is not
ours to fix.

The scene scale reached the panel only when this was first written; menus and
overlays drew unscaled, so on a denser output the bar was right and its
contextual surfaces were proportionally small. `PANEL-1-N` closes that: every
menu and overlay scales its own scene by the same factor, the geometry they are
handed is divided by it in the two controllers, and one case exercises a real
1.15 factor end to end.

That unit also corrects a defect this one shipped. Blur regions were published
by mapping the origin and using the item's own size, which disagree the moment
anything between the item and the window is scaled: on a 1.15 output the panel
asked the compositor to blur a region a third narrower and six pixels shorter
than the bar it painted. Both collectors now derive the rectangle from two
mapped corners.

`CELESTINA_SHELL_SCALE` delivers the manual override this record listed as
missing — a television at sofa distance, or a monitor whose EDID lies. It is
also what the tests needed: the offscreen platform reports a density of its
own, which silently rewrote every geometry contract stated in output pixels.

Nothing in this record is an author visual pass. `VAL-PANEL-1` remains
pending, and the nested session cannot substitute for it here: `winit`
publishes no physical size, so it resolves to 1.0 and never exercises the
scaled path at all.


## PANEL-1-O addendum — corrected live, on the author's own monitors

`PANEL-1-N`'s density-based factor was checked live and was wrong in two ways,
both found by the author comparing the nest against a real desktop rather than
by inspecting the arithmetic.

Density could not separate two of the author's monitors: their 24" 1080p panel
measures 91.73 dpi and their 32" 4K panel measures 93.34 dpi, 1.6 dpi apart.
They confirmed 1.00 on the first and 1.15 on the second live. The factor is now
derived from physical diagonal instead — 24.0" against 31.5" is a real
difference — floored at 1.0 so a smaller monitor is never shrunk below the
reference (the 24" resolves to 0.88 by size alone, which the author rejected).

Separately, a nested Niri without a physical size produced exactly 100.00 dpi
from Qt's fabricated fallback, which resolved to a real factor and drew the
nested shell a quarter larger than the session beside it. Both of Qt's known
fallback densities, 96 and 100, are now refused to a hair's width.

`shellscale_test.cpp` was rewritten around the author's three monitors and
their own judgement on each, including a case stating the density measurement
directly: the two monitors are within 2 dpi of each other and still resolve to
different factors. CTest 18/18. `CELESTINA_SHELL_SCALE` is unchanged.
