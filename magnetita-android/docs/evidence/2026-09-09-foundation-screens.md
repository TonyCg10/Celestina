# Pairing screens, device screen, battery, find — AND-1-C

- **Date:** 2026-09-09
- **Scope:** `AND-1-C` of
  [`../plans/active/2026-09-09-foundation.md`](../plans/active/2026-09-09-foundation.md):
  `app/src/main/java/org/celestina/magnetita/ui/screens/`, the signals,
  the ringer and the service actions in `link/`, `MainActivity`, the
  strings, the camera dependencies, and the three production scripts
  brought forward from `AND-1-D`
- **Environment:** as `AND-1-B`; the live `magnetitad` redeployed with
  the `MAG-P3-C` correction during this unit
- **Artifact:** `app/build/outputs/apk/debug/app-debug.apk`, sealed by
  the suite's runner in `app/build/production-manifest.toml`, not committed

## Design

- `DeviceScreen` is the home: the desktop's name as the title with the
  pairing as subtitle, the link row with its chip, the address, this
  phone's battery as it is sent, the pinned desktop with its fingerprint
  and the forget action, then this phone's identity. Without a pin the
  title is the app's and the one action is to scan. While the desktop is
  finding the phone a red stop action leads the page.
- `ScanScreen`: CameraX's `PreviewView` in a rounded group, ML Kit's
  bundled barcode scanner reading QR codes only, the first
  `magnetita://pair` link handed to the service; a foreign code says so
  in place. The camera permission is asked on the screen itself.
- `DesktopSignal` maps an envelope's capability and kind to what the
  phone does: ring, stop ringing, answer a battery request. The
  controller answers battery requests itself and emits the rest.
- `Ringer`: a `MediaPlayer` in the app's process on the alarm usage at
  full volume, looping, plus vibration, until the desktop's stop or the
  person's. A `Ringtone` was tried first: on Samsung it plays through the
  system's remote player, reports `isPlaying` false, and a second ring
  leaked a first one nothing could stop.
- Forget: the service drops the pin in the core and the controller
  closes the session; the loop then asks for pairing again.
- Scripts: `build-production.sh`, `verify-production.sh` and
  `status-production.sh` now delegate to the suite's production runner
  like every other project, and the registry's `artifact_manifest` is the
  runner's own file; the suite's fixture test had started failing on the
  hand-written scripts and blocked Magnetita's deployment.

## Procedure

```sh
cd magnetita-android
./gradlew --no-daemon -q assembleDebug testDebugUnitTest
scripts/build-production.sh && scripts/verify-production.sh && scripts/status-production.sh
adb install -r app/build/outputs/apk/debug/app-debug.apk
busctl --user call … Ring s 4eb6f9984054dd25
adb shell dumpsys audio | grep 'state:started'
adb shell input tap <stop button>   # the phone, never the desktop session
busctl --user call … StartPairing | qrencode -o pair-qr.png   # shown to the author
```

## Result

- **Exit:** 0; 10 unit tests (6 controller, 2 signals and link filter,
  2 theme). `scripts/verify-production.sh` passes through the runner and
  `status-production.sh` reports the artifact current and verified.
- **Observed on the S25U:**
  - The device screen renders as designed: "Celestina" as title,
    "Emparejado" in the accent, the link row with the mint chip, the
    address `10.0.0.134:1760`, the battery chip "73 % · cargando", the
    pinned desktop with its colon-hex fingerprint, the identity group.
  - `Ring` from the desktop: one player on `USAGE_ALARM` under the app's
    pid, the red stop action on the page; a second `Ring` added no player;
    the stop action left no alarm player. The author confirmed the sound.
  - Forget: the daemon logged `connection lost` and `[closed]`, the pin
    list emptied and the scan action appeared.
  - The scan screen opened the camera (the preview was dark on the desk;
    the camera indicator was on). The author scanned a QR rendered from
    `StartPairing`: the first attempt read the code and called `pair`, and
    timed out on the daemon's side (the `MAG-P3-C` defect: the desktop
    still pinned the phone); after the daemon's correction the second
    scan paired in one step and the screen showed "Conectado a Celestina".

## Limits

- The six-digit code screen waits for the wire: the daemon arms QR
  pairing only, and the code exchange is not on the link yet. It stays
  with the desktop half (`MAG-P3-B`).
- There is no ping message in the protocol; the find capability and, from
  `MAG-P4`, notifications cover what KDE Connect's ping did.
- The desktop's own battery is not shown: the daemon does not send a
  `BatteryStatus` yet.
- `lintDebug` still fails on the three `NewApi` findings in the generated
  bindings; the baseline is `AND-1-D`'s, with version reading and signing.
