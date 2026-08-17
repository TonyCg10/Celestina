# Celestina author validation

This is the manual validation lane. It is entered only when the author asks to
exercise the live Niri session, hardware, appearance or accessibility. Pending
or deferred cases here do not keep an implementation milestone open.

A failed case records its result here and creates a new corrective
implementation unit; it does not rewrite the completed milestone.

## VAL-GPU-01 — Noctalia-only GPU stability hold

- **Status:** passed
- **Related implementation:** LVR-3-B
- **Requires:** Noctalia alone; unchanged kernel, monitors and existing GPU
  mitigations; no Celestina process, provider, build, test or activation
- **Procedure:** use the normal session for a period longer than the prior
  observed failure window without performing a Noctalia to Celestina to
  Noctalia handover. Preserve the next affected boot journal if the PCIe loss
  recurs.
- **Pass condition:** an author-declared long observation completes without
  `device lost from bus!`, a full freeze or a green-screen terminal state.
- **Result:** passed on 2026-08-07 by author declaration after a long
  Noctalia-only observation completed without a freeze, green screen or PCIe
  device loss. Noctalia retained its configured DDC use, so this isolates
  Celestina and the handover sequence rather than DDC as a whole. Two later
  controlled handovers, first with Celestina DDC disabled and then with DDC,
  hotplug, brightness and media active, also crossed the retained crash's
  return-to-first-fence interval without a matching kernel error. This is
  strong negative reproduction evidence, not proof that a lower-probability
  driver or transition fault cannot recur.
  A final 0.6.8 transition repeated first-generation media, tray takeover,
  Bluetooth power cycling and output-triggered DDC discovery, then stopped the
  complete transient cgroup before restoring Noctalia. Four minutes after that
  return — well beyond the retained 82-second interval — the kernel still held
  no matching fence, VCN, flip, PCIe or device-loss error.
- **Interpretation:** recurrence disproves Celestina as a necessary condition;
  non-recurrence is strong evidence against the handover but does not identify
  the exact failing kernel, firmware, DDC, PCIe or hardware layer. This hold
  covers the *session*; two later losses caused by automation are recorded
  below and do not bear on it.
- **Evidence:** [GPU loss system audit](docs/evidence/2026-08-05-gpu-loss-system-audit.md)

### 2026-08-12 — two losses caused by automation, not by the session

Both were caused by an agent running the canonical production workflow while
the author's own desktop was live, and neither is evidence against the hold
above: the session itself was never the trigger.

The common factor is `ddcutil`. `complete-production.sh` ends in an
eight-second smoke that starts the real release host with the real provider
adapter, and that adapter probes DDC on the graphics card's own I²C buses —
the same buses Noctalia is already using. At 00:54 the first loss followed a
build that relinked the running nest's binary, producing a helper restart
storm and two concurrent `ddcutil` children on `busno=8`, then
`Fence fallback timer expired on ring gfx_0.0.0` and
`amdgpu: device lost from bus!`. At 14:05:33 the second followed a *second*,
purely cosmetic production run made only to refresh an artifact manifest: that
boot holds 35 `ddcutil` lines concentrated at 14:05, ending in
`Max wait time 0 milliseconds exceeded after 2 flock() calls` and
`flock() for /dev/i2c-8 failed on 2 calls`.

`PANEL-1-M` removes the cause rather than the symptom: DDC is now gated by
`CELESTINA_DDC`, and the smoke sets it to `0`. The helper still starts,
registers and publishes exactly as it does in a session; it simply opens no
bus, which is the same state a machine whose monitors do not speak DDC/CI
already produces. Verified on the real helper binary: with the gate closed the
journal records `ddc.disabled` and no `ddc.start`, `ddc.detected` or
`ddc.end`, and no `ddcutil` child is spawned.

This does not weaken the smoke. Its purpose is to prove the release host and
the compiled style module load and stay up for eight seconds with no QML
errors, and it already runs with no session bus at all, so every other
provider is degraded there by design. DDC was the one path still reaching real
hardware.

What remains for the author: DDC itself is still unproven against the freeze.
The invariant that one worker owns `ddcutil` holds inside a single helper, but
nothing coordinates between a Celestina helper and Noctalia's own detection,
and both losses had two children on one bus. Whether that is the cause or a
correlate is exactly what `VAL-GPU-01` cannot answer from non-recurrence.

## VAL-SHELL-01 — Request failure and compositor recovery

- **Status:** pending
- **Related implementation:** S1/R0 (complete)
- **Requires:** live Niri session and verified shell artifact
- **Procedure:** provoke a visible workspace request failure or timeout, then
  restart the compositor and observe state clearing and recovery.
- **Pass condition:** failure is visible, no stale state survives, and the
  reconnected snapshot restores only confirmed state.
- **Result:** not run
- **Evidence:** none

## VAL-SHELL-02 — Resource baseline

- **Status:** deferred
- **Related implementation:** S0/R0 (complete)
- **Requires:** live shell on the normal outputs and an accepted decision with
  numeric ceilings for artifact size, startup, PSS/RSS, wakeups and GPU cost
- **Procedure:** measure artifact size, startup, PSS/RSS, wakeups and GPU cost
  with commands and sampling conditions recorded.
- **Pass condition:** every measurement uses the recorded protocol and remains
  at or below its predeclared numeric ceiling; any exceeded ceiling fails.
- **Result:** deferred until numeric resource ceilings are accepted. A first
  measured baseline exists from the nested session on 2026-08-12 — host
  278.9 MiB PSS / 0.13 % idle CPU, provider adapter 138 MiB / 290 KB/s of idle
  journal writes — with proposed ceilings and ranked findings recorded in the
  [shell performance audit](docs/evidence/2026-08-12-shell-performance-audit.md).
  Accepting or amending those numbers is what closes this case's precondition.
- **Evidence:** [shell performance audit](docs/evidence/2026-08-12-shell-performance-audit.md)

## VAL-R1-01 — Integrated bar interactions

- **Status:** passed
- **Related implementation:** R1 (complete)
- **Requires:** live media, audio, DDC and tray providers
- **Procedure:** exercise artwork/transport, speaker and microphone gestures,
  DDC burst coalescing and one tray menu action with provider loss cases.
- **Pass condition:** every action reports confirmed or failed truthfully and a
  slow/missing provider does not block or stale the rest of the bar.
- **Result:** passed on 2026-08-07 against celestina 0.6.8. An already-present
  MPRIS player appeared in the first helper generation without a restart;
  title, play, pause, progress, disappearance and return followed the player.
  Audio, microphone, DDC, CPU/RAM, workspaces, OSD, all four registered tray
  items and provider isolation remained healthy in the same live session.
- **Remediation:** completed by `LVR-3-A` through `LVR-3-G` in
  [late provider insertion](docs/plans/archive/2026-08-05-late-provider-insertion.md).
- **Evidence:** [2026-08-05 follow-up](docs/evidence/2026-08-05-live-validation-follow-up.md)

### 2026-08-07 controlled transition observations

- First-generation media passed with DDC disabled: the already-present Firefox
  player appeared without restarting the provider helper, and play/pause
  updates worked in both directions.
- Bluetooth is powered but has no connected devices. The panel currently hides
  the Bluetooth reading in that state; this fails the author's requirement
  that Bluetooth remain visibly present even with zero connections.
- The Wi-Fi text appears and disappears intermittently while the underlying
  connection remains in use. Read-only sampling reproduced `nmcli` latency
  spikes between 2.37 and 3.00 seconds among normal 4--5 millisecond replies.
  The session provider applies the shared 750 millisecond tool deadline and
  withdraws `network` after that single missed sample, so the panel disappears
  until a later poll succeeds. This is provider sampling churn, not evidence
  that the Wi-Fi link itself disconnected; corrective work must retain the
  last confirmed link across transient probe failures and distinguish a real
  offline state.
- Solaar and Slack registered with Celestina's StatusNotifierWatcher but were
  absent from the rendered tray.
- Opening the session menu logged that `providerSource` was injected into a
  `SessionMenu` component that does not declare that property. The shell stayed
  alive, but this surface is not clean enough for further session-action tests.
- Closing the notification centre through the same unread-count button that
  opened it takes two clicks. The first click returns focus to the previously
  active application as if the focused overlay had been dismissed, but leaves
  the centre mapped; the second click closes it. The notification centre,
  launcher, clipboard, control centre and session menu all use the same
  `OverlayController` and focused `OverlaySurface`, so the shared dismissal and
  toggle boundary is the corrective scope. Only the notification path is
  confirmed by live validation: it is currently the only one of those overlays
  with a panel button that can exercise this exact open-button/close-button
  sequence.
