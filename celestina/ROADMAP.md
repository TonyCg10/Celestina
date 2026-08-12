# Celestina implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** PANEL-1

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
| WSG-1 | complete | Keep a workspace's monitor grouping legible after that monitor is switched off |
| DIAG-1 | complete | Make the seconds before a freeze reconstructable, without recording anything private |
| WMAP-1 | complete | Show what a workspace holds, as its real layout, without focusing it |
| PANEL-1 | active | Replace the hard panel plate with borderless compositor glass and route contextual content through the canonical shared glass material |
| UX-2 | planned | Establish and then implement one coherent shell-wide visual and interaction language after SHELL-D5 is applied |
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

## WSG-1 — Workspace groups survive their monitor (complete)

**Outcome:** a strip carrying workspaces from more than one monitor shows the
group that has the focus in full and every other group as one capsule, so
switching two monitors off stops turning fifteen workspaces into fifteen equal
pills in a row.

Niri publishes the output a workspace is on and never the one it was configured
for, so a displaced workspace is indistinguishable from a native one. The
grouping is therefore remembered from a frame that could see it, or declared by
the author, and an observation that cannot tell the two apart teaches nothing.
The bounded scope, exclusions and unit boundaries are in
[the archived WSG-1 plan](docs/plans/archive/2026-08-08-workspace-monitor-groups.md).
It closed in celestina 0.8.0 on the
[delivery evidence](docs/evidence/2026-08-08-workspace-monitor-groups.md): the
canonical production exit built, verified and deployed those bytes without
activating the session. The live capsule, its assistive route and the moment the
memory is first taught are `VAL-WSG-1` and remain the author's to run.

This checkpoint adds no token, shared component or anatomy, and does not
pre-empt SHELL-D5. A strip whose workspaces all belong to one monitor renders
exactly as it does today.

## DIAG-1 — A journal that survives the freeze (active)

**Outcome:** every Celestina process writes a structured, bounded, always-on
JSONL journal correlated by one `run_id`, so the seconds before a physical
freeze can be reconstructed from the disk rather than from a terminal buffer.

The GPU has been lost from the PCIe bus more than once while this shell was
running, most recently inside a **nested Niri session**. That nest separated the
surfaces and shared everything that matters: the GPU, VCN, the DDC/I²C buses and
the session bus. The handover is therefore not a necessary condition.

**This checkpoint asserts nothing about cause.** Coincidence is not causation and
the journal cannot establish either. What it fixes is a defect of this shell's
own: after a reset, nobody can say what Celestina did. The bounded scope,
exclusions, event classes and the deliberate omission of every private value are
in [the DIAG-1 plan](docs/plans/archive/2026-08-08-diagnostic-journal.md).

Nothing in this checkpoint investigates, touches or changes the GPU, DDC
behaviour, amdgpu, the kernel, Niri, systemd, Noctalia or Wi-Fi.

## WMAP-1 — The workspace window map (complete)

**Outcome:** a collapsed capsule stops being opaque. Clicking it opens a card
showing that monitor group's workspaces as the layouts they really are — real
columns, real rows, real proportions — with each window's icon, title and
application id, so the person can see what is in a workspace without focusing
it. A pill keeps its one-gesture focus and offers the same map on hover.

There are no window previews and none are proposed. Wayland gives a client no
access to another client's buffers, Niri composites its own overview inside the
compositor, and its IPC exposes no window pixels — checked against the command
surface rather than assumed. What the compositor does publish is each window's
column, row and tile size, which is a truthful map rather than a stale picture.

The bounded scope, exclusions, settled interaction decisions and the recorded
risk in the hover route are in
[the WMAP-1 plan](docs/plans/archive/2026-08-08-workspace-window-map.md).

## PANEL-1 — Borderless glass panel (active)

**Outcome:** the panel has no hard full-width plate or shadow. One nearly
transparent `ContextualVeil` reaches edge-to-edge without outer margins and
owns one finite Niri compositor-blur region for the complete 40-pixel bar.
Information groups remain ordinary rounded `ContentSurface` capsules inset at
y=5 with height 30. They share the dense dark matte material and fixed
light/white foreground of contextual content cards but publish no compositor
region of their own. A panel-opened primary carrier uses the clicked control for
placement and the exact glyph inside it as the droplet membrane's mouth target.
The membrane is only the nearly transparent `ContextualVeil`, shaped as a drop
falling out of the bar: one narrow icon-proportional mouth clings to the bar
seam with a horizontal-tangent meniscus, narrows to its neck just below the
bar and swells concavely until it lands tangent on the menu body's flat top
edge, which keeps its ordinary rounded corners outside the swell. Travel,
icon/body reference scales and horizontal displacement determine its tension,
which only thins the neck. The opener keeps its ordinary hover circle while
its own surface remains open. Its `PanelPill` and
every dense `ContentSurface` remain ordinary rounded surfaces with unchanged
geometry and material. No capsule opens into the menu and no dense bridge
crosses the surface boundary. Live glyph-anchor tracking keeps the waist aligned when
tray/provider layout changes. The membrane neither repaints nor reblurs the
bar, and the veil exposes no outline, lit edge, apparent halo or elevation
shadow.

The author selected this bounded panel direction from live screenshots. It does
not apply the rest of UX-2; menu, overlay and provider work is limited to the
exact corrections declared in the active plan. Scope, order and evidence are in
[the PANEL-1 plan](docs/plans/active/2026-08-08-panel-glass-redesign.md).

