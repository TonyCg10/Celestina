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

- **Status:** pending
- **Related implementation:** R1 (complete)
- **Requires:** live media, audio, DDC and tray providers
- **Procedure:** exercise artwork/transport, speaker and microphone gestures,
  DDC burst coalescing and one tray menu action with provider loss cases.
- **Pass condition:** every action reports confirmed or failed truthfully and a
  slow/missing provider does not block or stale the rest of the bar.
- **Result:** not run
- **Evidence:** none

## VAL-R1-02 — StatusNotifierWatcher takeover

- **Status:** pending
- **Related implementation:** R1 (complete)
- **Requires:** permission to stop the current watcher owner temporarily
- **Procedure:** inspect owners/items, remove the existing watcher and observe
  Celestina claiming the name and retaining registrations.
- **Pass condition:** exactly one watcher owns the name and valid tray items
  remain available through takeover and rollback.
- **Result:** not run
- **Evidence:** none

## VAL-R2-01 — Deferred launcher edge cases

- **Status:** deferred
- **Related implementation:** R2 (complete)
- **Requires:** screen reader, a real `Terminal=true` entry, a deliberately
  failing desktop entry and a password-manager clipboard selection
- **Procedure:** exercise announcements, both launch paths, error feedback and
  sensitive-selection exclusion.
- **Pass condition:** semantics and visible outcomes match each contract without
  persisting sensitive content.
- **Result:** deferred until the required clients/data are present
- **Evidence:** none

## VAL-R3 — Session verbs and lifecycle

- **Status:** deferred
- **Related implementation:** R3 (complete)
- **Requires:** R3 automated exit green plus explicit permission for each live
  mutation
- **Procedure:** exercise OSD, gamma release, caffeine/idle and DPMS with
  rollback ready; confirm lock-and-suspend refuses while no provider exists.
- **Pass condition:** each provider-confirmed state is truthful, external
  lifecycles release cleanly and the refusal path never suspends unlocked.
- **Result:** deferred until the author authorizes each live mutation; the R3
  automated exit is green and the verified bundle is deployed under `~/.local`
- **Evidence:** [R3 completion](docs/evidence/2026-08-04-r3-completion.md) covers
  the automated exit only, never the live checks above

## VAL-R4 — Notification server, toasts and handover

- **Status:** deferred
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
- **Result:** deferred until the author stops the session's current server and
  authorizes the handover; R4's automated exit is green and its verified bundle
  is deployed under `~/.local`
- **Evidence:** [R4 notifications](docs/evidence/2026-08-04-r4-notifications.md)
  covers the automated exit and a private bus only, never the live checks above

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
- **Result:** deferred until the author runs it against the deployed bundle;
  R5's automated exit is green
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