- Interaction requirement: every transient panel surface must dismiss on a
  click outside its own bounds. This applies both to the focused overlays above
  and to panel context menus, including menus opened by right-clicking tray
  items. The overlay and panel-menu implementations have different controllers,
  so corrective evidence must exercise both paths rather than infer one from
  the other.
- The controlled return began after 16 minutes of stable Celestina use. At
  00:18 EDT the `celestina-transition.service` cgroup stopped cleanly: host,
  both helpers, its inhibitor, `wlsunset`, session bus names and the transient
  unit were all absent before Noctalia started. No Celestina `ddcutil` process
  existed. Noctalia then reclaimed both StatusNotifierWatcher and Notifications
  from PID 1479019 and ran its own configured DDC detection, finding HDMI-A-1
  on bus 7 and DP-1 on bus 8. The kernel had recorded no matching GPU fence,
  timeout, flip or PCIe-loss error at the post-transition checkpoint.
- In the following DDC-enabled phase, Celestina started while DP-1 was
  intentionally disabled and Firefox was paused. Enabling DP-1 later mapped its
  panel correctly at 00:25:06 EDT, but did not add that output's brightness
  control. The brightness worker retains a non-empty startup detection for the
  full 300-second refresh interval; only an entirely empty detection uses the
  30-second rediscovery interval, and output hotplug does not wake the worker.
  Corrective work must trigger or schedule prompt DDC rediscovery when outputs
  appear without allowing concurrent probes. Live validation must also
  distinguish delayed recovery at the existing refresh from failure to recover
  at all.
- DP-1 brightness did appear at the existing refresh and then read and changed
  the monitor correctly. Recovery therefore works but is unacceptably delayed;
  the confirmed defect is missing prompt hotplug rediscovery, not a permanently
  lost connector or a broken DDC control after rediscovery.
- The combined DDC and media phase also passed: Firefox appeared, its transport
  controls worked, and a brightness change completed correctly while media was
  active. Discovery latency remains a quality defect even though steady-state
  operation passed. Media currently polls every five seconds while idle and
  every two seconds with a player; corrective work should subscribe to MPRIS
  owner and property signals, retaining only a light active-progress timer and
  bounded fallback reconciliation. Brightness must not compensate by polling
  DDC more aggressively. The host already observes output hotplug, so it should
  request one coalesced rediscovery from the single DDC worker, with global
  serialization and the existing bounded child lifecycle intact.
- The final return kept Firefox playing and stopped the DDC-enabled Celestina
  instance at 00:34 EDT. Its transient cgroup, host, helpers, `ddcutil`,
  `wlsunset`, inhibitor and bus names were all absent before Noctalia started.
  Noctalia reclaimed StatusNotifierWatcher and Notifications from PID 1488967,
  restored its `mpvpaper` supervisor, retained the playing MPRIS source and
  completed its own DDC detection. No matching kernel GPU error was present at
  the ten-second post-transition checkpoint. Noctalia and the kernel remained
  clean through 00:36:17 EDT, 100 seconds after Noctalia started and therefore
  beyond the approximately 82-second return-to-first-fence interval of the
  retained crash. This disproves deterministic reproduction by one controlled
  handover; it does not exclude a lower-probability transition or driver fault.

### 2026-08-07 remediation

`LVR-3-F` corrects four of the seven observations above in celestina 0.6.8. The
observations themselves are not rewritten; these are the reruns they earn.

- Bluetooth publishes the adapter's own state — absent, off, on — beside the
  connection count, so a powered adapter with nothing on it stays on the panel
  and an unreadable query still publishes nothing. Rerun: `VAL-R5-BT`.
- The network reading holds the last confirmed link across up to three
  unreadable polls, and ends it on the second confirmed-offline poll. The
  shared 750 ms tool deadline is unchanged. Rerun: `VAL-R1-NET`.
- Every overlay now receives only the properties it declares, so the session
  menu opens without a runtime property error. Rerun: `VAL-R5`.
- Output hotplug asks the single DDC worker for one coalesced rediscovery
  instead of waiting out the 300-second refresh. Neither interval was
  shortened and no second `ddcutil` child can exist. Rerun: `VAL-R1-DDC`.

The other three are corrected in the same delivery.

- Every transient surface — the five focused overlays, the panel's context menu
  and a tray item's own menu — now covers its output, so a click outside the
  card is the surface's own to answer and the button that opened an overlay is
  behind it rather than in front of it. Rerun: `VAL-R1-OVERLAY`.
- A tray item that registers and then fails to describe itself is retried once,
  logged, and shown under the name it registered with, instead of being dropped
  silently and permanently. Registry re-reads are also generation-tagged, so a
  superseded reply can no longer clear the current one. The exact reason Slack
  and Solaar were lost is still not known — read-only inspection of this
  session's bus showed all four registered items answering `GetAll` correctly —
  so this closes the chain that made such a loss silent rather than claiming to
  have found the cause. Rerun: `VAL-R1-02`.
- Media is driven by MPRIS owner and property signals. `playerctl` is gone from
  this shell; nothing is spawned for media at all. Rerun: `VAL-R1-01`.

## VAL-R1-OVERLAY — Every transient surface closes on a click outside it

- **Status:** passed
- **Related implementation:** `LVR-3-F` (complete)
- **Requires:** live Niri session and celestina 0.6.8
- **Procedure:** open the launcher, the clipboard history, the notification
  centre, the control centre and the session menu in turn, and close each with
  a single click outside its card. Then open the notification centre from the
  panel's unread indicator and close it with a single click on that same
  indicator. Then right-click the panel for its context menu, and right-click a
  tray item for its own menu, closing each with one click outside.
- **Pass condition:** every surface closes on the first click outside its
  bounds; the indicator that opened the notification centre closes it in one
  click; focus returns to the previously active application exactly once, and
  no surface reopens, flickers or stays mapped after a dismissal.
- **Result:** passed on 2026-08-07 against celestina 0.6.8. Focused overlays,
  the tray drawer and tray-item menus closed on the first outside click without
  losing an item, remapping the surface or requiring a second toggle.
- **Evidence:** [the 2026-08-07 corrections](docs/evidence/2026-08-07-one-poll-is-not-the-truth.md)

## VAL-R1-NET — A slow probe does not erase a live link

- **Status:** deferred
- **Related implementation:** `LVR-3-F` (complete)
- **Requires:** live Niri session, celestina 0.6.8 and a Wi-Fi link in use
- **Procedure:** use the session normally for long enough to cross several of
  the `nmcli` latency spikes measured on 2026-08-07, watching the panel's link
  text; then disconnect the link deliberately and watch it go.
- **Pass condition:** the link text never disappears while the connection is in
  use, and a real disconnection removes it within about ten seconds rather than
  persisting.
- **Result:** partial on the final 2026-08-07 rerun against celestina 0.6.8.
  Wi-Fi remained the default route and its panel text stayed present throughout
  the exercised Celestina session, including media, tray and monitor hotplug.
  The deliberate offline half was not run: the connected Ethernet link served
  a laptop image-mirroring path rather than Internet, so forcing NetworkManager
  down from the test terminal was not a safe way to preserve control of the
  session. The author closed this phase with that limitation explicit.
- **Remediation:** implemented by `LVR-3-G` in [late provider insertion](docs/plans/archive/2026-08-05-late-provider-insertion.md). The unreadable
  hold is removed rather than raised: a probe that saw nothing can no longer
  retire a link at any repetition count, and only a poll that positively found
  no default route can — twice in a row, so about ten seconds. A route naming a
  device the device list cannot explain is now classified as unreadable rather
  than as a disconnection, which is what a re-associating card looks like. The
  live rerun remains required and this case stays failed until the author runs
  it.
- **Evidence:** [what a probe did not see](docs/evidence/2026-08-07-what-a-probe-did-not-see.md)

## VAL-R1-DDC — Prompt brightness rediscovery on output hotplug

- **Status:** passed
- **Related implementation:** `LVR-3-F` (complete)
- **Requires:** live Niri session, celestina 0.6.8, and an output that can be
  disabled and enabled
- **Procedure:** start with one output disabled, enable it, and watch for that
  monitor's brightness control. Then disable it again.
- **Pass condition:** the control appears within seconds of the panel mapping
  rather than at the 300-second refresh, only one `ddcutil` process exists at
  any moment during the transition, and no `ddcutil` survives shutdown.
- **Result:** passed on 2026-08-07 against celestina 0.6.8. Enabling a disabled
  output mapped its panel and exposed its working brightness control within
  seconds; disabling it removed the affected surfaces. No concurrent or
  surviving `ddcutil` process, DDC contention or matching GPU error was
  observed.
