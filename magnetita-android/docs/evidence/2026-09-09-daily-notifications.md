# Notifications with actions and replies — AND-2-B

- **Date:** 2026-09-09
- **Scope:** `AND-2-B` of
  [`../plans/archive/2026-09-09-daily-set.md`](../plans/archive/2026-09-09-daily-set.md):
  `app/src/main/java/org/celestina/magnetita/notifications/`, the
  outbound queue and the notification signals in `link/`, the device
  screen's row, strings, the manifest, the tests
- **Environment:** as `AND-1-A`; the S25U over USB `adb`, the daemon of
  `MAG-P4-B`
- **Artifact:** `app/build/outputs/apk/release/app-release.apk`, sealed,
  installed on the S25U

## Design

- `PhoneNotifications` is the `NotificationListenerService`: for every
  posted notification that passes `NotificationPolicy` (not ours, not
  ongoing, not a group summary, with content) it sends key, app label,
  title, text, time, whether an action accepts a `RemoteInput`, the
  action labels, and the app icon as a small PNG once per package;
  removals send the key. The desktop's dismiss cancels by key, its action
  fires the action's intent, its reply fills the `RemoteInput` results
  and fires.
- The controller's clipboard slot became an ordered outbound channel
  (`Outbound`: clipboard, notification, notification gone), drained on
  each poll, newest kept on overflow; the listener hands objects to the
  service through a process-local deque and a poke.
- The device screen shows whether notification access is granted and
  opens the system page when it is not; Android grants a sideloaded
  listener only there (or by `adb shell cmd notification allow_listener`).

## Procedure

```sh
cd magnetita-android
./gradlew --no-daemon -q assembleDebug testDebugUnitTest lintRelease
scripts/build-production.sh && adb install -r app/build/outputs/apk/release/app-release.apk
adb shell cmd notification allow_listener org.celestina.magnetita/.notifications.PhoneNotifications
adb shell cmd notification post -S bigtext -t 'Prueba de Magnetita' magtest2 'Hola desde el S25U'
```

## Result

- **Exit:** 0. 14 unit tests: the policy, and the controller sending a
  notification and its removal in order and decoding the three inbound
  notification signals. `lintRelease`: 0 errors.
- **On the S25U:** once the listener was granted it posted the active
  notifications; the shell notification reached the desktop's log with
  its app and title; the desktop's dismiss reached the phone
  (`notification: dismiss`) and the notification was gone.

## Limits

- Actions and replies were exercised by the loopback tests and the JVM
  tests, not with a real messaging application: `VAL-MAG-12`.
- A media player's paused notification crossed too; Samsung does not flag
  it ongoing while paused. The policy may learn media styles later.
