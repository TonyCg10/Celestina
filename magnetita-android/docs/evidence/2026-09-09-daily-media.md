# Media control both ways — AND-2-D

- **Date:** 2026-09-09
- **Scope:** `AND-2-D` of
  [`../plans/archive/2026-09-09-daily-set.md`](../plans/archive/2026-09-09-daily-set.md):
  `app/src/main/java/org/celestina/magnetita/media/PhoneMedia.kt`, the
  media ports and signals in `link/`, the desktop player card on the
  device screen, the package queries in the manifest, the tests
- **Environment:** as `AND-1-A`; the S25U over USB `adb`, the daemon of
  `MAG-P4-D`
- **Artifact:** `app/build/outputs/apk/release/app-release.apk`, sealed,
  installed on the S25U

## Design

- `PhoneMedia` rides on the notification access grant: Android hands the
  active `MediaSession`s to its holder. The first active session is the
  player the desktop sees; every change of metadata or playback state
  goes out once, the desktop's request re-sends it, and the desktop's
  command (button, seek, volume) lands on the session whose label it
  names.
- The device screen shows the desktop's player when there is one, with
  previous, play/pause and next; while the screen is in front and the
  link is up, the desktop's players are wanted (a request a minute, the
  daemon's window being ninety seconds).
- The manifest now queries launchable packages so the labels of the apps
  behind notifications and players resolve instead of falling back to
  package names.

## Procedure

```sh
cd magnetita-android
./gradlew --no-daemon -q assembleDebug testDebugUnitTest lintRelease
scripts/build-production.sh && adb install -r app/build/outputs/apk/release/app-release.apk
busctl --user --json=short call … ListDevices
```

## Result

- **Exit:** 0. 15 unit tests, the controller's outbound test now also
  sends a state, a command and a request in order, and the signals test
  decodes the three media messages. `lintRelease`: 0 errors.
- **On the S25U, 22:44:** the phone's paused music session reached the
  desktop's registry with its title, artist and length as the session
  opened; the desktop's media request reached the phone.

## Limits

- The desktop card was not seen with content: the desktop had no player
  at the time. Driving the phone's player from the desktop was not
  exercised live either, to leave the author's music alone.
  `VAL-MAG-12`.