- **Evidence:** [one poll is not the truth](docs/evidence/2026-08-07-one-poll-is-not-the-truth.md)

## VAL-R5-BT — Bluetooth stays visible while the adapter is on

- **Status:** passed
- **Related implementation:** `LVR-3-F` (complete)
- **Requires:** live Niri session, celestina 0.6.8, and an adapter that can be
  powered off and on
- **Procedure:** with nothing paired, read the panel and the control centre;
  connect a device and read both again; power the adapter off and read both
  again.
- **Pass condition:** a powered adapter with no connections reads as present and
  idle rather than vanishing, a connected device is counted and named, and a
  powered-off adapter says so instead of disappearing.
- **Result:** passed on 2026-08-07 against celestina 0.6.8 for the retention
  defect this case remediates. The indicator remained present with no connected
  device, remained present while the adapter was powered off, and returned to
  its powered state without restarting the provider. Direct device selection
  is not an indicator action yet; the future `UX-1` checkpoint owns network and
  Bluetooth menus rather than extending this corrective unit.
- **Evidence:** [one poll is not the truth](docs/evidence/2026-08-07-one-poll-is-not-the-truth.md)

## VAL-R1-02 — StatusNotifierWatcher takeover

- **Status:** passed
- **Related implementation:** R1 (complete)
- **Requires:** permission to stop the current watcher owner temporarily
- **Procedure:** inspect owners/items, remove the existing watcher and observe
  Celestina claiming the name and retaining registrations.
- **Pass condition:** exactly one watcher owns the name and valid tray items
  remain available through takeover and rollback.
- **Result:** passed on 2026-08-05. Celestina became the sole watcher, retained
  Solaar, NetworkManager and Blueman, served left-click activation and
  right-click menus, removed only NetworkManager when `nm-applet` stopped and
  restored it on restart. Noctalia reclaimed the name and tray during rollback,
  and Celestina later reacquired it without losing registrations.
- **Evidence:** [2026-08-05 follow-up](docs/evidence/2026-08-05-live-validation-follow-up.md)

## VAL-R1-TRAY — Every registered active item reaches the open drawer

- **Status:** passed
- **Related implementation:** `LVR-3-F` (complete)
- **Requires:** a live Niri session, celestina 0.6.8, and Slack and Solaar
  registered as StatusNotifierItems
- **Procedure:** inspect the watcher's registry and each item's `GetAll`
  response, then open the tray drawer and compare the rendered controls.
- **Pass condition:** every bounded active registration that answers with a
  usable item is present in the open drawer, with a name fallback when its icon
  cannot be resolved.
- **Result:** passed on the final 2026-08-07 rerun against celestina 0.6.8. The
  watcher and folded count reported four active registrations, and Slack,
  Solaar, NetworkManager and Blueman all reached the open drawer. Left-click,
  right-click and outside-click dismissal worked without any item disappearing.
- **Remediation:** implemented by `LVR-3-G` in [late provider insertion](docs/plans/archive/2026-08-05-late-provider-insertion.md), which found a
  real defect by walking the whole D-Bus path against a private bus instead of
  reasoning about the parts. A registry read rebuilt the registration list
  wholesale from the snapshot its reply carried, so an application registering
  while that read was in flight was removed by an answer composed before it
  existed — permanently, because no second registration signal follows. The new
  `celestina-tray-watcher` integration test reproduced this session's symptom on
  its first run: four registered, two published, Slack and Solaar missing. A
  registry read is now a reconciliation against what was known when it was sent,
  and all four are published. The model, the open drawer and the 1920-pixel
  flank layout were checked too and hold; the folded drawer additionally now
  shows how many items are behind its chevron, which it never did. Whether this
  defect is what the author hit, or the unreadable folded state, or both, is an
  inference the automated build alone could not settle. No status is rewritten,
  nothing is permanently unfolded and no application is special-cased; the
  final live drawer observation closes the case.
- **Evidence:** [what a probe did not see](docs/evidence/2026-08-07-what-a-probe-did-not-see.md)

## VAL-R2-01 — Deferred launcher edge cases

- **Status:** passed
- **Related implementation:** R2 (complete)
- **Requires:** screen reader, a real `Terminal=true` entry, a deliberately
  failing desktop entry and a password-manager clipboard selection
- **Procedure:** exercise announcements, both launch paths, error feedback and
  sensitive-selection exclusion.
- **Pass condition:** semantics and visible outcomes match each contract without
  persisting sensitive content.
- **Result:** deferred until the required clients/data are present. The normal
  launcher path, name search and populated clipboard path worked; the short
  initial launcher list is the provider's current 24-row bound, not evidence of
  a missing application index.
- **Author declaration:** passed on 2026-08-14 by author declaration, superseding the deferral above: the launcher and the clipboard history are what the author uses, and the edge cases this section was holding open — a screen reader, a real `Terminal=true` entry — were never the reason it was unusable. Those two remain untested and are recorded as such rather than claimed.
- **Evidence:** [2026-08-05 follow-up](docs/evidence/2026-08-05-live-validation-follow-up.md)

## VAL-R2-02 — Clipboard deletion and empty-state dismissal

- **Status:** passed
- **Related implementation:** R2 (complete)
- **Requires:** live shell with at least one ordinary clipboard entry
- **Procedure:** open clipboard history, remove one row through its visible and
  keyboard paths, choose `Vaciar`, then dismiss the empty overlay with Escape
  and reopen it.
- **Pass condition:** deletion is discoverable without a context-menu guess,
  every delete path is keyboard and assistive-technology reachable, and the
  overlay retains a normal dismissal path when no list row exists.
- **Result:** passed on 2026-08-05. The visible row action, keyboard Delete,
  context-menu path and `Vaciar` worked; Escape dismissed the empty overlay,
  which then reopened and closed normally.
- **Evidence:** [2026-08-05 follow-up](docs/evidence/2026-08-05-live-validation-follow-up.md)

## VAL-R3 — Session verbs and lifecycle

- **Status:** passed
- **Related implementation:** R3 (complete)
- **Requires:** R3 automated exit green plus explicit permission for each live
  mutation
- **Procedure:** exercise OSD, gamma release, caffeine/idle and DPMS with
  rollback ready; confirm lock-and-suspend refuses while no provider exists.
- **Pass condition:** each provider-confirmed state is truthful, external
  lifecycles release cleanly and the refusal path never suspends unlocked.
- **Result:** passed on 2026-08-07. The earlier pass for OSD, night-light,
  caffeine, forced child death, aggregate-helper restart, DPMS/wake recovery
  and both locker refusals was retained. Two controlled Celestina shutdowns
  then proved the 0.6.2 lifecycle remediation live: the host, both helpers,
  `wlsunset`, `systemd-inhibit`, its held `sleep`, bus names and transient
  cgroup all disappeared before Noctalia restarted, with no manual cleanup.
- **Remediation:** implemented in 0.6.2 by `LVR-2-A` in [live validation follow-up](docs/plans/archive/2026-08-05-live-validation-follow-up.md); a process regression
  proves SIGTERM releases an active held child before helper exit, and the live
  repeated lifecycle rerun passed on 2026-08-07.
- **Evidence:** [2026-08-05 follow-up](docs/evidence/2026-08-05-live-validation-follow-up.md)

## VAL-NIGHT-1 — Smooth provider-confirmed night light

- **Status:** pending
- **Related implementation:** R8-P-Q
- **Requires:** Celestina on a real Niri TTY output that advertises
  `wlr-gamma-control-unstable-v1`, no competing gamma client, and an external
  camera at 120 fps or faster or a colorimeter. The nested winit backend is a
  refusal-path fixture only because it does not advertise gamma control.
- **Procedure:** start neutral, open the control centre and activate night light
  once. Record the physical output while it reaches the fixed 2700 K endpoint,
  then deactivate it and record the return to identity. Repeat once from the
  key binding, stop Celestina while warm, and try activation while a deliberate
  competing gamma client owns one output. Confirm the switch remains at its
  last provider-confirmed position while each request is pending and moves only
  when the final gamma commit succeeds.
- **Pass condition:** activation and deactivation each form one monotonic
  approximately 300 ms colour transition with no neutral/warm flash at either
  edge; every controlled output advances together; shutdown restores identity;
  a missing or competing gamma controller is refused without moving the switch
  or persisting an active state. Ordinary screen capture is not sufficient
  evidence because output gamma is applied after the captured scene.
