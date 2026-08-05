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
- **Result:** audio, microphone, DDC, per-output workspaces, OSD and focus
  behaviour passed their exercised paths, but Firefox exposed a playing MPRIS
  title and no media widget appeared on either panel. The later invalid
  notification frame also withdrew every unrelated bar provider and exposed an
  unguarded `AudioLevel` accessibility binding.
- **Remediation:** `LVR-1-A` in [the live validation remediation plan](docs/plans/archive/2026-08-04-live-validation-remediation.md), delivered in celestina 0.6.1: the media widget was being clipped out of the panel by the workspace strip, and the audio widget's accessible text read an absent provider; both are corrected and covered by PanelFlank and AudioLevel regressions..
  Ready for the author to run this case again; it is not passed until
  they do.
- **Evidence:** [2026-08-04 live validation failures](docs/evidence/2026-08-04-live-validation-failures.md)

## VAL-R1-02 — StatusNotifierWatcher takeover

- **Status:** pending
- **Related implementation:** R1 (complete)
- **Requires:** permission to stop the current watcher owner temporarily
- **Procedure:** inspect owners/items, remove the existing watcher and observe
  Celestina claiming the name and retaining registrations.
- **Pass condition:** exactly one watcher owns the name and valid tray items
  remain available through takeover and rollback.
- **Result:** partial; Celestina became the sole watcher and retained the three
  observed Solaar, NetworkManager and Blueman registrations. Item activation,
  context-menu behaviour and the rollback were not run before testing stopped.
- **Evidence:** [2026-08-04 live validation failures](docs/evidence/2026-08-04-live-validation-failures.md)

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
- **Evidence:** [2026-08-04 live validation failures](docs/evidence/2026-08-04-live-validation-failures.md)

## VAL-R2-02 — Clipboard deletion and empty-state dismissal

- **Status:** failed
- **Related implementation:** R2 (complete)
- **Requires:** live shell with at least one ordinary clipboard entry
- **Procedure:** open clipboard history, remove one row through its visible and
  keyboard paths, choose `Vaciar`, then dismiss the empty overlay with Escape
  and reopen it.
- **Pass condition:** deletion is discoverable without a context-menu guess,
  every delete path is keyboard and assistive-technology reachable, and the
  overlay retains a normal dismissal path when no list row exists.
- **Result:** the populated keyboard and context-menu paths worked, but no
  visible per-row delete action was discoverable. After `Vaciar`, the list that
  owned the key handler disappeared and the empty overlay could not be closed
  normally; `celestina msg clipboard-toggle` was the external workaround.
- **Remediation:** `LVR-1-A` in [the live validation remediation plan](docs/plans/archive/2026-08-04-live-validation-remediation.md), delivered in celestina 0.6.1: the emptied clipboard keeps the keyboard and Escape still closes it, and each row carries a visible, Tab-reachable, named delete beside the unchanged Delete/Backspace and context-menu paths..
  Ready for the author to run this case again; it is not passed until
  they do.
- **Evidence:** [2026-08-04 live validation failures](docs/evidence/2026-08-04-live-validation-failures.md)

## VAL-R3 — Session verbs and lifecycle

- **Status:** deferred
- **Related implementation:** R3 (complete)
- **Requires:** R3 automated exit green plus explicit permission for each live
  mutation
- **Procedure:** exercise OSD, gamma release, caffeine/idle and DPMS with
  rollback ready; confirm lock-and-suspend refuses while no provider exists.
- **Pass condition:** each provider-confirmed state is truthful, external
  lifecycles release cleanly and the refusal path never suspends unlocked.
- **Result:** partial; night light and caffeine each acquired and released their
  external hold, and both locker verbs refused safely. DPMS, forced child death,
  helper restart and resume were not run, so the full case remains deferred.
