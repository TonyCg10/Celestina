# CEL-D-008 Implementation Plan: the capsule that never narrows

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An open capsule sheet keeps every other piece of its row visible; the open piece leaves its slot without a gap and flies down into the menu as its subtitle.

**Architecture:** The sheet's target never narrows below its capsule. The open piece is taken out of its capsule's row (the bar), the row's pieces slide together with the sheet's open spring, and the capsule's painted extent follows its pieces while they slide, so the sheet, which follows the capsule live, takes its new width in the same motion. Every sheet body starts with a subtitle row built from what the piece shows (its `Face`); a flight paints the piece's icon and text between the slot and the subtitle. The sheet stops cutting the row and the indicator pill is removed; the painter keeps its progressive blur (E1) for later use (the author's call on 2026-11-26).

**Tech Stack:** Rust, `celestina-ui` (taffy tree, slides), `celestina-paint` (femtovg 0.27), Celestina's Island and sheet modules.

**Spec:** `docs/superpowers/specs/2026-11-25-celestina-capsule-sheets-design.md` §4.2 to §4.4 (amended 2026-11-26). Approved preview: the whole-bar preview, version 4, approved by the author on 2026-11-26 (artifact private to the author; copy in the sessions' notes `cel-d/preview-approved/`, `src12.html`).

**Branch:** `unit/celestina/CEL-D-008`, opened with `scripts/worktree.sh open celestina CEL-D-008 --from unit/celestina/CEL-D-007` (stacked: CEL-D-007 lands first).

## Global Constraints

- Sheet geometry: capsule y 8, h 40, radius 20; side margin 16; open spring 350 ms (`motion.slow`) OutBack, overshoot 1.15 (`motion.overshoot`); close and leave 150 ms (`motion.exit`) with the exit curve.
- Subtitle row: height 40, padding x 4 (`spacing.xs`), gap 10, icon 22 px tinted `TextHi`, text `type_scale.row_title` (15) semibold `TextHi`, growing; a page sheet's overflow button (`copy::MORE`, as today) at its right.
- Close flight: the slot reopens at once; the flight starts 60 ms later and lasts 200 ms with the exit curve. Switch: the leaving piece flies up in 150 ms with the exit curve while the new one flies down with the open spring.
- Icon size in flight: from the piece's icon size to 22; text: from `type_scale.body` (13) to `row_title` (15); weight by cross-fade, regular alpha `1 − k`, semibold alpha `k`.
- Reduced motion: no flight, no slide; geometry jumps, opacities still fade.
- At rest the bar's frame count does not grow (the smoke's rest checks).
- Product copy only in `src/copy.rs`; English everywhere else; no `unwrap`/`expect` in production; no `unsafe` outside `egl.rs` and `pam.rs`; the word `unlock` is banned in `src`.
- Never touch `src/lock`, `src/polkit`, `celestina/scripts/`, `.githooks`, any `AGENTS.md`, `docs/contracts|governance|standards`.
- Nested runs only through the guarded kit (`nest_guard`), `env -i`, `--own-bus`, `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`, fake `wpctl`/`pactl`, one nest at a time. Never run a `celestina` binary or `niri msg` from the agent's own shell.

## Review Focus

1. A sheet open on the only piece of a capsule: the capsule has no piece left. Expected: the sheet still has a 40 px row (empty glass at the capsule's padding width), the subtitle shows the piece; no zero-width rect, no panic.
2. A piece that vanishes from the bar while its sheet is open (layout reload, tray change). Expected: the sheet closes as today (last anchor), no flight lands on a missing slot.
3. Clicking another piece of the row during the open spring (switch mid-flight). Expected: one piece flying down, one flying up, no piece painted twice, slots settle without a gap.
4. A piece's text changes during the flight (volume moves while opening). Expected: the subtitle and the flyer show the live text; no stale string at rest.
5. A capsule at the surface's edge with a menu wider than the room (narrow output). Expected: the side-margin clamp still holds; the row stays inside the sheet.

---

### Task 1: The capsule never narrows and the open piece leaves its slot

**Files:**
- Modify: `src/sheet/geometry.rs`, `src/sheet/mod.rs`, `src/sheet/motion.rs`, `src/island/sheets.rs`, `src/island/bar_tree.rs`, `src/bar/view.rs` (the zone views), `crates/celestina-ui/src/{tree.rs,layout.rs,paint.rs}`
- Keep in `crates/celestina-paint`: `soft_edge.rs`, `CutRow`, `Painter::cut_row`, `row_layers`, the backend's `RowLayers`, `begin_row`, `draw_row` and their tests (the author keeps the progressive blur for later use); only the sheet stops calling them
- Test: next to each module, as today

**Interfaces:**
- Produces: `geometry::target(capsule, piece, body, surface) -> Rect` with the new rule; `geometry::body_box(sheet: Rect, piece: Rect, body_w: f32) -> Rect` (where the body sits under the row); the bar hides one piece: `BarTree::set_lifted(&mut self, piece: Option<Key>, now: Instant)` (the piece out of its row, the row's slides on the open spring); `BarTree::paint(&mut self, painter, now, skip: Option<usize>, hidden: Option<Key>)` and `paint_row(…, hidden: Option<Key>)` (a node painted by nobody while it flies); `BarTree::slot_rect(&self, piece: Key) -> Option<Rect>` (where the piece's slot is, or was: the anchor's `piece`).
- Removes: `Indicator`, `Sheets::quiet_piece`, CEL-D-007's quiet paint path (`paint_children_quiet` and its highlight mute) in favour of the hidden node; `geometry::{cuts, row_image}`.

- [ ] **Step 1: Write the failing tests**
  - `geometry`: `a_menu_narrower_than_its_capsule_takes_the_capsule_width` (capsule x 1000 w 400, menu 240 → sheet x 1000 w 400; `body_box` centred on the piece clamped inside); `a_wider_menu_still_contains_the_capsule` (today's wider case unchanged); `the_side_margins_still_clamp`.
  - `bar_tree`: `a_lifted_piece_leaves_no_gap` (zone of three pieces, lift the middle one: the capsule's rest width shrinks by exactly that piece's width plus one gap; neighbours abut with the zone's gap); `the_row_slides_with_the_open_spring` (at half of 350 ms a neighbour's painted x equals the OutBack lerp, not ONE_UI's); `the_capsule_extent_follows_its_sliding_pieces` (painted haze rect and `shapes()` equal the pieces' painted extent plus the padding during the slide, the placement rect at rest); `landing_the_piece_reopens_its_slot` (unlift: the width comes back, same spring); `the_slot_keeps_its_centre_while_lifted` (`slot_rect` returns the old slot's centre, width 0).
  - `sheet`: `the_sheet_follows_the_capsule_while_it_shrinks`; `no_cut_row_is_painted` (the sheet no longer calls `cut_row`); `a_lifted_only_piece_keeps_a_row` (Review Focus 1); remove the Indicator's and CEL-D-007's single-pill tests and every sheet-side cut-row test (the painter's E1 tests stay).
  - Reduced motion: `reduced_motion_jumps_the_slot`.
- [ ] **Step 2: Run them and see them fail:** `cargo test -p celestina --lib sheet:: bar_tree::` and `cargo test -p celestina-ui`.
- [ ] **Step 3: Implement.** The zone view leaves the lifted piece out (or keeps a zero-width slot: the builder's call, design §7) and the tree's slides for that apply run on the sheet's open spring through one narrow `celestina-ui` call (for example `Tree::slide_with(duration, easing)` for the next apply only). The capsule's painted extent while its pieces slide comes from their painted rects; the sheet's anchor reads that extent and `slot_rect`. Stop the sheet cutting the row (its sheet-side callers go; the painter keeps E1) and remove the indicator with its dead code; keep O1 (full opacity).
- [ ] **Step 4: Run the tests and see them pass**, then `cargo fmt --check`, both clippy passes (`cargo clippy --all-targets -- -D warnings` and the `--release` pass the repository uses), full workspace tests (sum), the three guards.
- [ ] **Step 5: Commit:** `celestina-maintenance: Replace the narrowing capsule sheet with one that keeps its row and lifts the open piece out of its slot`, and the ledger row first if the controller has not added it.

### Task 2: The subtitle row

**Files:**
- Modify: `src/pieces/mod.rs` (the `Piece` trait), the pieces that open sheets (`volume.rs`, `brightness.rs`, `session.rs`, `workspaces.rs`, `sysmon.rs`, `clock.rs`, `weather.rs`, `battery.rs` and the page pieces), `src/sheet/` (a `subtitle` view), `src/osd/popup.rs` (`heading`, `body_size`), `src/island/view.rs` (`page_sheet`, `page_sheet_size`), the session and workspaces bodies, `src/copy.rs` only if a word is missing
- Test: next to each

**Interfaces:**
- Produces: `pieces::Face { icon: &'static str, text: Option<String> }`; `Piece::face(&self, cx: &BarCx<'_>) -> Option<Face>` (default: the page entry's icon and no text; pieces with text return it, the same string their bar view shows); `sheet::subtitle(theme: &Theme, face: &Face, fallback: &str, trailing: Option<View>) -> View` with keys `sheet-subtitle`, `sheet-subtitle-icon`, `sheet-subtitle-text` (Task 3 flies to them).

- [ ] **Step 1: Write the failing tests**: `the_subtitle_is_40_high_with_a_22_px_icon_and_semibold_row_title_text`; `a_piece_without_text_shows_the_menu_name` (session → `copy::PIECE_SESSION`, a page → its title; workspaces → `copy::WORKSPACES_FALLBACK` with `layout-grid`); `the_volume_subtitle_shows_the_live_percent`; `a_page_sheet_keeps_more_at_the_subtitle_right`; the level body has no heading row any more and its `body_size` drops it; `page_sheet_size` swaps the 32 px header for the 40 px subtitle; `every_piece_that_opens_a_sheet_has_a_face`.
- [ ] **Step 2: Run them and see them fail.**
- [ ] **Step 3: Implement** the trait method, the faces and the subtitle view; build every sheet body under it.
- [ ] **Step 4: Run all checks** as in Task 1.
- [ ] **Step 5: Commit:** `celestina-maintenance: Add the open piece's subtitle at the top of every capsule sheet`.

### Task 3: The flight, the nest check and the documents

**Files:**
- Create: `src/sheet/flight.rs` (pure timing and interpolation, plus its paint)
- Modify: `src/sheet/mod.rs`, `src/island/sheets.rs`, `src/island/bar_tree.rs` (source rects of the piece's icon and text nodes)
- Documents: ledger row `CEL-D-008` in `celestina/docs/plans/active/2026-11-25-cel-tracks.md` (if not added in Task 1), evidence `celestina/docs/evidence/2026-11-26-cel-d-008-capsule-keeps-its-row.md` with nested shots, `VAL-CEL-D-008` in `celestina/VALIDATION.md` (format of VAL-CEL-D-007)

**Interfaces:**
- Consumes: `Face`, the subtitle keys, `BarTree::set_lifted`, `paint(… hidden)`.
- Produces: `Flight { from: (Rect, Rect), to: (Rect, Rect), start: Instant, delay: Duration, duration: Duration, easing, up: bool }` with `fn at(&self, now) -> Option<FlightFrame>` (icon rect, text origin, text size, weight cross-fade `k`), `fn done(&self, now) -> bool`.

- [ ] **Step 1: Write the failing tests**: `opening_flies_icon_and_text_down_with_the_open_spring` (frame at 0, 50 % and 100 %: rects match the OutBack lerp; icon 18 → 22; text 13 → 15; `k` 0 → 1); `closing_reopens_the_slot_then_flies_up_after_60_ms_in_200_ms`; `switching_flies_the_old_piece_up_in_150_ms_and_the_new_one_down`; `the_flying_piece_and_the_subtitle_are_hidden_until_the_flight_lands`; `reduced_motion_has_no_flight`; `a_vanished_piece_drops_its_flight` (Review Focus 2); `the_flyer_shows_the_live_text` (Review Focus 4); `no_frame_is_asked_at_rest`.
- [ ] **Step 2: Run them and see them fail.**
- [ ] **Step 3: Implement** the flight and paint it above the sheet (and above the bar after the sheet has closed, until it lands).
- [ ] **Step 4: Run all checks**, then the nest check with the guarded kit (`scratchpad/d8/`, built from `d7/`): shots of the volume sheet open (every other piece visible, no gap), a frame mid-open (icon and text between slot and subtitle), a switch to session mid-motion, the close mid-motion; read every crop. Smoke `--only bar,osd,notify`.
- [ ] **Step 5: Write the documents and commit:** `celestina-maintenance: Add the flight of the open piece into its sheet's subtitle` and `celestina-maintenance: Add the CEL-D-008 evidence and VAL-CEL-D-008`.
