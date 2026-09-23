<!-- language-contract: product-copy — the Spanish below is product copy quoted as string literals -->
# Evidence: 2026-09-22 the heading follows the scroll

- **Date:** 2026-09-22
- **Scope:** `SID-B1-B`; plan
  [bottom-chrome-and-notices](../plans/archive/2026-09-22-bottom-chrome-and-notices.md)
- **Environment:** Arch-derived Linux, Qt 6.9, `cargo` stable, release profile.
  QML interaction under `qmltestrunner` (Qt6) on the `offscreen` platform; no
  window was opened on the author's session.
- **Artifact:** `siderita/target/release/siderita`

## The defect

The author: scrolling has detents — one to show the big heading, another to
hide it once you scroll far enough — and each needs a second scroll.

Reading `FolderWheelHandler` showed why, and it was worse than a threshold. A
gesture that crossed a detent was **consumed whole**:

```qml
if (collapseTravel >= collapseThreshold) {
    root.collapseRequested()
    event.accepted = true
    return      // this gesture changed the heading; that is all
}
```

The same shape guarded the retire threshold. So the notch that folded the
heading did not scroll the listing, the notch that took the title away did not
either, and expanding again needed a separate armed push after arriving at the
top. Three transitions, three gestures spent on nothing but the heading.

## What was measured

The drawing was already continuous: `FolderHeading` interpolates its height,
its opacity and its metadata lines on two reals, and `FolderView` interpolates
the content frame's `y` on one of them. Only the driver was discrete — two
booleans behind `collapseThreshold` (0.75 wheel steps) and `retireThreshold`
(1.5), each with its own travel accumulator.

The first design tried was to make those two reals a function of `contentY`.
It does not work, and the reason is structural rather than a bug:

```
contentY → compactProgress → contentFrameY → contentTopInset → topMargin
         → clamps contentY → …
```

`contentTopInset` is `max(0, expandedFrameY - contentFrameY)`, and because the
content frame rises as the heading folds, the list's top margin is **zero with
the heading expanded and largest with it compact**. A heading that read the
position it helps decide closes that loop.

## What changed

One number. `HeadingScroll` replaces `HeadingState`: travel runs from
`-expandedExtra` (the metadata block fully open) through zero (the compact
title, at rest) to `+retireSpan` (listing only), and the two progresses are a
plain function of it. `retireSpan` is `compWheelStep * 1.5` — the distance the
old threshold already asked for, so *how far* did not change, only that it is a
ramp rather than a step.

It is moved by the gesture, not by `contentY`, which is what keeps the loop
open. Every wheel delta advances the travel **and** scrolls the listing; no
branch returns early any more.

One rule survived the old machine: growing the metadata block is still
something asked for at the top. `advance(amount, atTop)` floors the travel at
zero unless the listing is already at its top, and `atTop` is read *before* the
listing moves, so the gesture that arrives does not also expand — but it is no
longer swallowed either.

Deleted with it: `HeadingState.qml`, the four signals on the wheel handler and
their four relays through both views, the four functions on `FolderView`,
`canReveal`, `revealArmed`, `maybeArmReveal`, both thresholds, both travel
accumulators, and the two `Behavior`s on the progresses — with a gesture-driven
value, an animation on top only runs behind the finger.

`FolderView` kept one verb, `foldHeading()`, for changing mode or location: it
clamps travel at zero, so it puts the metadata block away without bringing back
a title the person had scrolled off. Navigating to a new folder sets travel to
zero outright, which is what the compact resting state means.

## Procedure

1. `scripts/qml-tests.sh` — seven new functions in `tst_heading_scroll.qml`,
   replacing `tst_heading_retire.qml`, which asserted the detents.
2. `scripts/check-language-contract.py`,
   `scripts/check-architecture-contract.sh`,
   `scripts/check-documentation-contract.sh`.
3. `scripts/build-production.sh` and `scripts/verify-production.sh`.

## Result

```
scripts/qml-tests.sh
Totals: 140 passed, 0 failed, 0 skipped, 0 blacklisted
```

The two that name the defect:

- `test_e_one_notch_moves_the_heading_and_the_listing` — one notch must do
  both. It fails against the old handler by construction.
- `test_f_scrolling_down_retires_it_without_losing_a_notch` — four notches down
  leave the title gone *and* the listing four notches lower, where the old
  handler spent two of them on the heading.

```
scripts/build-production.sh    Finished `release` profile
scripts/verify-production.sh   manifest: siderita/target/production-artifact.toml (verified)
smoke                          OK — binario vivo 8 s, sin errores QML, sin auto-bindings
```

Two guards caught real mistakes on the way:

- The `x: x` scanner refused `heading: heading` inside the view delegates — the
  injected property shadows the outer id and resolves to itself. The object is
  now `headingScroll`. This is the exact trap `FolderView` already carries a
  comment about, and it is why that scanner exists.
- `scripts/architecture-baseline.tsv` required `FolderView.qml`'s row to fall
  from 745 to 744, and `scripts/qmllint-baseline.tsv`'s `siderita` row from 260
  to 259.

## Limits

`qmltestrunner` delivers synthetic wheel events on `offscreen`; it is not a
real wheel or a real touchpad on a real compositor, and the feel of a ramp is
exactly what a synthetic event cannot report. Kinetic flicks, dragging the
scroll bar and keyboard paging move the listing without moving the heading —
the travel is fed by the wheel handler alone, which is the price of keeping the
binding loop open and covers the two devices this machine has. Whether the
ramp's distance is right, and whether parking the metadata block still feels
deliberate, is `VAL-SID-15`.

## Not covered here

Appearance, glass and reduced motion. No window was opened on the author's
session.