- **Result:** not run. Automated tests cover the whitepoint, transition samples,
  LUT bounds and provider-confirmed switch state; the development nest can only
  prove the unsupported-backend refusal.
- **Evidence:** none

## VAL-R4 — Notification server, toasts and handover

- **Status:** passed
- **Related implementation:** R4 (complete)
- **Requires:** the R4 automated exit green, a session where
  `org.freedesktop.Notifications` can be observed, a paired phone for
  Magnetita's mirror and a screen reader
- **Procedure:** confirm the shell declines the name while another server owns
  it; then, with that server stopped, exercise a real notification, a
  replacement, a close, an action, do-not-disturb and the unread indicator.
- **Pass condition:** no name is ever taken from a running server, every toast
  shows only what the producer sent within its bounds, and history, unread count
  and assistive-technology announcements stay truthful.
- **Result:** failed on keyboard dismissal. Server handover, ordinary and
  critical toasts, replacement, close, action return, DND, unread/history,
  deletion, clearing and rollback all worked without unrelated providers
  disappearing. Escape did not close the notification centre after focus left
  its inner list; the panel indicator remained the workaround. Paired-phone and
  screen-reader paths remain deferred.
- **Remediation:** implemented in 0.6.2 by `LVR-2-A` in [live validation follow-up](docs/plans/archive/2026-08-05-live-validation-follow-up.md); Escape is owned by the
  notification window and its offscreen regression passes. Live focus-state
  validation remains required.
- **Author declaration:** passed on 2026-08-14 by author declaration, superseding the failure above. The defect it recorded is fixed in the meantime: Escape is now a window-context `Shortcut` rather than a key handler on the inner list, so it closes the centre wherever focus sits. That fix was read in the source, not re-observed in a session; the paired-phone and screen-reader paths stay deferred.
- **Evidence:** [2026-08-05 follow-up](docs/evidence/2026-08-05-live-validation-follow-up.md)

## VAL-R5 — Control centre, weather and calendar

- **Status:** passed
- **Related implementation:** R5 (complete)
- **Requires:** the R5 automated exit green, a real network and Bluetooth to
  switch, a weather location the author chooses, and a screen reader
- **Procedure:** exercise every control in the centre, confirm each shows the
  provider's own reading rather than the requested one, force one write to fail
  and check the control reports it, then restart the shell and confirm settings
  survived.
- **Pass condition:** no control paints a value its provider never reported, a
  failed request is visible as failed, settings survive a restart, and an
  absent weather reading reads as absent rather than as stale.
- **Result:** partial; normal controls, two-step session-menu paths and DND
  persistence across a full restart worked. No location was configured, so
  absent weather remained expected. Forced provider-write failure and
  screen-reader paths remain deferred.
- **Author declaration:** passed on 2026-08-14 by author declaration, superseding the partial above. The controls and the session menu are in daily use. A configured weather location, a forced provider-write failure and the screen-reader paths remain untested.
- **Evidence:** [2026-08-05 follow-up](docs/evidence/2026-08-05-live-validation-follow-up.md)

## VAL-R7 — Wallpaper, portal values and Niri colours

- **Status:** passed
- **Related implementation:** R7 (complete)
- **Requires:** the R7 automated exit green, physical monitors to hotplug, and
  the author choosing to reference the generated colour include
- **Procedure:** map a wallpaper on each output and check the image belongs to
  the screen showing it; unplug and replug a monitor; point an application at
  the portal and read the colour scheme back; reference the generated include
  in Niri and compare its borders with the panel.
- **Pass condition:** no output shows another output's image or a black
  rectangle standing in for one, hotplug changes only the affected surface, the
  portal values match the sealed tokens, and Niri's borders match the panel's.
- **Result:** partial; wallpapers were correct on both outputs, physical
  removal/reconnection of `DP-2` changed only that output, and the public portal
  returned the sealed dark scheme and accent through a proven GTK rollback and
  restoration. Descriptor installation also required explicit
  `Settings=celestina-shell` selection in the live Niri preference file, which
  the README does not currently say. The author explicitly omitted the Niri
  colour-include comparison, so the complete case remains deferred.
- **Remediation:** implemented in 0.6.2 by `LVR-2-A`; the README records the
  missing portal selection, broker restart and exact rollback.
- **Author declaration:** passed on 2026-08-14 by author declaration, superseding the partial above. The wallpaper, the portal values and the hotplug behaviour were already observed; what was missing was the Niri colour-include comparison, which the author omitted then and does not require now. It stays untested.
- **Evidence:** [2026-08-05 follow-up](docs/evidence/2026-08-05-live-validation-follow-up.md)

## VAL-SHELL-03 — Live accessibility and application identity diagnostics

- **Status:** passed
- **Related implementation:** S0/R7 (complete)
- **Requires:** verified production bundle activated on a live Niri session
- **Procedure:** start the two-output shell, retain its startup diagnostics and
  exercise portal registration while the wallpaper surfaces map.
- **Pass condition:** every `Accessible` property is attached to a supported
  QML object, the deployed application id is discoverable by the host portal,
  and neither path emits a runtime contract error.
- **Result:** passed on 2026-08-05. A clean two-output restart emitted none of
  the prior accessibility, application-id, provider-frame or audio binding
  diagnostics; both session names were reacquired and a post-restart
  notification worked.
- **Evidence:** [2026-08-05 follow-up](docs/evidence/2026-08-05-live-validation-follow-up.md)

## VAL-COPY-01 — Spanish product copy

- **Status:** passed
- **Related implementation:** current QML surfaces and the Spanish product-copy
  contract
- **Requires:** verified production bundle and each implemented overlay exposed
  at least once
- **Procedure:** open the launcher, clipboard, notification centre, control
  centre and session menu and inspect visible and accessibility copy as one
  product surface.
- **Pass condition:** every person-facing string is Spanish throughout each
  surface; no half-translated overlay remains.
- **Result:** passed on 2026-08-05 for launcher, clipboard, notification centre,
  control centre, session menu and panel copy.
- **Evidence:** [2026-08-05 follow-up](docs/evidence/2026-08-05-live-validation-follow-up.md)

## VAL-UX-1 — Network and Bluetooth indicator menus

- **Status:** passed
- **Related implementation:** UX-1 (complete in celestina 0.7.0)
- **Requires:** the verified 0.7.0 bundle, explicit authorization to activate
  Celestina and Noctalia retained as the rollback owner
- **Procedure:** with Celestina intentionally active, open the network and
  Bluetooth menus from their panel indicators. For each menu, confirm one-click
  opening and closing, Escape and outside-click dismissal, placement on both
  output scales, readable focus restoration, truthful current/empty/list states
  and a refresh that reaches a terminal result. Exercise only a saved network
  activation that is safe for the author's unusual Ethernet/Wi-Fi layout; do
  not disconnect Wi-Fi merely to manufacture an offline state. On Bluetooth,
  switch the adapter and connect or disconnect one already-known device only
  when doing so is safe. Reopen after each request and confirm pending, success
  or failure survived the first menu's destruction. Restore Noctalia afterward.
- **Pass condition:** every offered action names only a published stable target,
  never paints requested state as confirmed, reaches a terminal result, keeps a
  failure visible even if its target row disappears, and all four dismissal
  paths leave focus and the session usable.
- **Result:** passed on 2026-08-08. Both indicators remained present, opened
  their menus and exposed truthful current state and usable actions; dismissal,
  multimedia and brightness remained functional in the live Niri session. The
  author accepted the functional checkpoint while identifying three follow-up
  product needs that do not rewrite UX-1: transient cards appear anchored to a
  fixed output height instead of the invoking control, the menus need a
  deliberate iconography and visual-usability pass, and the clock/date region
  needs a separate calendar-and-weather menu with location management.
- **Evidence:** [UX-1 delivery](docs/evidence/2026-08-08-ux-1-delivery.md)

**2026-08-08 placement follow-up.**

The menu card stopped using the top edge that an unstacked 40-pixel panel would
have occupied and instead followed the compositor's real exclusive-zone
placement while retaining the invoking control's horizontal anchor. The author
confirmed that this removed the fixed-height defect, then requested a smaller
visual gap. The final automated correction lets only the card's shadow occupy
that gap. A later attempt to change directly from one open menu to another still
required two clicks in the live compositor; `modal: false` has automated
coverage but no author-confirmed live pass, so the interaction remains an
explicit input to the next visual-design checkpoint rather than a claimed UX-1
result.

- **Evidence:** [menu anchor correction](docs/evidence/2026-08-08-menu-anchor-correction.md)

## VAL-R8 — Living without Noctalia

