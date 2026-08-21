# Celestina implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** SURF-1

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
| PANEL-1 | complete | Replace the hard panel plate with borderless compositor glass and route contextual content through the canonical shared glass material |
| UX-2 | planned | Establish and then implement one coherent shell-wide visual and interaction language after SHELL-D5 is applied |
| R6 | complete | First-party `ext-session-lock` and deterministic lock-before-suspend, with `VAL-R6` still unrun |
| LOCK-1 | complete | Let the session recede behind its own blurred wallpaper instead of vanishing into an opaque slab, and uncover it continuously |
| LIVE-1 | complete | Make the shell survive and look right on the real session: the crash, the membrane that only reached the primary monitor, the missing connectivity indicators, and two providers that misread the machine |
| BUBBLE-1 | complete | Present Melibea's native minimized windows as a compact shell bubble group and accessible selector |
| SURF-1 | active | End the per-popup whole-output map/unmap churn with persistent parked carriers |
| R8 | complete | Reversible Noctalia removal and the first-party Polkit agent are delivered; live departure remains `VAL-R8` |
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
- [x] Compose fixed 2700 K night light through the aggregate provider's bounded
      Wayland gamma transition, which reaches identity before releasing gamma
      on normal shutdown and refuses unsupported compositor backends.
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

The concrete locker integration is not an R3 item. SHELL-D1 asked which
external locker to compose and is superseded by
[ADR 0004](docs/decisions/0004-first-party-session-lock.md): the shell owns the
lock itself, under R6, and R3's typed command and refusal path stay as they
are.

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

## R6 — First-party lock and idle

**Outcome:** this session can lock itself, stay locked through anything that
goes wrong, and never suspend before the lock is confirmed up — with no part
of Noctalia and no password verification of Celestina's own.

Authorized on 2026-08-14 and bounded by
[ADR 0004](docs/decisions/0004-first-party-session-lock.md), which is this
checkpoint's threat model and must be read before any of it is written.

- [x] An `ext-session-lock-v1` client that covers every output, creates a
      surface for outputs that arrive while locked, and treats every failure
      as "stay locked".
- [x] PAM verification in a separate short-lived process that holds no
      compositor state, reporting one verdict and never a passphrase.
- [x] A logind delay inhibitor that releases only on a confirmed active lock,
      so lock-and-suspend refuses rather than suspends unlocked.
- [x] The locked surface itself: time, prompt and failure state, and
      deliberately no session content.

R6 closes on the evidence in
[the archived R6 plan](docs/plans/archive/2026-08-14-first-party-session-lock.md).
Unlocking a real machine with a real passphrase, a real lid close and a real
suspend are `VAL-R6` and have not been run — the checkpoint releases on its
implementation exit, not on that.

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

## R8 — Polkit and Noctalia departure

- [x] Supply reversible Noctalia removal and rollback tooling without applying
      it to the live session automatically.

R8 closes on the evidence in
[the archived R8 plan](docs/plans/archive/2026-08-04-r8-noctalia-departure.md).
Actually removing Noctalia is `VAL-R8` and is the author's decision on their
own session.

Polkit integration is now an R8 implementation item, authorized on 2026-08-14
and bounded by
[ADR 0005](docs/decisions/0005-first-party-polkit-agent.md):

- [x] Register as this session's `org.freedesktop.PolicyKit1.AuthenticationAgent`.
- [x] Prompt on a dedicated surface that holds a keyboard grab, showing the
      action id, message and identity exactly as `polkitd` gave them.
- [x] Delegate every verification to `polkit-agent-helper-1` over its pipe;
      implement no PAM conversation and deny on every failure.

The implementation and its corrective follow-ups are delivered under
[the archived polkit plan](docs/plans/archive/2026-08-14-polkit-authentication-agent.md).
polkitd accepts one agent per session and Noctalia's own plugin currently
holds this one, so Celestina's registration is refused and says so rather than
fighting for the slot — which makes a real `pkexec` against a real password
part of the same `VAL-R8` as removing Noctalia, not a separate check.

The dock question is closed: [ADR 0003](docs/decisions/0003-no-running-app-dock.md)
decided against one. No dock slice is planned.

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
[the PANEL-1 plan](docs/plans/archive/2026-08-08-panel-glass-redesign.md).

`PANEL-1-S` puts the on-screen display and the notification toasts on the
shell's own glass, hangs them from the bar and gives the display a card file.

Both were painted on a `GlassCard`, which takes its material from an in-scene
capture — and an overlay window has no scene behind it, so each fell back to
its opaque tint and read as a solid plate over the desktop while the bar it
came from was transparent. Each card is now a `SoftMenuField` veil carrying
one dense `ContentSurface` section, and the toast buttons stop being the
shell's one direct style-control exception.

