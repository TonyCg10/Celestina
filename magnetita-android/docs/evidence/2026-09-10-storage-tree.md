# The phone's files from the shared tree — AND-5-A

- **Date:** 2026-09-10
- **Scope:** `AND-5-A` of
  [`../plans/archive/2026-09-10-storage.md`](../plans/archive/2026-09-10-storage.md):
  `app/src/main/java/org/celestina/magnetita/storage/{PhoneStorage,DocumentPaths}.kt`,
  the storage ports and signals in `link/`, the service's storage thread
  and state, the device screen's row, strings, the tests; the plan and
  roadmap records that archive `AND-4` and open `AND-5`
- **Environment:** as `AND-1-A`; the S25U over USB `adb` for the install
- **Artifact:** `app/build/outputs/apk/release/app-release.apk`, sealed,
  installed on the S25U

## Design

- The "Archivos" row opens the system's tree picker; the grant is taken
  as persistable and its URI kept in preferences. `available()` is true
  only while the grant still stands, and the link sends `state` on every
  connect and when the grant changes.
- `DocumentPaths` maps a wire path onto a document id by appending it to
  the tree's id (`primary:` plus the path), refusing what the wire
  forbids; it is pure and JVM-tested.
- `PhoneStorage` answers each request on one storage thread: a listing is
  one query of the children (paged at 256 by the wire), a stat one query
  of the document, a read a positioned `FileInputStream` on the document's
  descriptor, a write a positioned `FileOutputStream` with an optional
  truncate, `mkdir` and file creation `createDocument`, a rename
  `moveDocument` across parents then `renameDocument`, a delete
  `deleteDocument`. Failures go back as the reply's error.

## Procedure

```sh
cd magnetita-android
./gradlew --no-daemon -q testDebugUnitTest lintDebug
scripts/build-production.sh && adb install -r app/build/outputs/apk/release/app-release.apk
```

## Result

- **Exit:** 0. 26 unit tests (the id mapping, the split, the signal); lint
  0 errors.

## Limits

- The document tree is what the person picked: one root, and moves between
  providers are refused by Android.
- Listing sorts by name on the phone; the daemon shows the tree the phone
  gives.