- **Status:** deferred
- **Related implementation:** R8 (complete)
- **Requires:** `VAL-R1-01`, `VAL-R1-02`, `VAL-R2-02`, `VAL-R3`, `VAL-R4`,
  `VAL-R5`, `VAL-R7`, `VAL-SHELL-03` and `VAL-COPY-01` passed and recorded, and
  the author's decision to depend on this shell alone
- **Procedure:** run the handover report, confirm it is complete, disable
  Noctalia's autostart through the tool, use the session for a normal day, and
  restore Noctalia from the written rollback at least once to prove the way
  back works.
- **Pass condition:** nothing the author relies on is missing for a full day,
  and the rollback restores Noctalia without a manual repair.
- **Result:** deferred. Watcher and notification rollback to Noctalia worked,
  but first-generation media, notification-centre Escape and four orphan sleep
  inhibitors failed. Screen lock and Polkit remain unbuilt, Niri colour
  adoption was omitted, and several AT/configuration-dependent cases remain
  deferred. The removal tool must continue to refuse.
- **Evidence:** [2026-08-05 follow-up](docs/evidence/2026-08-05-live-validation-follow-up.md)

## VAL-SHELL-LOCK — Concrete lock, suspend and resume lifecycle

- **Status:** deferred
- **Related implementation:** future unit created after SHELL-D1 is applied
- **Requires:** an approved locker, a verified composed-lock artifact and
  explicit permission for the live lock/suspend mutation
- **Procedure:** lock, confirm readiness, suspend, resume and exercise one
  locker failure with rollback ready.
- **Pass condition:** suspend never starts before confirmed lock, failure leaves
  the session awake and usable, and resume remains locked until authentication.
- **Result:** deferred while SHELL-D1 remains open
- **Evidence:** none

## VAL-WSG-1 — Workspace groups with a monitor switched off

- **Status:** pending
- **Related implementation:** WSG-1
- **Requires:** the verified bundle carrying WSG-1, explicit authorization to
  activate Celestina, Noctalia retained as the rollback owner, and the three
  configured monitors available so the memory can be taught once
- **Procedure:** with all three monitors connected, confirm each panel shows
  only its own five workspaces and no capsule appears. Switch two monitors off.
  Confirm the strip now shows the focused monitor's five plus one capsule per
  absent monitor, each naming its output and its count. Move the focus into an
  absent monitor's workspace by keyboard bind and confirm that group expands
  while the previous one collapses. Click a capsule and confirm it asks for the
  workspace that was last active on that monitor. Make a window on a collapsed
  monitor urgent and confirm the capsule reports it. Reconnect the monitors and
  confirm the strips return to five each. Restore Noctalia afterward.
- **Pass condition:** no workspace is ever hidden without its group being
  reachable in one gesture, a collapsed capsule never conceals urgency, the
  expansion honours `CelestinaTheme.reducedMotion`, and a session that has never
  seen more than one monitor degrades to today's flat strip rather than
  inventing groups.
- **Result:** not run
- **Evidence:** none

## VAL-DIAG-1 — The diagnostic journal in a real session

- **Status:** pending
- **Related implementation:** DIAG-1
- **Requires:** the verified 0.8.1 bundle carrying DIAG-1, which is deployed,
  plus explicit authorization to run Celestina in the live session
- **Procedure:** with the journal directory left in place, start the shell the
  ordinary way and let it run. Confirm that
  `~/.local/state/celestina/diagnostics/` contains one file per process, that all
  three carry the same `run_id`, and that `host` lines, `niri-adapter` lines and
  `provider-adapter` lines interleave into one sensible ordering. Exercise
  brightness so `ddc.start`/`ddc.end` pairs appear, and confirm
  `grep -c '"event":"ddc.overlap"'` returns zero. Copy something to the
  clipboard, receive a notification and play a track, then search the journals
  for that text and confirm none of it is there. Stop the shell and confirm the
  `journal.stop` line closes each file. Finally run
  `celestina/scripts/diagnostic-report.sh --boot 0` and confirm it starts
  nothing, reports exactly what it read, and produces a bundle.
- **Pass condition:** three correlated files exist, no private content appears
  in any of them, no DDC overlap is recorded, the journal never delays or
  disturbs the shell, and the report script collects a readable bundle without
  starting or changing anything.
- **Result:** not run
- **Evidence:** none

## VAL-WMAP-1 — The workspace window map

- **Status:** pending
- **Related implementation:** WMAP-1
- **Requires:** the verified bundle carrying WMAP-1, explicit authorization to
  activate Celestina, Noctalia retained as the rollback owner, and at least one
  monitor switched off so a capsule exists to open
- **Procedure:** with windows open across several workspaces on an absent
  monitor, right-click its capsule. Confirm the card opens anchored to it and
  lists every window of every workspace the capsule folded, each with its
  application's icon, title and application id. Right-click a single workspace
  and confirm the same card lists only its windows. Confirm a focused and an
  urgent window are distinguishable, and that a workspace with no windows says so
  rather than appearing broken. Click a window row and confirm the session goes
  to that window, not merely to its workspace; click a workspace row and confirm
  it goes to the workspace. Walk the card with the arrow keys and confirm the
  visible focus is legible and that Return takes the row it is on. Dismiss by
  outside click and by Escape, and confirm focus returns. Confirm a left click on
  a workspace still goes there in one gesture, and that a left click on a capsule
  expands its group in the strip. Repeat on both output scales. Restore Noctalia
  afterward.
- **Pass condition:** no window is listed that is not there and none is missing,
  a window whose application no theme knows still shows its name, the card never
  traps the pointer or the keyboard, both dismissal paths restore focus, and the
  hover and press feedback honours `CelestinaTheme.reducedMotion`.
- **Result:** not run
- **Evidence:** none

## VAL-PANEL-1 — Borderless glass panel

- **Status:** pending
- **Related implementation:** PANEL-1
- **Requires:** the incremental or verified PANEL-1 bundle inside a real Niri
  compositor, a detailed wallpaper, a visually distinct application window and
  the nested reference blur profile
