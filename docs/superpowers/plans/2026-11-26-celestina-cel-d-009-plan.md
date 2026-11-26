# CEL-D-009 Implementation Plan: each menu its own window

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A capsule menu becomes a window of its own under the bar, as large as its content, grown out of its piece and lined up with its capsule's outer edge.

**Architecture:** The sheet stops being one shape with its capsule. The bar always paints its capsules, and the sheet paints a separate rounded window (its own Haze, blur and input regions) at y 56. The window's rect comes from a new geometry rule (outer edge of the capsule, 16 px side clamp) and its motion starts from the open piece's pill and folds back into the piece's slot. A switch folds the leaving window into its own piece while the new one grows, so the leaving body gets a motion of its own. Page-backed windows measure their content instead of taking the Island page's size. The D-008 slot, flight and subtitle gestures stay; their ends move from the sheet's body box to the window.

**Tech Stack:** Rust, `celestina-ui` (taffy tree), `celestina-paint` (femtovg 0.27), Celestina's Island and sheet modules.

**Spec:** `docs/superpowers/specs/2026-11-25-celestina-capsule-sheets-design.md` §4.2 (amended for `CEL-D-009`) and §4.3. Approved preview: version 2 of the menu-window preview, approved by the author on 2026-11-26 (artifact private to the author; source in the sessions' notes `cel-d/preview-approved/`, `src13.html`).

**Branch:** `unit/celestina/CEL-D-009`, opened with `scripts/worktree.sh open celestina CEL-D-009` from `main`.

## Global Constraints

- Window: radius 20 (`radius_lg`), Haze glass of the capsules, top at y 56 (capsule y 8 + h 40 + gap 8), side margin 16 (`SIDE_MARGIN`), height capped at `560 − 56 − 8 = 496` on a 560 surface (scrolls beyond).
- Alignment: end zone (index 3) → the window's right edge on the capsule's right edge; start zone (index 0) → its left edge on the capsule's left edge; centre zones (1, 2) → centred on the capsule; then clamped to the 16 px margins (a window wider than its capsule grows inward).
- Motion: open from the piece's pill (the piece's rect in the bar, radius 14) to the window, 350 ms (`motion.slow`) OutBack 1.15 (`motion.overshoot`), radius 14 → 20, body fades in over the last 65 %; close folds into a pill at the piece's live slot, 150 ms (`motion.exit`) exit curve, body fades out over the first 60 %; a switch folds the leaving window into its own piece's pill (exit, 150 ms) while the new one opens. Reduced motion: geometry jumps, opacities still fade.
- Widths: volume 320, brightness 320, session 240, workspaces 240; page-backed windows take the content's natural width clamped to 240..420; heights are the subtitle plus the laid-out content, never the Island page's minimum.
- Unchanged from CEL-D-008: the slot closes and reopens on the spring, the icon and text fly to and from the subtitle (now the window's), the subtitle takes the piece's gestures, the other pieces keep full opacity and their hover.
- At rest the bar's frame count does not grow (the smoke's rest checks).
- Product copy only in `src/copy.rs`; English everywhere else; no `unwrap`/`expect` in production; no `unsafe` outside `egl.rs` and `pam.rs`; the word `unlock` is banned in `src`.
- Never touch `src/lock`, `src/polkit`, `celestina/scripts/`, `.githooks`, any `AGENTS.md`, `docs/contracts|governance|standards`.
- Nested runs only through the guarded kit (`nest_guard`), `env -i`, `--own-bus`, `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`, fake `wpctl`/`pactl`, one nest at a time. Never run a `celestina` binary or `niri msg` from the agent's own shell.

## Review Focus

1. A press on the bar while a window is open (another piece of the same capsule, another capsule, the Island capsule): the sheet no longer covers the row, so the bar must get the press and §4.1's switch or close-then-open must hold.
2. A switch inside one capsule: for 150 ms two windows show; only the new one takes input, both are in the blur regions, neither leaves a stale region after the fold.
3. A start-zone window on a narrow output (capsule near the left edge, window wider than the capsule): the left edge rule and the 16 px clamp agree, and the right side never leaves the surface.
4. A tall page window scrolled, then closed or switched: the flight up starts from the subtitle where it is painted, and the fold ends on the live slot.
5. The open piece vanishes while its window is open or folding: the window folds into the last slot it had and nothing stays hidden.

---

### Task 1: The window apart from its capsule

**Files:**
- Modify: `celestina/src/sheet/geometry.rs`, `celestina/src/sheet/motion.rs`, `celestina/src/sheet/mod.rs`, `celestina/src/island/sheets.rs`, `celestina/src/island/mod.rs` (paint order, `shapes`, the press routing), `celestina/src/island/bar_tree.rs` (the capsule always painted), and their tests.