They appear at the top right, attached by the same drop membrane every menu
uses, the mouth on the panel icon of what they report — the volume,
microphone or brightness glyph for a reading, the notification bell for a
toast — resolved by the host without a click, in the same output-local shell
units a click would produce. Each yields the zone rather than paint over
something interactive already there: the display retreats to the bottom-right
corner, the toasts to the bottom centre, two different fallbacks so the
retreats cannot collide either, and each counts the other. A level changed
from inside its own open menu raises no display, and the notification centre
being open keeps the corner quiet the same way. The display's window takes
input only where its cards are and the attached toast window passes the strip
above the seam through to the panel, so the wheel that raised a card keeps
stepping the control under it.

The display is a file of live cards rather than one card overwritten: a volume
change while a brightness card is still up slides a second card in front, each
kind carries its own clock, the cards behind peek out under the front one, and
hovering one raises it. It lives on two persistent surfaces — the attached
home and the fallback corner — so a menu opening over it moves the file with a
property push instead of a remap that would arrive invisible.

Three lifecycle defects were found and fixed underneath all of that, each one
isolated on the nested session rather than reasoned about: premapping the
persistent surfaces during the shell's own start stopped the compositor
drawing the whole overlay layer; the compositor-effect withdraw was gated on
`isExposed()`, a flag that flaps on idle Wayland windows, so an expired card's
blur region kept blurring bare wallpaper; and a dying delegate never
republished its glass, so a window could keep a dead card's region. A quiet
window also carries a one-pixel heartbeat, because a Wayland surface that
commits nothing loses its frame callbacks and with them every animation. The
layer surfaces now follow their window's size, so a growing stack stops
committing buffers larger than the size the compositor was told.

`PANEL-1-R` gives the clock, the phone reading, brightness and audio the menus
they never had, collapses four hand-rolled cards into one measured `SoftCard`,
converts five unit seams that were invisible at factor 1, and moves the tray
child menu onto the same output-covering carrier as every other menu so its
sideways push reads as one piece. The UI hierarchy becomes icon-first.

`PANEL-1-Q` refuses to build or deploy over a running shell, after a third GPU
loss with the same shape as the first two: files replaced under a live session,
its provider adapter restarted seven times in a second, and seven concurrent
`ddcutil` children contending for one I²C bus.

`PANEL-1-P` keeps the falling drop's blur alive for the whole fall instead of
only after it lands, and narrows the diagnostics journal's `Critical` level to
what can actually reach the graphics card plus genuine anomalies, after a
performance audit measured routine poll bookkeeping writing ~290 KB/s to the
SSD at idle for no operational reason.

`PANEL-1-O` corrects the per-output factor to derive from a monitor's
physical diagonal rather than its density, floors it at the reference so a
smaller screen is never shrunk, and refuses the density Qt fabricates when a
compositor publishes no physical size — found live on the author's own three
monitors, where density could not separate two of them.

`PANEL-1-M` stops the canonical verification workflow reaching the graphics
card: DDC is gated by `CELESTINA_DDC` and the release smoke sets it to `0`,
after two GPU losses whose journals both end in concurrent `ddcutil` children
on one I²C bus. `PANEL-1-N` then finishes per-output sizing — every menu and
overlay scales its own scene like the panel, the geometry they are handed is
divided once in the controllers, blur regions are published from mapped bounds
rather than a mapped origin with an unmapped size, and `CELESTINA_SHELL_SCALE`
lets the author name the factor.

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

## LOCK-1 — The session recedes instead of vanishing (active)

**Outcome:** a locked output shows its own wallpaper pushed back and blurred,
with the clock and prompt on the shell's own glass above it, and unlocking
returns that backdrop to the session's real geometry before the compositor
uncovers it.

The lock delivered by `R6` is correct and unpleasant: it covers every output
and refuses to unlock on any error, and it does so on a flat opaque slab that
shares nothing with the session. This checkpoint changes what a locked screen
looks like and nothing about what unlocks it.

- [ ] Hand `celestina-lock` its per-output wallpaper over a bounded,
      non-blocking channel, and paint the deliberate canvas whenever that image
      is absent or unreadable.
- [ ] Recede and blur that backdrop, fade the overlay in above it, and give the
      prompt card a real in-scene glass backdrop instead of a declared one.
- [ ] Sequence the retreat before `unlock_and_destroy`, with the release
      guaranteed by a timer rather than by the animation completing.

The compositor will not help: Niri 26.04 publishes no session-lock animation,
and `ext-session-lock-v1` has already stopped it showing the session, so the
receding backdrop is the wallpaper and never a picture of the desktop — the
same limit `WMAP-1` recorded against window previews. `ADR 0004` is unchanged
and unrelaxed by this work: the surface still shows only time, prompt and
failure state, verification stays in `polkit`-style delegation to a separate
PAM child, and no error path recovers by unlocking. The bounded scope,
exclusions, measured feasibility results and unit boundaries are in
[the LOCK-1 plan](docs/plans/archive/2026-08-17-lock-depth-transition.md).