- **Procedure:** inspect the panel at scale 1 and scale 2. Confirm one nearly
  transparent, shadowless `ContextualVeil` covers the complete 40-pixel panel
  edge-to-edge with no outer margin, gap or hard plate. Compare composed detail
  immediately below the bar with the same application or wallpaper detail
  inside it, and confirm blur remains continuous across exactly one finite
  panel region. Inspect every information group and both ends of the phone
  reading: each must remain an ordinary rounded `ContentSurface` capsule at
  output-local y=5 with height 30, and no capsule may introduce its own
  compositor region or a local blur discontinuity. Add and remove a late
  provider and confirm the one panel region remains the complete 40-pixel bar.
  Open network, Bluetooth, tray, workspace map, control centre, clipboard,
  notification centre, session, launcher, performance, toolbox and wallpaper.
  Pin at least two foreign tray applications. Their icons must appear to the
  inventory opener's left, each pin and unpin must fade that icon in or out,
  and right-clicking either icon must reveal its foreign menu from that exact
  glyph with the same top membrane and fall as the first-party panel menus.
  Confirm each menu keeps a nearly transparent shadowless outer field, dense
  dark matte internal cards that match the panel capsules, fixed light/white
  foregrounds and no shadow on either material. Confirm the bar, menu body and
  membrane expose no outline, lit edge or apparent edge halo. Confirm
  panel-opened surfaces sit beneath and near their actual clicked opener, while
  a command-opened centred launcher does not acquire a false panel anchor. For
  each surface opened by a real panel control, identify both the clicked
  control and its exact glyph anchor. The body must remain placed from the
  clicked control, but the membrane must begin at the bar's lower edge
  (`attachmentStartY == barHeight`) as one narrow droplet mouth centred under
  the glyph — never a body-wide seam — cling to the bar with a meniscus on
  both sides, narrow to its neck just below the bar and swell until it lands
  tangent on the body's flat top edge inside its ordinary rounded top
  corners. The mouth must clamp only enough to stay inside that flat span.
  The invoking control must retain the same
  circle it shows on hover
  while its own surface remains open. The owning `PanelPill` and every
  `ContentSurface` must keep the same rounded silhouette, y=5/height=30 geometry
  and dense material while the menu is open. The membrane itself must remain only the
  light `ContextualVeil`: no dense fill, transition layer, shadow, closed
  capsule stroke or dark cap may bridge the two Wayland surfaces. Closing by
  Escape, outside click or the surface's own action must leave those rounded
  capsules unchanged.
  Compare real menu widths 328, 360, 424, 460, 530 and 620 and confirm vertical
  travel is respectively 20, 22, 25, 28, 32 and 36 pixels. Compare glyph
  anchors of different widths, two controls at different positions inside the
  same group, and a menu clamped near each output edge. The hanging neck
  must visibly thin or preserve its icon-relative width as travel,
  icon/body reference-scale spread or horizontal displacement increases. It
  must remain a liquid icon-scaled neck — never an icon-thin thread and never
  a body-proportional band — and the meniscus, neck and swelling lower lobe
  must stay rounded, with no hourglass flank or visible pinch where the
  curves meet. In every
  case the painted membrane and real blur boundary must coincide from the
  mouth through the neck to the landing. Change model-driven content height and
  confirm it does not change the width-derived vertical travel.
  Open the same available route by command or keybind and confirm it remains a
  floating rounded field. Right-click a workspace dot and a collapsed
  monitor dot and confirm the workspace map hangs from the bar with the same
  droplet beneath the exact invoking dot, and that the collapsed monitor
  group is a single larger dot without a visible count.
  Then watch the opening itself on a panel-opened menu, an overlay and a
  sideways child menu: each must be born as a drop at its own seam and fall
  into place. The stretched middle must be legible rather than a flash, the
  neck must never appear to separate from the seam, no row may appear outside
  the glass carrying it, and every surface must land exactly where it lands
  without the motion. Note whether the blur arrival reads as a pop, since the
  region is published once from the landed shape. Repeat with reduced motion,
  which must open with no animation at all.
  Open a foreign tray child menu from the mapped inventory
  on both sides of the output and confirm the same droplet grows sideways out
  of the parent card's facing edge at the invoking tile's height, with the
  mouth following that tile. Scroll an overflowing foreign menu and confirm
  its header card and section label stay pinned, no row is painted above the
  dark body section, and no separate scroll bar appears while wheel, keyboard
  and drag still scroll the rows. On a cold opening, step through
  the first frames and confirm the membrane remains aligned while placement
  moves from its bootstrap value to the final card position; only opacity may
  reveal that surface, and its blur must not arrive noticeably after opacity
  has finished. On a sufficiently high output, confirm the control centre has
  enough membrane travel. On a 768-pixel output, confirm the 732-pixel card
  keeps its body origin at y=72 and never paints or blurs over the 40-pixel
  panel; record the expected 36-pixel bottom clipping as a prototype limit
  rather than accepting that low-height interaction path as complete.
  Arm and disarm one session action and make sure its changing copy neither
  moves the card onto the panel nor shrinks the outside-click surface. In the
  tray inventory, pin one item, hide another and use the selector beside
  `Aplicaciones` to alternate between visible and hidden icon grids. Confirm
  that only the eye glyphs are painted, their accessible names still announce
  the mode and count, foreign application artwork is legible at the enlarged
  size, the card and its top stay fixed, producer titles are not painted, the
  last tile remains reachable by scrolling and keyboard, and the hidden tile
  can be restored. While that menu stays open, change pin/hide state so the
  panel tray capsule moves or changes width; confirm the clicked tray glyph and
  membrane waist update together while the capsule itself remains unchanged,
  and that the membrane never detaches against a stale rectangle. Confirm the
  pin appears directly beside the opener and that
  opening either item's foreign menu leaves the inventory visible. Exercise a
  foreign menu taller than the output with the wheel, draggable scroll bar and
  arrow keys; reach its final action, then press Escape and confirm only the
  child closes. Hover every compact shell action that carries an assistive name
  and confirm its hover feedback remains but no tooltip is painted. In the
  wallpaper menu choose a local folder, confirm its supported images appear as
  thumbnails, traverse every page when it contains more than 64 accepted
  images, click one bright and one dark image without reopening the menu, and
  confirm that only the invoking output's wallpaper changes immediately. The
  foreground must remain light/white and the content material dark for both
  images. Place the launcher so one half crosses a uniform application window
  and the other half crosses the wallpaper. With the documented Celestina layer
  rule active, confirm each half blurs the scene actually below it rather than
  both halves reusing the wallpaper. Repeat above one bright and one dark
  application, confirm the same fixed foreground/material pair remains legible,
  then move a tiled window and open/close the surface to record Niri's
  documented experimental non-xray artifacts and the practical GPU cost.
- **Pass condition:** composed backdrop detail is visibly dispersed throughout
  one finite edge-to-edge panel region and remains sharp immediately below the
  40-pixel bar. The bar has no exterior margin, shadow or hard plate. Its
  ordinary rounded capsules stay at y=5 with height 30, retain both ends and add
  no compositor regions; the outer contextual field remains visibly lighter
  than its content cards, and those cards and panel capsules read as one dense
  matte dark material with one fixed light/white foreground on bright and dark
  wallpapers. A late geometry change preserves that same single panel region
  without briefly blurring outside it. The contextual bar, body and membrane
  expose no outline, lit edge or apparent halo. Every panel-opened primary
  membrane begins at `barHeight`, spans the complete contextual body at both
  outer edges, and changes its broad body-proportional, glyph-centred or
  body-edge-clamped waist with real tension while retaining rounded shoulders,
  a longer lower lobe and a continuous unpinched join. The body still follows
  the clicked control, which retains its ordinary hover circle only while its
  own surface is open.
  Every `PanelPill` and
  `ContentSurface` remains rounded, fixed at its resting geometry and
  materially unchanged; the membrane is only `ContextualVeil`, with no dense
  bridge or transition layer. Live glyph-anchor changes move or resize the
  membrane waist without changing the capsule. The membrane never repaints or
  reblurs the bar;
  its vertical travel matches every real-width row and remains independent of
  content height. It stays aligned
  throughout bootstrap and reveal, while command/keybind, workspace and foreign
  child routes retain their floating geometry. Every
  interactive menu uses the same material hierarchy without changing its
  keyboard, Escape, outside-click, two-step destructive or provider-command
  semantics. Tray
  preferences survive reconstruction without painting hidden tiles in the
  visible mode or changing the inventory geometry, both tray surfaces coexist,
  an overflowing foreign menu keeps every action reachable without adding a
  false keyboard row, no shell tooltip obscures adjacent controls, and every
  accepted wallpaper remains reachable from a user-chosen folder rather than a
  fixed source path. Interactive glass disperses the application or wallpaper
  actually composed below each pixel instead of showing the wallpaper-only xray
  cache over an application, and the fixed light/white foreground remains
  readable over the dense dark material in the tested bright/dark application
  matrix. On 768p the tall card keeps its membrane and never overlaps the
  panel blur; its 36-pixel bottom clipping is a recorded prototype limit rather
  than acceptance of complete low-height interaction.
