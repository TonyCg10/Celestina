# MAG-P6 — The mirror as a capability of the link

- **Opened:** 2026-09-09
- **Closed:** 2026-09-10
- **Plan ID:** link-mirror
- **Status:** done
- **Authorization:** the author said "sigue con todo" on 2026-09-09 with
  `MAG-P5`'s exit met
- **Scope:** magnetita, magnetitad, magnetita-mobile, magnetita-peer
- **Implementation checkpoint:** MAG-P6
- **Author-validation checkpoint:** `VAL-MAG-14` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)
- **Successor:** MAG-P7

## Hypothesis

The mirror is one more capability of the paired link: the phone captures
and encodes on consent, the raw stream travels on a bulk stream of a fixed
id, the daemon presents it in a window it owns through the tools already
on the host, and touches go back as messages. Nothing needs `adb`, a
developer setting or a reboot ritual.

## Tangible outcome

The desktop app's Mirror control opens the phone's screen over the link
when the daemon offers it and falls back to the `adb` path otherwise;
`Mirror1` gains link methods next to the old ones.

## Scope

- `MAG-P6-A` — the phone's side of the wire in the core and the peer: a
  bulk stream opened by fixed id, `MirrorStarted` and `MirrorStop`, the
  desktop's mirror messages decoded into events; the peer answers a start
  by streaming a file; the wire document names the stream ids.
- `MAG-P6-B` — the daemon's mirror: the intent, the session's tick, the
  video stream into a decoder window (`ffmpeg` remux into `mpv` on the
  session's display), owned by the session that streams.
- `MAG-P6-C` — input back: `Mirror1`'s `StartLink`, `StopLink`,
  `LinkState`, `LinkTouch`, `LinkKey`, `LinkGlobal`, queued to the phone.
- `MAG-P6-D` — the desktop app's Mirror control prefers the link and the
  card shows the link mirror's state.

## Exclusions

- Audio capture (`AUDIO_STREAM` is reserved and ignored), recording, OTG,
  screen-off mirroring.
- The phone's capture and accessibility services: `magnetita-android`'s
  `AND-4`.
- Retiring the `adb` path: `MAG-P7`, after `VAL-MAG-14`.

## Build order

1. `MAG-P6-A` and `-B`, then `-C`, then `-D`.

## Implementation exit

The loopback test streams bytes on the fixed id into the recording window
and carries a touch back; a synthetic HEVC stream survives the daemon's
remux; `scripts/complete-production.sh` passes; the mirror after a phone
reboot on the S25U is `VAL-MAG-14`.

Met on 2026-09-09 as far as the records allow: the loopback and the
synthetic stream pass, production deployed, and the S25U received the
capture request live. The picture on the author's desk after a reboot is
`VAL-MAG-14`, which stays pending; `MAG-P7-D` waits on it.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| MAG-P6-A | `magnetita:` | done | [inventory](../../inventories/2026-09-09-link-mirror/MAG-P6-A.numstat.tsv) | 8 files, +366/-5 | The core's mirror stream and messages; the peer streams a file; the wire document | [record](../../evidence/2026-09-09-mirror-stream.md) | `VAL-MAG-14` |
| MAG-P6-B | `magnetita:` | done | [inventory](../../inventories/2026-09-09-link-mirror/MAG-P6-B.numstat.tsv) | 9 files, +814/-18 | The daemon's mirror intent, stream and decoder window; `MAG-P5` archived and `MAG-P6` opened | [record](../../evidence/2026-09-09-mirror-window.md) | `VAL-MAG-14` |
| MAG-P6-C | `magnetita:` | done | [inventory](../../inventories/2026-09-09-link-mirror/MAG-P6-C.numstat.tsv) | 4 files, +194/-1 | `Mirror1`'s link methods and input back | [record](../../evidence/2026-09-09-mirror-input.md) | `VAL-MAG-14` |
| MAG-P6-D | `magnetita:` | done | [inventory](../../inventories/2026-09-09-link-mirror/MAG-P6-D.numstat.tsv) | 4 files, +135/-9 | The Mirror control prefers the link | [record](../../evidence/2026-09-09-mirror-control.md) | `VAL-MAG-14` |
