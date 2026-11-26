# CEL-D-010 Implementation Plan: a menu window that blurs out towards its piece

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Closing a menu window blurs it out, fades it and shrinks it towards its piece in one 260 ms motion, and the open piece crossfades in the bar (blur, 10 px towards the bar's bottom edge) instead of flying to and from the subtitle.

**Architecture:** The window's close becomes its own curve set in `SheetMotion` (rect towards the slot's pill, radius, glass and content opacities, blur). The painter gains one narrow call that paints a closure into an offscreen layer and draws it back blurred, built on the offscreen images and Gaussian filter the progressive blur already has (`CutRow` stays as it is). The flight module goes away; the bar paints the open piece with an opacity, a vertical offset and a blur on their own clock.

**Tech Stack:** Rust, `celestina-ui`, `celestina-paint` (femtovg 0.27), Celestina's Island and sheet modules.

**Spec:** `docs/superpowers/specs/2026-11-25-celestina-capsule-sheets-design.md` §4.2 (Motion, close of `CEL-D-010`) and §4.3 (Crossfade). Approved preview: version 3 of the menu-close preview, approved by the author on 2026-11-26 (artifact private to the author; source in the sessions' notes `cel-d/preview-approved/src14.html`, stage `v-D`).

**Branch:** `unit/celestina/CEL-D-010`, opened with `scripts/worktree.sh open celestina CEL-D-010` from `main`.

## Global Constraints

- Close: 260 ms. With `t` the time fraction: blur `12 · min(1, t/0.6)^0.7` px over glass and content together; content opacity `1 − (t/0.8)^1.4`; glass opacity `1 − t^1.6`; rect `lerp(window, slot pill, 0.7 · (1 − (1 − t)²))`; radius `lerp(20, 14, same factor)`. The window being left in a switch closes the same way.
- Open: unchanged (pill grows on the 350 ms OutBack 1.15 spring; body fades in over the last 65 %).
- Piece: open, 160 ms ONE_UI, opacity 1 → 0, offset 0 → +10 px (down), blur 0 → 4 px; close, 200 ms ONE_UI from the first frame of the close, opacity 0 → 1, offset +10 → 0, blur 4 → 0. The slot still closes and reopens on the spring (CEL-D-008). The subtitle is never hidden: it fades in with the body and out with the window.
- Reduced motion: no blur, no shrink, no offset; opacities still fade.
- The compositor blur region of a closing window follows its rect and leaves once its glass opacity is below 0.35 (Claude's call: the backdrop blur cannot fade).
- If the GPU cannot make the offscreen images, the window closes without the blur (fade and shrink only).
- At rest no frames are requested; no `unwrap`/`expect` in production; no `unsafe` outside `egl.rs`/`pam.rs`; product copy only in `src/copy.rs`; never touch `src/lock`, `src/polkit`, `celestina/scripts/`, `.githooks`, any `AGENTS.md`, `docs/contracts|governance|standards`.
- Nested runs only through the guarded kit (`nest_guard`), `env -i`, `--own-bus`, `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`, fake `wpctl`/`pactl`, one nest at a time. Never run a `celestina` binary or `niri msg` from the agent's own shell.

## Review Focus

1. Reopening the same piece during its close: the closing window and the crossfade hand over without a piece painted twice or left invisible.
2. A switch while the old window closes: two windows, only the new one takes input, the old one's blur layer is released.
3. Offscreen image failure or a window larger than the GPU's texture side: the close still completes (no blur), no panic.
4. A piece that vanishes during its fade: its clock is dropped, nothing stays hidden.
5. Frames at rest after a close (the blur layer and the piece's clock both end).

---

### Task 1: The window blurs out towards its piece

**Files:** `celestina/crates/celestina-paint/src/{painter.rs,backend.rs}` (one call: `Painter::blurred(rect: Rect, sigma: f32, draw: &mut dyn FnMut(&mut Painter))`, offscreen, falls back to plain drawing), `celestina/src/sheet/motion.rs`, `celestina/src/sheet/mod.rs`, `celestina/src/island/mod.rs` (`shapes` while closing), their tests.

- [ ] **Step 1: Write the failing tests**: `closing_blurs_fades_and_shrinks_towards_the_slot` (frames at 0, 0.3, 0.6, 1.0 of 260 ms: the blur, both opacities, the rect at the stated factor, radius); `a_switch_closes_the_old_window_the_same_way`; `the_blur_region_leaves_below_35_percent_glass`; `reduced_motion_closes_by_fading_only`; `a_failed_offscreen_layer_still_closes` (Review Focus 3); `no_frame_is_asked_after_a_close` (Review Focus 5).
- [ ] **Step 2: Run them and see them fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run all checks**: fmt, `cargo clippy --workspace --all-targets --locked -- -D warnings`, the same with `+1.100`, `cargo test --workspace --locked` (sum), the three guards.
- [ ] **Step 5: Commit**: `celestina-maintenance: Replace the menu window's fold with a close that blurs it out towards its piece`.

### Task 2: The piece crossfades in the bar, the nest check and the documents

**Files:** `celestina/src/island/bar_tree.rs` (the open piece's fade clock and its paint: opacity, offset, blur through `Painter::blurred`), `celestina/src/sheet/mod.rs` and `celestina/src/island/sheets.rs` (the flights go), delete `celestina/src/sheet/flight.rs` and `celestina/src/island/tests/flight.rs` (move the tests that still apply), ledger row `CEL-D-010` (if not added), evidence `celestina/docs/evidence/2026-11-26-cel-d-010-menu-close.md`, `VAL-CEL-D-010` in `celestina/VALIDATION.md`.

- [ ] **Step 1: Write the failing tests**: `opening_fades_the_piece_down_and_blurred_in_160_ms`; `closing_fades_the_piece_up_and_sharp_in_200_ms_from_the_first_frame`; `the_subtitle_is_never_hidden`; `reopening_during_a_close_hands_over` (Review Focus 1); `a_vanished_piece_drops_its_fade` (Review Focus 4); `reduced_motion_fades_the_piece_without_blur_or_offset`; the subtitle gestures and the slot tests of CEL-D-008 still pass.
- [ ] **Step 2: Run them and see them fail.**
- [ ] **Step 3: Implement** and remove the flight code and its dead paths.
- [ ] **Step 4: Run all checks** as in Task 1, then the nest check with the guarded kit (`scratchpad/d9/`, kept from CEL-D-009): bursts of frames during an open and a close of the energy and system-load windows (the piece fading down and up, the window blurring and shrinking towards the piece), read every crop; smoke `--only bar,osd,notify`.
- [ ] **Step 5: Write the documents and commit**: `celestina-maintenance: Replace the open piece's flight with a crossfade in the bar` and `celestina-maintenance: Add the CEL-D-010 evidence and VAL-CEL-D-010`.
