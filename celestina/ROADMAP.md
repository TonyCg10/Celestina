# Celestina implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** R3

This roadmap contains only work an agent can implement and verify. Real Niri,
hardware, visual and assistive-technology checks live in
[VALIDATION.md](VALIDATION.md) and never keep an implementation milestone open.
The detailed R0-R2 record is preserved in
[the historical roadmap](docs/history/roadmap-through-2026-08-03.md).

## Current direction

Celestina replaces the responsibilities currently supplied by Noctalia one
reversible bundle at a time. The parity target is the author's lived session,
not every upstream Noctalia feature. Mature external tools remain valid parts
of the design when they provide the narrow capability the shell needs.

| Phase | Implementation | Outcome |
|---|---|---|
| S0/S1 | complete | Per-output layer-shell panel and truthful Niri state/control |
| R0 | complete | Shared surface recipe, popup path and versioned session command channel |
| R1 | complete | Daily bar providers, DDC, media, audio and complete SNI host/watcher path |
| R2 | complete | Keyboard launcher and shell-owned clipboard history overlays |
| R3 | active | OSD, night light, caffeine/idle, DPMS and fail-closed session verbs |
| R4-R5, R7 | planned | Notifications, control center and session look |
| R6 | conditional | First-party lock starts only if SHELL-D2 is applied |
| R8 | partially planned | Reversible Noctalia removal; Polkit/dock slices remain conditional |
| R9 | conditional | Keep the independent greeter unless a demonstrated regression reopens it |

Recorded live observations and remaining author checks are status on the
validation lane, not implementation status.

## R3 — Session verbs

**Outcome:** keyboard-driven session actions enter through
`org.celestina.Shell1`, expose confirmed or failed state, and can raise a
truthful OSD without depending on a Noctalia command path.

- [ ] Add typed, bounded volume, brightness, DPMS and session verbs to the
      shell command vocabulary and cover success, refusal and provider loss.
- [ ] Add the top-right OSD surface using the existing `LayerSurfaceSpec`,
      `CelestinaSlider`/toast contract and the reduced-motion path.
- [ ] Compose fixed 2700 K night light through an owned, bounded `wlsunset`
      lifecycle that releases gamma on normal shutdown and failure.
- [ ] Add shell-owned caffeine/idle-inhibit state; keep the idle chain disabled
      by default until the author explicitly enables it.
- [ ] Compose DPMS through Niri and expose a fail-closed lock-and-suspend
      contract that refuses while no approved locker provider exists.
- [ ] Supply exact opt-in configuration and rollback instructions without
      mutating the author's live Niri configuration.
- [ ] Run the automated exit in
      [the active R3 plan](docs/plans/active/2026-08-03-r3-session-verbs.md) and let
      `scripts/complete-production.sh` build the release once, verify those
      exact bytes and update the on-disk bundle without a second build or
      replacement of the live session.

The concrete locker integration is not part of the active R3 plan while
[SHELL-D1](docs/discussions/2026-08-03-external-locker.md) remains open. Applying
that discussion creates a separate implementation unit; it is not appended to
R3 by assumption.

R3 closes when these implementation items and their automated evidence are
complete. Its real-session checks then proceed independently under `VAL-R3`.

## R4 — Notifications

- [ ] Implement the freedesktop notification state machine in
      `celestina-shell-core`, including replacement, expiry, actions and caps.
- [ ] Add the bounded bus producer and hostile-image handling to the aggregate
      provider runtime.
- [ ] Add compact toasts, capped history, DND and the unread panel indicator.
- [ ] Prove producer/consumer compatibility automatically, including
      Magnetita's `Notify`, replacement and close flows.

## R5 — Control center, session menu, weather and calendar

- [ ] Implement the multi-provider write surface with confirmed network,
      Bluetooth, night-light, caffeine, DND, power, audio and brightness state.
- [ ] Implement typed session actions with visible request outcomes.
- [ ] Add bounded Open-Meteo policy/cache and a local calendar month view.
- [ ] Persist settings atomically before publishing them.

## R6 — Conditional first-party lock and idle

This is not planned implementation while
[SHELL-D2](docs/discussions/2026-08-03-first-party-session-lock.md) remains open.
If that discussion is applied with explicit authorization, a new roadmap
checkpoint and plan may define the threat model and exit tests. The possible
scope is retained here only to preserve product direction:

- an `ext-session-lock` and PAM path that remains locked on process failure and
  covers output hotplug;
- a logind sleep inhibitor and deterministic lock lifecycle.

## R7 — Wallpaper and session look

- [ ] Add per-output wallpaper surfaces with truthful fallback and reduced
      motion.
- [ ] Serve the `Settings` portal values owned by the shell.
- [ ] Generate the Niri colour include from the sealed theme contract.

## R8 — Polkit, optional dock and Noctalia departure

- [ ] Supply reversible Noctalia removal and rollback tooling without applying
      it to the live session automatically.

Polkit integration is not an R8 implementation item until
[SHELL-D3](docs/discussions/2026-08-03-polkit-agent.md) is applied. Any
first-party agent remains a separate security-sensitive authorization.

The dock is not an R8 implementation item unless
[SHELL-D4](docs/discussions/2026-08-03-running-app-dock.md) concludes that it is
retained and that conclusion is applied through a new bounded unit.

## R9 — Greeter

No implementation is planned. `noctalia-greeter` is an independent greetd
package and remains in place unless observed failures justify a replacement.

## Beyond replacement

The workspace overview remains a conditional post-R8 feature. It starts only
after a new active plan defines the Niri window snapshot extension and an
honest icon/title layout; Wayland does not provide live thumbnails of foreign
windows.

## Implementation exit rule

An item becomes complete only with code, same-change automated tests, updated
contracts and the deployed bundle that `scripts/complete-production.sh`
produces. A build is not compositor, hardware, visual or
accessibility evidence. Those results are
recorded only in [VALIDATION.md](VALIDATION.md); a failed validation creates a
new corrective implementation item instead of reopening the completed one.
