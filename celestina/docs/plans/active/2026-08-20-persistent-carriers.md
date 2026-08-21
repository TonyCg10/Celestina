# SURF-1 — persistent carriers end the per-popup scene change

- **Opened:** 2026-08-20
- **Closed:** —
- **Plan ID:** persistent-carriers
- **Status:** active
- **Scope:** celestina
- **Implementation checkpoint:** SURF-1
- **Author-validation checkpoint:** VAL-SURF-1
- **Predecessor:** [BUBBLE-1 Melibea bubbles](../archive/2026-08-17-melibea-bubbles.md)
- **Successor:** none

## Hypothesis

The author measured (2026-08-18, recorded in `src/denseglass.cpp` beside
`parkedCompanionGraceMs`) that mapping and unmapping a whole-output surface is
a scene change the compositor answers by rebuilding that output's element
list, and that this churn is a slight physical flicker of exactly that
monitor — menus and the OSD alike, while Noctalia's persistently mapped
surfaces never flickered. The 2026-08-20 audit found the mitigation covers
only the dense-glass companions, and only for twenty seconds: every menu and
overlay still creates, maps, unmaps and destroys its own whole-output carrier
on every open, and any open spaced more than the grace re-maps the companions
too. If no shell surface is mapped or unmapped during ordinary popup use, the
churn class is gone regardless of which driver-level step turns it into the
visible blink.

## Tangible outcome

Opening and closing any panel menu, focused overlay, on-screen display or
toast changes only content inside surfaces that are already mapped. The
interactive carrier of each output is created once, parks invisibly with an
empty input region, no keyboard interactivity and a one-pixel effect region,
and every open is a content swap plus an input/keyboard/effect-region update
on that same surface. The dense-glass companions stop unparking on a timer:
they stay parked until Niri reports a fullscreen window on that output, which
is the one tenant the park was yielding direct scanout to.

## Scope

- **SURF-1-A — persistent interactive carriers.** One pre-mapped whole-output
  carrier per output for the panel-menu route and one for the focused-overlay
  route, owned by the existing surface lifecycle (`PanelMenuSurface`,
  `OverlaySurface`, `surfacemanager`). Menu and overlay content loads inside
  the carrier instead of being its own mapped window. Parked state: hidden
  content, empty input region, `KeyboardInteractivityNone`, no activation,
  one-pixel effect region so the rule's pipeline stays warm without the
  resting-companion saturation of 2026-08-15. Open state: content shown at
  the requested output position, input region following the carrier,
  `KeyboardInteractivityOnDemand` and activation requested per open, blur
  region following the published glass as today. Compositor dismissal hides
  and parks rather than destroys.
- **SURF-1-B — quiet surfaces join the same parking.** The on-screen display
  and toast carriers park mapped between bursts with the same one-pixel
  region, keeping their never-focusable contract.
- **SURF-1-C — event-driven companion unpark.** Replace the twenty-second
  `parkedCompanionGraceMs` sweep with a subscription to the existing Niri
  client's window state: companions (and the SURF-1-A carriers) unmap only
  while a fullscreen window occupies their output, and return once it leaves.

- **SURF-1-D — the popup family earns its parking.** The live exercise showed
  the nine indicator menus stand on three bases, and only SoftCard's family
  keeps its whole visual lifecycle in the shared revive seam; the popup-backed
  SoftMenu family rides its Popup's own open and aboutToHide, which nothing
  replays on a resumed window, so it was excluded from parking by the
  `carrierReusable` capability. This unit teaches SoftMenu the missing half:
  a park closes its popup without announcing a dismissal, and a resume
  reopens it after the attachment is re-established, so the reveal flows
  through the popup's own gates exactly as a fresh open. Wallpaper stays
  hard-closing: moving it onto SoftCard is its own cleanup, not this unit.

## Exclusions

- No change to placement, dismissal semantics, focus return, glass anatomy,
  animation or any other author-validated behavior; this unit moves surface
  lifetime, not looks.
- No compositor patch change; `packaging/niri/` stays as delivered.
- No investigation of the driver-level step (amdgpu full updates, DET
  reallocation) that converts the scene change into the blink. If the churn
  removal does not end the flicker, that investigation is a new unit with its
  own evidence, not an extension of this one.
- No change to the panel, lock, polkit prompt or wallpaper surfaces: they are
  already persistent.
- The blur teardown ordering contract (withdraw before hide, niri #3660
  class) and the layer-shell no-zero-size-on-unopposed-axis contract are
  load-bearing and must be preserved, not simplified away.

## Build order

1. Teach the carrier types a parked state and a reuse path under headless
   tests: park/open/park cycles, input-region and keyboard transitions,
   dismissal parking, and the retiring-window guards that exist today.
2. Move panel-menu and overlay content creation inside the persistent
   carriers (`PanelMenuController`, `OverlayController`), keeping every
   controller-facing signal (`dismissed`, toggle semantics) unchanged.
3. Convert the quiet OSD and toast carriers to the same parked lifecycle.
4. Replace the companion grace sweep with the Niri fullscreen subscription
   shared by SURF-1-A carriers.
5. Exercise the slice in the nested session (open/close bursts, spaced opens,
   outside-click and Escape dismissal, keyboard focus return, multi-output),
   then build, verify and deploy the production bundle.

## Implementation exit

- Headless tests prove a carrier parks with an empty input region and no
  keyboard interactivity, opens with both restored, survives compositor
  dismissal parked rather than destroyed, and never calls a blur withdraw
  after its surface hid.
- Tests prove menu and overlay opens reuse the mapped carrier — no new
  `wl_surface` per open on the popup routes — and that spaced opens do not
  re-map companions while no fullscreen window is present.
- The nested session proves every existing dismissal, focus and placement
  behavior unchanged, including the one-click close and focus return the
  current whole-output coverage exists for.
- The common architecture guard, Rust format/Clippy/tests, QML lint, affected
  CTest coverage and `scripts/complete-production.sh` pass against the exact
  deployed bundle.
- The physical flicker itself is author-eyes-only and stays in `VAL-SURF-1`:
  spaced menu opens on the live session, compared against the 2026-08-18
  observation.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Intended change | Diffstat | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| SURF-1-A | `celestina:` | active | `src/panelmenusurface.*`, `src/overlaysurface.*`, `src/surfacemanager.*`, `src/panelmenucontroller.*`, `src/overlaycontroller.*` | Persistent parked interactive carriers reused across opens | — | [nest exercise](../../evidence/2026-08-20-persistent-carriers-nest-exercise.md) | `VAL-SURF-1` |
| SURF-1-B | `celestina:` | active | `src/osdcontroller.*`, `src/toastcontroller.*` | Quiet surfaces park mapped between bursts | — | [nest exercise](../../evidence/2026-08-20-persistent-carriers-nest-exercise.md) | `VAL-SURF-1` |
| SURF-1-C | `celestina:` | active | `src/denseglass.*`, `src/niriclient.*`, `src/niri_adapter.rs` | Companion unpark driven by Niri fullscreen state instead of a timer | — | [nest exercise](../../evidence/2026-08-20-persistent-carriers-nest-exercise.md) | `VAL-SURF-1` |
| SURF-1-D | `celestina:` | active | `qml/SoftMenu.qml`, `qml/AnchoredMenu.qml`, `src/softclose.h`, `src/panelmenucontroller.cpp` | The popup-backed menus park by closing their popup silently and resume by replaying its open | — | — | `VAL-SURF-1` |
