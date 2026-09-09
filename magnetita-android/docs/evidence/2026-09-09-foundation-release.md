# Verify and deploy scripts; version reading; signed build — AND-1-D

- **Date:** 2026-09-09
- **Scope:** `AND-1-D` of
  [`../plans/archive/2026-09-09-foundation.md`](../plans/archive/2026-09-09-foundation.md):
  `app/build.gradle.kts`, `app/lint.xml`, `scripts/`, `.gitignore`, the
  resources lint named, this record; and, in the suite, the version
  contract's new Gradle source kind with the project's baseline row
- **Environment:** as `AND-1-A`; the S25U over USB `adb`
- **Artifact:** `app/build/outputs/apk/release/app-release.apk`, sealed in
  `app/build/production-manifest.toml`, not committed

## Design

- **Version:** the suite's version contract reads
  `gradle-version-name`: exactly one `applicationId` naming the registered
  application and exactly one `versionName`, both in
  `app/build.gradle.kts`; the version tool rewrites the same line on a
  bump. The project is registered versioned at `0.1.0` (baseline row
  `AND-1-D` in `docs/version-history.tsv`), so `magnetita-android-bug` and
  `-milestone` commits bump it like every other product.
- **Signing:** `keystore.properties` at the project root (gitignored, with
  `*.jks` and `*.keystore`) names `storeFile`, `storePassword`, `keyAlias`
  and `keyPassword`; when present the release build signs with that key,
  otherwise with the debug key, so `app-release.apk` always exists under
  one name and the runner can seal it.
- **Lint:** `lintRelease` is part of the verification, `abortOnError`;
  `app/lint.xml` scopes the only exception, `NewApi`, to the generated
  UniFFI bindings under `build/native/kotlin`, which choose
  `java.lang.ref.Cleaner` behind a `Class.forName` check lint does not
  follow. The cheap warnings lint raised were fixed instead of ignored:
  five unused strings, an unused colour, the redundant activity label,
  the `-v26` launcher folder below `minSdk`, and a scroll value read in
  composition (now `derivedStateOf`).
- **Scripts:** `build-production.sh` builds `assembleRelease`;
  `verify-production.sh` runs the core's tests, the unit tests and
  `lintRelease` and checks the artifact; `status-production.sh` reports
  through the runner. Nothing installs on the host: the APK goes to the
  phone with `adb install -r`.

## Procedure

```sh
python3 scripts/test-version-contract.py
cd magnetita-android
scripts/build-production.sh && scripts/verify-production.sh && scripts/status-production.sh
adb install -r app/build/outputs/apk/release/app-release.apk
adb shell dumpsys package org.celestina.magnetita | grep versionName
```

## Result

- **Exit:** 0. Version contract tests: 22, including the new Gradle
  read-and-rewrite test and the static check that now lists
  `magnetita-android` among the versioned owners. `lintRelease`: 0
  errors; the remaining warnings are dependency and SDK version
  suggestions. Unit tests: 10. The runner seals the release artifact and
  reports it current and verified.
- **Observed on the S25U:** the release build installed over the debug
  one (same debug key while no `keystore.properties` exists), reported
  `versionName=0.1.0`, and reconnected to the live daemon by itself:
  `ListDevices` shows the phone connected with its battery.

## Limits

- The author's key is not on this host's checkout; the release stays
  debug-signed until `keystore.properties` names it. The build does not
  fail without it, by design.
- Instrumented tests do not exist; the session survival of `AND-1`'s
  exit is the manual run recorded in `AND-1-B`.
