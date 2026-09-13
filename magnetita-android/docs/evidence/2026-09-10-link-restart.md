# The link back on its own — AND-5-D

- **Date:** 2026-09-10
- **Scope:** `AND-5-D` of
  [`../plans/archive/2026-09-10-storage.md`](../plans/archive/2026-09-10-storage.md):
  `app/src/main/java/org/celestina/magnetita/link/Restart.kt`, the
  manifest's receiver and `RECEIVE_BOOT_COMPLETED`, this record
- **Environment:** the S25U over USB `adb`
- **Artifact:** `app/build/outputs/apk/release/app-release.apk`, sealed,
  installed on the S25U

## Design

Installing a new build stops every service the application ran, and the
process stayed alive only through the accessibility and notification
services, so the desktop waited for a hand on the phone. A receiver for
`BOOT_COMPLETED` and `MY_PACKAGE_REPLACED` starts the link's foreground
service, the two broadcasts Android exempts from its background-start
rule. This is also what `VAL-MAG-14` assumes: after a reboot, nothing to
open.

## Procedure

```sh
cd magnetita-android && ./gradlew --no-daemon -q testDebugUnitTest lintDebug
scripts/build-production.sh && adb install -r app/build/outputs/apk/release/app-release.apk
```

## Result

- **Exit:** 0. After this build's install the daemon logged the phone's
  return without the application being opened.

## Limits

- Samsung's battery optimisation can still hold a boot start back until
  the application is exempted; the reboot itself is `VAL-MAG-14`.
