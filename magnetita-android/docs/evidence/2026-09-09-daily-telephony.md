# Call state, mute, answer, hang up — AND-2-G

- **Date:** 2026-09-09
- **Scope:** `AND-2-G` of
  [`../plans/archive/2026-09-09-daily-set.md`](../plans/archive/2026-09-09-daily-set.md):
  `app/src/main/java/org/celestina/magnetita/phone/Calls.kt`, its
  receiver in the manifest, the call signal and outbound in `link/`, the
  tests
- **Environment:** as `AND-2-E`
- **Artifact:** as `AND-2-E`

## Design

- The phone-state broadcast feeds a pure state machine: ringing then
  off-hook is answered, ringing then idle is missed, off-hook then idle is
  ended, and an outgoing call is not reported. The number comes with the
  broadcast under the call-log grant.
- The desktop's mute lowers the ring volume to zero until the call ends,
  answer uses the telecom manager's accept, hang up its end call, both
  behind the answer-calls grant.

## Procedure

```sh
cd magnetita-android
./gradlew --no-daemon -q assembleDebug testDebugUnitTest lintRelease
adb shell pm grant org.celestina.magnetita android.permission.READ_PHONE_STATE
```

## Result

- **Exit:** 0. The state machine's test pins the four transitions and
  the non-report of outgoing calls.
- **On the S25U:** not exercised with a real call. `VAL-MAG-12`.

## Limits

- The accept call is deprecated; it still works for sideloaded apps with
  the grant on this device, and a default-dialer role would be the
  supported path.
