# Celestina implementation roadmap

- **Status:** idle
- **Active implementation checkpoint:** none

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
| R3 | complete | OSD, night light, caffeine/idle, DPMS and fail-closed session verbs |
| R4 | complete | Freedesktop notification server, toasts, history and do-not-disturb |
| R5 | complete | Control centre, session menu, weather and calendar |
| R7 | complete | Wallpaper, portal values and the generated Niri colours |
| LVR-1 | complete | Correct the failures exposed by the 2026-08-04 live validation run |
| LVR-2 | complete | Correct the failures exposed by the 2026-08-05 follow-up run |
| R6 | conditional | First-party lock starts only if SHELL-D2 is applied |
| R8 | complete | Reversible Noctalia removal; Polkit/dock slices remain conditional |
| R9 | conditional | Keep the independent greeter unless a demonstrated regression reopens it |

Recorded live observations and remaining author checks are status on the
validation lane, not implementation status.

## LVR-2 — Live validation follow-up

**Outcome:** media is present on the first helper generation, overlays always
retain their Escape dismissal path, held children cannot survive their helper,
and the appearance-portal instructions describe the selection step a real Niri
session requires.

The author authorized and completed the bounded corrective implementation on
2026-08-05. Its scope and evidence are in
[the archived LVR-2 plan](docs/plans/archive/2026-08-05-live-validation-follow-up.md).
Screen lock, Polkit, Niri colour adoption and deferred assistive-technology
checks remain outside it.

## LVR-1 — Live validation remediation

**Outcome:** the live shell keeps valid media and unrelated provider readings
visible, remains dismissible in clipboard empty state, starts without the
recorded accessibility or application-id diagnostics, and presents complete
Spanish product copy.

This is a corrective checkpoint; it does not reopen or rewrite the completed
R1-R8 milestones. Its record is
[the archived remediation plan](docs/plans/archive/2026-08-04-live-validation-remediation.md).
The corrections landed in celestina 0.6.1; the live cases they answer are the
author's to run again, and none of them is passed until they do.

- [x] Reproduce the media absence — measured, not assumed: `playerctl` answers
      in 3-5 ms and the provider publishes a valid player, so the timeout
      hypothesis was wrong and the widget was being clipped off the panel by
      the workspace strip. Guard absent audio readings at the QML boundary.
- [x] Preserve clipboard dismissal after clearing and expose an accessible
      visible delete action (delivered in `LVR-1-A`).
- [x] Align the bounded notification action payload with the host decoder and
      isolate malformed provider state from unrelated readings (delivered in `LVR-1-A`).
- [x] Repair wallpaper accessibility attachment and deployed application
      identity (delivered in `LVR-1-A`).
- [x] Translate all exposed shell product copy into Spanish as complete
      surfaces (delivered in `LVR-1-A`).

The source observation, confirmed notification failure chain and unrun live
checks are recorded in
[the 2026-08-04 evidence](docs/evidence/2026-08-04-live-validation-failures.md).

## R3 — Session verbs

**Outcome:** keyboard-driven session actions enter through
`org.celestina.Shell1`, expose confirmed or failed state, and can raise a
truthful OSD without depending on a Noctalia command path.

- [x] Add typed, bounded volume, brightness, DPMS and session verbs to the
      shell command vocabulary and cover success, refusal and provider loss.
- [x] Add the top-right OSD surface using the existing `LayerSurfaceSpec` and
      the shared track/typography contract, driven by published readings and
      honouring the reduced-motion path. It draws a meter rather than a
      `CelestinaSlider`: the surface never takes a pointer or the keyboard, so
      offering a control it cannot accept would be a lie about what it is.
- [x] Compose fixed 2700 K night light through an owned, bounded `wlsunset`
      lifecycle that releases gamma on normal shutdown and failure.
- [x] Add shell-owned caffeine/idle-inhibit state; keep the idle chain disabled
      by default until the author explicitly enables it.
- [x] Compose DPMS through Niri and expose a fail-closed lock-and-suspend
      contract that refuses while no approved locker provider exists.
- [x] Supply exact opt-in configuration and rollback instructions without
      mutating the author's live Niri configuration.
- [x] Run the automated exit in
      [the archived R3 plan](docs/plans/archive/2026-08-03-r3-session-verbs.md) and let
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

**Outcome:** the shell serves `org.freedesktop.Notifications` when nothing else
owns it, shows a capped toast stack and history, and answers Magnetita's real
producer flow. It never takes the name from a server that is already running.

- [x] Implement the freedesktop notification state machine in
      `celestina-shell-core`, including replacement, expiry, actions and caps.
- [x] Add the bounded notification server and hostile-image handling to the
      aggregate provider runtime, claiming the bus name only when it is free.
- [x] Add compact toasts, capped history, DND and the unread panel indicator.
- [x] Prove producer/consumer compatibility automatically, including
      Magnetita's `Notify`, replacement and close flows.

R4 closed on the evidence in
[the archived R4 plan](docs/plans/archive/2026-08-04-r4-notifications.md). Real
toast appearance, the handover from Noctalia's server and over-the-air phone
notifications remain an independent `VAL-R4` run.

## R5 — Control center, session menu, weather and calendar

**Outcome:** one surface writes to every provider the panel already reads from,
showing what each provider reported rather than what was asked for, and the
settings behind it survive a restart because they were written durably first.

- [x] Implement the multi-provider write surface with confirmed network,
      Bluetooth, night-light, caffeine, DND, power, audio and brightness state.
- [x] Implement typed session actions with visible request outcomes.
- [x] Add bounded Open-Meteo policy/cache and a local calendar month view.
- [x] Persist settings atomically before publishing them.

R5 closes on the evidence in
[the archived R5 plan](docs/plans/archive/2026-08-04-r5-control-centre.md). Real
network and Bluetooth switching, a real weather location and appearance remain
an independent `VAL-R5` run.

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

**Outcome:** the look of this session has one source — the sealed theme — and
the wallpaper, the portal values and Niri's own colours are derived from it
rather than restated.

- [x] Add per-output wallpaper surfaces with truthful fallback and reduced
      motion.
- [x] Serve the `Settings` portal values owned by the shell.
- [x] Generate the Niri colour include from the sealed theme contract.

R7 closes on the evidence in
[the archived R7 plan](docs/plans/archive/2026-08-04-r7-session-look.md). Real
wallpaper appearance, hotplug on physical monitors and Niri drawing the
generated colours remain an independent `VAL-R7` run.

## R8 — Polkit, optional dock and Noctalia departure

- [x] Supply reversible Noctalia removal and rollback tooling without applying
      it to the live session automatically.

R8 closes on the evidence in
[the archived R8 plan](docs/plans/archive/2026-08-04-r8-noctalia-departure.md).
Actually removing Noctalia is `VAL-R8` and is the author's decision on their
own session.

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
