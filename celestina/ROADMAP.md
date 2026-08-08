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
| LVR-3 | complete | Correct late provider insertion and provider lifecycle defects exposed during the GPU-loss audit |
| AUD-1 | complete | Static-audit hardening was absorbed by LVR-3-B and its follow-up corrections; residual findings remain recorded separately |
| UX-1 | complete | Give the network and Bluetooth indicators direct, truthful menus for their devices and actions |
| R6 | conditional | First-party lock starts only if SHELL-D2 is applied |
| R8 | complete | Reversible Noctalia removal; Polkit/dock slices remain conditional |
| R9 | conditional | Keep the independent greeter unless a demonstrated regression reopens it |

Recorded live observations and remaining author checks are status on the
validation lane, not implementation status.

## AUD-1 — Static audit hardening (complete)

**Outcome:** no session-menu verb can crash the panel, no producer text can
freeze or stale the provider frame, an unclean helper death cannot overlap
automatic DDC work, and a hostile peer cannot hang, grow or misdirect the
shell's channels.

This checkpoint records the defects from the
[2026-08-05 static shell audit](docs/evidence/2026-08-05-static-shell-audit.md).
The implementation was absorbed by `LVR-3-B` in 0.6.4 and tightened by
`LVR-3-C` through `LVR-3-G`; the original decomposition below describes
delivered coverage, not pending implementation.

Delivered coverage:

- **AUD-1-A — In-process session refusals stop crashing the panel.** Guard
  every `sendErrorReply` and `QDBusContext` access in `ShellService` behind
  `calledFromDBus()`; an in-process caller receives the same refusal as a
  failed outcome through the existing return/`commandOutcome` path, so the
  session menu shows the refusal it was designed to show. Bound the hostile
  verb text reflected into error replies. Regression: invoking `suspend`,
  `lock`, an unknown verb and an adapterless `log-out` in-process completes
  without a crash and reports failure; the D-Bus reply path is unchanged.
- **AUD-1-B — One text bound across the frame pipeline.** Bound array-row
  strings in `Snapshot::publish` in the same unit the host counts (UTF-16
  code units), with the row limit owned by `celestina-shell-core` and merely
  revalidated by the host; make the notification body bound fit the row bound
  (or raise the host bound deliberately, in one place); truncate media,
  launcher and notification row text at publish; cap the outbound frame line
  size in `SharedWriter::emit` so an oversized provider degrades alone instead
  of invalidating the channel; refuse oversized host-to-helper command lines
  in `sendCommand` by returning no request id. Regressions: an 800-character
  body, emoji-dense text at the boundary and an oversized `.desktop` name all
  publish bounded and never invalidate a frame.
- **AUD-1-C — An unclean helper death cannot overlap DDC.** In
  `ShellProvidersClient::helperError`, escalate TERM-then-KILL instead of
  immediate SIGKILL; after any unclean helper exit, delay the first restart by
  at least the bounded DDC child's worst case so an orphan cannot coexist with
  the replacement's `ddcutil detect`. Make the `sessionholds` thread observe
  shutdown and be joined; run `release_all` on every helper exit path
  including early initialization failures; make `Held` kill and reap its child
  on a `try_wait` error exactly as `tools.rs` does; stop reusing detect-time
  `ddcutil` display numbers across output changes so a brightness write cannot
  target a renumbered monitor. Regressions: process regressions for restart
  spacing after an unclean exit and for hold release on early-init failure.
- **AUD-1-D — Clipboard channel survives hostile peers and files.** Give the
  selection pipe read a deadline like `pump` already has, keeping the size
  bound; re-apply `is_recordable` and a total size bound when loading the
  persisted history and bound the state-file read; resolve the never-arriving
  self-echo edge so one real copy cannot be silently swallowed. Regressions: a
  stuck fake source times out without wedging the thread; a corrupt oversized
  history file loads bounded.
