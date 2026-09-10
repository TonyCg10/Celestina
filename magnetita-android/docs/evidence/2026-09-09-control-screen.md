# The control screen: trackpad, keyboard, commands — AND-3-A

- **Date:** 2026-09-09
- **Scope:** `AND-3-A` of
  [`../plans/archive/2026-09-09-remote-control.md`](../plans/archive/2026-09-09-remote-control.md):
  `app/src/main/java/org/celestina/magnetita/control/TrackpadMath.kt`,
  `ui/screens/ControlScreen.kt`, the input ports, the fast path and the
  command signals in `link/`, the device screen's entry, strings, the
  tests; the plan and roadmap records that archive `AND-2` and open
  `AND-3`
- **Environment:** as `AND-1-A`; the S25U over USB `adb`
- **Artifact:** `app/build/outputs/apk/release/app-release.apk`, sealed,
  installed on the S25U

## Design

- `TrackpadMath` is pure: fractional motion accumulates until a whole
  pixel moves, gain rises with speed, two fingers scroll in the wire's
  1/120 steps, and a text field change is a diff of appended text and
  deletions.
- `ControlScreen`: the pad on top (one finger moves, a tap clicks, two
  fingers scroll, a long press is the right button), a field that types
  straight to the desktop and forgets what it sent, a row of the keys a
  field cannot type (Esc, Tab, arrows, Enter), and the registered
  commands by name with a run button and the last result.
- Input takes a fast path: the controller exposes the held session and
  the service runs the call on one input thread, so motion never waits
  for the loop's poll. Commands go through the ordered outbound.

## Procedure

```sh
cd magnetita-android
./gradlew --no-daemon -q assembleDebug testDebugUnitTest lintRelease
scripts/build-production.sh && adb install -r app/build/outputs/apk/release/app-release.apk
```

## Result

- **Exit:** 0. 20 unit tests: the trackpad arithmetic (accumulation,
  gain, scroll steps, the field diff), the command signals, the run
  through the outbound and the exposed session. `lintRelease`: 0 errors.
- **On the S25U:** installed; the control screen opens from the device
  screen while connected. Its effect on the desktop's pointer is the
  author's to observe (`VAL-MAG-13`): no input is injected into the live
  session from here.

## Limits

- Hardware keyboards and non-ASCII characters type nothing yet; the
  daemon's table is a US layout.
