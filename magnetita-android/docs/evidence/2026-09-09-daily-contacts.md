# Contacts sync — AND-2-E

- **Date:** 2026-09-09
- **Scope:** `AND-2-E` of
  [`../plans/archive/2026-09-09-daily-set.md`](../plans/archive/2026-09-09-daily-set.md):
  `app/src/main/java/org/celestina/magnetita/phone/{PhoneBook,PhonePermissions}.kt`,
  the phone grants row on the device screen, the contacts signal and
  outbound in `link/`, the manifest's permissions, the tests
- **Environment:** as `AND-1-A`; the S25U with the grants given by `adb`
- **Artifact:** `app/build/outputs/apk/release/app-release.apk`, sealed,
  installed on the S25U

## Design

- `PhoneBook` answers the desktop's request with every contact updated
  after the version it names: a vCard 4.0 with the display name and the
  numbers, nothing else, in pages of 200, the last page marked complete
  with the newest update time as the version.
- The device screen's row "Contactos, SMS y llamadas" asks for the seven
  grants at once; when they arrive the service sends the contacts and the
  conversations without waiting for the next session.

## Procedure

```sh
cd magnetita-android
./gradlew --no-daemon -q assembleDebug testDebugUnitTest lintRelease
adb shell pm grant org.celestina.magnetita android.permission.READ_CONTACTS
journalctl --user -u magnetitad -o cat | grep contacts
```

## Result

- **Exit:** 0. 17 unit tests; the vCard test pins the escaping of the
  name and the digits of the numbers.
- **On the S25U:** with the grant the contacts went out on the session's
  request; the daemon logged their count.

## Limits

- Removed contacts are not reported; the provider does not expose them to
  a one-way reader.
