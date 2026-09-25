<!-- language-contract: product-copy — the Spanish below is product copy quoted as string literals -->
# Evidence: 2026-09-25 the rows that slid while the heading folded

- **Date:** 2026-09-25
- **Scope:** `SID-B1-D`, a direct bug fix; no plan is open (`SID-B1` closed as
  `1.6.0` on 2026-09-23). Continues the accepted limitation `1.6.0`'s own
  evidence recorded: see
  [the heading corrections](2026-09-22-heading-corrections.md), "A fix that
  was wrong, and was reverted" and its known-and-accepted note.
- **Environment:** Arch-derived Linux, Qt 6.9, `cargo` stable, release
  profile. The author ran the deployed binary on their own session.
- **Artifact:** `siderita/target/release/siderita`

## The defect

The content frame the rows sit in only moves while the heading is expanding
or collapsing back (`HeadingScroll.travel < 0`); it is static the rest of the
time. `contentTopInset` compensates for that movement *exactly at* `contentY
== originY` (resting, fully scrolled up) — the arithmetic cancels there. Away
from that exact point, `view.contentY` was moving at the wheel's raw,
per-notch rate while the frame moved at the much slower rate the heading's own
span sets (`D / expandSpan`, roughly a quarter as fast). The mismatch is what
the author saw as the rows sliding.

## What changed

`FolderWheelHandler.qml` only. While `heading.travel < 0`, a wheel notch spends
its whole delta on the heading and does not also scroll the listing; the
listing is pinned to `view.originY` by a handler on `onTopMarginChanged`,
which only ever fires during exactly this span (retiring never moves the
frame, confirmed: `compactProgress` is pinned at 1 throughout that whole
range). That handler follows a value already driven by the one clock that
exists here — `HeadingScroll`'s own `glide` — so this is not a second
animation on `contentY`, which is the mistake that broke scrolling entirely
during the first attempt at this fix on `SID-B1-C`.

**Accepted behavioural change:** while the heading is expanding or
collapsing, scrolling and that motion are no longer simultaneous — the whole
notch goes to the heading until it is fully compact again. Before, they moved
together at mismatched rates, which is what produced the slide. There is
nothing above row 0 to reveal during this span, so nothing is lost.

**Accepted imprecision:** the one notch that crosses back from expanding to
compact is entirely spent on the heading, even though a small remainder of it
could in principle have started scrolling too. This gives up at most one
notch's worth of scroll distance at that exact crossing — self-correcting on
the next notch, and not the visible defect this fix addresses.

## Procedure

1. `scripts/qml-tests.sh` — four new functions (`test_p`–`test_s`) written
   before the fix, confirmed to fail without it (verified by hand: the
   implementation file was stashed and the suite re-run, all four failed with
   the expected messages, then restored byte-identical).
2. `scripts/check-language-contract.py`, `scripts/check-architecture-contract.sh`.
3. `scripts/build-production.sh`, `scripts/verify-production.sh`,
   `scripts/deploy-production.sh`.
4. The author tested the deployed `1.6.1` on the real session and confirmed
   the rows no longer slide while the heading folds.

## Result

```
scripts/qml-tests.sh
Totals: 149 passed, 0 failed, 0 skipped, 0 blacklisted
```

```
scripts/verify-production.sh   manifest: siderita/target/production-artifact.toml (verified)
smoke                          OK — binario vivo 8 s, sin errores QML, sin auto-bindings
```

Author's report, 2026-09-25: tested and confirmed working.

## Limits

`qmltestrunner` delivers synthetic wheel events on `offscreen` and cannot
measure feel; this defect and its fix were both found and confirmed by a
person scrolling. Kinetic flicks, the scroll bar and keyboard paging still
move the listing without moving the heading — unaffected by this fix, and
already recorded as a limit in the `1.6.0` evidence.

## Not covered here

Appearance, glass and reduced motion. No window was opened by an agent on the
author's session; the author ran the deployed binary themselves.
