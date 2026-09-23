<!-- language-contract: product-copy — the Spanish below is product copy quoted as string literals -->
# Evidence: 2026-09-22 what testing the heading found

- **Date:** 2026-09-22
- **Scope:** `SID-B1-C`; plan
  [bottom-chrome-and-notices](../plans/archive/2026-09-22-bottom-chrome-and-notices.md)
- **Environment:** Arch-derived Linux, Qt 6.9, `cargo` stable, release profile.
  The author tested the deployed binary on their own session and recorded it;
  the frames quoted below come from those recordings.
- **Artifact:** `siderita/target/release/siderita`

## The defects

Four, all found by the author using the heading `SID-B1-A` delivered, and all
of them mine.

**1 · The transitions were hard.** Removing the `Behavior`s on the two
progresses was justified as "the gesture is the clock" — true for a touchpad,
wrong for a wheel. A notch is a jump, and the listing that notch scrolls is
tweened over `motionNormal`, so the heading snapped while the rows glided.

**2 · The detailed phase was shorter than one notch.** `compWheelStep` is
`rowHeight * 2` = **108px**; the expanded-to-compact range was the heading's
own height difference, **56px** — and only 38px in a folder with no dates. A
single notch crossed the whole phase, spent what was left starting to retire
the title, and dragged the listing a full notch on the way. Tying a scroll
distance to a pixel height was the mistake.

**3 · The title came back after one notch.** Recorded: the compact title fully
restored while the listing was still on `.eclipse` / `.gradle`, hundreds of
rows down. With a 162px fade and a 108px notch, two notches upward anywhere in
a folder brought the whole heading back.

**4 · The gesture died while the heading animated.** The title returning moves
the top bar, which moves the content frame, which changes the list's top
margin — every frame of the return. `onTopMarginChanged` called `resetTarget()`,
which stops the wheel's tween. So the scroll stopped dead for the length of the
animation. The author: "el scroll no debe detenerse por nada".

## What changed

**The travel is tweened, on the same clock as the listing.** `HeadingScroll`
animates `travel` over `motionNormal` with `easeStandard` — the wheel
animation's own duration and curve — so the two arrive together. Pixel deltas
are still applied straight through: a touchpad is already smooth and tweening
it would put the heading behind the finger.

**Both phases are scroll distances now**, sized in wheel notches rather than
borrowed from the heading's pixels: `expandSpan` two and a half notches,
`retireSpan` one and a half. A folder without dates behaves like one with them.

**The travel keeps a bounded credit past the fade.** `returnDelay` (two notches)
is accumulated once the title is already gone and must be spent before it
starts returning, so coming back is asked for rather than stumbled into — and
because it is bounded, returning never costs more than those two notches however
far down the person went.

**A geometry change no longer cancels a gesture in flight.** `rebound()`
replaces `resetTarget()` on the three geometry signals and intervenes only when
the destination has actually stopped being legal, which a growing top margin
never does.

## A fix that was wrong, and was reverted

Between the second and third rounds the author reported that all scrolling had
broken: the heading returned by itself mid-listing and the view jumped back to
the top.

That was a compensation I added for something real — as the heading folds the
content frame rises and the rows rise with it, which the previous handler's own
comment describes and which it paid for by consuming the gesture whole. The
implementation was wrong twice over: it assigned `contentY` on every frame while
the wheel's tween was also animating `contentY`, and it restarted that tween on
every margin change. Two clocks on one property, which is the rule this
checkpoint quotes and that commit broke.

It was reverted in full. The frame's rise during a fold is **not** compensated
today; what remains is a slide of the rows while the heading closes, recorded
here rather than left for someone to rediscover. Doing it properly means the
scroll and the heading sharing one clock instead of owning separate animations,
which is a restructuring of the handler and not a patch.

## Procedure

1. `scripts/qml-tests.sh` — one function per defect, written before the fix.
2. `scripts/check-language-contract.py`,
   `scripts/check-architecture-contract.sh`,
   `scripts/check-documentation-contract.sh`.
3. `scripts/build-production.sh`, `scripts/verify-production.sh`, and
   `scripts/deploy-production.sh` for each round the author tested.

## Result

```
scripts/qml-tests.sh
Totals: 145 passed, 0 failed, 0 skipped, 0 blacklisted
```

The four that name the defects:

- `test_h_a_wheel_notch_tweens_the_heading_as_it_tweens_the_listing`
- `test_f_scrolling_down_retires_it_without_losing_a_notch` — which also caught
  a defect the author had not seen: rapid notches lost distance, because the
  advance accumulated on the drawn value instead of on the destination.
- `test_n_coming_back_spends_the_credit_before_the_title_returns`
- `test_o_geometry_moving_does_not_stop_the_gesture`

```
scripts/verify-production.sh   manifest: siderita/target/production-artifact.toml (verified)
smoke                          OK — binario vivo 8 s, sin errores QML, sin auto-bindings
```

## Limits

`qmltestrunner` delivers synthetic wheel events on `offscreen`, so none of this
measures feel; every one of these four defects was found by a person scrolling,
not by a test. The three distances are tuned by eye and are plain numbers in
`FolderView.qml`. Kinetic flicks, the scroll bar and keyboard paging still move
the listing without moving the heading. The uncompensated slide of the rows
while the heading folds is known and unfixed.

## Not covered here

Appearance, glass and reduced motion. No window was opened by an agent on the
author's session; the author ran the deployed binary themselves.
