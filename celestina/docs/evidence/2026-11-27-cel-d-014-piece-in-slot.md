# Evidence: the open piece stays in its slot (CEL-D-014)

- **Date:** 2026-11-27
- **Scope:** `CEL-D-014`: `celestina/src/island`, `celestina/src/sheet`, `celestina/src/pieces`, `celestina/crates/celestina-ui`, `celestina/crates/celestina-paint`, their tests, the ledger row and `VAL-CEL-D-014`
- **Environment:** worktree `celestina-CEL-D-014` from `main`
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** "Celestina · la gota", version 3, approved by the author on 2026-11-27 (an artifact private to the author)

## Procedure

```sh
cargo test --workspace --locked
CARGO_TARGET_DIR=$SCRATCH/tgt cargo build --locked --bins --examples
scripts/smoke.sh --binary $SCRATCH/tgt/debug/celestina --only bar,osd,notify
```

The nested look ran through the guarded kit of `CEL-D-010` (`env -i`,
`--own-bus`, `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`, fake `wpctl`, `pactl`
and `brightnessctl`, the private fake system bus, nested niri on a 2560 by
1440 logical output at scale 1.5, a bar layout with the system load, network,
Bluetooth, volume, clipboard, battery, power profile and session pieces in the
end capsule). Every helper that talks to the nest calls `nest_guard` first; the
shell and the virtual pointer ran only inside the nest. A probe ran a burst of
six `grim` shots in a row (about 85 ms each) while a virtual pointer pressed
the volume piece, then the Bluetooth piece (a switch), then the empty area
(the close), at three delays between the first shot and the press (0, 40 and
80 ms), so the bursts sample different phases. The kit logged the start and
end of every shot and the end of every press; the offsets below are measured
from the press (taken as 62 ms before the pointer helper returned) to the
start and end of the shot. The shots are cropped to the end capsule and the
window under it; every crop was read.

## Result

- **Tests:** `cargo test --workspace --locked`: 2933 passed, 0 failed; fmt, clippy (`-D warnings`) and the architecture, documentation and language contracts pass
- **Rest:** [rest](./2026-11-27-cel-d-014-piece-in-slot/rest.png): the end capsule with every piece at full opacity and no plate
- **Open, volume:** four phases of one open, offsets from the press. [-9..+72 ms](./2026-11-27-cel-d-014-piece-in-slot/open-a-plus0.png): only the pointer's hover plate on the volume piece, the window not yet shown. [+72..+152 ms](./2026-11-27-cel-d-014-piece-in-slot/open-b-plus75.png): the hover plate is gone, the piece's icon and text are dimmed in place, the bar has no gap, no pill yet, and the window is a small dark rounded rectangle growing from the piece. [+153..+238 ms](./2026-11-27-cel-d-014-piece-in-slot/open-c-plus150.png): the piece still in its slot, the pill faintly lit behind it, the window larger and empty. [+329..+420 ms](./2026-11-27-cel-d-014-piece-in-slot/open-d-plus350.png): the pill grey behind the piece, the piece's content dimmer than the capsule's other pieces, the window's body fading in. [Settled](./2026-11-27-cel-d-014-piece-in-slot/open-settled.png): the pill behind the volume piece, its content at 80 %, the volume window below
- **Hover over the open piece:** [the pointer on the open piece](./2026-11-27-cel-d-014-piece-in-slot/hover-over-open-piece.png) is byte-identical to the settled shot: one pill, no second darker plate
- **Switch, volume to Bluetooth:** [+133..+219 ms](./2026-11-27-cel-d-014-piece-in-slot/switch-a-plus175.png): the pill is between the two pieces, over Bluetooth's left and the volume's left, the volume piece back at its full colour, the old window blurred and shrinking while the new one grows; [+306..+394 ms](./2026-11-27-cel-d-014-piece-in-slot/switch-b-plus350.png): the pill sits behind the Bluetooth piece, the volume piece at full opacity, the new window's content fading in
- **Close, by pressing outside:** [+49..+137 ms](./2026-11-27-cel-d-014-piece-in-slot/close-a-plus75.png): the pill still behind the Bluetooth piece, the window blurring out below it; [+98..+182 ms, another burst](./2026-11-27-cel-d-014-piece-in-slot/close-b-plus150.png): the pill still on Bluetooth, the window a blurred shrinking glass; [after the close](./2026-11-27-cel-d-014-piece-in-slot/close-c-after.png): the bar alone, no pill, every piece at full opacity
- **Reduced motion:** [open, +70..+159 ms](./2026-11-27-cel-d-014-piece-in-slot/reduced-open-a.png): the pill already behind the piece (no fade-in) and the window at its final size, empty; [open, +248..+336 ms](./2026-11-27-cel-d-014-piece-in-slot/reduced-open-b.png): the window's body fading in, the pill and the dimmed piece unchanged; [close, +47..+134 ms](./2026-11-27-cel-d-014-piece-in-slot/reduced-close.png): the pill and the dimming gone at once while the window's content still fades
- **Smoke:** `smoke: OK (only: osd,notify,bar)`; bar `rest_frames=0` (342 frames), notify `rest_frames=2` (532 frames, `late=0`), osd 488 frames; `exit 0`

