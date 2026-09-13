# One walk per directory — AND-5-C

- **Date:** 2026-09-10
- **Scope:** `AND-5-C` of
  [`../plans/archive/2026-09-10-storage.md`](../plans/archive/2026-09-10-storage.md):
  `app/src/main/java/org/celestina/magnetita/storage/PhoneStorage.kt`,
  this record
- **Environment:** the author's browse of the S25U
- **Artifact:** `app/build/outputs/apk/release/app-release.apk`, sealed,
  installed on the S25U

## Design

The desktop reads a directory in pages of 256 and the phone walked the
whole directory, three stats per file, for every page: the camera folder
of 1715 photos cost seven walks of a second each. The last listing is
now kept ten seconds by path, so the pages of one directory cost one
walk; a change made through the link drops it.

## Procedure

```sh
cd magnetita-android && ./gradlew --no-daemon -q testDebugUnitTest lintDebug
```

## Result

- **Exit:** 0; 26 tests, lint 0 errors.

## Limits

- A file added on the phone within ten seconds of a listing is missing
  from the desktop's pages until the next walk.