**Interfaces:**
- Produces: `geometry::Edge { Start, End, Centre }` and `geometry::Edge::of_zone(zone: usize) -> Edge`; `geometry::window(capsule: Rect, edge: Edge, body: (f32, f32), surface: (f32, f32)) -> Rect`; `geometry::pill(piece: Rect) -> Rect` (the piece's rect in the bar; its radius is half its height); `geometry::WINDOW_TOP: f32 = 56.0`; `geometry::hit(window: Rect, x, y) -> Hit` with `Hit::{Body, Outside}` (no row window); `SheetMotion::radius(&self, now) -> f32`; the leaving body carries its own `SheetMotion` and its own slot so it folds into its piece; `subtitle_parts` takes the window rect (the body sits at the window's origin).
- Removes: `geometry::body_box`, `geometry::row_window`, `Hit::Row`, the sheet's painting of the capsule's row (`Sheets::paint`'s `row` closure) and the bar's `skip` of the open zone.

- [ ] **Step 1: Write the failing tests**
  - `geometry`: `an_end_window_lines_up_with_the_capsules_right_edge` (capsule x 1376 w 528 on 1920, body 360×300 → window x 1544, y 56, w 360, h 300); `a_start_window_lines_up_with_the_capsules_left_edge`; `a_centre_window_is_centred_on_its_capsule`; `a_window_wider_than_its_capsule_grows_inward_and_keeps_16_px` (both zones); `the_window_is_capped_at_496_on_a_560_surface`; `the_pill_is_the_pieces_rect`.
  - `motion`: `opening_grows_from_the_pill_on_the_spring` (rect at 0, 50 %, 100 % = OutBack lerp from pill to window; radius 14 → 20); `closing_folds_into_the_live_slot_in_150_ms`; `reduced_motion_jumps`.
  - `sheet`/`island`: `the_bar_paints_every_capsule_while_a_window_is_open` (no skipped zone); `the_window_is_its_own_shape` (`shapes` holds the capsule and the window as two rects; the window's top is 56); `a_press_on_another_piece_of_the_capsule_switches` and `a_press_on_another_capsule_closes_then_opens` (Review Focus 1); `a_switch_folds_the_old_window_into_its_piece_while_the_new_one_grows` (two shapes for 150 ms, then one; only the new one takes input; Review Focus 2); `a_narrow_output_keeps_a_start_window_inside` (Review Focus 3); `a_vanished_piece_folds_into_its_last_slot` (Review Focus 5); the D-008 flight tests now land on the window's subtitle; remove the tests of the row window and the body box.
- [ ] **Step 2: Run them and see them fail**: `cargo test -p celestina --lib sheet:: island::`.
- [ ] **Step 3: Implement.** The sheet's target is `geometry::window` from the live capsule extent (its outer edge does not move while the slot closes); the motion starts from `geometry::pill` of the open piece and closes to the pill of its live slot; `Sheets::paint` paints only the window(s): Haze, then the body clipped to the window; the leaving window paints itself with its own motion; `Island::shapes` lists the window(s) next to every capsule; presses on the bar while open go to the bar's input as §4.1 says.
- [ ] **Step 4: Run all checks**: the tests, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo +1.100 clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` (sum), the three guards.
- [ ] **Step 5: Commit**: `celestina-maintenance: Replace the capsule sheet with a window of its own that grows out of the open piece and lines up with its capsule`, after the ledger row if the controller has not added it.

### Task 2: Windows as large as their content

**Files:**
- Modify: `celestina/src/island/page_sheets.rs`, `celestina/src/island/view.rs` (`page_sheet_size`, `page_sheet`, `PAGE_SHEET_W`), the pages whose view needs a natural size (energy, Bluetooth, the system monitor, network, clipboard, notifications, media, phone, clock), and their tests.

**Interfaces:**
- Consumes: Task 1's window (its size is the body's size).
- Produces: `view::page_sheet_size(theme, content: (f32, f32)) -> (f32, f32)` where `content` is the measured natural size; the width is clamped to `PAGE_SHEET_MIN_W = 240.0 ..= PAGE_SHEET_W = 420.0`; the height is `spacing.xs + SUBTITLE_H + spacing.sm + content.1 + spacing.lg`, capped by the surface's room.

- [ ] **Step 1: Write the failing tests**: `the_energy_window_has_no_empty_area_under_its_rows` (three rows of 36: height = xs + 40 + sm + rows + lg); `a_page_window_is_as_wide_as_its_content` (between 240 and 420); `bluetooth_with_five_devices_measures_every_row`; `a_tall_page_still_scrolls_at_496`; the level, session and workspaces widths stay 320, 320, 240, 240.
- [ ] **Step 2: Run them and see them fail.**
- [ ] **Step 3: Implement** the measure (design §7 leaves the way to the builder: a layout pass of the page's view at the largest width, or a size each page declares), with no row cut and no empty area under the last row.
- [ ] **Step 4: Run all checks** as in Task 1.
- [ ] **Step 5: Commit**: `celestina-maintenance: Size each page-backed window to its content`.

### Task 3: The nest check and the documents

**Files:**
- Documents: ledger row `CEL-D-009` in `celestina/docs/plans/active/2026-11-25-cel-tracks.md` (if not added earlier), evidence `celestina/docs/evidence/2026-11-26-cel-d-009-menu-windows.md` with nested shots, `VAL-CEL-D-009` in `celestina/VALIDATION.md` (format of VAL-CEL-D-008).

- [ ] **Step 1: Nest check** with the guarded kit (`scratchpad/d9/`, built from `d8/`), the author's layout with the end capsule of the recording (system load, battery, network, Bluetooth, volume, brightness, clipboard, energy, session): shots at rest of the energy menu, Bluetooth, brightness and the system load (each lined up with the capsule's right edge, no empty area), a frame mid-open (the pill growing out of the piece), a switch mid-fold (two windows), a close mid-fold, and one start-zone window; read every crop; smoke `--only bar,osd,notify`.
- [ ] **Step 2: Write the documents and commit**: `celestina-maintenance: Add the CEL-D-009 evidence and VAL-CEL-D-009`.