- **Result:** the author accepted the `PANEL-1-A` nested-session baseline after
  iterative review: the hard plate was gone, the full-width shadow remained,
  capsules showed compositor blur without borders, the phone capsule retained
  both ends, monitor groups remained distinct, the status/action glyphs were
  readable and the four-item tray returned after its visibility correction.
  This did not cover a separate scale-1 and scale-2 matrix, so the checkpoint is
  partial and stays open with `PANEL-1-B`. On 2026-08-10 an agent-run nested
  scale-1 comparison exercised the final rebuilt adaptive foreground without
  replacing the host session: the requested dark wallpaper used light ink, the
  previous bright wallpaper changed the panel and an already-open control centre
  to dark ink, and restoring the dark wallpaper changed both back. The temporary
  selector was removed. Automated coverage also replaces bytes at one unchanged
  path and requires their new revision to reach `Image.Ready` before its ink is
  admitted. This is live implementation evidence but not author acceptance of
  the remaining menu, scale or multi-output matrix. A later agent-run scale-1
  cycle rebuilt only the nested Celestina host, inspected the compact semantic
  panel groups, and kept the corrected compositor blur armed for more than 30
  seconds and through a whole-output capture. Contextual hierarchy, placement,
  keyboard/dismissal lifecycle and the new performance/capture actions passed
  focused integration tests; scripted nested pointer opening of those two new
  menus and author perceptual acceptance remain pending. The latest extension
  passed the canonical production exit (17 of 17 CTest cases) and was deployed
  without activation. A targeted restart replaced only Celestina in the
  already-running nested `wayland-2` session, kept host Noctalia alive and
  produced `/tmp/celestina-contextual-tools-live.png` with the count-free tray,
  toolbox and wallpaper openers. That cycle's shell PID was 846641; nested Niri
  PID 633476, host Niri PID 1224 and Noctalia PID 1276 were preserved. Automated
  tests cover durable pin/hide/restore,
  simultaneous tray parent/child surfaces, folder-chooser hand-off, bounded
  catalogue scanning, stale catalogue/file rejection and atomic per-output
  import. The native portal folder chooser and those tray gestures have not been
  clicked in the nested compositor; its parallel portal connection emitted an
  application-id ownership warning, so those remain explicit author-run checks
  rather than a claimed live pass. The `PANEL-1-C` corrective reload then
  replaced only the host inside the same recorded `wayland-2` nest: shell PID
  926222 and adapters 927209/927210 own socket
  `/run/user/1000/niri.wayland-2.865247.sock`, while nested Niri PID 865247,
  host Niri PID 1224 and Noctalia PID 1276 remained alive. The panel remapped,
  rearmed seven blur shapes and emitted no QML construction or binding error.
  Automated coverage now requires one fixed visible/hidden icon grid, disjoint
  pin and visibility hit targets, focus restoration only after the requested
  durable mode, exact theme/pixmap icon fallback and navigation across every
  64-item page of a bounded catalogue. Real pointer gestures and visual
  acceptance of that corrected menu remain pending. The D prototype then passed
  its production exit and deployed without activation: Rust shell
  core passed 333 cases, the provider adapter passed 80, the QML QuickTest
  runner passed 200 and CTest passed 17 of 17. Focused coverage requires the two
  eye controls to retain mode/count names, both foreign and fallback grid art to
  measure 23 pixels, inherited and direct-toast tooltips to stay empty and
  hidden under hover, and a 64-action foreign menu to retain its requested top
  while its bounded viewport reaches the final action by scroll mapping and
  arrow key. The first registered `PANEL-1-D` restart verified old owner PID
  926222 on `wayland-2` and socket
  `/run/user/1000/niri.wayland-2.865247.sock`, then replaced only that host with
  PID 992071 and adapters 992276/992277. After the final opener-relative
  overflow correction, the final registered restart verified that owner on the
  same nested display and socket, then replaced only it with PID 1018620 and
  adapters 1018811/1018812. Nested Niri PID 865247, host Niri PID 1224 and
  Noctalia PID 1276 remained alive; the final captured startup stream has no QML
  construction or binding error. That reload proves construction and
  isolation, not perceptual acceptance: real hover, wheel/drag and the complete
  author visual matrix remain pending. `PANEL-1-E` then reproduced the reported
  wallpaper-only blur with a uniform teal application beneath a launcher that
  crossed the application's right edge. Niri 26.04's automatic xray path was
  the cause; QML contained no wallpaper copy inside the menu. An initial live
  reload changed the over-application sample from wallpaper-like
  `srgb(42,47,43)` to `srgb(24,106,116)`, but a later surface reconstruction
  returned to the wallpaper. That changing-config sequence was rejected as
  non-durable evidence. The final control restarted only the nested compositor
  from the stable exact namespace rule, leaving host Niri PID 1224 and Noctalia
  PID 1276 untouched. With nested Niri PID 1102853, launcher pixel `(186,291)`
  above the teal application was `srgb(31,106,115)` beside the uncovered
  `(16,56)` reference `srgb(0,91,102)`, while `(686,291)` in the same glass
  above the wallpaper was `srgb(33,39,33)`.
  `/tmp/celestina-non-xray-clean-start.png` records that
  split. Closing and reopening the launcher produced the same samples in
  `/tmp/celestina-non-xray-clean-reopen.png`. Restarting only Celestina then
  created PID 1106789 and adapters 1107007/1107009 without replacing Niri; the
  same samples remained in
  `/tmp/celestina-non-xray-after-celestina-restart.png`. A second registered
  Celestina-only restart created PID 1110628 and adapters 1110890/1110891,
  again without reloading Niri. The launcher remained namespace
  `celestina-overlay`, and
  `/tmp/celestina-blur-control-launcher-clean-after-restart.png` retained the
  same values at those three coordinates. After the canonical production exit,
  a final nested-only restart loaded the verified bytes as current PID 1127567
  with adapters 1127828/1127829. The launcher retained its namespace and the
  same samples in `/tmp/celestina-blur-control-launcher-production-final.png`.
  Only the launcher namespace
  `celestina-overlay` was sampled live; the panel, primary-menu and child-menu
  namespaces share the exact validated matcher but were not separately
  sampled. The current process reports `wayland-2` and
  `/run/user/1000/niri.wayland-2.1102853.sock`; nested
  Niri remained PID 1102853. The live host Niri config was not edited. This is
  controlled scale-1 implementation evidence, not the still-pending author
  bright/dark app, motion, GPU-cost or full menu matrix. On 2026-08-11 the
  `PANEL-1-F` final nested-only restart replaced shell PID 1214100 with PID
  1224284 and adapters 1224469/1224470 on `wayland-2` and
  `/run/user/1000/niri.wayland-2.1144687.sock`. Nested Niri PID 1144687, host
  Niri PID 1224 and Noctalia PID 1276 remained alive. The session command
  `control-centre-toggle` returned `confirmed`; the opened overlay armed one
  compositor shape with 30 region fragments, and the observed stream contained
  no QML construction, required-property or binding error. This proves live
  construction, one-region ownership and session isolation.
  `/tmp/celestina-shared-glass-menu.png` records the opened card without the
  surrounding desktop: wallpaper structure remains visible through the light
  carrier and its denser sections. It is agent-run visual evidence, not author
  acceptance of the shared material. The G prototype production exit then
  passed all registered Rust, QML and C++ verification, including CTest 17/17,
  and deployed its candidate without activating the host session. Its registered
  nested-only restart replaced PID 1224284 with PID 1336218 and adapters
  1336400/1336401 while nested Niri PID 1144687, host Niri PID 1224 and
  Noctalia PID 1276 remained intact. Opening Control Centre returned
  `confirmed` and retained one compositor shape with 30 region fragments. In
  `/tmp/celestina-oneui-content-glass.png`, the bright wallpaper selects dark
  ink over dense light matte content cards while the outer contextual field
  remains nearly transparent; `/tmp/celestina-oneui-panel-glass.png` records
  the matching panel capsules. No elevation shadow or QML construction/binding
  error was observed. This remains agent-run scale-1 evidence rather than the
  complete author-owned visual matrix. The author then retracted adaptive
  foreground selection. The H prototype removed that complete computation and
  transport path, and the cumulative 0.11.0 snapshot passed the canonical
  production exit with CTest 17/17
  and deployed without activating the host session. Its nested-only restart
  replaced shell PID 1349330 with PID 1424970 and adapters 1425141/1425145,
  while nested Niri PID 1349248, host Niri PID 1224 and Noctalia PID 1276
  remained intact. Opening Control Centre returned `confirmed` and
  `/tmp/celestina-fixed-white-ink.png` records the fixed white foreground over
  dense dark content material and the near-transparent carrier on the current
  dark wallpaper. This is implementation evidence at scale 1; author review on
  a bright wallpaper and the remaining surface matrix stays pending. The
  first `PANEL-1-I` droplet experiment then passed focused Style construction
  7/7, shell QuickTest 198/198, three affected C++ tests 3/3, QML lint and the
  canonical production workflow. Its offscreen preview showed the intended
  droplet anatomy, and the workflow deployed the verified test bundle without
  activation or live-session replacement. That experiment is now superseded
  within the same active and uncommitted unit. It does not verify the current
  continuous panel veil, single panel blur region, ordinary y=5/height=30
  capsules or bar-bottom connector alignment. The next whole-capsule iteration
  opened the active `PanelPill`, used its complete width as the mouth and
  painted a dense-to-veil bridge. Its architecture contract and canonical
  Style build and verification passed. The focused
  Celestina surface-manager, overlay-contract, indicator-menu and complete
  QuickTest selection passes 4/4 with 208/208 QuickTest cases. Its
  surface-manager regressions covered live width and ancestor movement,
  hide/restore, internal reparenting, successor tokens, destruction and
  ambiguous-owner floating fallback. The registered Celestina completion
  passes its Rust suites, QML lint, CTest 17/17 and release smoke, deploys the
  verified bundle to the normal test prefix and reports every installed
  artifact current without activating a session. The earlier restricted run
  could not bind the tray-watcher fixture's private D-Bus socket in `/tmp`;
  the registered unrestricted runs pass that test. That whole-capsule result is
  now superseded. The following glyph-mouth correction kept capsules unchanged
  and made the membrane veil-only. Its focused CTest selection passed 4/4, its
  complete offscreen QuickTest runner passed 208/208, and its architecture,
  canonical Style and registered Celestina production checks passed before
  deployment without activation. That glyph-mouth geometry is also superseded:
  neither earlier result verifies the current droplet membrane or persistent
  opener circle. The next body-wide
  revision passed 210/210 but its 9..11-pixel waist read as a straight
  hourglass in the author-provided screenshot and is superseded too, and the
  fluid body-proportional-waist correction that followed passed 211/211 with
  full registered completion but was rejected live by the author on
  2026-08-11 as a strange hourglass. The current droplet revision — narrow
  glyph-centred mouth, meniscus, tangent body landing and restored rounded
  body-top corners — passes its focused selection 4/4 and
  complete offscreen QuickTest runner 211/211. Registered production completion
  passes CTest 17/17 and the eight-second release smoke.
  The verified bundle is deployed to `~/.local` and reports current without
  activating a session. The author-run nested-Niri perceptual pass remains
  pending in this active prototype record. On 2026-08-15 the author's first
  60 fps recording showed a calendar frame over the panel seam and phone/tray
  departures split between rows, material and carrier. `R8-P-N` corrected the
  departure lifecycle, and the author's next retry confirmed that
  synchronization result. The corrected source recording nevertheless shows
  four first-frame failures: calendar frames 97–101, tray inventory 176–181,
  Notification Centre 271–279 and audio 317–325 paint in the panel strip before
  becoming coherent below it. `R8-P-O` moves the layer-surface buffer boundary
  itself below the panel, fixes the Popup viewport at that seam and clips dense
  compositor regions through their ancestors. Its intermediate registered
  production completion passed without activating the main session. Nested
  Niri remained PID 80685 and the changed
  build-tree shell restarted there as PID 306790 with
  `WAYLAND_DISPLAY=wayland-2` and
  `NIRI_SOCKET=/run/user/1000/niri.wayland-2.80685.sock`. The author then
  confirmed that the panel-seam defect is fixed. A subsequent 778-frame,
  1920x1080 recording at 60 fps
  (`recording_20260816_001614.mp4`, SHA-256
  `c2aec9dee5120a8aa37a8e3f709434a3b4a4b7cfabccda791e710c2aa0548c5f`)
  exposed an independent temporal split. The bottom-right OSD first published
  bare blur at frames 113–114, paint at frame 115 and dense material at frame
  126; during departure, frames 340–341 retained bare blur after paint had
  almost vanished. A second cycle reproduced the same ordering at frames
  383/386/396 and 503–505. During an overlay switch, Launcher vanished after
  frame 516, frames 517–520 were empty, Clipboard began at frame 521 and its
  settled dense material appeared at frame 536. `R8-P-P` now couples animated
  paint and compositor regions, defers overlay readiness until a painted
  exposed swap, softly retires the previous overlay, and gives all toast
  placements one re-entrant whole-block departure. Registered production
  completion passes every Rust suite, the complete QML runner, all 23 CTest
  contracts and the eight-second smoke, and deploys Celestina 0.29.11 without
  activating the main session. The nested Niri remains PID 80685; its changed
  build-tree host is PID 464884 on `wayland-2`. Repeat two bottom-right OSD
  cycles and a Launcher-to-Clipboard switch in that nest. Paint, weak blur and
  dense material must appear, move and disappear as one block, with no empty
  handoff frames or late material snap. Test a toast only in a nest that owns
  `org.freedesktop.Notifications`; the current shared bus is owned elsewhere.
  For `R8-P-Q`, the first author retest showed that the direct foreign menu
  still appeared without its membrane: the lease named the optional foreign
  `Image`, so a visible fallback glyph left its semantic anchor hidden. The
  icon slot is now the stable source across both rendering branches. The next
  60 fps recording from PID 530038 still showed already-settled foreign menus
  at frames 131 and 198, and the host journal placed their blur at local
  `y == 21` instead of the seam. A fractional 18-pixel glyph had been widened
  to 19 pixels by `toAlignedRect()`, causing the semantic lease to reject it.
  Pending tray geometry now remains `QRectF`. The author-requested build-tree
  restart replaced PID 530038 with PID 548897 on `wayland-2`; nested Niri PID
  80685 retained its 1920x1080 scale-1 `winit` output. That host acquired
  `org.celestina.Shell`, mapped the panel and armed one panel blur shape without
  a QML construction error. The next recording confirms that membrane at
  carrier-local `y == 0`, but shows the custom header already landed while the
  foreign actions are still entering. The header now follows the exact same
  hidden distance without joining the scrollable content, and the request
  carries the real application title for the application-specific heading. Visual
  acceptance of that corrected whole-block fall, the app-specific heading and
  the left-side icon fades remains pending. The author-requested restart then
  replaced only the build-tree host with PID 565451 on the same nested output;
  it acquired `org.celestina.Shell`, mapped the panel and reported no QML
  construction error. The following screenshots exposed raw bridge IDs in the
  new app-specific heading: `Slack_status_icon_1` and
  `chrome_status_icon_1`. The adapter now keeps those raw values only as stable
  preference identity, publishes `Slack` from the app-specific Id and
  `ChatGPT` from the generic Chrome bridge's tooltip, and ignores Slack's
  transient tooltip state as a name. Repeat both pinned menu openings after the
  next build-tree restart and require the Slack and ChatGPT application-specific headings.
  That restart replaced PID 565451 with PID 579663; the new host acquired
  `org.celestina.Shell`, mapped the same 1920x1080 nested panel and published
  four tray items without a QML construction error.