- **AUD-1-E — Producer text renders inert.** Set `textFormat: Text.PlainText`
  on every `Text` element that renders producer text in the toast and
  notification surfaces; compose accessibility names without chained `.arg`
  re-substitution; watch for `NameLost` after the notifications claim and
  withdraw the provider truthfully. Regressions: a markup body renders
  literally; an offscreen name loss publishes absence.
- **AUD-1-F — The late-insertion correction covers every surface.** Route the
  provider reads of `ControlCentre`, `NotificationCenter`, `LauncherOverlay`
  and `ClipboardOverlay` through the same revision-coupled access `Panel.qml`
  uses — one shared access point, not four copies. Regression: a provider key
  inserted while each overlay is open becomes visible, with `weather` as the
  canonical case.
- **AUD-1-G — The Niri channel is bounded and expires.** Bound title, label
  and output-name lengths and the workspace count in the adapter before emit,
  with the same `bounded` treatment reasons already get; sweep screenshot and
  action pendings with a deadline in `NiriClient::expireRequests`; give the
  action worker's socket a read deadline; refuse oversized outbound command
  lines on this channel as in AUD-1-B. Regressions: a giant window title
  yields a bounded snapshot; an unanswered action expires as failed.
- **AUD-1-H — The tray cannot be grown or misdirected by peers.** Bound
  registration count and id length in the watcher service; disconnect the
  per-item signal matches on unregister and teardown; drop stale `GetAll`
  replies for items already unregistered; correct the vanished-owner cleanup
  to use what `take` returned; key property refresh by registration so
  well-known-name items update; bound the internal read/icon maps; clear the
  pending tray-menu target once its answer is consumed. Regression: a
  register/unregister churn loop leaves no residual state and a
  well-known-name item still updates.

One medium residual remains explicit: after the notification helper acquires
`org.freedesktop.Notifications`, it does not observe a later `NameLost` and
withdraw its published state. The remaining low findings — notification-id
wrap, transient `GetLayout` allocation, GUI-thread icon decode and the busless
single-instance lapse — stay recorded in the audit. None is silently folded
into UX-1; each needs a future corrective unit if prioritized.

## LVR-3 — Late provider insertion and safe provider lifecycle

**Outcome:** a provider added to a later frame of the first helper generation
becomes visible without restarting that helper, and a rejected or terminating
host cannot start, overlap or abandon an automatic DDC operation.

The 0.6.2 live rerun proved that Firefox, `playerctl` and the Rust media
provider were healthy: an isolated helper published media immediately, while
the original host showed it only after replacing its helper. The bounded work
is recorded in
[the archived LVR-3 plan](docs/plans/archive/2026-08-05-late-provider-insertion.md).

The separate GPU-loss audit found two confirmed PCIe device-loss boots after
Celestina-shaped DDC activity and concrete process-lifecycle defects in the
shell. It did not prove causation. The author authorized source and record
corrections during a long Noctalia-only observation, then ended that hold and
completed repeated controlled transitions without recurrence. The evidence
boundaries are recorded in the
[system audit](docs/evidence/2026-08-05-gpu-loss-system-audit.md) and
[Celestina lifecycle record](docs/evidence/2026-08-05-ddc-process-lifecycle.md).

LVR-3 closed on 2026-08-07 after the corrected first-generation media, tray,
Bluetooth retention, output-triggered DDC discovery and clean
Noctalia-to-Celestina-to-Noctalia lifecycle all passed live. The Wi-Fi reading
remained present throughout the exercised session; a deliberate offline test
was not safe in that network layout and remains explicitly deferred rather
than inferred.

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

## UX-1 — Network and Bluetooth indicator menus (complete)

**Outcome:** each panel indicator opens a keyboard- and pointer-accessible menu
that shows bounded provider-owned state and exposes only actions whose result is
confirmed by a later provider reading.

The delivered implementation order, exclusions and exit checks are in
[the UX-1 plan](docs/plans/archive/2026-08-07-network-bluetooth-indicator-menus.md).
This checkpoint does not add Wi-Fi credential handling, Bluetooth pairing,
radio discovery policy or a second polling/runtime path.

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
