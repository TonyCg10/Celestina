# SURF-1-D delivery — the popup family's parking survives a fast toggle

- **Date:** 2026-08-22
- **Scope:** Celestina unit `SURF-1-D`
- **Artifact:** `qml/AnchoredMenu.qml`, `qml/SoftMenu.qml`, `src/softclose.h`,
  `src/panelmenucontroller.{h,cpp}`, `src/denseglass.cpp`,
  `src/panelblurcontroller.cpp`, `tests/indicatormenu_test.cpp`
- **Environment:** `ctest --test-dir celestina/build`, on the author's own
  machine; the canonical production build and verify ran clean and the bundle
  was activated on the live session, which the author drove directly
- **Plan:** [persistent carriers](../plans/active/2026-08-20-persistent-carriers.md)
- **Validation:** the author's own live exercise (2026-08-22), reported as "va
  bien" after opening and closing menus fast enough to have produced a ghost
  before this unit

## Procedure

SURF-1-D was accepted with the popup family's park/resume gates (`aboutToHide`
suppressed by `parkingForReuse`, the popup replayed by `reopenForReuse`) but
without exercising what a *second* toggle does while the first one's beat is
still running. Two agents audited the whole close/park/revive path
independently — one over the QML popup lifecycle, one over the C++ carrier and
blur/glass controllers — each without sight of the other's findings, and both
converged on the same shape: state that a fast toggle can leave outstanding
because the beat that owns it was interrupted.

Five races were confirmed against the code as staged, each with a concrete
interleaving, and closed:

1. **The closing fade never stopped.** `softCloseWindow`'s `QVariantAnimation`
   had no guard at all — not even `celestinaRetiring` — so a park landing
   mid-fade kept the animation writing opacity over the freshly zeroed
   content, and a revive landing mid-fade let the animation fade the
   carrier back out from under the person who had just reopened it. The
   fade is now named (`celestina-soft-close-fade`) and both park and revive
   stop it before setting the opacity they mean to hold.
2. **The dense-glass collapse outlived its retirement.** `DenseGlassAggregator::retire`'s
   80 ms collapse had no `celestinaRetiring` check either: a fast reopen saw
   its live shapes overwritten by the old menu's shrinking ones, then stripped
   outright when the collapse's `finished` fired. It now checks
   `celestinaRetiring` on every tick and at `finished`.
3. **`publishDenseSections` missed the parked state.** It refused only on
   `celestinaRetiring`, not `celestinaParked` — a queued reveal (`SoftMenu`'s
   `reopenForReuse` schedules its open with `Qt.callLater`) landing after a
   park published the settled sections onto the companion surfaces, standing
   as a bare slab over an output whose menu was resting. It now refuses on
   both.
4. **The same-kind reopen was swallowed for the whole 170 ms beat.**
   `m_openMenuKind` is only cleared inside `close()`, which a soft close does
   not reach until its finish callback — so a click on the same indicator
   during that window still read `sameAgain`, and `requestPopupDismissal`
   returned `celestinaRetiring == true`, so the toggle did neither open nor
   close. This is the "first click does nothing" defect the surrounding
   comment already names, resurrected through the retirement beat instead of
   the open path. `sameAgain` now also requires the carrier not be retiring.
5. **The tray child's soft close was pointer-guarded only.** Unlike the parent
   carrier's beat (`window`, `generation`), a fast row-to-row hover could
   retire one child and open the next at a recycled window address before the
   first beat's finish fired, closing the just-opened menu. It now carries its
   own generation counter, `m_trayChildGeneration`.

Two more were found and closed at the QML layer, both in the deferred
`Qt.callLater(menu.open())` that both the fresh open (`AnchoredMenu.onReady`)
and the resume (`SoftMenu.reopenForReuse`) use: neither re-checked
`parkingForReuse` at fire time, so a park landing between the queue and the
tick opened the popup inside the resting scene — leaving it visible on a
carrier believed dark, and leaving the *next* resume with no `aboutToShow` to
reveal through, because the popup was already open. Both call sites now
re-check the flag immediately before opening.

A regression, `aParkThatRacesTheQueuedOpenLeavesNoStrandedPopup`, reproduces
the exact interleaving: open, park, queue a resume, park again before its tick
fires, and confirm the popup never opens inside the resting scene — then
confirm the next honest resume still opens normally.

## Result

`ctest --test-dir celestina/build` passes all 25 registered suites, including
the new regression. `cargo fmt --check`, `cargo clippy --all-targets --
-D warnings` and the full Rust test suite pass. `qmllint-production.sh` passes.
The canonical production build and verify pass against the exact deployed
bundle, which the author activated on the live session and exercised directly,
including deliberately fast open/close bursts across several indicator kinds.

Three lower-confidence findings from the audit were recorded but not acted on,
because they describe no observable ghost today: `revealResumedWindow`'s retry
chain can abandon a reveal permanently if a park lands mid-retry and the
carrier is later resumed by a path that does not re-issue the reveal (no such
path exists yet); `publishHiddenAnchor`'s per-second anchor blank can in
principle land right after a resumed reveal and reproduce the tall-then-small
`blur.armed` pair from a different direction than the one 2026-08-21 already
fixed; and `passPanelStripThrough`'s 120 ms reapply closes over a stale
`bar`/`tracked` pair that is currently always the same window. Each is a
plausible future unit, not a unit of its own yet.

## Limits

Offscreen and headless only: the regression constructs the menu without a
compositor and drives its C++ slots directly. The physical live exercise —
opening and closing every indicator kind fast enough to have produced a ghost
before this unit, across all three monitors — is the author's own, reported
directly and not filed as a separate `VAL-*` case because it duplicates no
open validation checkpoint; `VAL-SURF-1`'s own physical-flicker procedure is
unaffected and remains pending until SURF-1-A/B/C land.
