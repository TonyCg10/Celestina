# SEL-3 — Fixes after the first live use of the preview

- **Opened:** 2026-10-10
- **Plan ID:** sel-3-preview-fixes
- **Status:** active
- **Authorization:** the author used the corner preview live on
  2026-10-10 and reported three faults; the coordinator approved the fixes
  of this plan's one unit (a preview decoded at its shown size, the screen
  key binding on the focused monitor, recordings into `Grabaciones`) and
  the `Adopt` rule as the single owner Fluorita uses too. The preview's
  design is
  [the capture preview spec](../../../../docs/superpowers/specs/2026-10-10-capture-preview-design.md)
  (§4); the previous checkpoint is
  [SEL-2](../archive/2026-10-10-sel-2-preview.md)
- **Scope:** selenita
- **Implementation checkpoint:** SEL-3
- **Author-validation checkpoint:** VAL-SEL-FIXES in
  [VALIDATION.md](../../../VALIDATION.md) is pending (it does not block)

## Hypothesis

The preview of 1.1.0 is right in what it does and wrong in three details
the author met at once: the picture is decoded at a fixed width and shrunk
without smoothing, so a wide capture looks blurred and jagged; the screen
key binding takes every monitor, where a key pressed on one monitor means
that monitor; and recordings land in `Recordings` beside «Capturas», where
the design and the author want `Grabaciones`. Fixing the three in
Selenita, and making the `Adopt` rule's folder names one owner, makes the
preview usable day to day.

## Tangible outcome

The preview's picture (or poster) is decoded at its frame's size times the
screen's pixel ratio, both dimensions with the shape kept, and drawn with
smoothing and mipmaps. `selenita --screenshot screen` and
`Selenita1.Capture("screen")` take only the output niri reports focused
(`FocusedOutput`, then `grim -o <name>`), falling back to the card's
choice, logged, when niri cannot say; the window's «Pantalla» keeps its
chips. A recording lands in the videos folder's `Grabaciones` and
Selenita no longer makes `Recordings`. `selenita_core::names` owns both
folder names (`CAPTURES_FOLDER`, `RECORDINGS_FOLDER`) and
`selenita_core::history::FolderPaths::inside(pictures, videos)` applies
them for `adopted`.

## Scope

- `SEL-3-A` — `PreviewWindow.qml`'s decoded size, smoothing and mipmaps;
  `selenita_core::niri::focused_output` with its fixture; the capture
  worker's `ScreenOutput` and the controller's `captureFromBinding`; the
  activation route; `names::CAPTURES_FOLDER`, `names::RECORDINGS_FOLDER`
  set to `Grabaciones` and `history::FolderPaths`; the smoke, the tests
  and the documents.

## Exclusions

- Moving the author's files out of `Recordings`: the coordinator does it
  after the landing.
- The niri rule in the author's configuration and the suite's documents
  (spec, ADR, `HOST-HYGIENE.md`): the coordinator corrected them; this
  plan corrects only Selenita's README.
- Fluorita's adoption of `FolderPaths` and `names::CAPTURES_FOLDER`
  (FLU-P2-A); `adopted` and `Folders` keep their names and signatures.
- Any new external dependency, and any reference to the Celestina shell.

## Build order

1. `SEL-3-A`, in parallel with Fluorita's `FLU-P2-A`, which consumes
   `selenita_core::history::{adopted, Folders}` and `selenita_core::names`
   unchanged.

## Implementation exit

The checkpoint closes when `SEL-3-A` is `done`: the three fixes are in
with their tests, the smoke checks the focused output and `Grabaciones`,
and `scripts/complete-production.sh` succeeds on the landed tree.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| SEL-3-A | `selenita:` | done | [inventory](../../inventories/2026-10-10-sel-3-preview-fixes/SEL-3-A.numstat.tsv) | 33 files, +1254/-172 | Fix the preview's sharpness, capture the focused monitor and record into Grabaciones | [evidence](../../evidence/2026-10-10-preview-fixes.md) | VAL-SEL-FIXES pending |