Perceptual confirmation on a real output is `VAL-LOCK-1` and does not keep this
checkpoint open.

## LIVE-1 — The real session stops being a different shell (complete)

**Outcome:** Celestina survives ordinary use on the author's three monitors —
it does not die when a menu closes, its membrane connects on every output
rather than only the primary one, the panel shows Wi-Fi and Bluetooth, and no
provider misreads the machine.

Every defect here is a consequence of the nest being one output and the
session being three, or of the compositor being older than the one the shell
was verified against. None of them needed a redesign; each needed measuring on
the real session instead of reasoning about.

Delivered:

- [x] **LIVE-1-A — The shell survives three monitors.** One predicate owns
      whether compositor blur may be spoken to at all — Qt keeps
      `QPlatformWindow` alive after destroying the `wl_surface`, and a withdraw
      sent into that gap is a fatal protocol error. The attachment lease
      records its output at acquisition rather than re-deriving it from a
      surface Qt answers with the *primary* screen until `wl_surface.enter`,
      which cancelled the membrane on every other monitor. And the connectivity
      group's presence comes from the readings rather than from its own
      children's rendered visibility, which was a cycle the tray had already
      met and solved.
- [x] **LIVE-1-B — Two providers stop misreading the machine.** A young gamma
      controller is no longer mistaken for a broken one, and a genuinely
      failing output serves a bounded backoff instead of a rebuild loop whose
      every pass snapped the session back to neutral. The device listing is
      read with the escaping `nmcli` actually uses.

The blur was investigated and needed no code: the compositor patch delivers
`passes` and `offset` to the shader, proved with tracing compiled into a
separate binary. The live configuration simply carried no global `blur {}`
block, so the veil ran at niri's default and matched the dense profile. That
is a configuration the author owns; the values the material was tuned against
are recorded in
[the three-monitor evidence](docs/evidence/2026-08-17-three-monitors.md).

Still open, and deliberately not claimed: night light's temperature is a
setting with no control surface, and whether the shell survives a full day is
`VAL-R8`. See
[the archived LIVE-1 plan](docs/plans/archive/2026-08-17-live-session-repairs.md).

## BUBBLE-1 — Native minimized windows join the shell (complete)

**Outcome:** a window removed from Niri's layout by native minimization remains
reachable as one compact application bubble in Celestina. The panel shows one
overlapping group rather than a running-app dock; opening it reveals the
ordered minimized windows with explicit restore and close actions.

Celestina consumes Melibea's versioned local protocol through its existing
aggregate provider helper. Niri remains authoritative for surface lifetime and
minimized state, so action acceptance never removes a row. A later subscribed
snapshot or incremental revision confirms restoration or closure and only then
changes the UI.

The pure protocol, reconnecting provider, group, selector, keyboard and pointer
routes passed their automated contracts. A disposable Niri/Melibea session
proved ordered reconstruction, restore and authoritative close, and the
canonical 0.32.0 bundle passed and deployed without replacing live bytes.
The complete record is in [the archived BUBBLE-1 plan](docs/plans/archive/2026-08-17-melibea-bubbles.md)
and [delivery evidence](docs/evidence/2026-08-18-melibea-bubbles.md).
Coordinated window-to-bubble motion and any future preview contract remain
Melibea M7 work, not part of this checkpoint.

## SURF-1 — Persistent carriers end the per-popup scene change (active)

**Outcome:** opening or closing any panel menu, focused overlay, on-screen
display or toast changes only content inside surfaces that are already
mapped. No popup route maps or unmaps a whole-output surface during ordinary
use.

The author measured (2026-08-18) that mapping and unmapping a whole-output
surface per popup is a slight physical flicker of exactly that monitor, while
persistently mapped surfaces never flicker. The 2026-08-20 audit found the
parking mitigation covers only the dense-glass companions and expires after
twenty seconds, while every menu and overlay carrier still churns on every
open. This checkpoint extends the already-measured parked pattern — mapped,
empty input region, one-pixel effect region — to the interactive and quiet
carriers, and replaces the companions' timed unpark with Niri's fullscreen
state, the one tenant the park yields direct scanout to.

This checkpoint asserts nothing about the driver-level step that turns the
scene change into the visible blink; if removing the churn does not end the
flicker, that investigation is a new unit. Scope, order, exclusions and exit
are in [the active SURF-1 plan](docs/plans/active/2026-08-20-persistent-carriers.md);
the perceptual acceptance is `VAL-SURF-1`.

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
assumption. Lock and Polkit decisions remain under SHELL-D1 through SHELL-D3;
the dock question is closed (ADR 0003, no dock).

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