## Limits

- The shots show the shape of the motion, not its timing: a `grim` shot takes about 85 ms and its capture instant inside that window is unknown, so the offsets above are windows, not instants. The 150 ms pill fade, the slide spring and the 80 % level are asserted by the clock tests. The look in the author's real session is left to `VAL-CEL-D-014`.
- The shots at +0, +75, +150 and +350 ms of the open, +175 and +350 ms of the switch and +75 and +150 ms of the close are the bursts' frames nearest those offsets, not frames taken at them.
- The shots predate one controller fix: in them a short stretch of the open (about +70 to +150 ms) shows the piece dimmed with neither the hover plate nor the pill, because the pill faded in over 150 ms on the exit curve. After the shots the controller made a fresh pill show at once, taking over the hover the press already showed (`OpenPiece::set`; test `opening_shows_the_pill_at_once_and_mutes_the_content_in_200_ms`). The shots were not retaken; the final review checks the change.
- The chevron's window was not shot: the nest has no tray item, so the end capsule has no tray piece (and no brightness piece, `CELESTINA_BACKLIGHT=0`); the switch was shot from volume to Bluetooth. The chevron's pill and turn are covered by `the_chevron_gets_the_pill_and_keeps_its_turn` and `the_chevron_turns_while_its_window_is_open`.
- The wheel, the right click and the click on the open piece were not driven in the nest; their rules are covered by the tests in `island/tests/open_piece.rs`.
- Reduced motion was shot only for the volume piece (open and close); the switch under reduced motion was shot and read but not copied here: the pill is already behind Bluetooth in the first frame after the press (+80..+167 ms), with no slide.
- "No frame is drawn at rest after a close" rests on the smoke's `rest_frames=0` for the bar (which opens no menu) and on the tests (`no_frame_is_asked_at_rest`); no nest log of frames after a close was read. Notify's `rest_frames=2` was one higher than in `CEL-D-010`; the smoke accepted it.
- The window still starts from a radius of half the piece's height (14) while the pill has radius 12; `CEL-D-013` replaces the open.
- Test-first order was not kept for Tasks 1 and 2 (see their reports).
- **Final review (Opus, high, `/code-review`), 2 rounds.** Round 1: 0 Critical, 1 Important, 3 Minor, 3 Nit. The Important (the hover fill stacked on the fading pill when closing with the pointer on the piece), one Minor (a fading pill reused for another piece) and one Nit (a no-op lint allowance) were fixed in `f1f02894`, each with a test seen failing first. Round 2: 0 Critical, 0 Important, 0 Minor, 2 Nit.
- **Left open:** a fresh pill shows at once also when a menu opens without the pointer (the controller's ruling, recorded in the plan); the held box's opacity does not dim a clipped node's focus ring (nothing triggers it today); the press recoil shrinks the piece but not its pill; closing with the pointer on the piece cuts every zone's fading pill, not only that piece's; a pointer that comes back onto the piece during the 150 ms fade stacks the hover on the fading pill. `VAL-CEL-D-014` decides whether any of these shows in the real session.

## Landing

- **Base revision:** `a99257a8eb7af7738ce68a637f17745ab722e014`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases bar,osd,notify only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:80d3cba47ec3484d612150a5672c7cf6d2be2b50e6db8d672ec8062e35dac0de, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
