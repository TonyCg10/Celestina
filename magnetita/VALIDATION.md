# Magnetita author validation

This manual lane requires the real phone, LAN, mounts or Wayland session. It
does not contain implementation and does not block [ROADMAP.md](ROADMAP.md).

## VAL-MAG-01 — Hardened pairing from both initiators

- **Status:** pending
- **Related implementation:** post-1.0 pairing-v8 hardening
- **Requires:** verified app/daemon artifacts, a disposable forgotten pairing
  and the stock KDE Connect Android client
- **Procedure:** pair once from the phone and once from the desktop; compare the
  temporary code, accept/reject, restart both sides and reconnect as trusted
- **Pass condition:** both sides show the same code only during the exchange,
  it clears on resolution, rejection leaves no trust and reconnect uses the
  stable pin without inventing a new code
- **Result:** not run after the hardening
- **Evidence:** dates, initiator, displayed codes redacted to equality only and
  relevant connection-log events

## VAL-MAG-02 — Revocation during file and artwork payloads

- **Status:** pending
- **Related implementation:** durable `Forget` publication barrier
- **Requires:** paired phone and payloads long enough to overlap revocation
- **Procedure:** start one incoming file transfer and one artwork transfer, use
  `Forget` before each completes and inspect destination/runtime state
- **Pass condition:** neither completed result publishes after revocation,
  partial data is removed and reconnect requires a fresh pairing
- **Result:** not run on real payloads after the hardening
- **Evidence:** payload sizes, timing, resulting paths and log events

## VAL-MAG-03 — Ping in both directions

- **Status:** pending
- **Related implementation:** CP0 trusted channel
- **Requires:** connected trusted phone
- **Procedure:** send a ping from Magnetita and then from the phone
- **Pass condition:** the phone presents the desktop ping and Magnetita records
  the phone-originated ping exactly once
- **Result:** only the send path was previously observed
- **Evidence:** dated phone/app observations and event log

## VAL-MAG-04 — Corrected media and responsive app UI

- **Status:** pending
- **Related implementation:** MPRIS/action/progress/artwork hardening
- **Requires:** real phone media session, Magnetita window and a desktop player
- **Procedure:** for 60 seconds exercise live/finite progress, capability
  buttons, artwork failure/retry, transport both directions and ten burst
  device/settings refreshes while recording Qt event-loop gaps
- **Pass condition:** controls match capabilities, progress is truthful, retry
  recovers, actions stay ordered and no recorded Qt event-loop gap exceeds
  100 ms
- **Result:** not run after the corrected paths
- **Evidence:** media source, visible states, response timing and logs

## VAL-MAG-05 — Always-on mount resource cost

- **Status:** deferred
- **Related implementation:** CP2 mount lifecycle
- **Requires:** connected/mounted phone, stable idle session and accepted
  numeric ceilings for daemon/app PSS, descriptors and wakeups
- **Procedure:** record daemon/app PSS, descriptors, wakeups and mount activity
  over a declared idle interval, then disconnect/reconnect once
- **Pass condition:** every sample stays at or below its predeclared ceiling,
  no metric grows monotonically across the interval, disconnect unmounts,
  reconnect remounts once and no stale device path remains
- **Result:** deferred until numeric resource ceilings are accepted
- **Evidence:** commands, interval, samples and mount paths

## Closed historical observations

`VAL-MAG-1.0` is preserved in the
[migration evidence](../docs/evidence/2026-08-03-migrated-author-observations.md).
