# AND-4 — The mirror over the link

- **Opened:** 2026-09-09
- **Plan ID:** link-mirror
- **Status:** active
- **Authorization:** the author said "sigue con todo" on 2026-09-09 with
  `AND-3`'s exit met
- **Scope:** magnetita-android
- **Implementation checkpoint:** AND-4
- **Author-validation checkpoint:** `VAL-MAG-14` in Magnetita's
  [`../../../../magnetita/VALIDATION.md`](../../../../magnetita/VALIDATION.md)

## Hypothesis

The phone's screen is a `MediaProjection` into a `MediaCodec` surface
encoder whose output goes straight down the link's video stream; consent
is the system's own dialog, reached from the application when it is in
front and from a notification otherwise; touches come back through an
accessibility service, the one way an application may act on another's
screen without `adb`.

## Tangible outcome

The desktop's Mirror control shows the phone with no cable, no developer
setting and nothing re-enabled after a reboot beyond the two grants the
device screen offers.

## Scope

- `AND-4-A` — capture, encode and stream: the consent activity, the
  `mediaProjection` foreground service, the encoder into the fixed video
  stream, the started and stop answers, the desktop's mirror signals, the
  geometry that is pure and tested.
- `AND-4-B` — input back: the accessibility service that plays the
  desktop's touches as continued strokes and its back, home and recents;
  the device screen's grant row.

## Exclusions

- Audio capture, key injection (the accessibility API has none), secure
  surfaces, screen-off capture.

## Build order

1. `AND-4-A`, then `-B`.

## Implementation exit

The geometry and the signals have JVM tests, `lintRelease` passes, the
release build is on the S25U; the mirror after a reboot is `VAL-MAG-14`.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| AND-4-A | `magnetita-android:` | done | [inventory](../../inventories/2026-09-09-link-mirror/AND-4-A.numstat.tsv) | 24 files, +806/-60 | Capture, encode and stream on consent; `AND-3` archived and `AND-4` opened | [record](../../evidence/2026-09-09-mirror-capture.md) | `VAL-MAG-14` |
| AND-4-B | `magnetita-android:` | done | [inventory](../../inventories/2026-09-09-link-mirror/AND-4-B.numstat.tsv) | 5 files, +210/-0 | Touches and navigation through the accessibility service | [record](../../evidence/2026-09-09-mirror-touch.md) | `VAL-MAG-14` |
