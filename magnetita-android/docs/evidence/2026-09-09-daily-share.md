# File share both ways — AND-2-C

- **Date:** 2026-09-09
- **Scope:** `AND-2-C` of
  [`../plans/archive/2026-09-09-daily-set.md`](../plans/archive/2026-09-09-daily-set.md):
  `app/src/main/java/org/celestina/magnetita/share/Downloads.kt`, the
  share signals and the transfer pump in `link/`, the share target for
  streams, the service's publication of received files, strings, the
  manifest, the tests
- **Environment:** as `AND-1-A`; the S25U over USB `adb`, the daemon of
  `MAG-P4-C`
- **Artifact:** `app/build/outputs/apk/release/app-release.apk`, sealed,
  installed on the S25U

## Design

- **Phone to desktop:** the share target takes `ACTION_SEND` and
  `ACTION_SEND_MULTIPLE` streams of any type, asks the provider for name
  and size, and queues an `Outbound.File`; the controller offers it and,
  on the desktop's acceptance, streams the bytes from the accepted offset
  through a `FileSource` port (the content resolver in the service, a byte
  array in tests), 64 KiB at a time, off the loop's dispatcher.
- **Desktop to phone:** an offer is accepted into the app's cache (the
  core resumes a partial there); a complete file is copied into the
  public `Downloads/Magnetita` through `MediaStore` and announced with a
  notification; the device screen's note names it.
- A shared text from the desktop becomes the clipboard.

## Procedure

```sh
cd magnetita-android
./gradlew --no-daemon -q assembleDebug testDebugUnitTest lintRelease
scripts/build-production.sh && adb install -r app/build/outputs/apk/release/app-release.apk
busctl --user call … SendFileUri ss 4eb6f9984054dd25 file://…/magnetita-icon.svg
adb shell ls -la /sdcard/Download/Magnetita/
```

## Result

- **Exit:** 0. 15 unit tests; the new one offers a 200 000 byte file,
  receives an acceptance at offset 150 000, and checks that exactly the
  last 50 000 bytes were written and the transfer finished, and that an
  offer is accepted into the receive directory. `lintRelease`: 0 errors.
- **On the S25U, 22:34:** the desktop's `SendFileUri` produced the offer,
  the received-file event and the done line in the phone's log, and the
  SVG appeared in `Download/Magnetita` with its 6055 bytes.

## Limits

- Phone-to-desktop with real files goes through the share sheet, which
  the author drives: `VAL-MAG-12`. The loop and the offset arithmetic
  are covered by the JVM test and the desktop side by the peer run.
- No progress on screen yet; the note only names the outcome.