`PANEL-1-K` welds the bar's own reading capsules to the screen's top edge,
with the centred clock alone held by a visibly elastic skin and every flanked
capsule keeping straight sides so nothing overlaps and no gap widens.
`PANEL-1-L` then makes what the shell draws the same physical size on every
output — one bounded factor per screen from its real density, applied as a
scene scale so no token or layout number moves — and stops the shell degrading
what it draws: a tray icon is rasterized once at a size that survives any
scale, every raster is asked for at the density it will be drawn at, glyph
strokes and panel reading weights thicken without any size changing, and three
provider-driven menus stop rebuilding their complete row list on every reading
tick.

`PANEL-1-J` gives that settled droplet its opening motion. One bounded
progress value drives the same geometry source: the body opens out of its own
mouth, the neck thins under flight tension, and an elastic recoil hauls the
landed body back toward its seam before letting it settle. The mouth stays
welded to the seam and the neck keeps a hard floor at every frame, so the drop
never detaches, and the carried content rides inside the drop rather than
waiting at the resting place. Reduced motion resolves the settled geometry
immediately.

`PANEL-1-I` gives the panel one finite region and gives only the contextual
membrane its matched painted path and sampled polygon. A real panel request
transports the clicked control and its exact glyph anchor separately; the body
follows the former while the membrane waist follows the latter. Its origin
remains independently fixed at `attachmentStartY == barHeight`. A tokened
tracker prevents an older retiring surface from clearing its successor and
remeasures live glyph geometry through the panel's global coordinate space
while that anchor or its ancestors move or resize, then publishes only the
output-local rectangle to the attached surface. It never mutates the panel or
changes the opener's capsule; the invoking control alone retains its normal
hover-circle fill for the lifetime of its surface lease.
Command and keybind routes keep their rounded floating geometry. The
workspace map now attaches with the same droplet from the exact workspace or
monitor dot that opened it, and the collapsed monitor group is one larger
dot without its former numbered capsule. A foreign tray child born from a row of the mapped
inventory now attaches the same droplet sideways: its surface sits flush
against the parent card, the membrane strip inside the child window is the
horizontal travel, and the mouth follows the invoking tile on the edge facing
the parent. The foreign menu's scrolled rows stay clipped inside its dark
body section beneath a pinned header, with no separate scroll bar. The immediately preceding whole-capsule
revision built and linted cleanly; its focused selection passed 4/4 with
208/208 QuickTest cases, the architecture and Style checks passed, and
Celestina's registered completion passed CTest 17/17 plus its release smoke
before deployment without session activation. That revision, the later
glyph-mouth revision with the same verified counts, and the earlier droplet and
narrow-connector runs are superseded. The first body-wide revision is also
superseded because its icon-scaled 9..11-pixel waist read as a straight
hourglass. The body-wide-edge, fluid body-proportional-waist revision that
followed it verified cleanly but the author rejected its live read as a
strange hourglass on 2026-08-11; it is superseded by the current droplet
contract, whose narrow mouth is the only geometry touching the bar seam.
The droplet revision passes its focused selection 4/4 and offscreen QuickTest
runner 211/211; its registered production completion and author screenshot
review are recorded in the milestone evidence. Those bytes shipped in
`a97eb55` as celestina 0.12.0.

`PANEL-1-J` then gives that settled droplet its motion: an attached surface
is born as a drop at its own seam and falls into place from one bounded
progress value on the same geometry source. The body's span and extent open
together out of the mouth while flight tension thins the neck and relaxes it
on landing; the mouth never scales and the neck keeps a hard floor, so the
drop is always under tension and never pinches off. Progress 1 is exactly the
settled geometry, content reveals as the body arrives, and reduced motion
resolves that settled shape with no animation. The author-run nested-Niri
scale matrix remains pending for both.
On 768p
the tall Control Centre keeps its complete membrane and a blur region disjoint
from the panel;
the output clips its last 36 pixels.
Reachable low-height overflow is not claimed by this prototype.

## UX-2 — Shell visual and interaction language (planned)

**Outcome:** the panel, overlays, context menus and future clock/date surface
read as one deliberate shell: clear hierarchy, purposeful iconography,
consistent geometry and motion, and predictable dismissal and menu-switching
behavior at every supported output scale.

Implementation is not active. [SHELL-D5](docs/discussions/2026-08-08-shell-visual-design.md)
owns the open product questions and must be applied through an accepted decision
before a UX-2 implementation plan exists. Until then no QML, style token,
provider, surface or product version change belongs to this checkpoint.

The discussion begins from observed needs rather than a predetermined mockup:

- visual hierarchy, iconography, density and spacing across every existing
  panel region, overlay and left- or right-click menu;
- one-click replacement of an already-open transient menu, outside-click and
  Escape dismissal, opener-relative placement and focus restoration;
- a clock/date surface combining richer calendar information, weather details
  and explicit location management without guessing the person's location;
- coherent empty, pending, failed, disabled, selected and focused states;
- reduced motion, contrast, assistive semantics and both current output scales.

Functional provider behavior already delivered by UX-1 is not redesigned by
assumption. Lock, Polkit and dock decisions remain under SHELL-D1 through
SHELL-D4.

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
