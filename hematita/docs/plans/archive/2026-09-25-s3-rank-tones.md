# S3-B — The storage tiles all sharing one colour

- **Opened:** 2026-09-25
- **Plan ID:** s3-rank-tones
- **Status:** done
- **Closed:** 2026-09-25
- **Successor:** none
- **Authorization:** the author asked on 2026-09-25 for a distinct colour
  per entry in the storage map and list after `S3` shipped every folder in
  blue; `celestina-style` 1.9.1 added `CelestinaTheme.usagePalette` and
  fixed the shared controls' corner geometry for it
- **Scope:** hematita
- **Implementation checkpoint:** S3-B
- **Author-validation checkpoint:** `VAL-S3` in [VALIDATION.md](../../../VALIDATION.md)

## Hypothesis

Painting each entry by its size rank from the six-tone palette tells
neighbouring tiles apart, while the filters keep their own colours.

## Tangible outcome

The installed 1.2.1 paints each list row's bar and its tile in one of six
tones rotating by size rank; duplicate, empty and unreadable entries keep
their colours and the remainder stays neutral.

## Scope

- `S3-B` — the rank tones in `StoragePage.qml`, and 1.2.1.

## Exclusions

No change to `celestina-style`, the projection in `hematita-core`, or the
shared controls; the corner fix arrives through the existing symlinks.

## Build order

1. `S3-B` alone, after `S3`.

## Implementation exit

`scripts/complete-production.sh` succeeds and the deployed binary is 1.2.1.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| S3-B | `hematita:` | done | [inventory](../../inventories/2026-09-25-s3-rank-tones/S3-B.numstat.tsv) | 12 files, +160/-13 | `toneColors` gains `p0`..`p5` from `CelestinaTheme.usagePalette` and drops `dir`/`file`; `toneOf(rank, ...)` keeps `unreadable`/`duplicate`/`empty` and otherwise returns `"p" + rank % 6`; tiles inherit their row's tone; 1.2.1 | [rank tones](../../evidence/2026-09-25-s3-rank-tones.md) | `VAL-S3` |
