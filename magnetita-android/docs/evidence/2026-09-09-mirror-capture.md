# Capture, encode and stream on consent — AND-4-A

- **Date:** 2026-09-09
- **Scope:** `AND-4-A` of
  [`../plans/archive/2026-09-09-link-mirror.md`](../plans/archive/2026-09-09-link-mirror.md):
  `app/src/main/java/org/celestina/magnetita/mirror/{ScreenMirror,MirrorService,MirrorConsentActivity,MirrorGeometry}.kt`,
  the mirror ports and signals in `link/`, the link service's ask, the
  manifest, strings, the tests; the plan and roadmap records that archive
  `AND-3` and open `AND-4`
- **Environment:** as `AND-1-A`; the S25U over USB `adb` for the install
- **Artifact:** `app/build/outputs/apk/release/app-release.apk`, sealed,
  installed on the S25U

## Design

- The desktop's `MirrorStart` reaches the link service as a signal. In
  front, the service opens `MirrorConsentActivity` at once; otherwise it
  posts a notification that opens it, because a service may not raise an
  activity from the background. The activity asks the system's capture
  consent and starts `MirrorService` with the answer; a refusal sends
  `MirrorStop` back so the desktop's state returns to idle.
- `MirrorService` is a foreground service of the `mediaProjection` type.
  `ScreenMirror` sizes the picture with `MirrorGeometry.fit` (the longer
  side capped by the desktop's `max_size`, both sides even), configures
  an HEVC (or H.264) surface encoder at the desktop's rate and bit rate
  with one key frame a second and no B-frames, feeds it from a virtual
  display that mirrors the screen, opens the video stream on the fixed
  id, sends `MirrorStarted` with the picture's shape and writes each
  output buffer as it comes on the encoder's own thread. A stalled write
  stalls the encoder, which drops frames rather than queueing them.
- The projection's end, the desktop's stop, the notification's action or
  the link's loss end everything once and tell the desktop.

## Procedure

```sh
cd magnetita-android
./gradlew --no-daemon -q testDebugUnitTest lintDebug
scripts/build-production.sh && adb install -r app/build/outputs/apk/release/app-release.apk
```

## Result

- **Exit:** 0. 23 unit tests: the picture's fit, a touch's landing and
  the mirror signals join the earlier twenty; lint 0 errors.

## Limits

- Android grants one capture per consent: every start of the mirror asks
  again on the phone. The mirror after a reboot is `VAL-MAG-14`.
- The audio stream is not captured.
