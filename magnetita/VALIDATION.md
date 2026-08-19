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

## VAL-MAG-06 — The hardened boundaries against the real phone

- **Status:** pending
- **Related implementation:** `MAG-S1-A`, `MAG-S1-C`, `MAG-S1-E`
- **Requires:** the corrected daemon deployed, the author's real phone with the
  stock KDE Connect Android client, and the usual LAN
- **Procedure:** connect from both initiators; confirm the phone still pairs,
  reconnects as trusted, mounts its storage and shows its volumes; raise,
  update and cancel a phone notification; set a phone device name containing
  `<b>bold</b>` and a media title containing markup and read every Magnetita
  surface that shows them; then inspect `privateKey.pem`'s mode on disk
- **Pass condition:** pairing, reconnection and the mount all still work with
  no visible change; notifications still replace and withdraw correctly; the
  markup is drawn literally on every surface rather than rendered; the key is
  `0600` and its directory `0700`
- **Result:** not run — the corrections were made without a production build,
  so the installed daemon is still the uncorrected one
- **Evidence:** dates, the phone's announced protocol version, the log lines
  around the handshake, a screenshot of each surface showing the literal
  markup, and the `stat` output for the key

## VAL-MAG-07 — Bounded clipboard and mount under a real compositor

- **Status:** pending
- **Related implementation:** `MAG-S1-D`
- **Requires:** the corrected daemon deployed, a live Wayland session with
  `wl-copy`/`wl-paste`, and a paired phone with storage permission granted
- **Procedure:** copy text on the phone and confirm it lands on the desktop
  selection and survives — the desktop clipboard must still hold it a minute
  later, with the backgrounded `wl-copy` alive; mount and unmount the phone's
  storage several times; then disconnect the phone mid-mount
- **Pass condition:** the clipboard value persists (the process-group teardown
  must not kill `wl-copy`'s background child), the mount appears and is
  browsable, unmount leaves no stale path, and no operation freezes the link
- **Result:** not run — this is the correction most exposed to a difference
  between `sh` in a unit test and the real tools, and it has not been observed
- **Evidence:** `pgrep -a wl-copy` after a copy, `findmnt` around each mount,
  and the daemon log for the interval

## VAL-MAG-08 — One-button mirror against the real S25U

- **Status:** pending
- **Related implementation:** `MAG-R1`
- **Requires:** the mirror-capable daemon deployed, the phone and desktop on the
  same LAN, and Wireless debugging enabled on the phone
- **Procedure:** with the phone never before paired for wireless debugging on
  this host, press Mirror and complete the one pairing step the app offers;
  confirm scrcpy opens. Then turn Wireless debugging off and on, which gives the
  phone a new random port, and press Mirror again without touching anything
  else. Suspend and resume the desktop and repeat. Finally, start an unrelated
  `scrcpy` by hand, stop the mirror from the app, and confirm the unrelated
  window survives
- **Pass condition:** the first mirror needs no terminal, no address and no
  port; every later mirror needs no input at all; the app explains rather than
  hangs when Wireless debugging is off; and stopping the mirror never kills a
  scrcpy this daemon did not start
- **Result:** partially observed on 2026-08-19, by hand rather than through the
  app. With Wireless debugging on, the S25U advertised at `10.0.0.190:39799`;
  `adb connect` failed on trust alone while the port accepted TCP, so the phone
  was paired from its discovered pairing port `41059` with only the six digits
  read off the screen, after which connect reached `device` state and scrcpy
  came up on the daemon's exact argument vector. The phone's address was not
  the one `~/Scripts/cpy.sh` hardcodes, so the old script would have declared it
  unreachable. Recorded in
  [mirror discovery evidence](docs/evidence/2026-08-19-mirror-discovery.md).
  The daemon was then deployed and driven over the bus: it discovered the phone
  unprompted and reached `mirroring` with a scrcpy of its own. Toggling Wireless
  debugging **failed** the reconnection clause — the port moved to `45461` and
  `adb` followed, but the mirror stayed dark, because scrcpy exits before the
  mDNS record lapses and that exit was being read as the author closing the
  window. Corrected and unit-covered; the corrected daemon is not yet deployed,
  so the toggle has not been re-run. The control has still not been pressed in
  the app itself
- **Evidence:** `avahi-browse -rtp _adb-tls-connect._tcp` before and after the
  toggle to record that the port changed, `adb devices -l`, and the daemon log

## Closed historical observations

`VAL-MAG-1.0` is preserved in the
[migration evidence](../docs/evidence/2026-08-03-migrated-author-observations.md).
