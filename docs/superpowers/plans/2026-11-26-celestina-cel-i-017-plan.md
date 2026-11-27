# CEL-I-017 Implementation Plan: the launcher scrolls

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The launcher lists every answer, not eight. The wheel scrolls the list, a keyboard move keeps the selected row in view, and a thin bar shows while the list moves.

**Architecture:**
- The rows' area becomes a clipped 320 px box.
- An inner column, offset by a `celestina_ui::Scroll`, holds only the rows near the view, each placed at its full position.
- Wheel scrolls jump: the inner column is marked still for that layout.
- Keyboard reveals use the tree's normal slide.
- A pure helper module does the arithmetic.

**Tech Stack:** Rust; `celestina-ui` (`Scroll`, `Tree::slide_with`, `View::clip`), `celestina/src/launcher`.

**Spec:** `docs/superpowers/specs/2026-11-26-celestina-launcher-design.md` §5.4 and §9 call 2.

**Branch:** `unit/celestina/CEL-I-017` from `main` (after `CEL-I-015` if it has landed; rebase otherwise). Kind `milestone`.

## Global Constraints

- **Area.** 320 px high (8 rows of `theme.row_height`), the card's content width, clipped. The card's size is unchanged in this unit: the tab row is `CEL-D-012`'s.
- **Answer size.** The launcher's answer is no longer capped at `MAX_ROWS`; the index's own bound applies. The selector keeps its 8-row cap: rename the constant it uses to `SELECTOR_MAX_ROWS` (= 8) and leave the selector unscrolled.
- **Wheel.** The vertical amount of `UiEvent::Scroll`, in logical px, through `Scroll::wheel`, with no animation.
- **Keyboard reveal.** The smallest offset change that brings the selected row fully into view, sliding over `theme.motion` normal (200 ms) on the standard curve, or at once under reduced motion. A wrap from the last row to the first scrolls to the top, and the reverse to the bottom.
- **Scroll bar.**
  - Shown while the offset changes and for 800 ms after, then faded by the tree's opacity animation.
  - 4 px wide, 2 px from the area's right edge, radius 2, `Role::ControlHover`: the paint crate has no strong-hairline role, and `controlHover` (16 % white) is the nearest one to `dividerStrong` (14 %).
  - Its length is `viewport² / content`, at least 24 px. It is absent when nothing scrolls.
- **Built rows.** Only the rows in `visible ± 4` are built, so a row that enters with the tree's entrance animation enters out of view.
- **Language and safety.** English only; no new dependency; live-session safety and checks as in `celestina/AGENTS.md`.

## Review Focus

1. The focus ring of a selected row that the wheel scrolled out of view is not drawn outside the card: the ring is drawn only for a row that intersects the area, and is clipped to it.
2. A new answer (typing) resets the offset to the top; a new index (`set_index`) keeps the offset when the selected row still exists, else clamps it.
3. A wheel event over the field or the tab row does not scroll the list; only events whose key is the area or a row do.
4. A click on a row partly hidden under the area's edge launches that row, not the one under it (hit tests use laid-out rects; slides are visual only).
5. Holding Down through 300 rows does not pile up animations: each reveal restarts the slide from where the column is drawn.

---

### Task 1: the arithmetic

**Files:**
- Create: `celestina/src/launcher/scrolling.rs` (with tests)
- Modify: `celestina/src/launcher/mod.rs` (`pub mod scrolling;`)

**Interfaces:**
- Produces:
  - `pub fn reveal(offset: f32, top: f32, height: f32, viewport: f32) -> f32`: the new offset that shows `[top, top+height)` with the least change.
  - `pub fn built(offset: f32, item: f32, viewport: f32, count: usize, margin: usize) -> Range<usize>`: the items to build.
  - `pub fn thumb(offset: f32, content: f32, viewport: f32) -> Option<(f32, f32)>`: the bar's y and height, `None` when `content <= viewport`.
  - `pub const THUMB_MIN: f32 = 24.0; pub const THUMB_LINGER: Duration = Duration::from_millis(800);`

- [ ] **Step 1: Write the failing tests:**
  - `reveal(0, 400, 40, 320) == 120`;
  - `reveal(200, 40, 40, 320) == 40`;
  - an already visible row leaves the offset;
  - `built(85, 40, 320, 100, 4)` is `0..15`;
  - the `built` range is clamped at both ends;
  - `thumb(0, 640, 320) == Some((0, 160))`;
  - the thumb's length is never under 24;
  - the thumb is `None` at `content == viewport`;
  - NaN and negative inputs give finite, clamped results.
- [ ] **Step 2: Run** `cargo test -p celestina --locked launcher::scrolling`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the same tests. Expected: PASS.
- [ ] **Step 5: Commit** `Add the launcher's scroll arithmetic`

