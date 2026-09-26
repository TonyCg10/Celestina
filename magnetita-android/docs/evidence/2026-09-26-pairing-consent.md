# Pairing consent, mirror input gate and safe folder delete — AND-6-D

- **Date:** 2026-09-26
- **Scope:** `AND-6-D` (program unit P-3) of
  [`../plans/active/2026-09-13-app-design.md`](../plans/active/2026-09-13-app-design.md),
  closing audit findings AND-1 (Critical), AND-6, the phone half of AND-4
  and AND-8 of the
  [Magnetita audit record](../../../docs/evidence/2026-09-26-monorepo-audit-magnetita.md):
  `MainActivity.kt`, `link/PairingConsent.kt`,
  `ui/screens/PairConfirmScreen.kt`, `link/LinkService.kt`,
  `mirror/{MirrorControl,MirrorService,MirrorInput}.kt`,
  `storage/{DeleteRule,PhoneStorage}.kt`, `res/values/strings.xml`,
  `res/xml/{data_extraction_rules,backup_rules}.xml` and three JVM test
  classes under `app/src/test/`
- **Environment:** session worktree
  `Celestina.worktrees/magnetita-android-AND-6-D` in a Linux container;
  OpenJDK 21, the Kotlin 2.0.21 compiler embedded in the container's
  Gradle 8.14.3 distribution, its JUnit 4.13.2, and hamcrest-core 1.3
  fetched from Maven Central into the session scratchpad. No
  Android SDK, NDK, AGP or `cargo-ndk`, so neither Gradle's
  `testDebugUnitTest` nor lint nor the APK build could run; the project
  itself uses Kotlin 2.2
- **Artifact:** the landing builds it (`app-release.apk` through
  `scripts/build-production.sh` and `scripts/verify-production.sh`)

## Design

- **AND-1.** `MainActivity` no longer pairs from an intent. Every
  `magnetita://pair` link, from the exported deep link or from the
  scanner, goes to one `PairingConsent` held for the process.
  `PairPreview.of` reads the link for the screen. It applies
  `PairLink.accepts` (also on the deep-link path, where it was missing),
  and it refuses a repeated `id` or `fp` so the screen cannot show one
  value while the core uses another. It refuses a link with no address,
  and a link with any address outside the LAN, because the core dials
  every address in order. `LanAddress.isLan` accepts only strict
  `ip:port` literals in RFC 1918 or `fc00::/7`; loopback (another app on
  the phone), link-local, CGNAT, public, host names and scope ids are
  refused. The consent screen shows the id, the fingerprint (as the
  desktop prints fingerprints, colon-separated hex) and the addresses, and
  only its confirm button calls `LinkService.pair`, the one pairing entry.
  While an offer waits, any other link is ignored, and a confirm names the
  offer it answers, so a stale screen cannot confirm a different offer. A
  recreated activity and a launch from Recents do not re-offer the
  intent.
- **AND-6.** `MirrorControl` passes `MirrorKey` and `MirrorGlobal` to
  the accessibility service only while `MirrorService.state` is
  `Streaming`, and drops them otherwise. `MirrorInput` implements the
  `MirrorSink` it drives. However the stream ends, including by the
  projection's own end, the touch geometry is cleared, which the old
  callback path left set, and every held finger is lifted.
- **AND-4 (phone half).** `DeleteRule.refusal` answers `not found` for a
  missing target and `cannot list` for a directory the provider cannot
  count. It answers `not empty` for a directory with children; that is the
  error text the daemon half (P-11, `MAG-D1-D`) maps to `ENOTEMPTY`.
  `PhoneStorage` asks it before `DocumentsContract.deleteDocument` in
  document-tree mode, and before `File.delete` in whole-phone mode, so both
  roots answer alike. A file never costs a listing.
- **AND-8.** `data_extraction_rules.xml` excludes `file/magnetita` (the
  private key, the certificate and `trust.json` that `Core` opens there)
  from `cloud-backup` and `device-transfer`. `backup_rules.xml` does the
  same for the legacy form. Both template comments and their TODO are gone.
- **Owners searched.** The pairing parse owner is
  `magnetita_proto::pair::QrPayload::parse_uri`, and the reachability owner
  is `magnetita_link::discovery::reachability_rank`. Neither is exported
  through UniFFI, and `magnetita-mobile` is outside this unit's commit
  scope, so the preview and the LAN rule are Kotlin for now. The preview
  only displays: the core still parses and pins. Both carry the AND-5 debt
  that P-13 (`AUD-1-E`) retires by exporting a preview and the address rule
  from the crate and deleting these Kotlin copies with their tests.

## Procedure

