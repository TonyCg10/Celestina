# The whole phone as the shared root — AND-5-B

- **Date:** 2026-09-10
- **Scope:** `AND-5-B` of
  [`../plans/archive/2026-09-10-storage.md`](../plans/archive/2026-09-10-storage.md):
  `app/src/main/java/org/celestina/magnetita/storage/PhoneStorage.kt`,
  the manifest's `MANAGE_EXTERNAL_STORAGE`, the device screen's row with
  its two actions, the link service's state on focus, strings, this
  record
- **Environment:** as `AND-1-A`; the S25U over USB `adb` for the install
- **Artifact:** `app/build/outputs/apk/release/app-release.apk`, sealed,
  installed on the S25U

## Design

The author could not pick the root: Android's tree picker refuses the
storage root and the Download folder. The "Archivos" row now offers
"Todo", which opens the system's all-files page for this application, and
"Carpeta", the picker. With the grant, the root is the external storage
directory and the requests are answered over plain files (`listFiles`,
`RandomAccessFile` for ranges and truncation, `mkdir`, `renameTo`,
`delete`), the wire's path rule still applied before any join. The link
re-sends the storage state when the application comes back to the front,
so the desktop mounts as soon as the grant is given.

## Procedure

```sh
cd magnetita-android && ./gradlew --no-daemon -q testDebugUnitTest lintDebug
scripts/build-production.sh && adb install -r app/build/outputs/apk/release/app-release.apk
```

## Result

- **Exit:** 0; 26 unit tests, lint 0 errors (the scoped-storage lint is
  acknowledged in the manifest: the application is sideloaded, not
  distributed through a store).

## Limits

- The all-files grant lives in the system settings; revoking it there
  drops the desktop back to the picked folder, or to nothing.