### Task 2: the scrolled view

**Files:**
- Modify: `celestina/src/launcher/state.rs` (lift the cap for `Source::Apps`; `SELECTOR_MAX_ROWS` in `selector.rs`)
- Modify: `celestina/src/launcher/view.rs` (the area; `card` takes the scroll)

**Interfaces:**
- Consumes: Task 1.
- Produces:
  - `pub struct ListScroll { pub offset: f32, pub thumb: Option<(f32, f32)>, pub thumb_shown: bool }`;
  - `view::card(theme, state, sweep, scroll: &ListScroll)` and `view::launcher(.., scroll)`;
  - `pub fn area_key() -> Key` (`launcher-area`), `pub fn inner_key() -> Key` (`launcher-inner`), `pub fn thumb_key() -> Key` (`launcher-thumb`);
  - `pub const AREA_ROWS: usize = 8;`

**Shape of the view:**
- The area is `view::col(area_key())`: the content width, height `AREA_ROWS × row_height`, `.clip()`, `.interactive()` so it takes wheel events.
- Inside it, the inner column is placed with `.absolute(0.0, -offset)` and sized to `rows × row_height`.
- Each built row is `.absolute(0.0, i × row_height)` and keeps `row_key(i)`.
- The thumb is absolute at the area's right edge, at opacity 1 when `thumb_shown` and 0 otherwise.
- The selector's state builds the same area with no scrolling (8 rows at most).

- [ ] **Step 1: Write the failing tests** in `view.rs`:
  - 30 answers with offset 0 build rows 0..12 (8 + 4), absolute at `i × 40`, inside a clipped 320 px area;
  - offset 400 builds rows 6..22 and places the inner column at −400;
  - the thumb is present with 30 rows and absent with 5;
  - the selector builds at most 8 rows.
  
  Update the existing tests that index `card(..).children[1]` to look the area up by key.
- [ ] **Step 2: Run** `cargo test -p celestina --locked launcher::view`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the same tests. Expected: PASS.
- [ ] **Step 5: Commit** `Lay the launcher's rows out in a clipped, scrolled area`

### Task 3: wheel, reveal, ring and bar timing

**Files:**
- Modify: `celestina/src/launcher/mod.rs` (`Launcher` gains `scroll: Scroll` and `thumb_until: Option<Instant>`; `handle`, `frame`/`advance`, `ring`, `layout`/`apply`)

**Behaviour:**
- **Wheel.** On `UiEvent::Scroll(key, _, vertical)` with `key` the area or a row (`row_of`):
  1. `scroll.wheel(vertical)`;
  2. if it moved: call `tree.slide_with(Duration::ZERO, Easing::default(), &[view::inner_key()])` before the next apply, set `thumb_until = now + THUMB_LINGER`, and mark the view stale.
  
  Use the `Easing` type `slide_with` takes.
- **Keyboard move.** After `move_selection`, compute `reveal` for the selected row, set the scroll to that offset (through `set_extent` and a wheel by the difference), and let the inner column slide with the normal duration. Also set `thumb_until`.
- **Extent and resets.**
  - `set_extent(rows × row_height, AREA_ROWS × row_height)` on every apply.
  - A new query resets the scroll to the top.
  - A new index keeps the offset when the selected row still exists (Review Focus 2).
- **`advance`.** While `thumb_until` is in the future it requests a redraw. When it passes, `thumb_shown` turns false and the view is rebuilt once.
- **`ring`.** It returns the selected row's rect intersected with the area's rect, or `None` when they do not meet.

- [ ] **Step 1: Write the failing tests:**
  - a wheel of +120 over a row moves the offset to 120 and shows the thumb;
  - a wheel over the field does nothing;
  - Down from row 7 to row 8 reveals it (offset 40);
  - Up from row 0 wraps to the last row at the bottom offset;
  - typing resets the offset;
  - the thumb hides after 800 ms (drive `advance` with fake instants);
  - the ring is `None` for a row scrolled fully out of view, and clipped for a half-visible one.
- [ ] **Step 2: Run** `cargo test -p celestina --locked launcher::`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the three workspace checks from `celestina/`. Expected: PASS.
- [ ] **Step 5: Commit** `Scroll the launcher's list by the wheel and keep the selected row in view`

### Task 4: evidence in a nest

- [ ] Private target dir and one nest, as in `celestina/AGENTS.md`, with a throwaway `XDG_DATA_HOME` holding 40 `.desktop` files.
  - Take shots: the list at the top, after a wheel (the bar visible), after Down × 12 (the selected row revealed, the ring inside the card).
  - Note whether rows entering at the edge fade visibly. If they do, raise the margin from 4 and say so.
- [ ] Run the launcher's smoke phases.
- [ ] Write the evidence.