- **Evidence:** [2026-08-04 live validation failures](docs/evidence/2026-08-04-live-validation-failures.md)
  records the live subset; [R3 completion](docs/evidence/2026-08-04-r3-completion.md)
  records the automated exit

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
- **Result:** name handover succeeded, but the first ordinary `Notify` published
  a nested action list that the C++ host decoder rejects. The invalid aggregate
  frame then cleared Wi-Fi, Bluetooth, audio, CPU and RAM state, so testing
  stopped before replacement, close, actions, DND, history or accessibility.
- **Remediation:** `LVR-1-A` in [the live validation remediation plan](docs/plans/archive/2026-08-04-live-validation-remediation.md), delivered in celestina 0.6.1: notification actions now travel as a bounded flat sibling list the C++ decoder accepts, and an unreadable frame no longer clears unrelated providers..
  Ready for the author to run this case again; it is not passed until
  they do.
- **Evidence:** [2026-08-04 live validation failures](docs/evidence/2026-08-04-live-validation-failures.md)
  records the live failure; [R4 notifications](docs/evidence/2026-08-04-r4-notifications.md)
  covers the automated and private-bus paths

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
- **Result:** partial; the exercised normal controls and two-step session-menu
  paths behaved correctly. No location was configured, so absent weather was
  expected; forced write failure, restart persistence and screen-reader paths
  remain deferred. Visible copy was English and is tracked separately.
- **Evidence:** [2026-08-04 live validation failures](docs/evidence/2026-08-04-live-validation-failures.md)

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
- **Result:** partial; both physical outputs showed their shell wallpaper and no
  black fallback was reported. Hotplug, portal-value consumption and Niri
  colour comparison remain deferred; startup diagnostics are tracked in
  `VAL-SHELL-03`.
- **Evidence:** [2026-08-04 live validation failures](docs/evidence/2026-08-04-live-validation-failures.md)

## VAL-SHELL-03 — Live accessibility and application identity diagnostics

- **Status:** failed
- **Related implementation:** S0/R7 (complete)
- **Requires:** verified production bundle activated on a live Niri session
- **Procedure:** start the two-output shell, retain its startup diagnostics and
  exercise portal registration while the wallpaper surfaces map.
- **Pass condition:** every `Accessible` property is attached to a supported
  QML object, the deployed application id is discoverable by the host portal,
  and neither path emits a runtime contract error.
- **Result:** each wallpaper surface reported that `Accessible` was attached to
  an unsupported root `Window`, and Qt failed host-portal registration because
  application information for `celestina` was not found.
- **Remediation:** `LVR-1-A` in [the live validation remediation plan](docs/plans/archive/2026-08-04-live-validation-remediation.md), delivered in celestina 0.6.1: wallpaper accessibility hangs on an Item rather than the Window, and the desktop entry is a sealed production artifact installed to the user's applications directory..
  Ready for the author to run this case again; it is not passed until
  they do.
- **Evidence:** [2026-08-04 live validation failures](docs/evidence/2026-08-04-live-validation-failures.md)

## VAL-COPY-01 — Spanish product copy

- **Status:** failed
- **Related implementation:** current QML surfaces and the Spanish product-copy
  contract
- **Requires:** verified production bundle and each implemented overlay exposed
  at least once
- **Procedure:** open the launcher, clipboard, notification centre, control
  centre and session menu and inspect visible and accessibility copy as one
  product surface.
- **Pass condition:** every person-facing string is Spanish throughout each
  surface; no half-translated overlay remains.
- **Result:** the control centre and session menu visibly remained in English;
  the same product-copy pass has not yet been validated across all overlays.
- **Remediation:** `LVR-1-A` in [the live validation remediation plan](docs/plans/archive/2026-08-04-live-validation-remediation.md), delivered in celestina 0.6.1: every exposed surface is Spanish, including the panel title that was still English..
  Ready for the author to run this case again; it is not passed until
  they do.
- **Evidence:** [2026-08-04 live validation failures](docs/evidence/2026-08-04-live-validation-failures.md)

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
- **Result:** deferred; the report is incomplete, live failures now have the
  corrective LVR-1 checkpoint, and the tool refuses on purpose
- **Evidence:** none

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
