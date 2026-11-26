# Evidence: a menu window that blurs out towards its piece, and a piece that crossfades (CEL-D-010)

- **Date:** 2026-11-26
- **Scope:** `CEL-D-010`: `celestina/src/sheet`, `celestina/src/island`, `celestina/crates/celestina-paint`, their tests, the ledger row and `VAL-CEL-D-010`
- **Environment:** worktree `celestina-CEL-D-010` from `main`
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** version 3 of the menu-close preview, approved by the author on 2026-11-26 (an artifact private to the author)

## Procedure

```sh
cargo test --workspace --locked
CARGO_TARGET_DIR=$SCRATCH/tgt cargo build --bin celestina
scripts/smoke.sh --binary $SCRATCH/tgt/debug/celestina --only bar,osd,notify
```

The nested look ran through the guarded kit of `CEL-D-009` (`env -i`,
`--own-bus`, `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`, fake `wpctl`, `pactl`
and `brightnessctl`, the private fake system bus, nested niri on a 2560 by
1440 logical output at scale 1.5, the author's bar layout). Every helper that
talks to the nest calls `nest_guard` first; the shell and the virtual pointer
ran only inside the nest. A probe started `grim` shots in the background and
pressed the piece (or the empty area) 0 to 100 ms later, so each burst has
five frames about 100 ms apart at different phases of the motion. The shots
are cropped to the end capsule and the window under it; every crop was read.

## Result

- **Tests:** `cargo test --workspace --locked`: 2643 passed, 0 failed. New: `opening_fades_the_piece_down_and_blurred_in_160_ms`, `closing_fades_the_piece_up_and_sharp_in_200_ms_from_the_first_frame`, `the_subtitle_is_never_hidden`, `reopening_during_a_close_hands_over`, `a_switch_back_hands_the_fade_over_from_where_the_piece_is`, `a_vanished_piece_drops_its_fade`, `reduced_motion_fades_the_piece_without_blur_or_offset`, the clock's own tests in `piece_fade.rs`, the walk inside a hidden node in `celestina-ui`; after review: `the_fading_piece_stays_in_its_slot_and_its_capsule`, `the_fading_piece_is_cut_to_a_slot_that_is_still_shut` (both seen failing with the piece painted at its final rect), `a_press_on_a_window_still_closing_does_nothing`, the switch input test (a press on the window being left no longer closes the new one), `a_strong_blur_is_drawn_at_a_lower_resolution_to_keep_its_sigma`, `the_body_shrinks_with_its_window`
- **Open, energy:** [first phase](./2026-11-26-cel-d-010-menu-close/energy-open-a.jpg) and [second phase](./2026-11-26-cel-d-010-menu-close/energy-open-b.jpg): two frames before the press (the piece with its hover plate), then the window as an empty dark rounded rectangle under the capsule with the piece already gone from the bar (in the second burst a faint blurred smudge of it is still at the capsule's right end), then the body fading in
- **Close, energy:** [first phase](./2026-11-26-cel-d-010-menu-close/energy-close-a.jpg) and [second phase](./2026-11-26-cel-d-010-menu-close/energy-close-b.jpg), retaken after the review fixes: the settled window, then the window's glass as a crisp shrinking rounded rectangle with its content blurred and shrunk with it towards the slot, fainter at each frame, while the piece's soft icon shows inside the capsule's end, then a faint empty glass and the piece nearly sharp, then the bar alone. Nothing flies
- **System load:** [open](./2026-11-26-cel-d-010-menu-close/sysmon-open.jpg) (the piece still fading at the capsule's left end while the window is empty; the body then fades in); [close](./2026-11-26-cel-d-010-menu-close/sysmon-close.jpg): the window's content blurs and shrinks towards the slot inside a sharp-edged glass, the piece's text shows only inside the capsule's end, cut to its slot (no overlap with the signal icon beside it), then sharp
- **Switch:** [switch](./2026-11-26-cel-d-010-menu-close/switch.jpg), energy menu open and the system load piece pressed: two windows in the second frame (the energy one with a crisp glass edge and blurred content shrinking towards its piece, the system load one growing), the old piece fading back in, then only the system load window
- **Exit:** smoke OK (only: notify,bar,osd); the bar at rest with `rest_frames=0`, the notify run with `rest_frames=1`
- **Nest environment:** the energy and Bluetooth pieces show the fake system bus's states (64 % battery, the Balanced profile)
- **Review:** 2 rounds (Opus, high; the implementers were Sonnet). Round 1: 0 critical, 3 important (the fading piece outside its slot and capsule; femtovg capping the blur at sigma 8, now drawn downsampled; the glass edge, now sharp with only the content blurred and scaled as in the approved preview), 5 minor and 3 nits, fixed. Round 2 re-read the fixes: 0 critical, 1 important, 3 minor and 2 nits. The controller fixed the important one (two neighbours that switch now share their slot room by width and opacity; `neighbours_that_switch_share_the_slot_room`, seen failing without the fix), the blur of a fading piece leaking past its slot (cut after the blur now), its layer remade on alternate frames (edges on a 32 px grid), the inside walk dropping its root's slide and two failing blur sizes evicting each other

## Limits

- The shots show the shape of the motion, not its timing (frames about 100 ms apart, capture instant unknown); the 160, 200 and 260 ms clocks are asserted by tests. The look in the author's real session is left to `VAL-CEL-D-010`.
- While a piece fades, it is painted inside its slot and cut to it (the room its neighbours leave, within the capsule as painted); it can still be pressed at its final rect while the slot is opening.
- The first frames of an open show the empty window and no piece: the piece's 160 ms fade is shorter than the 350 ms spring, so the piece has gone before the body has faded in. This is the approved open (unchanged) plus the approved fade.
- Reduced motion (opacity only, no blur, no offset, the piece back with the window instead of after it) is tested, not shot.
- A GPU that cannot make the offscreen images, and a piece larger than the texture side, fall back to a plain fade by the painter's own path (tested for the pure parts, not provoked in the nest).
- The piece's plate (hover or press) is not painted while the piece fades; only its icon and text move.
- femtovg's Gaussian is exact only to sigma 8 px: a stronger blur draws its layer at 1 / k of the resolution (k = ceil(sigma / 8), 3 at scale 1.5 for the 12 px close). The scaling-back is not checked against a reference image, and the nest's flat grey backdrop cannot show a box-shaped blur profile.
- A window still painted as it closes takes no press; a press on the old window of a switch does nothing (the review's rule).
- Test-first order was not kept for the clock: the tests were written after the code, then run.
- After review: a piece fading back in is hit-tested at its final rect while its slot still opens; a close that starts during an open paints one frame cropped before it shrinks.

## Follow-up

None.

## Landing

- **Base revision:** `efa1c367a226d2cb10dda2b5d41e620762945a4b`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases bar,osd,notify only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:21fe56828ba9b9eacc1353e0dcf81da3777c7dd4e90db30b4e49f2af7116ca73, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