- **Evidence:** [PANEL-1-A delivery](docs/evidence/2026-08-08-panel-glass-baseline.md),
  [PANEL-1-B adaptive ink nested comparison](docs/evidence/2026-08-10-panel-adaptive-ink-nested.md),
  [PANEL-1-B contextual hierarchy and grouping](docs/evidence/2026-08-10-contextual-menu-hierarchy-nested.md),
  [PANEL-1-F shared menu glass](docs/evidence/2026-08-11-contextual-menu-shared-glass.md),
  [PANEL-1-G content glass](docs/evidence/2026-08-11-one-ui-content-glass.md),
  [PANEL-1-H fixed white shell ink](docs/evidence/2026-08-11-fixed-white-shell-ink.md),
  [PANEL-1-I continuous bar veil and contextual connectors](docs/evidence/2026-08-11-edge-attached-shell-prototype.md),
  [R8-P-N panel-menu lifecycle audit](docs/evidence/2026-08-15-one-panel-menu-lifecycle.md),
  [R8-P-O panel seam carriers](docs/evidence/2026-08-16-panel-seam-carriers.md),
  [R8-P-P quiet-surface temporal lifecycle](docs/evidence/2026-08-16-quiet-surface-temporal-lifecycle.md),
  [R8-P-Q pinned tray attachment](docs/evidence/2026-08-16-pinned-tray-menu-attachment.md)

## VAL-LOCK-1 — The locked session recedes and returns

- **Status:** pending
- **Related implementation:** LOCK-1
- **Requires:** a real Niri session running the verified LOCK-1 bundle, a
  detailed wallpaper on at least one output, and the author's own passphrase
- **Procedure:** lock the session from a real binding. Confirm the wallpaper of
  each locked output appears as its own image rather than a flat canvas,
  recedes to a smaller scale and reaches an intense blur, and that the clock,
  date and prompt fade in above it and remain legible against the blurred
  photograph. Confirm no window, panel or notification content is visible at
  any point in the transition. On a second output confirm the same treatment
  uses that output's own wallpaper. Unlock with the real passphrase and watch
  the uncovering specifically: the prompt must fade out, the backdrop must
  return to full scale and sharpness, and the session must be revealed on a
  frame where the wallpaper is already in its true position — no jump in scale,
  sharpness or alignment at the moment the compositor uncovers. Enter a wrong
  passphrase first and confirm the refusal leaves the receded, blurred state
  exactly as it was and unlocks nothing. Confirm an output configured with no
  wallpaper shows the deliberate canvas rather than a black rectangle. Repeat
  with reduced motion enabled and confirm the travel is gone while the prompt
  stays legible.
- **Pass condition:** the backdrop is the output's own wallpaper rather than a
  flat canvas, no session content of any kind appears during either direction
  of the transition, the passphrase stays legible against the blurred image, a
  refusal changes nothing and unlocks nothing, the uncovering shows no jump in
  scale or sharpness at the moment the compositor reveals the session, and an
  output without a usable wallpaper degrades to today's deliberate canvas.
- **Result:** not run
- **Evidence:** none

## Closed historical observations

`VAL-SHELL-R0-BASE` and `VAL-SHELL-R2-BASE` are preserved in the
[migration evidence](../docs/evidence/2026-08-03-migrated-author-observations.md).
Current authorization rules remain in [AGENTS.md](AGENTS.md); offscreen evidence
does not substitute for any case above.
