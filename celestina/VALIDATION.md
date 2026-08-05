# Celestina author validation

This is the manual validation lane. It is entered only when the author asks to
exercise the live Niri session, hardware, appearance or accessibility. Pending
or deferred cases here do not keep an implementation milestone open.

A failed case records its result here and creates a new corrective
implementation unit; it does not rewrite the completed milestone.

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
- **Remediation:** implemented in 0.6.2 by `LVR-2-A`; a bounded startup probe
  regression passes, and the live full-shell rerun remains required.
- **Evidence:** [2026-08-05 follow-up](docs/evidence/2026-08-05-live-validation-follow-up.md)

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

- **Status:** failed
- **Related implementation:** R3 (complete)
- **Requires:** R3 automated exit green plus explicit permission for each live
  mutation
- **Procedure:** exercise OSD, gamma release, caffeine/idle and DPMS with
  rollback ready; confirm lock-and-suspend refuses while no provider exists.
- **Pass condition:** each provider-confirmed state is truthful, external
  lifecycles release cleanly and the refusal path never suspends unlocked.
- **Result:** failed on final rollback. OSD, night-light and caffeine toggles,
  forced child death, aggregate-helper restart, DPMS/wake recovery and both
  locker refusals behaved correctly. After Celestina exited, however, four
  reparented `systemd-inhibit --what=idle:sleep` children remained and blocked
  explicit suspend until terminated individually.
- **Remediation:** implemented in 0.6.2 by `LVR-2-A`; a process regression
  proves SIGTERM releases an active held child before helper exit, and the live
  repeated lifecycle rerun remains required.
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
- **Remediation:** implemented in 0.6.2 by `LVR-2-A`; Escape is owned by the
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
