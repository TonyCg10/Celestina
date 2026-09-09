# Mirror capture: `MediaProjection` in the own app, or `scrcpy` as the engine

- **Opened:** 2026-09-04
- **Status:** applied
- **Question:** does the integral mirror capture the screen from the own
  application over the own link, or keep `scrcpy-server` as the engine under a
  first-party client?

## Context

The author wants the mirror to be part of the protocol and the project rather
than an external tool. Today it is `adb` plus `scrcpy` over Android's
wireless debugging (`MAG-R1`, `MAG-R2`), which works well and has two costs
the author has felt: wireless debugging is off after every phone reboot, and
the mirror is a separate trust domain, port and process from the link. This
blocks `MAG-P6` and decides whether the app needs `MediaProjection`, an
accessibility service and a video encoder.

## Strongest case

Capturing with `MediaProjection` and encoding with `MediaCodec` inside the own
app puts the mirror on the paired link: it survives phone reboots, needs no
`adb`, uses the same trust and the same discovery, and the video is just
another QUIC stream next to the clipboard. Input goes back on the same link
and is injected by an accessibility service (`dispatchGesture` for taps,
swipes and drags; global actions for Back/Home/Recents; text into the focused
field). The desktop decodes with the video stack the suite already ships
(`fluorita-engine` over `libmpv` can play a raw elementary stream from a
file descriptor; a small `ffmpeg`-based decoder is the alternative if that
seam is judged too wide) and shows it in a window the daemon owns. Audio
comes from `AudioPlaybackCapture` on the same projection.

## Counter-case

`scrcpy-server` runs as the `shell` user, which is why it can inject input
with near-zero latency, capture without a consent dialog, see secure
surfaces, and turn the phone screen off while mirroring. An ordinary app
cannot: Android 14+ asks for consent on every capture session, secure and
DRM surfaces render black, apps may opt out of audio capture, and
accessibility-service gestures are visibly slower than `shell` injection and
cannot do everything (no raw key events into arbitrary apps). The author's
own-mirror could be a worse mirror.

## Alternatives

- Own app for capture and the link (survives reboot, integral), `adb`-backed
  `scrcpy` kept as an optional "precision" mode when wireless debugging
  happens to be on. Two paths, which the suite's rules only allow while one
  is proving itself.
- Keep `scrcpy-server` (Apache-2.0, vendorable into a GPL-3 project) as the
  capture engine pushed over `adb`, and replace only the `scrcpy` client with
  a first-party decoder window. Integral on the desktop, still not on the
  link, still dies with wireless debugging.
- Shizuku-style elevation: the app receives `shell` privileges from an
  `adb`-started service once per boot. The same reboot cost, in a different
  place.

## Falsifiers and evidence needed

`MAG-P0-B`: on the S25U, `MediaProjection` → HEVC → QUIC → desktop decode,
glass-to-glass latency measured with a millisecond clock filmed on both
screens, at 1080p60; and `MAG-P0-C`: tap and drag latency through an
accessibility service. The author judges the numbers. Latency the author
calls unusable keeps `scrcpy` as the mirror and drops the `mirror`
capability from `MAG-P6`; acceptable latency retires the `adb` path in
`MAG-P7` or keeps it as precision mode by the author's choice.

## Conclusion

**`MediaProjection` in the own application**, concluded by the author on
2026-09-04: the mirror belongs on the paired link, survives reboots and
needs no `adb`. The counter-case is accepted as the cost — consent per
session, black secure surfaces, accessibility-service gestures — and
`MAG-P0-B`/`MAG-P0-C` measure it so the cost is known before `MAG-P6`. The
`adb`/`scrcpy` path stays untouched through `MAG-P6` and is retired in
`MAG-P7` once `VAL-MAG-14` observes the own mirror after a phone reboot; it
is not kept as a precision mode, because two mirrors would be two active
paths. The desktop decodes through the `fluorita-engine` `libmpv` seam as a
bounded exception under
[ADR 0005](../../../docs/decisions/0005-bounded-qt-bridge-crates.md), not a
second decoder. Applied in
[ADR 0001](../decisions/0001-own-protocol-and-android-app.md) §7 and
`MAG-P6`/`MAG-P7`. Measured on 2026-09-09 — see
[the own-mirror record](../evidence/2026-09-09-own-mirror-latency.md):
capture to decoded frame at the median 59–67 ms, best 51 ms, p90 under
80 ms, at the phone's 120 fps over the author's Wi-Fi, with the consent
dialog, the whole-screen configuration, keep-alive and frame repetition all
found necessary and recorded for `MAG-P6`. On 2026-09-09 the author judged that
latency usable ("me sirve"); the accessibility-service input half
was measured the same night — see
[the input record](../evidence/2026-09-09-accessibility-input-latency.md):
a desktop tap lands on the phone at the median 46 ms, p90 88 ms, with the
service adding 10–20 ms over the Wi-Fi's own delay, and 60 of 60 gestures
delivered. Both halves of the counter-case are now numbers, and neither
triggers the ADR's fallback.
