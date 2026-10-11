# SEL-4 — Recording controls

- **Opened:** 2026-10-10
- **Plan ID:** sel-4-recording-controls
- **Status:** active
- **Authorization:** the author approved the design on 2026-10-10: a
  floating recording bar (title «Grabando», «Detener» and «Descartar»,
  room for a pause button), `selenita --record-focused` recording the
  focused monitor without the portal's output dialog through
  `selenita --pick-output` as xdg-desktop-portal-wlr's chooser, and the
  documents for the niri rule, the `V` binding and the portal's
  `chooser_cmd`. Pausing by segments is the next unit. The previous
  checkpoint is [SEL-3](../archive/2026-10-10-sel-3-preview-fixes.md)
- **Scope:** selenita
- **Implementation checkpoint:** SEL-4
- **Author-validation checkpoint:** VAL-SEL-BAR in
  [VALIDATION.md](../../../VALIDATION.md) is pending (it does not block)

## Hypothesis

Recording works, but nothing on screen says it is under way when it was
started from a key binding (the main window stays hidden), stopping needs
a second key, a recording cannot be thrown away without a trip to the
history, and every recording asks in the portal's dialog for an output
the person usually means to be the one they are looking at. A small
floating bar while it records, and a start that answers the portal's
chooser with the focused monitor, make recording a key press away.

## Tangible outcome

While a recording is under way, however it started, a frameless window
titled «Grabando» shows at the top centre of the focused output without
the focus (niri's rule): a pulsing red dot (still under reduced motion),
the elapsed time (the recording card's clock) and «Detener» (stop and
save; the preview follows) and «Descartar» (stop and delete: no history
row, no preview). A compositor's close is refused. A key-binding launch
shows only the bar while it records. `selenita --record-focused`
(`Selenita1.RecordFocused()` for a running instance) asks niri for the
focused output and leaves a one-shot request in
`$XDG_RUNTIME_DIR/selenita/pick-output`; `selenita --pick-output`, the
portal's chooser, answers a fresh request with `Monitor: <name>` and
otherwise becomes the previous chooser, so `R` keeps its dialog.

## Scope

- `SEL-4-A` — `RecordingBar.qml`, the shared `RecordingClock.qml`, the
  bar in `Main.qml`, the `focused` route in `ActivationRoute.qml`;
  `selenita_core::pick` (the request's format, freshness, writing and
  consumption) with its tests; the recording worker's discard job and
  focused start (`record.rs`), the controller's
  `toggleFocusedRecording` and `discardRecording`, the activation's
  `RecordFocused`, the flags `--record-focused` and `--pick-output`, the
  chooser (`picker.rs`); the QML tests and stub, the smoke, the documents.
- `SEL-4-B` — pausing a recording by segments: a pause button in the
  bar, the segments joined into one file at the stop.

## Exclusions

- The author's niri and xdg-desktop-portal-wlr configurations: the
  coordinator edits them after the landing, as the README describes.
- The suite's `scripts/activation_contract.py`: its `Selenita1` reason
  names the methods in prose; listing `RecordFocused` there is a suite
  follow-up, and the allowlist itself does not change.
- Pausing (SEL-4-B), recording one window alone, and anything in the
  halted shell beyond running its installed chooser as the fallback.

## Build order

1. `SEL-4-A`.
2. `SEL-4-B`, on SEL-4-A's bar.

## Implementation exit

The checkpoint closes when `SEL-4-A` and `SEL-4-B` are `done`: the bar,
the discard, the focused start and the chooser are in with their tests,
pausing by segments joins them, the smoke covers the bar, the discard and
`--pick-output`, and `scripts/complete-production.sh` succeeds on each
landed tree.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| SEL-4-A | `selenita:` | done | [inventory](../../inventories/2026-10-10-sel-4-recording-controls/SEL-4-A.numstat.tsv) | 31 files, +2542/-114 | Add the recording bar and recording the focused monitor without the chooser | [evidence](../../evidence/2026-10-10-recording-bar.md) | VAL-SEL-BAR pending |
| SEL-4-B | `selenita:` | planned | `selenita/qml/RecordingBar.qml`, `selenita/src/record.rs`, `celestina-rs/crates/selenita-core/src/record.rs` | — | Add pausing a recording by segments | — | None yet |
