# Foreground service holding a session; discovery; pins — AND-1-B

- **Date:** 2026-09-09
- **Scope:** `AND-1-B` of
  [`../plans/active/2026-09-09-foundation.md`](../plans/active/2026-09-09-foundation.md):
  `app/src/main/java/org/celestina/magnetita/link/`, the service entry in
  the manifest, the link row of the identity screen, the controller tests
- **Environment:** as `AND-1-A`; the live `magnetitad` at `10.0.0.134`
  (installed at `3419cff`), the S25U at `10.0.0.16` over USB `adb`
- **Artifact:** `app/build/outputs/apk/debug/app-debug.apk`, not committed

## Design

- `LinkController` is pure Kotlin over two ports, `Connector` (pins, dial,
  pair) and `Discovery` (one bounded browse), plus a `BatterySource`. It
  runs one loop: no pins, ask for pairing; pins, browse, dial only the
  advertised ids that are pinned, hold the session, and on a drop wait the
  schedule (`Backoff`, 250 ms doubling to 60 s, reset by a session that
  was up) and start over. A `magnetita://pair` URI handed to `pair()`
  pins and connects on the loop's next turn. A battery change is reported
  on the next poll.
- `CoreConnector` wraps `MobilePhone` and `MobileSession` from the Rust
  core; `NsdDiscovery` wraps `NsdManager` for `_magnetita._udp` with the
  multicast lock held only while browsing, IPv4 first.
- `LinkService` is a `LifecycleService` of foreground type
  `connectedDevice`; it owns the controller, listens to the battery
  broadcast, and publishes `LinkState` for the screens through a
  `StateFlow`. `MainActivity` starts it and forwards a `VIEW` intent of
  scheme `magnetita`, host `pair`, so the system camera's QR scan can
  pair before the app has its own scanner (`AND-1-C`).

## Procedure

```sh
cd magnetita-android
./gradlew --no-daemon -q assembleDebug testDebugUnitTest
adb install -r app/build/outputs/apk/debug/app-debug.apk
adb shell pm grant org.celestina.magnetita android.permission.POST_NOTIFICATIONS
adb shell am start -n org.celestina.magnetita/.MainActivity
URI=$(busctl --user call org.celestina.Magnetita /org/celestina/Devices1 org.celestina.Devices1 StartPairing | sed -E 's/^s "(.*)"$/\1/')
adb shell am start -a android.intent.action.VIEW -d "'$URI'"
busctl --user --json=short call … ListDevices
busctl --user call … Ring s 4eb6f9984054dd25
adb shell input keyevent KEYCODE_SLEEP; sleep 70; busctl … ListDevices
adb shell am start -a android.settings.SETTINGS; sleep 20; busctl … ListDevices
adb shell svc wifi disable; sleep 8; adb shell svc wifi enable; sleep 25; busctl … ListDevices
```

## Result

- **Exit:** 0 for the build and the unit tests, 7 tests (5 controller,
  2 theme). The controller tests drive the loop with fakes on virtual
  time: no pins dials nobody; only pinned ids are dialled and the battery
  is reported on connect and on change; a drop waits and reconnects; a
  refused dial backs off 250 then 500 ms; a scanned URI pairs, holds that
  session, and reconnects through discovery afterwards.
- **Observed on the S25U:**
  - The service is foreground (`isForeground=true`, type `0x10`,
    `connectedDevice`) with an ongoing low-importance notification.
  - The pairing link pinned the desktop and connected: the daemon logged
    `[paired] SM-S938U at 10.0.0.16:56951 on the own wire`, `ListDevices`
    shows `4eb6f9984054dd25` with `battery 73`, `connected true`,
    `paired true`.
  - `Ring` reached the phone: `logcat` shows `desktop: find: ring` (the
    ringing itself is the device screen's job, `AND-1-C`).
  - The same session stayed `connected true` through 70 s with the screen
    off, 20 s with Settings in front, and a Wi-Fi off/on of 8 s; the daemon
    logged no second pairing and no close on the own wire (the `[closed]`
    line in the same window is the 1.x KDE Connect wire's).

## Limits

- `lintDebug` fails on three `NewApi` errors inside the generated UniFFI
  bindings (`java.lang.ref.Cleaner`, API 33, chosen at runtime behind a
  `Class.forName` check that lint does not follow). `AND-1-D` adds the
  lint baseline or a `minSdk`-aware suppression for the generated tree.
- Discovery is a bounded browse per attempt, not a standing subscription;
  reconnection latency after a drop is the backoff plus up to four
  seconds of browse.
- The device screen, the ping and the find handling are `AND-1-C`.
