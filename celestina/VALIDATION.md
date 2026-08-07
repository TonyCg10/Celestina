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
- **Interpretation:** recurrence disproves Celestina as a necessary condition;
  non-recurrence is strong evidence against the handover but does not identify
  the exact failing kernel, firmware, DDC, PCIe or hardware layer.
- **Evidence:** [GPU loss system audit](docs/evidence/2026-08-05-gpu-loss-system-audit.md)

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
- **Result:** deferred until numeric resource ceilings are accepted
- **Evidence:** none

## VAL-R1-01 — Integrated bar interactions

- **Status:** failed
- **Related implementation:** R1 (complete)
- **Requires:** live media, audio, DDC and tray providers
- **Procedure:** exercise artwork/transport, speaker and microphone gestures,
  DDC burst coalescing and one tray menu action with provider loss cases.
- **Pass condition:** every action reports confirmed or failed truthfully and a
  slow/missing provider does not block or stale the rest of the bar.
- **Result:** failed again on 2026-08-05. Audio, microphone, DDC, CPU/RAM,
  workspaces, OSD, tray interactions and provider isolation passed. Media is
  usable once published, including pause/resume and player disappearance and
  return, but a full Celestina start misses an already-playing Firefox source
  that `playerctl` sees. Restarting only `celestina-provider-adapter` makes that
  source appear immediately.
- **Remediation:** active `LVR-3-A` in [late provider insertion](docs/plans/active/2026-08-05-late-provider-insertion.md) owns the late provider insertion boundary.
  The 0.6.2 full-shell rerun failed again: its original helper omitted media
  visually, an isolated helper published it, and replacing only the original
  helper made it appear.
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

- **Status:** pending
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
- **Result:** not run
- **Evidence:** [the 2026-08-07 corrections](docs/evidence/2026-08-07-one-poll-is-not-the-truth.md)

## VAL-R1-NET — A slow probe does not erase a live link

- **Status:** failed
- **Related implementation:** `LVR-3-F` (complete)
- **Requires:** live Niri session, celestina 0.6.8 and a Wi-Fi link in use
- **Procedure:** use the session normally for long enough to cross several of
  the `nmcli` latency spikes measured on 2026-08-07, watching the panel's link
  text; then disconnect the link deliberately and watch it go.
- **Pass condition:** the link text never disappears while the connection is in
  use, and a real disconnection removes it within about ten seconds rather than
  persisting.
- **Result:** failed on 2026-08-07 against celestina 0.6.8. The Wi-Fi link
  remained connected and in use, but its panel reading still disappeared.
  `LinkTracker` retains an unreadable observation for only three five-second
  polls and withdraws the provider on the fourth; the corrective policy does
  not yet survive the longer unreadable run observed live.
- **Remediation:** implemented by `LVR-3-G` in [late provider insertion](docs/plans/active/2026-08-05-late-provider-insertion.md). The unreadable
  hold is removed rather than raised: a probe that saw nothing can no longer
  retire a link at any repetition count, and only a poll that positively found
  no default route can — twice in a row, so about ten seconds. A route naming a
  device the device list cannot explain is now classified as unreadable rather
  than as a disconnection, which is what a re-associating card looks like. The
  live rerun remains required and this case stays failed until the author runs
  it.
- **Evidence:** [what a probe did not see](docs/evidence/2026-08-07-what-a-probe-did-not-see.md)

## VAL-R1-DDC — Prompt brightness rediscovery on output hotplug

- **Status:** pending
- **Related implementation:** `LVR-3-F` (complete)
- **Requires:** live Niri session, celestina 0.6.8, and an output that can be
  disabled and enabled
- **Procedure:** start with one output disabled, enable it, and watch for that
  monitor's brightness control. Then disable it again.
- **Pass condition:** the control appears within seconds of the panel mapping
  rather than at the 300-second refresh, only one `ddcutil` process exists at
  any moment during the transition, and no `ddcutil` survives shutdown.
- **Result:** not run
- **Evidence:** [one poll is not the truth](docs/evidence/2026-08-07-one-poll-is-not-the-truth.md)

## VAL-R5-BT — Bluetooth stays visible while the adapter is on

- **Status:** pending
- **Related implementation:** `LVR-3-F` (complete)
- **Requires:** live Niri session, celestina 0.6.8, and an adapter that can be
  powered off and on
- **Procedure:** with nothing paired, read the panel and the control centre;
  connect a device and read both again; power the adapter off and read both
  again.
- **Pass condition:** a powered adapter with no connections reads as present and
  idle rather than vanishing, a connected device is counted and named, and a
  powered-off adapter says so instead of disappearing.
- **Result:** not run
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

- **Status:** failed
- **Related implementation:** `LVR-3-F` (complete)
- **Requires:** a live Niri session, celestina 0.6.8, and Slack and Solaar
  registered as StatusNotifierItems
- **Procedure:** inspect the watcher's registry and each item's `GetAll`
  response, then open the tray drawer and compare the rendered controls.
- **Pass condition:** every bounded active registration that answers with a
  usable item is present in the open drawer, with a name fallback when its icon
  cannot be resolved.
- **Result:** failed on 2026-08-07. Celestina's watcher listed four active
  registrations. Slack's Chromium item answered with its pixmap and menu;
  Solaar answered with `battery-good`, title `Solaar` and its menu. The author
  still reported both absent from the tray. This proves registration and
  foreign-item properties are healthy, but does not yet isolate whether the
  loss is in the host's published model, the drawer's collapsed/open state or
  right-flank clipping. The next pass must observe those three boundaries
  separately rather than modify D-Bus parsing on inference.
- **Remediation:** implemented by `LVR-3-G` in [late provider insertion](docs/plans/active/2026-08-05-late-provider-insertion.md), which found a
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
  inference this build cannot settle. No status is rewritten, nothing is
  permanently unfolded and no application is special-cased. This case stays
  failed until the author opens the drawer and reports what is in it.
- **Evidence:** [what a probe did not see](docs/evidence/2026-08-07-what-a-probe-did-not-see.md)

## VAL-R2-01 — Deferred launcher edge cases

- **Status:** deferred
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

## VAL-R4 — Notification server, toasts and handover

- **Status:** failed
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
- **Evidence:** [2026-08-05 follow-up](docs/evidence/2026-08-05-live-validation-follow-up.md)

## VAL-R5 — Control centre, weather and calendar

- **Status:** deferred
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
- **Evidence:** [2026-08-05 follow-up](docs/evidence/2026-08-05-live-validation-follow-up.md)

## VAL-R7 — Wallpaper, portal values and Niri colours

- **Status:** deferred
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

## Closed historical observations

`VAL-SHELL-R0-BASE` and `VAL-SHELL-R2-BASE` are preserved in the
[migration evidence](../docs/evidence/2026-08-03-migrated-author-observations.md).
Current authorization rules remain in [AGENTS.md](AGENTS.md); offscreen evidence
does not substitute for any case above.
