# AND-6 — The application's design

- **Opened:** 2026-09-13
- **Plan ID:** app-design
- **Status:** active
- **Authorization:** the author asked on 2026-09-13 to close the whole
  development and leave one checkpoint open, the applications' design
- **Scope:** magnetita-android
- **Implementation checkpoint:** AND-6
- **Author-validation checkpoint:** none; the author's eye is the measure

## Hypothesis

With every capability of the link in the application and in daily use,
what remains is how it looks and feels, against MilaHub's One UI reading
of the suite's language; refined as the author judges it, one change at a
time, nothing behind it changes.

## Tangible outcome

An application the author calls finished.

## Scope

- `AND-6-A` — the checkpoint opened: `AND-5` archived with `VAL-MAG-15`
  passed.
- `AND-6-B` — the screens reviewed against MilaHub's language: the tab
  pill, the collapsing header, the groups, the icons.
- `AND-6-C` — the author's refinements, as they come.
- `AND-6-D` (P-3) — the audit's consent and data-loss fixes: a pairing
  intent pairs only after a confirmation screen and only with a LAN address,
  desktop keys and global actions need an active mirror stream, a non-empty
  folder delete is refused instead of recursive, and keys and pins are
  excluded from backup and transfer. It comes from the 2026-09-26 monorepo
  audit, whose whole program the author asked for, and is a row of this plan
  because the project has one active checkpoint (ruling R-A8 of
  [the audit evidence](../../../../docs/evidence/2026-09-26-monorepo-audit.md)).
  The audit finding named AND-6 (keys with no mirror session) is unrelated to
  this checkpoint's name; the findings are in
  [the Magnetita audit record](../../../../docs/evidence/2026-09-26-monorepo-audit-magnetita.md).

## Exclusions

- The core, the link, the mirror's capture, except for the recorded audit
  defects `AND-6-D` fixes.

## Build order

1. `AND-6-A`, `-B`, then `-C` for as long as the author asks.
2. `AND-6-D` from `main`, landed after the suite plan's `AUD-1-D` (P-2); the
   suite plan's `AUD-1-E` (P-13) follows it.

## Implementation exit

The author says the application looks finished. `AND-6-D` closes on its own
row's automated evidence and Magnetita Android's build and verify entries.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| AND-6-A | `magnetita-android:` | done | [inventory](../../inventories/2026-09-13-app-design/AND-6-A.numstat.tsv) | 13 files, +225/-75 | The checkpoint opened; `AND-5` archived | [record](../../evidence/2026-09-13-checkpoint-opened.md) | none |
| AND-6-B | `magnetita-android:` | planned | `app/src/main/java/org/celestina/magnetita/ui/` | — | The screens reviewed against MilaHub's language | record | none |
| AND-6-C | `magnetita-android:` | planned | `app/src/main/` | — | The author's refinements | record | none |
| AND-6-D | `magnetita-android:` | active | `app/src/main/java/org/celestina/magnetita/MainActivity.kt` (pairing intent); `app/src/main/java/org/celestina/magnetita/link/PairingConsent.kt` (pairing preview, LAN rule, consent state); `app/src/main/java/org/celestina/magnetita/ui/screens/PairConfirmScreen.kt`; `app/src/main/java/org/celestina/magnetita/link/LinkService.kt` (mirror key and global actions); `app/src/main/java/org/celestina/magnetita/mirror/MirrorControl.kt`, `MirrorService.kt`, `MirrorInput.kt`; `app/src/main/java/org/celestina/magnetita/storage/PhoneStorage.kt` and `DeleteRule.kt` (document delete); `app/src/main/res/values/strings.xml`; `app/src/main/res/xml/data_extraction_rules.xml` and `backup_rules.xml`; JVM tests under `app/src/test/` | — | Pair from an intent only after a confirmation screen showing id, fingerprint and address, and refuse non-LAN addresses. Gate `MirrorKey`/`MirrorGlobal` on `Streaming`. Answer ENOTEMPTY for non-empty document-tree deletes. Exclude keys and pins from backup and transfer. (P-3: AND-1, AND-6, AND-4 (phone half), AND-8) | [record](../../evidence/2026-09-26-pairing-consent.md): new JVM tests (intent without confirmation refused; public address refused; key without stream ignored; non-empty delete refused); `magnetita-android/scripts/verify-production.sh` (`testDebugUnitTest`, `lintRelease`) | none |