```sh
cd magnetita-android/app/src
G=/opt/gradle-8.14.3/lib
CP=$G/kotlin-stdlib-2.0.21.jar:$G/kotlinx-coroutines-core-jvm-1.6.4.jar:$G/junit-4.13.2.jar:hamcrest-core-1.3.jar
KOTLINC="java -cp $G/kotlin-compiler-embeddable-2.0.21.jar:$G/kotlin-stdlib-2.0.21.jar:$G/kotlin-script-runtime-2.0.21.jar:$G/kotlin-reflect-2.0.21.jar:$G/kotlinx-coroutines-core-jvm-1.6.4.jar:$G/trove4j-1.0.20200330.jar:$G/annotations-24.0.1.jar:$G/kotlin-daemon-embeddable-2.0.21.jar org.jetbrains.kotlin.cli.jvm.K2JVMCompiler -no-reflect -no-stdlib -jvm-target 17"
M=main/java/org/celestina/magnetita; T=test/java/org/celestina/magnetita
# RED: the new tests against the unchanged sources
$KOTLINC -cp $CP -d out-red $M/link/Ports.kt $M/link/DesktopSignal.kt \
  $T/link/PairingConsentTest.kt $T/mirror/MirrorControlTest.kt $T/storage/DeleteRuleTest.kt
# GREEN: the pure owners with the new and the neighbouring tests
$KOTLINC -cp $CP -d out-green $M/link/Ports.kt $M/link/DesktopSignal.kt $M/link/PairingConsent.kt \
  $M/mirror/MirrorControl.kt $M/storage/DeleteRule.kt $M/storage/DocumentPaths.kt \
  $T/link/PairingConsentTest.kt $T/link/DesktopSignalTest.kt $T/mirror/MirrorControlTest.kt \
  $T/storage/DeleteRuleTest.kt $T/storage/DocumentPathsTest.kt
java -cp out-green:$CP org.junit.runner.JUnitCore org.celestina.magnetita.link.PairingConsentTest \
  org.celestina.magnetita.mirror.MirrorControlTest org.celestina.magnetita.storage.DeleteRuleTest \
  org.celestina.magnetita.link.DesktopSignalTest org.celestina.magnetita.storage.DocumentPathsTest
cd ../../..
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
```

## Result

- **Exit:** RED compile failed with 68 errors, all unresolved references
  to the missing owners (`PairingConsent`, `PairingState`, `PairPreview`,
  `LanAddress`, `MirrorControl`, `MirrorSink`, `DeleteRule`). GREEN
  compiled with no warning, and JUnit printed `OK (18 tests)`: 6 new
  pairing tests, 3 mirror tests and 4 delete tests, plus the 5 existing
  `DesktopSignalTest` and `DocumentPathsTest` tests that share the
  compiled sources. The architecture, language (148 ratcheted) and
  documentation guards printed OK.
- **Observed:** the regression tests named by the program row exist and
  pass on the pure owners. A link offered without a confirm never yields
  a URI to pair. A public address, or one public address among LAN
  addresses, is refused before the person is asked. A key or global
  action without a stream reaches no sink. A non-empty directory answers
  `not empty`. A second link while one waits is ignored, and a stale
  offer cannot be confirmed.

## Limits

- Gradle, the Android SDK and lint are not in this container. The Android
  files (`MainActivity`, `PairConfirmScreen`, `LinkService`,
  `MirrorService`, `MirrorInput`, `PhoneStorage` and the XML resources)
  were checked by reading, not by a compiler or lint. The author runs
  `magnetita-android/scripts/verify-production.sh` (`testDebugUnitTest`,
  `lintRelease`), and the landing runs it on the landed tree.
- The JVM tests ran on Kotlin 2.0.21, not the project's 2.2.
- On the S25U, still to check by hand:
  - a `magnetita://pair` link from another app or a web page opens the
    consent screen and pairs only on "Emparejar";
  - a scanned QR shows the same screen;
  - a link naming a public address shows the refusal;
  - desktop keys and Back/Home/Recents do nothing without a mirror
    stream;
  - `rmdir` of a non-empty folder through the mount keeps the folder.
    Until P-11 maps the error, the desktop reports `EIO`, not
    `ENOTEMPTY`.
- The child count and the delete are two provider calls: an entry created
  between them is deleted with the folder. The storage access framework
  has no remove-if-empty call.
- A desktop whose only address is outside RFC 1918 or `fc00::/7` (for
  example a CGNAT or VPN address) can no longer be paired by link. The
  daemon's QR names the address of its IPv4 default route.
- The desktop does not yet show its own fingerprint on the pairing page,
  so the person confirms by intent and id, and cannot compare fingerprints
  side by side.

## Follow-up

- P-11 (`MAG-D1-D`) maps the phone's `not empty` answer to `ENOTEMPTY` on
  the daemon.
- P-13 (`AUD-1-E`) exports a pairing preview and the address rule from
  `magnetita-mobile` and deletes `PairPreview` and `LanAddress` with their
  tests (AND-5).
- Proposed for the Magnetita desktop: show the desktop's own certificate
  fingerprint next to its pairing QR, so the phone's consent screen can be
  compared with it.
