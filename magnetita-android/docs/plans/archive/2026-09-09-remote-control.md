# AND-3 — Remote control

- **Opened:** 2026-09-09
- **Closed:** 2026-09-09
- **Plan ID:** remote-control
- **Status:** done
- **Authorization:** the author said "sigue con todo" on 2026-09-09 with
  `AND-2`'s exit met
- **Scope:** magnetita-android
- **Implementation checkpoint:** AND-3
- **Author-validation checkpoint:** `VAL-MAG-13` in Magnetita's
  [`../../../../magnetita/VALIDATION.md`](../../../../magnetita/VALIDATION.md)
- **Successor:** AND-4

## Hypothesis

A control screen is one Compose page over the core's input and command
calls: the arithmetic between a finger and a pointer is pure Kotlin with
JVM tests, and every event leaves as a code, never a string a shell sees.

## Tangible outcome

The phone as trackpad, keyboard and command deck for the desktop.

## Scope

- `AND-3-A` — the control screen: trackpad (move, tap, two-finger scroll,
  long-press right button), the typing field and key row, the registered
  commands by name; a fast path for input that cannot wait for a poll.

## Exclusions

- Presenter mode, gamepad, absolute positioning.

## Build order

1. `AND-3-A`.

## Implementation exit

The screen's arithmetic and signals have JVM tests, `lintRelease` passes,
and the author moves the desktop's pointer from the phone (`VAL-MAG-13`).

Met on 2026-09-09 as far as this project can meet it: 20 JVM tests and
`lintRelease` pass and the release build is on the S25U; the pointer's
motion is the author's `VAL-MAG-13`.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| AND-3-A | `magnetita-android:` | done | [inventory](../../inventories/2026-09-09-remote-control/AND-3-A.numstat.tsv) | 29 files, +640/-79 | The control screen: trackpad, keyboard, commands; `AND-2` archived and `AND-3` opened | [record](../../evidence/2026-09-09-control-screen.md) | `VAL-MAG-13` |
