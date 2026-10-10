# SEL-1 — Foundation, capture, recording and 1.0

- **Opened:** 2026-10-09
- **Plan ID:** sel-1-foundation
- **Status:** active
- **Authorization:** the author approved the design in brainstorming on
  2026-10-09 and asked for the program to be implemented; the design is
  [the spec](../../../../docs/superpowers/specs/2026-10-09-reading-and-capture-design.md)
  and the task breakdown is
  [the plan](../../../../docs/superpowers/plans/2026-10-09-reading-and-capture.md)
- **Scope:** selenita
- **Implementation checkpoint:** SEL-1
- **Author-validation checkpoint:** VAL-SEL-SHOT (SEL-1-A) and VAL-SEL-REC
  (SEL-1-B) are pending; each unit adds its entry to
  [VALIDATION.md](../../../VALIDATION.md)

## Hypothesis

A capture tool built on `grim`, `slurp`, niri's IPC and the ScreenCast portal,
in the suite's conventions, can take screenshots and screen recordings out of
the terminal: one window with the capture, the recording and the history.

## Tangible outcome

A release binary installed under the author's prefix, at 1.0.0, that captures
the screen, a window or a region to the clipboard and the pictures folder,
records the screen to MP4, keeps a history, and answers niri key bindings.

## Scope

The skeleton is not a row of this ledger: it lands with the registration, in
suite unit EXT-1-B of
[the suite plan](../../../../docs/plans/active/2026-10-09-reading-and-capture.md).
It is the transparent window over the backdrop, the appearance follower, the
claim-first activation adapter (an `Open` is ignored), the
`SelenitaController` singleton reading `SELENITA_FAKE`, the three empty cards
(capture, recording, history) in the CUP-1-H grammar, the scripts,
the QML test harness and the document set; its record is
[the skeleton evidence](../../evidence/2026-10-09-skeleton.md).

- `SEL-1-A` — screenshots (screen, window, region), delay, destinations,
  history, open in Fluorita, show in Siderita, the niri key-binding flags.
- `SEL-1-B` — recording through the ScreenCast portal and GStreamer, the
  sound option, `--stop`.
- `SEL-1-C` — the keyboard and accessibility pass, 1.0.0.

## Exclusions

- Everything the design's §1 lists as out of scope for Selenita.
- Any reference to the Celestina shell.

## Build order

1. `SEL-1-A`, then `SEL-1-B`, then `SEL-1-C`.

## Implementation exit

`scripts/complete-production.sh` succeeds at 1.0.0 and the installed binary
captures and records with every tool of the scope.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| SEL-1-A | `selenita:` | done | [inventory](../../inventories/2026-10-09-sel-1-foundation/SEL-1-A.numstat.tsv) | 51 files, +5841/-133 | Screenshots of the screen, a window or a region with delay, clipboard and folder, the history card and the niri key-binding flags | [evidence](../../evidence/2026-10-09-screenshots.md) | VAL-SEL-SHOT pending |
| SEL-1-B | `selenita:` | done | [inventory](../../inventories/2026-10-09-sel-1-foundation/SEL-1-B.numstat.tsv) | 36 files, +3446/-173 | Recording to MP4 through the ScreenCast portal and `gst-launch-1.0` (the spike's choice), the sound option, `--record` and `--stop`, `ToggleRecording()` and `StopRecording()` | [evidence](../../evidence/2026-10-09-recording.md) | VAL-SEL-REC pending |
| SEL-1-C | `selenita:` | done | [inventory](../../inventories/2026-10-09-sel-1-foundation/SEL-1-C.numstat.tsv) | 21 files, +858/-51 | The keyboard (`1`/`2`/`3`, Enter, `R`, Escape, the history rows' arrows, Home/End, Enter and Delete, no key in a text field) and the accessibility pass (names and roles everywhere, the rows as list items, the last recording's line); 1.0.0 at the landing (`--kind release`), plan archived after it | [evidence](../../evidence/2026-10-09-exit.md) | VAL-SEL-SHOT and VAL-SEL-REC (keyboard and Orca steps added) |
