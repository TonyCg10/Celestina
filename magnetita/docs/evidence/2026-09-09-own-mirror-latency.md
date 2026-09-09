# The own-app mirror from the S25U: capture, HEVC, QUIC, decode — MAG-P0-B

- **Date:** 2026-09-09
- **Scope:** `MAG-P0-B` of
  [`../plans/archive/2026-09-07-own-protocol-spikes.md`](../plans/archive/2026-09-07-own-protocol-spikes.md);
  verifies the [mirror discussion](../discussions/2026-09-04-mirror-capture-path.md)
- **Environment:** the phone, Wi-Fi and toolchain of
  [the QUIC phone record](2026-09-08-quic-on-the-phone.md). Phone: a
  throwaway `MirrorActivity` + `MirrorService` (foreground service of type
  `mediaProjection`), `MediaProjection` with
  `MediaProjectionConfig.createConfigForDefaultDisplay()`, a 1080×2340
  `VirtualDisplay` (`AUTO_MIRROR`) into a `MediaCodec` HEVC surface encoder
  (`c2.qti.hevc.encoder`, 12 Mbit/s CBR requested, I-frame every 2 s, no
  B-frames, `KEY_LATENCY 1`, `KEY_PRIORITY 0`,
  `KEY_REPEAT_PREVIOUS_FRAME_AFTER 16.7 ms`), output buffers written raw
  (Annex B) to one QUIC unidirectional stream by the `magspike` core
  (keep-alive 1 s, idle timeout 30 s, initial MTU 1200). Desktop: scratch
  `mirrorspike` receiver on UDP 1764 feeding `ffmpeg 9.0.1` (`-c:v hevc`,
  software, `-threads 1`, `-flags low_delay`, scaled to 270×585 grey) for
  measurement and `mpv` for a window the author watched
- **Artifact:** not applicable

## Procedure

How the latency measures itself, with no camera and no second clock: the
phone first synchronises its clock to the desktop over the link (nine
exchanges, best round-trip kept; offset 180 ms at a 5 ms round-trip), then
paints, on every display frame, a strip of 26 cells across the top of its
screen — a white marker, the desktop-time in milliseconds as 24 bits MSB
first, a black marker — plus the same time as text. The desktop reads each
decoded frame the moment `ffmpeg` emits it, finds the marker, reads the 24
bits and subtracts them from its own clock. The number is therefore
**capture → encode → Wi-Fi → decode complete**, excluding only the
desktop's own display scan-out (one frame, 8–16 ms) and the phone's
compose-to-scan-out ahead of the capture.

```sh
./mirrorspike                          # desktop: waits on UDP 1764, spawns ffmpeg + mpv
adb shell am start -S -n com.example.magnetita/.MirrorActivity --es addr 10.0.0.134:1764 --ei bitrate 12000000
# the author confirms the whole-screen capture dialog and leaves the clock on screen
```

## Result

- **Exit:** 0; 4 565 HEVC Main frames decoded, 4 521 of them with a
  readable strip, 44 before the strip was on screen
- **Observed** (38 s of mirroring, the phone at its 120 Hz refresh):

```
 8.1s frames=902  fps=112.5 latency p50=59 p90=65 p99=101 min=51 ms
24.1s frames=2835 fps=117.6 latency p50=64 p90=78 p99=208 min=51 ms
38.1s frames=4521 fps=118.5 latency p50=67 p90=77 p99=194 min=51 ms
```

  - Median 59–67 ms from capture to decoded frame, best 51 ms, p90 under
    80 ms; the p99 tail of about 200 ms is the same Wi-Fi jitter the QUIC
    record measured. Adding one display frame on the desktop, the author
    sees the phone about 70–80 ms behind reality at the median.
  - The encoder ran at the display's 120 fps although 60 was requested;
    `KEY_FRAME_RATE` is a hint. 24.3 MB in 38 s is 5.1 Mbit/s for a mostly
    static screen at 12 Mbit/s CBR requested.
  - The author watched the mirror live in the `mpv` window on the same link.

### What it took to get here, all of it product knowledge for `MAG-P6`

- Android 14+ opens the capture dialog on "one app" by default, and that
  list cannot contain the requesting app; only
  `createConfigForDefaultDisplay()` yields a whole-screen dialog.
- `AUTO_MIRROR` captures whatever is in front. With another app in front
  the screen was static, the encoder emitted one frame a second and, with
  no keep-alive, QUIC closed the connection after 30 s on both sides.
  Keep-alive on the link and `KEY_REPEAT_PREVIOUS_FRAME_AFTER` on the
  encoder are both required.
- The consumer must never apply back-pressure to the QUIC reader: a slow
  `mpv` stdin stalled the reader, filled flow control, and stalled the
  phone's encoder to 0.3 fps. Feed decoders through queues, drop for the
  viewer, never for the measurement.
- `ffmpeg 9` picks a hardware HEVC decoder that emits nothing on this host
  unless `-c:v hevc` is given, and `-fflags nobuffer` silences the raw
  HEVC demuxer entirely.
- The phone-side runtime that drives the QUIC endpoint must outlive the
  connect call.

## Limits

- One evening, one phone, the same Wi-Fi as the QUIC record, a mostly
  static screen; a game or a scrolling page will cost more bits, not more
  latency.
- The 24-bit clock wraps every 16.8 s and the strip is read on the
  decoded frame, not on the desktop's glass; scan-out is added by
  estimate above, not measured.
- Input back to the phone is `MAG-P0-C`, not measured here.
- Certificate verification was disabled on the phone side, as in every
  spike; pairing is `MAG-P1`.

## Follow-up

The author judges the number against the `scrcpy` mirror they use today;
the mirror discussion cites this record. Remaining in `MAG-P0`: `-C` and
the nest half of `-D`.
