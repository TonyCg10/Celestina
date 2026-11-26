# CEL-D-002 to CEL-D-005: capsule sheets, implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking. Each task names a suggested model for its implementer; it is a heuristic estimate.

**Goal:** the bar's menus open like the Dynamic Island. The zone capsule of the pressed piece grows into that piece's menu, keeping its row of pieces on top, so the author can switch menus with one click. The centre Island reads the time, the date and the weather. The workspaces piece opens a list of workspaces with their windows' icons.

**Architecture:**
- A new product module, `celestina/src/sheet/`, owns the open **capsule sheet** of one output: its geometry, motion, body, scroll, open-piece indicator and spotlight.
- The `Island` stays the coordinator of its surface. It keeps one rule: one open thing per output, the Island page or one sheet. It delegates the sheet's state to `Sheets`, through a second `impl Island` block in `src/island/sheets.rs`.
- The painter gains an offscreen progressive row blur and a cached spotlight image. The toolkit gains an OutBack easing, a scroll offset and scoped tree painting.
- The level, session and page contents lose their own popup card and become sheet bodies, laid out in body-local coordinates and painted through a translate.

**Tech Stack:** Rust on the celestina workspace (pinned toolchain plus stable 1.100); femtovg 0.27 (render-to-image, `filter_image` Gaussian blur, composite operations); taffy through `celestina-ui`; `celestina-rs/crates/celestina-shell` for the bar layout.

**Spec:** [`docs/superpowers/specs/2026-11-25-celestina-capsule-sheets-design.md`](../specs/2026-11-25-celestina-capsule-sheets-design.md). It supersedes §6.2–§6.5 of the standing-tracks design. Where the spec and the preview differ, the spec wins.

**Approved preview:** version 7 of "Celestina · paneles anclados", approved by the author on 2026-11-25. Until `CEL-D-002` commits its frames, the reference copy is outside the repository:
- the page and its source: `/home/toni/.config/Claude/side-session-notes/local_306a3d35-bdfe-402d-9860-465ab8c52ddd/cel-d/preview-approved/` (`src6.html`: `sheet()`, `masks()`, the `.beam/.cone/.core` CSS, the workspaces list);
- the blur spike's diff: `.../cel-d/spike-progressive-blur.diff`;
- the geometry and architecture fact sheet: `.../cel-d/fact-sheet.md`.

## Global Constraints

- **Worktrees.** Open each unit from the canonical checkout `/home/toni/CODIGO/CELESTINA` with `sh scripts/worktree.sh open celestina CEL-D-00N` (each part gives the exact line). Never edit the canonical checkout.
- **Read first.** Run `python3 scripts/agent-context.py celestina/src` and read every "Read now" document, then `celestina/AGENTS.md` completely. Read `docs/standards/architecture.md` (a new module) and `docs/contracts/landing.md` "Stacked branches" before opening a stacked unit.
- **Language.** English in code, comments, tests, evidence and commits. Spanish product copy only in `celestina/src/copy.rs`; the one new string is `copy::WORKSPACE_EMPTY`, the spec's empty-workspace label (`CEL-D-005`).
- **Checks, from `celestina/`; each must pass:**
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo +1.100 clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`; sum every `test result` line yourself and write the sum in the evidence.
- **Guards, from the worktree root:** `bash scripts/check-architecture-contract.sh`, `bash scripts/check-documentation-contract.sh`, `python3 scripts/check-language-contract.py`.
- **Commits.** Branch subjects read `celestina-maintenance: <Verb> …`. The verb is one `scripts/commit_scope.py` accepts: Add, Fix, Update, Replace or Remove ("Show", "Mute" and "Open" are refused). The landing (`land-unit.py`, run by the controller) sets the final kind (milestone) and the version. Never bump a version.
- **Ledger and validation.**
  - Each unit changes only its own row of `celestina/docs/plans/active/2026-11-25-cel-tracks.md`: status, Files/areas, an intended change starting with Add, Fix, Update, Replace or Remove, the evidence link and its VAL id. It is the only active plan a unit may change.
  - Each unit appends its own `VAL-CEL-D-00N` row to `celestina/VALIDATION.md`, in the field format of `VAL-CEL-D-006`, with `Result: not run`. Sibling units conflict at the file's end: keep both sides.
  - Evidence goes to `celestina/docs/evidence/YYYY-MM-DD-cel-d-00N-<slug>.md` (the day the evidence is written), with screenshots under `celestina/docs/evidence/YYYY-MM-DD-cel-d-00N-<slug>/`.
  - The rows link the design and this plan: `[design](../../../../docs/superpowers/specs/2026-11-25-celestina-capsule-sheets-design.md)` and `[plan](../../../../docs/superpowers/plans/2026-11-25-celestina-capsule-sheets-plan.md)`.
- **Never touch:**
  - `src/lock`, `src/polkit`, `celestina/scripts/` and `.githooks`;
  - any `AGENTS.md`;
  - `docs/contracts`, `docs/governance` and `docs/standards`.

  `tests/lock_guards.rs` bans the word `unlock` anywhere in `src`.
- **No `unsafe`** outside `crates/celestina-wayland/src/egl.rs` and `crates/celestina-lock-verify/src/pam.rs`. The spike's `gl_finisher`, its `glFinish` timing and its `SPIKE-STAT` report are measurement only: never carry them over.
- **No live session.** Never run `celestina` against the author's session. Use one of the two nested paths, never two nests at once:
  - **The capture kit** `/tmp/claude-1000/-home-toni-CODIGO/9ae1fc06-d8f7-4b4d-9fe5-37a07268db87/scratchpad/nest/`. Copy the `run-proto.sh` pattern: host-wide nest lock, `env -i`, fake `wpctl`/`pactl` first on `PATH`, `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`, `--own-bus`. Point `--binary` at your private build.
  - **The smoke:** `timeout -k 60 2700 sh celestina/scripts/smoke.sh --binary <tgt>/debug/celestina --only <phases>`. Build into a private target dir (`CARGO_TARGET_DIR=<your scratchpad>/tgt`). Never `timeout -s KILL`.

  Wait at least 400 ms after a click before a screenshot.
- **Painter trap.**
  - Inside a frame the painter is already borrowed (`PopupShell::frame`), and `Island::input` receives `&mut Painter`. Never `try_borrow_mut` the painter from a stored closure.
  - Content that measures text takes the measure in hand as an argument: the frame's painter, or the `Measure` of `Island::input`.
  - After Part A no sheet content keeps a stored measuring closure.
- **Preserve, with their tests** (`osd/popup.rs`, `osd/monitor_rows.rs`, `osd/audio_rows.rs`):
  - the asked-level hold of CEL-I-007 and CEL-I-009: `MonitorMirror::asked`, `adopt(cell, now)`, `wait(now)`, `ASK_HOLD`, `brightness_asked`, `hold_brightness(now)`;
  - the slider wheel of CEL-I-008: `wheel`, `ask_level`, `shown_level`, `wheel_rest`, `wheel_last`, `WHEEL_BURST`;
  - the CEL-I-004 device and application rows and folded sections;
  - the CEL-D-006 speaker mute button;
  - the CEL-I-003 multi-monitor brightness;
  - the 80 ms `SEND_INTERVAL` throttle;
  - the OSD `Suppression::Open` / `Suppression::Closed { kinds, asked, written }` semantics.
- **Unchanged surfaces:**
  - hover and press stay as today (instant `ControlHover`/`ControlPressed`, 0.96 recoil);
  - tray item menus and the Island's overflow menu (its button reads `copy::MORE`) stay `xdg_popup`s;
  - the panel surface stays output width × 560, layer Top.
- **Rest.** At rest the bar's frame count must not grow (the smoke's rest checks). The progressive blur runs only while a window cuts a row. An open sheet at rest asks for no frame.
- **Smoke phases** (`smoke.sh --list`: `notify launcher bar wallpaper energy glance clipboard connectivity osd phone media overview polkit cutover`). Each part names the phases its area touches. The controller lands A to C with `land-unit.py --smoke <phases>` and D with the full smoke.
- **Review rounds:** at most 2 per unit (no sensitive path is touched).
- **Every implementer report says what was not done or could not be checked.**

## Review Focus

These are the five input classes most likely to bite. Each names its test and the task that owns it.

1. **Geometry at the edges:**
   - a narrow output (a body wider than the room);
   - a capsule touching the side margin;
   - a NaN or empty piece rect;
   - the OutBack overshoot past the 544 cap.

   `f32::clamp` panics when min > max. Test: `the_target_never_panics_and_keeps_the_margins_when_it_can` (A3).
2. **Two things open at once:**
   - rapid presses during the 150 ms close;
   - the Island capsule ↔ a sheet;
   - an `island open` verb while a sheet is open;
   - Escape while a request is queued.

   Test: `never_two_things_open_at_once_through_any_sequence` (A4).
3. **The display left quiet for good.** A level sheet closes by a path that never releases its kinds: Escape, an outside press, a switch, the Island page, another output, the duty off, the output gone, the piece vanishing. Test: `every_way_a_level_sheet_closes_releases_the_display_once` (A5).
4. **The painter borrowed in `Island::input`.** Folding a volume section or opening the session confirm card measures text. Tests: `a_fold_inside_island_input_cuts_with_the_measure_in_hand` (A4) and `the_confirm_card_measures_its_question_with_the_measure_in_hand` (B2).
5. **The bar changes under an open sheet:**
   - its piece leaves (the reading is lost, the layout is edited);
   - its capsule widens ("9 %" → "100 %");
   - the output resizes.

   Tests: `a_sheet_whose_piece_leaves_the_bar_closes` and `a_capsule_that_changes_width_while_open_carries_the_sheet` (A4).

---

## Part A — `CEL-D-002`: the open-sheet owner, the capsule sheet, the volume sheet

**Worktree:** `sh scripts/worktree.sh open celestina CEL-D-002` (from `main`).
**Smoke phases for its landing:** `bar,osd`.

### Decisions (controller's calls, final)

1. **Module layout.**
   - New module `celestina/src/sheet/`, declared in `src/lib.rs`:
     - `mod.rs`: `Sheets`, the owner;
     - `geometry.rs`: the pure rect rules;
     - `motion.rs`: the pure clocks;
     - `body.rs`: `SheetBody`.
   - The Island-side wiring is a second `impl Island` block in new `src/island/sheets.rs`. `island/mod.rs` only gains the field `sheets: Sheets` and call sites in `frame`, `input`, `dispatch`, `bar_input`, `paint`, `shapes`, `sync_ctl`, `open`, `close` and `command`.
   - Add a line for `src/sheet/` to the Architecture table of `celestina/README.md`.
2. **What is open.**
   - **Shown means painted.** The Island shows nothing, its page (`IslandState`), or one sheet (`Sheets`); a closing sheet still counts as shown.
   - **A request while something else is shown** closes that thing and is queued as `Next::Sheet(Request)` or `Next::Page(PageId)`. It opens once the other's close has ended: the Island's progress is 0, or the sheet's close flight has ended. Under reduced motion that is the same input.
   - **Escape, an outside press and `FocusLost`** drop the queue.
   - **Verbs:**
     - `island open <page>` closes a sheet, then opens the page in sequence;
     - `island close` closes whichever is open;
     - `island toggle` closes whichever is open, or opens the last page.
   - `App` code that asks `island.page().is_some()` before closing (app.rs ~2490) asks a new `Island::anything_open()` instead.
3. **Press rules while a sheet is open.** These are the spec's §4.1. `geometry::hit` only tells `Row` (inside the row window), `Body` (inside the viewport) and `Outside`; the Island then asks the bar tree which piece, if any, is under the pointer. One routing table:
   - **`Hit::Row` on a piece of the sheet's own zone (visible in the window):**
     - a Primary that asks for a sheet switches to it;
     - the open piece pressed again closes its sheet (the preview's rule);
     - any other gesture acts, and the sheet closes;
     - the wheel never closes it.
   - **On another capsule's piece, or on the Island capsule (outside the sheet):**
     - a sheet or page request closes the sheet and opens the request in sequence;
     - any other gesture acts and closes it.
   - **`Hit::Row` not on a piece** (the row's glass): nothing.
   - **`Hit::Body`:** the body.
   - **`Hit::Outside`** on another capsule's piece or the Island capsule: the rules above; anywhere else: closes.
   - **Overlap:** the sheet is hit before the bar and the Island capsule, so it takes the input of whatever it covers.
4. **Kinds.**
   - `pieces/mod.rs` gains `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum SheetKind { Volume }` (Part B adds `Brightness`, `Session` and `Page(PageId)`; Part D adds `Workspaces`) and `PieceAction::Sheet(SheetKind)`.
   - The Volume piece's Primary gives `PieceAction::Sheet(SheetKind::Volume)`. `AudioAction::Popup` is removed. `BrightnessAction::Popup` and `PieceAction::SessionMenu` stay until Part B.
5. **Bodies.**
   - `SheetBody::Level(Box<LevelCore>)` in Part A.
   - A body is laid out in body-local coordinates: (0, 0) is the body box's top-left.
   - The body box sits at `sheet.y + 40`. Its padding is `(top spacing.xs, sides P, bottom P)`, with P = `spacing.md` for levels, `spacing.sm` for session and workspaces, `spacing.lg` for pages.
   - It is painted with a new `Painter::translate(dx, dy)`: (`sheet.x`, `sheet.y + 40 − scroll`), rounded to device px.
   - Input reaches it translated back the same way.
6. **How LevelCore loses its card.**
   - `card()` takes `frame: CardFrame` (`Popup | Sheet`). `Sheet` has no haze, width `SHEET_W = 320.0`, the padding above and gap `spacing.sm`.
   - `body_size(theme, kind, microphone, shape) -> (f32, f32)` = `(320, xs + Σ children + gaps·sm + md)`.
   - `LevelCore::size()` returns the frame's size.
   - The stored `fit: Box<dyn FnMut>` field is removed:
     - `LevelCore::new` takes `fit: &mut dyn FnMut(&str, f32) -> String`, used only during the call;
     - `LevelCore::input` and `sync` take the same argument.
   - `LevelPopup` (brightness, until Part B) builds a local fit closure per `input` call; that is not a stored closure.
7. **Scroll.**
   - `celestina_ui::Scroll` is a pure offset. The sheet sets its extent each frame: content = body box height, viewport = `sheet.h − 40`.
   - A wheel over the body goes to the body when `Tree::slider_at(x, y, now)` names a slider under the pointer (CEL-I-008 keeps working). Otherwise it scrolls, one logical px per px of axis value, clamped.
   - A body that fits never scrolls.
8. **Paint order** (`Island::paint`):
   1. the bar, except the sheet's zone (`BarTree::paint(.., skip)`);
   2. the Island haze, capsule and page, as today;
   3. the sheet's haze over `sheet.rect` (radius `radius_lg`);
   4. the row: through `Painter::cut_row` when `geometry::cuts` finds a cut; otherwise under one rounded clip of `row_window(sheet)`. Either way it draws the indicator pill, then `BarTree::paint_row(zone)` (the zone's children; its own box and haze are skipped);
   5. the spotlight, under `push_clip(sheet.rect, radius_lg)`;
   6. the leaving body, then the body, each under one rounded clip of `viewport(sheet)` (radius `radius_lg`). The painter keeps exact corners only for a first clip, so the body never nests a clip inside the sheet's.
9. **Spotlight painting.** `Painter::spotlight(pill: Rect, alpha: f32)` draws one cached premultiplied image (one blurred image, the spec's §7 option):
   - a cone and the preview's core, each a trapezoid filled with a vertical gradient;
   - each rendered offscreen and blurred with `filter_image(GaussianBlur)`: cone σ 7, core σ 5;
   - cached while `(round(pill.w · dpr), dpr)` holds.

   The core is the preview's `.core`; the spec names only the cone. It is a faint inner highlight of the approved page, and it is kept.
10. **Caching of the blurred row.**
    - The GPU images are kept while their pixel size holds: the source, three blur levels, four masks and the sum.
    - The content is composed again every painted frame while a cut exists (the spike measured about +0.19 ms of GPU).
    - Nothing is composed while no window cuts a row.
    - No content cache: frames while open come from content changes, which would invalidate it.
11. **Regions and keyboard.**
    - Blur region: the bar's shapes with the sheet's zone replaced by `sheet.rect` (`BarTree::shapes(.., skip)`), plus the Island's rect.
    - While anything is shown, the input region is the whole surface and the keyboard is `OnDemand`. At rest both are as today.
12. **What replaces `PopupClosed` for the level sheet.**
    - The Island owns the sheet's lifecycle; no popup event exists.
    - After every `Island::input` and every Island frame the app calls `Controls::settle(surface, island.level_sheet_open())`. It returns `Suppression::Closed` once, when a level sheet that `Controls` holds is no longer open or queued on that Island.
    - Verbs need no own call: a closing sheet animates, so a frame follows.
13. **One level sheet across outputs.**
    - `Controls::open` while one is open elsewhere releases it (`Suppression::Closed`) and returns `Released { elsewhere: Some(surface) }`.
    - The app posts `Message::CloseLevelSheet(surface)`, and the message handler (which has `RuntimeCtl`) closes it there.
14. **Close flush.** Closing a `Level` body by any path, or leaving it in a switch, calls `flush(now, true)` first, so what the throttle holds is sent.
15. **A vanished piece.** If the open piece's node is no longer on the bar after a layout, or its zone has no capsule, the sheet closes on the exit motion, from its current rect to the last capsule rect it had.
16. **Tests of today's popup path that move or change in Part A:**
    - In `osd/popup.rs`, the rig helpers pass a fit to `new`, `input` and `sync`.
    - `a_default_that_appeared_is_cut_with_the_frames_measure_not_the_stored_one` now asserts that the measure passed in is the one used; no stored cutter remains.
    - `the_card_fills_the_popup_in_every_shape` keeps `CardFrame::Popup` and gains a Sheet twin, `the_volume_body_is_320_wide_and_as_tall_as_its_rows`.
    - A body that changes size (a fold) is noticed by `Sheets::sync`, which compares `body.size()` with the last one and starts the resize motion; there is no `resize_wanted` on the sheet path.
    - `folded_or_not_the_popup_fits_eight_rows_each_within_1080` becomes `the_tallest_volume_body_scrolls_inside_a_544_sheet`.
    - In `island/mod.rs`, `the_level_pieces_appear_with_a_reading_and_ask_for_their_workers_and_popup` expects `OpenLevelSheet { kind: Volume, piece }` for the volume. The brightness half is unchanged until Part B.
    - In `osd/control.rs`, the `opened` helper records `Place::Sheet`, and `a_new_graph_refreshes_only_an_open_volume_popup` expects `LevelRedraw::Island(surface)`.

### Task A1: Toolkit motion, scroll and scoped painting — **Model: Sonnet**

**Files:**
- Modify: `crates/celestina-ui/src/curve.rs`, `anim.rs`, `lib.rs`, `tree.rs`, `paint.rs`, `input.rs`
- Create: `crates/celestina-ui/src/scroll.rs`
- Modify: `crates/celestina-paint/src/theme.rs`
- Modify: the `CEL-D-002` ledger row, set to `active`, Files `pending`, Diffstat `pending`, with the intended change of Task A6.

**Interfaces — Produces:**
```rust
// curve.rs
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing { Bezier(CubicBezier), OutBack(f32) }
impl Easing { pub fn eval(&self, x: f32) -> f32 } // x clamped to 0..=1, NaN → 1; OutBack not clamped above 1
impl From<CubicBezier> for Easing
// anim.rs: the curve becomes `impl Into<Easing>`; every caller compiles unchanged
pub fn animate_to(&mut self, v: f32, duration: Duration, curve: impl Into<Easing>, now: Instant, reduced: Reduced)
// scroll.rs
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Scroll { /* offset, max */ }
impl Scroll {
    pub fn set_extent(&mut self, content: f32, viewport: f32); // max = (content − viewport).max(0); offset clamped
    pub fn wheel(&mut self, vertical: f32) -> bool;            // whether the offset moved; non-finite moves nothing
    pub fn offset(&self) -> f32;
    pub fn can_scroll(&self) -> bool;
}
// tree.rs / paint.rs
impl Tree {
    pub fn slider_at(&self, x: f32, y: f32, now: Instant) -> Option<Key>;
    pub fn paint_except(&mut self, painter: &mut Painter, now: Instant, skip: NodeId);
    pub fn paint_children(&mut self, painter: &mut Painter, now: Instant, parent: NodeId);
}
// theme.rs
pub overshoot: f32, // = celestina_tokens::motion::OVERSHOOT (1.15)
```
OutBack is `f(t) = 1 + (s+1)(t−1)³ + s(t−1)²`. `paint` becomes `paint_except` with no skip, through one scoped walk: `pub(crate) fn walk(&self, scope: Scope) -> Vec<Visit>`.

- [ ] **Write the failing tests first:**
  - `out_back_overshoots_then_lands_on_one`: `eval(0) == 0`, `eval(1) == 1`, `(eval(0.5) − 1.01875).abs() < 1e-4`, `(eval(0.6434) − 1.04874).abs() < 1e-4`, `eval(f32::NAN) == 1`;
  - `a_bezier_easing_is_the_curve_itself`: for each curve of `ALL`, `Easing::from(c).eval(x) == c.eval(x)`;
  - `an_animated_value_carries_an_overshoot_past_its_target`;
  - `the_scroll_offset_stays_within_its_content`;
  - `a_body_that_fits_does_not_scroll`;
  - `a_shrinking_content_pulls_the_offset_back`;
  - `a_non_finite_wheel_moves_nothing`;
  - `slider_at_names_only_a_slider_under_the_point`;
  - `a_scoped_walk_skips_one_subtree_or_keeps_only_its_children`;
  - `the_overshoot_is_the_tokens` (`Theme::dark().overshoot == 1.15`).
- [ ] Run `cargo test -p celestina-ui && cargo test -p celestina-paint theme`. Expected: compile errors or failures naming the new items.
- [ ] Implement. Run them again; all pass. Run `cargo test --workspace --locked` (the `animate_to` change touches every caller). All pass.
- [ ] Commit: `celestina-maintenance: Add the OutBack easing, a scroll offset and scoped tree painting to the toolkit`.

### Task A2: The progressive row blur, the spotlight and a translate — **Model: Opus**

**Files:**
- Create: `crates/celestina-paint/src/soft_edge.rs` (pure), `crates/celestina-paint/src/spotlight.rs` (pure geometry and constants)
- Modify: `crates/celestina-paint/src/backend.rs`, `painter.rs`, `lib.rs`

**Interfaces:**
- **Consumes:** femtovg 0.27 `RenderTarget`, `ImageFilter::GaussianBlur`, `CompositeOperation::{DestinationIn, Lighter}`, `Paint::linear_gradient_stops`, `ImageFlags::FLIP_Y | PREMULTIPLIED`, in the pattern of the spike's diff.
- **Produces:**
```rust
// soft_edge.rs
pub const SOFT_SIGMAS: [f32; 3] = [1.5, 3.0, 6.0];   // logical px; level 0 is the sharp row
pub const SOFT_BAND_MAX: f32 = 96.0;
pub const SOFT_EDGE_ALPHA: f32 = 0.4;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cuts { pub left: bool, pub right: bool }
pub fn band_width(window_w: f32) -> f32;              // min(96, w · 2/5), 0 for w ≤ 0 or NaN
pub fn level_weight(level: usize, t: f32) -> f32;     // max(0, 1 − |3t − level|); t = 1 at the cut edge
pub fn edge_alpha(t: f32) -> f32;                     // 1 − 0.6 t
pub fn level_stops(level: usize, window_w: f32, band: f32, cuts: Cuts) -> Vec<(f32, f32)>; // (offset 0..=1 across the window, alpha)
// spotlight.rs: the preview's `.beam` in logical px
pub const SPOT_WIDTH: f32 = 2.5;                  // × the pill's width, at the foot
pub const CONE_H: f32 = 116.0;                    // top at the pill's bottom edge
pub const CONE_TOP: (f32, f32) = (0.3, 0.7);      // fraction of the foot width
pub const CONE_STOPS: [(f32, f32); 3] = [(0.0, 0.13), (0.42, 0.04), (0.85, 0.0)];
pub const CONE_SIGMA: f32 = 7.0;
pub const CORE_SPAN: (f32, f32) = (0.34, 0.66);   // of the foot width
pub const CORE_H: f32 = 50.0;
pub const CORE_TOP: (f32, f32) = (0.1, 0.9);      // of the core's own width
pub const CORE_STOPS: [(f32, f32); 2] = [(0.0, 0.09), (1.0, 0.0)];
pub const CORE_SIGMA: f32 = 5.0;
pub fn spot_bounds(pill: Rect) -> Rect;            // foot rect inflated by 3 · CONE_SIGMA
// painter.rs
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CutRow { pub image: Rect, pub window: Rect, pub radius: f32, pub cuts: Cuts }
impl Painter {
    pub fn translate(&mut self, dx: f32, dy: f32);   // scoped by push_state/pop_state
    pub fn cut_row(&mut self, row: &CutRow, draw: &mut dyn FnMut(&mut Painter));
    pub fn spotlight(&mut self, pill: Rect, alpha: f32);
}
```
- **`cut_row`'s pipeline:**
  1. Draw `draw` into the source image of `row.image` at `dpr`.
  2. Blur it into three levels at `SOFT_SIGMAS × dpr`.
  3. Mask each of the four levels with `level_stops` (`DestinationIn`).
  4. Sum the four with `Lighter`.
  5. Draw the sum under `push_clip(row.window, row.radius)`.
- **Layers and fallback.** The layer images live in the backend while `(w_px, h_px)` holds; a new size frees and remakes them. If an image cannot be made, `cut_row` falls back to a hard clip with one `tracing::warn!` per painter, never per frame.
- **Uncut sides.** A side without a cut has the sharp level at full alpha all the way to the window's edge on that side.

- [ ] **Write the failing tests first** (pure, no GL):
  - `the_band_is_96_or_two_fifths_of_the_window`: 320 → 96, 200 → 80, 0 → 0, NaN → 0;
  - `the_four_levels_always_sum_to_one`: t in 0..=1 by 0.01;
  - `each_level_peaks_at_its_third`: `level_weight(k, k/3) == 1`;
  - `the_edge_fades_to_forty_percent`: `edge_alpha(1) == 0.4`, `edge_alpha(0) == 1`;
  - `level_stops_on_two_cuts_match_the_spike`;
  - `an_uncut_side_keeps_the_sharp_row_at_full_alpha`: `cuts.left == false` gives level 0 alpha 1 at offset 0, and levels 1 to 3 alpha 0 there;
  - `the_spot_is_two_and_a_half_pills_wide_and_starts_at_the_pills_foot`.
- [ ] Run `cargo test -p celestina-paint soft_edge spotlight`. Expected: failures naming the missing items.
- [ ] Implement the pure modules, then the backend layers and the three `Painter` methods. The GL path has no unit test; it is checked in the nest in A6.
- [ ] Run `cargo test -p celestina-paint` and the clippy pair. All pass.
- [ ] Commit: `celestina-maintenance: Add the progressive row blur, the spotlight and a translate to the painter`.

### Task A3: The sheet's geometry and motion — **Model: Opus**

**Files:**
- Create: `src/sheet/mod.rs` (module declarations only in this task), `src/sheet/geometry.rs`, `src/sheet/motion.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- **Consumes:** A1's `Easing`, `Theme::overshoot` and `Motion`; A2's `Cuts` and `band_width`; `island::state::{TOP_GAP, CAPSULE_H, PAGE_MARGIN}`.
- **Produces:**
```rust
// geometry.rs
pub const SIDE_MARGIN: f32 = 16.0;
pub const ROW_H: f32 = CAPSULE_H;                  // 40
pub fn max_height(surface: (f32, f32)) -> f32;    // surface.1 − TOP_GAP − PAGE_MARGIN (544 on 560)
pub fn target(capsule: Rect, piece: Rect, body: (f32, f32), surface: (f32, f32)) -> Rect;
pub fn cuts(capsule: Rect, sheet: Rect) -> Cuts;  // 0.5 px tolerance; never at a capsule's own end
pub fn row_window(sheet: Rect) -> Rect;           // (x, y, w, 40)
pub fn viewport(sheet: Rect) -> Rect;             // (x, y + 40, w, max(0, h − 40))
pub fn row_image(capsule: Rect, surface: (f32, f32)) -> Rect; // capsule inflated by 3 · 6 = 18, clamped to the surface
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit { Row, Body, Outside }
pub fn hit(sheet: Rect, capsule: Rect, x: f32, y: f32) -> Hit;
// motion.rs
pub struct SheetMotion { /* flight: from rect, start, duration, easing, kind; resize clock; reduced */ }
impl SheetMotion {
    pub fn open(capsule: Rect, theme: &Theme, now: Instant, reduced: bool) -> SheetMotion;
    pub fn switch(&mut self, shown: Rect, now: Instant);
    pub fn resize(&mut self, shown: Rect, now: Instant);    // the body changed size while at rest
    pub fn close(&mut self, shown: Rect, now: Instant);
    pub fn rect(&self, now: Instant, to: Rect, surface: (f32, f32)) -> Rect; // `to` is the live target, or the capsule while closing; the OutBack overshoot is clipped to the surface
    pub fn body_opacity(&self, now: Instant) -> f32;
    pub fn leaving_opacity(&self, now: Instant) -> f32;
    pub fn is_closing(&self) -> bool;
    pub fn closed(&self, now: Instant) -> bool;              // the close flight has ended
    pub fn wants_frame(&self, now: Instant) -> bool;
}
pub struct Indicator { /* pill x and w with their slide; opacity */ }
impl Indicator {
    pub fn show(&mut self, pill: Rect, theme: &Theme, now: Instant, reduced: bool);
    pub fn slide(&mut self, pill: Rect, now: Instant);
    pub fn hide(&mut self, now: Instant);
    pub fn pill(&self, now: Instant) -> Option<(Rect, f32)>;   // rect and pill alpha
    pub fn spot_opacity(&self, now: Instant) -> f32;
    pub fn wants_frame(&self, now: Instant) -> bool;
}
```
- **`target`.** W is `body.0` and c is the piece's centre. The steps:
  1. If `W ≥ capsule.w`, `x = clamp(c − W/2, R − W, L)`; else `x = clamp(c − W/2, L, R − W)`.
  2. Then `x` is clamped to `[16, surface.0 − 16 − W]`.
  3. `y = 8` and `h = min(40 + body.1, max_height(surface))`.

  Every clamp is written as `max(lo).min(hi)` with `hi` lifted to `lo` first, so it never panics.
- **Pinned values:**

  | Clock | Duration and curve |
  |---|---|
  | Open and switch | `Motion::Slow` (350 ms), `Easing::OutBack(theme.overshoot)` |
  | Close | `Motion::Exit` (150 ms), `EXIT` |
  | Resize at rest | `Motion::Normal` (200 ms), `ONE_UI`; during a flight, the flight carries the new target |

  | Body opacity (linear in the flight's time fraction t) | Formula |
  |---|---|
  | Open | `clamp((t − 0.35) / 0.65)` |
  | Close | `clamp(1 − t / 0.6)` |
  | Switch, leaving body | `clamp(1 − t / 0.3)` |
  | Switch, new body | `clamp((t − 0.35) / 0.65)` |

  | Indicator | Value |
  |---|---|
  | Pill fade in | 100 ms linear (`Motion::Fast`) |
  | Pill fade out | 150 ms `EXIT` |
  | Slide | 350 ms OutBack 1.15 |
  | Spotlight | 0 until 140 ms after `show`, then 200 ms (`Motion::Normal`) on `ONE_UI`; out in 120 ms on `EXIT` |

  **Reduced motion:** rects and the slide jump, the opacities still fade, and the spotlight has no delay.

- [ ] **Write the failing tests first,** with the spec's numbers:
  - `a_wider_menu_grows_from_the_capsule_and_keeps_its_row`: capsule (1000, 8, 100, 40), piece centre 1050, body (240, 236), surface (1920, 560) → `(930, 8, 240, 276)`; the capsule lies inside the result;
  - `a_wide_menu_at_the_start_zone_is_held_by_the_margin`: capsule (16, 8, 120, 40), piece centre 40, W 240 → x 16;
  - `a_narrower_menu_is_a_window_over_the_capsule`: capsule (1300, 8, 604, 40), centre 1600, W 320 → x 1440; centre 1880 → x 1584, and its right edge is 1904 = R;
  - `a_tall_body_is_capped_at_544`: body (320, 505) → h 544;
  - `the_target_never_panics_and_keeps_the_margins_when_it_can`: surface (300, 560) with W 420 → x 16; NaN piece → a finite rect;
  - `cuts_appear_only_where_the_window_is_inside_the_capsule`;
  - `hit_tells_row_body_and_outside_apart`;
  - `the_overshoot_never_leaves_the_surface` (Review Focus 1): a 544-high target on a 560 surface; at 175 ms the rect's bottom is ≤ 560;
  - `the_row_image_extends_eighteen_px_past_the_capsule_within_the_surface`;
  - `the_open_flight_starts_at_the_capsule_and_overshoots_its_target` (the rect at 0 ms is the capsule; at 175 ms w is past the target);
  - `the_body_fades_in_over_the_last_65_percent` (at 122.5 ms 0; at 175 ms 0.2308; at 350 ms 1);
  - `closing_fades_the_body_out_over_the_first_60_percent` (at 45 ms 0.5);
  - `a_switch_fades_the_old_body_over_the_first_30_percent` (at 52.5 ms 0.5);
  - `a_resize_at_rest_morphs_on_the_normal_motion`;
  - `reduced_motion_jumps_the_rect_and_still_fades_the_body`;
  - `the_spotlight_waits_140_ms_then_fades_in_on_one_ui` (at 240 ms ≈ `ONE_UI.eval(0.5)` = 0.9047);
  - `the_pill_slides_with_the_open_spring`;
  - `nothing_wants_a_frame_once_every_clock_has_settled`.
- [ ] Run `cargo test -p celestina sheet::`. Expected: failures naming the missing items.
- [ ] Implement. Run it again; all pass. Run the clippy pair.
- [ ] Commit: `celestina-maintenance: Add the capsule sheet's geometry and motion`.

### Task A4: The owner, the Island's wiring and the volume body — **Model: Opus**

**Files:**
- Modify: `src/sheet/mod.rs`
- Create: `src/sheet/body.rs`, `src/island/sheets.rs`
- Modify: `src/island/mod.rs`, `src/island/bar_tree.rs`, `src/bar/silhouette.rs` (only if a signature needs it), `src/osd/popup.rs`, `src/pieces/mod.rs`, `src/pieces/volume.rs`

**Interfaces:**
- **Consumes:** A1, A2 and A3; `LevelCore`.
- **Produces:**
```rust
// sheet/mod.rs
pub type Measure<'m> = dyn FnMut(&str, &TextStyle, Option<f32>) -> (f32, f32) + 'm;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor { pub capsule: Rect, pub piece: Rect, pub surface: (f32, f32) }
pub struct Request { pub zone: usize, pub piece: Key, pub kind: SheetKind, pub body: SheetBody }
pub enum Next { Sheet(Request), Page(PageId) }
pub struct Sheets { /* open: Option<OpenSheet>, leaving: Option<SheetBody>, queued: Option<Next>, scroll, indicator */ }
impl Sheets {
    pub fn is_open(&self) -> bool;                    // open, not closing
    pub fn is_shown(&self, now: Instant) -> bool;     // open or closing
    pub fn zone(&self, now: Instant) -> Option<usize>;
    pub fn piece(&self) -> Option<Key>;
    pub fn kind(&self) -> Option<SheetKind>;          // the open one, else the queued one
    pub fn rect(&self, anchor: &Anchor, now: Instant) -> Option<Rect>;
    pub fn open(&mut self, request: Request, anchor: &Anchor, theme: &Theme, now: Instant, reduced: bool);
    pub fn close(&mut self, anchor: &Anchor, now: Instant);
    pub fn queue(&mut self, next: Next);
    pub fn drop_queue(&mut self);
    pub fn take_due(&mut self, now: Instant, island_closed: bool) -> Option<Next>;
    pub fn input(&mut self, ev: &InputEvent, anchor: &Anchor, now: Instant, measure: &mut Measure<'_>) -> BodyOutcome;
    pub fn sync(&mut self, theme: &Theme, now: Instant, measure: &mut Measure<'_>) -> bool;
    pub fn layout(&mut self, lay: &mut dyn FnMut(&mut Tree, (f32, f32)));
    pub fn paint(&mut self, painter: &mut Painter, anchor: &Anchor, now: Instant, row: &mut dyn FnMut(&mut Painter));
    pub fn wants_frame(&self, now: Instant) -> bool;
    pub fn wait(&self, now: Instant) -> Option<Duration>; // throttle and asked-level holds
}
// sheet/body.rs
pub enum SheetBody { Level(Box<LevelCore>) }
pub enum BodyOutcome { Keep { changed: bool }, Close }
impl SheetBody {
    pub fn size(&self) -> (f32, f32);
    pub fn takes_wheel(&self, x: f32, y: f32, now: Instant) -> bool;
    pub fn flush(&mut self, now: Instant);            // force: what waits goes out
}
// island/sheets.rs (impl Island)
pub fn open_level_sheet(&mut self, piece: Key, core: LevelCore, ctl: &mut SurfaceCtl<'_>) -> bool;
pub fn level_sheet_open(&self) -> bool;               // a Level body open or queued
pub fn close_level_sheet(&mut self, ctl: &mut SurfaceCtl<'_>);
pub fn anything_open(&self) -> bool;
// bar_tree.rs
pub fn zone_of(&self, key: Key) -> Option<usize>;     // the zone whose rect holds the node's centre
pub fn zone_rect(&self, zone: usize) -> Option<Rect>;
pub fn paint(&mut self, painter: &mut Painter, now: Instant, skip: Option<usize>);
pub fn paint_row(&mut self, painter: &mut Painter, now: Instant, zone: usize);
pub fn shapes(&self, now: Instant, radius: f32, skip: Option<usize>) -> Vec<Shape>;
// island/mod.rs
IslandAction::OpenLevelSheet { kind: level_popup::Kind, piece: Key }
// osd/popup.rs: decision 6's CardFrame, SHEET_W, body_size, and the new/input/sync signatures
```
- **Input.**
  - `Island::dispatch` gains `measure: &mut Measure<'_>`. `Island::input` builds the measure from the painter in hand; the tests' `feed` passes the fake measure of `lay_fake`.
  - The sheet branch runs first, before `on_bar`.
  - After a dispatch, a `Sheets::wait` arms `ctl.redraw_after`, as `LevelPopup::input` did.
- **Frame.** `Island::frame`:
  1. runs `take_due` (the queued open);
  2. runs `sync` with the frame's painter as the measure;
  3. closes the sheet when its anchor is gone (decision 15);
  4. requests a frame when `Sheets::wants_frame`;
  5. arms `Sheets::wait`.
- **Page invariant.** `IslandState` stays the page's own; `Island::open(page)` and `close` go through the one-open rule of decision 2.

- [ ] **Write the failing tests first** (`island/mod.rs` tests use `bar_island(1920.0)` with a volume reading and a LevelCore built by a test helper `volume_core(theme)`, with cells, an empty graph, `MonitorFeed::default()` and a no-op sink):
  - `a_volume_click_asks_for_its_sheet_at_its_node`: `[IslandAction::OpenLevelSheet { kind: Volume, piece: Key::named("volume") }]`;
  - `the_sheet_grows_from_the_end_capsule_and_settles_on_its_target`: the rect at t0 is the end capsule's rect; at t0 + 350 ms it is `geometry::target(..)`;
  - `a_narrow_sheet_keeps_the_row_where_it_is` (every bar node rect unchanged);
  - `a_second_piece_of_the_same_capsule_switches_with_one_morph`: `open_level_sheet` on the brightness node while the volume is open; `is_open` throughout; the old body is `leaving`; no close in between;
  - `pressing_the_open_piece_again_closes_its_sheet`;
  - `a_piece_without_a_sheet_acts_and_closes_it_and_the_wheel_never_does`;
  - `a_press_outside_escape_and_focus_loss_close_the_sheet`;
  - `the_sheet_paints_over_and_takes_the_input_of_what_it_covers`: a press inside the sheet over the Island capsule opens no page;
  - `the_island_capsule_while_a_sheet_is_open_closes_it_then_opens_the_page`: at +0 the sheet is closing and the page is closed; at +150 ms the page is opening;
  - `a_volume_click_while_a_page_is_open_closes_the_page_then_opens_the_sheet`;
  - `island_close_and_toggle_close_a_sheet_and_open_goes_in_sequence`;
  - `never_two_things_open_at_once_through_any_sequence` (Review Focus 2): a scripted mix of presses, Escape and verbs at 0, 40, 90 and 160 ms steps. After every step, `!(island.state.is_open() && island.sheets.is_open())`, and never both painted with nonzero area;
  - `the_regions_and_keyboard_follow_the_sheet`: the blur bands cover the sheet rect and no band covers the end zone outside it; the input region is the whole surface while shown; `OnDemand`; back to today's at rest after the close;
  - `a_tall_body_scrolls_and_a_wheel_over_a_slider_moves_the_slider`;
  - `an_open_sheet_at_rest_asks_for_no_frame`: cut row, t0 + 1 s, `wants_frame == false` and `wait == None`;
  - `a_sheet_whose_piece_leaves_the_bar_closes` (Review Focus 5);
  - `a_capsule_that_changes_width_while_open_carries_the_sheet` (Review Focus 5);
  - `a_fold_inside_island_input_cuts_with_the_measure_in_hand` (Review Focus 4): a measure that counts its calls; folding the Output section through `feed` calls it; `LevelCore` has no measuring field;
  - `closing_a_level_sheet_sends_what_the_throttle_holds`;
  - `the_volume_body_is_320_wide_and_as_tall_as_its_rows` and `the_tallest_volume_body_scrolls_inside_a_544_sheet` (`osd/popup.rs`).
- [ ] Run `cargo test -p celestina island:: sheet:: osd::`. Expected: failures naming the missing items.
- [ ] Implement. Port the `osd/popup.rs` rig helpers to the new signatures (decision 16). Every CEL-I-003/004/007/008/009 and CEL-D-006 test still passes unchanged in its assertions.
- [ ] Run `cargo test --workspace --locked`, the clippy pair and the three guards. All pass.
- [ ] Commit: `celestina-maintenance: Add the capsule sheet owner to the Island with the volume as its first body`.

### Task A5: The app and the level controls — **Model: Sonnet**

**Files:**
- Modify: `src/osd/control.rs`, `src/app.rs`, `src/message.rs`

**Interfaces:**
- **Consumes:** `Island::{open_level_sheet, level_sheet_open, close_level_sheet}`, `IslandAction::OpenLevelSheet` and `LevelCore::new(.., CardFrame::Sheet, .., fit)`.
- **Produces:**
```rust
pub enum Place { Sheet(Key), Popup(Rect) }          // Popup: brightness until B1
pub struct Released { pub suppression: Suppression, pub elsewhere: Option<SurfaceId> }
pub struct Opened { pub opened: bool, pub released: Option<Released> }
pub enum LevelRedraw { Island(SurfaceId), Popup(PopupId) }
impl Controls {
    pub fn open(&mut self, kind: Kind, place: Place, island: &mut Island, ctl: &mut SurfaceCtl<'_>, with: &Opening<'_>) -> Opened;
    pub fn settle(&mut self, surface: SurfaceId, level_open: bool) -> Option<Suppression>;
    pub fn refresh(&self) -> Option<LevelRedraw>;
}
// message.rs
Message::CloseLevelSheet(SurfaceId)
```
- **`Controls::open`:**
  - A held level menu is released first:
    - on another surface it gives `elsewhere: Some(it)`;
    - on this surface, when the new one is a popup, `island.close_level_sheet(ctl)`.
  - It builds the `LevelCore` with a fit that borrows the painter for the duration of `new` only; the app's `act` runs after `Island::input` has given the painter back.
  - It asks the fresh reading as today.
- **The app:**
  - posts `OsdSuppressed(released)` first, then `OsdSuppressed(Suppression::Open(kinds))`;
  - posts `CloseLevelSheet(elsewhere)`;
  - calls `settle` after `island.input` and after `island.frame`, posting what it returns;
  - in `levels_settings_changed` (duty off), calls `close_level_sheet` then `closed()`;
  - makes `popup_refreshed` redraw the Island's surface for `LevelRedraw::Island`.
- The volume's `LevelPopup` path is gone. `LevelPopup` remains for brightness only.

- [ ] **Write the failing tests first** (`osd/control.rs`, without an Island; `Open` is recorded as the tests' `opened` helper does today):
  - `a_volume_sheet_quiets_its_kinds_and_releases_them_once`;
  - `every_way_a_level_sheet_closes_releases_the_display_once` (Review Focus 3): `settle(s, false)` gives `Closed` once and `None` after. Opening on another surface releases with `elsewhere`. `surfaces_gone`, and `closed` from the duty off, each release once. A switch on the same surface releases the old kinds and quiets the new;
  - `opening_a_level_sheet_on_another_output_closes_the_first`;
  - `a_new_graph_redraws_the_island_of_an_open_volume_sheet` (`LevelRedraw::Island(surface)`);
  - `no_graph_message_reaches_the_osd` keeps passing.
- [ ] Run `cargo test -p celestina osd::control`. Expected: failures naming the missing items.
- [ ] Implement. Run `cargo test --workspace --locked`, the clippy pair and the guards. All pass.
- [ ] Commit: `celestina-maintenance: Replace the volume popup with the volume sheet in the app and the level controls`.

### Task A6: Nested check, evidence and documents — **Model: Sonnet**

**Files:**
- Create: `celestina/docs/evidence/YYYY-MM-DD-cel-d-002-capsule-sheets.md` and its directory, with `preview/` for the approved preview's frames, which the controller provides
- Modify: `celestina/VALIDATION.md` (`VAL-CEL-D-002`), the `CEL-D-002` ledger row
- Modify: the "In progress" lines of the CEL-TRACKS sections of `celestina/STATUS.md` and `celestina/ROADMAP.md`, naming `CEL-D-002` to `CEL-D-005` with the design and plan links

- [ ] Build a debug binary in a private target dir. Run `timeout -k 60 2700 sh celestina/scripts/smoke.sh --binary <tgt>/debug/celestina --only bar,osd`; it must print `smoke: OK`.
- [ ] In the capture kit, with the author's layout (`cfg/`), take these shots:
  1. the volume sheet open (narrow: cut on both sides);
  2. a zoom of a cut edge (no seam, no halo, nothing dimmed where the window meets the capsule's end);
  3. the spotlight under the volume piece;
  4. a frame about 175 ms into the open (the overshoot);
  5. the body scrolled with every section unfolded;
  6. the bar at rest after closing (as before).

  Record the rest-frame counts the smoke printed.
- [ ] **Write the evidence:** scope, the preview approval (2026-11-25, version 7) with its frames, commands and exits, the summed test count, the shots, and the limits (no real monitor or fractional scale measured; GPU cost not measured, since the spike's timer is not carried over). Add a Follow-up with the backlog idea from the author: tint hover and press with the wallpaper's colours (a later `CEL-D` entry). Record that same idea as a line of the ledger's Backlog.
- [ ] **The ledger row.** Status `active`, Files/areas `src/sheet/`, `src/island/`, `src/osd/`, `crates/celestina-ui/`, `crates/celestina-paint/`, `src/app.rs`. Intended change: "Add the capsule sheet: a bar capsule grows into the menu of the piece pressed (one open thing per output, OutBack morph, switching between the pieces of a capsule, the progressive blur where the sheet cuts the row, a spotlight under the open piece, scrolling), with the volume menu as its first content and its popup removed ([design], [plan]; preview approved 2026-11-25)". Add the evidence link and `VAL-CEL-D-002`.
- [ ] **`VAL-CEL-D-002`:**
  - open the volume menu: the end capsule grows into it with a small bounce and the row stays put;
  - cut icons blur and fade toward the edge, and a cut icon still opens its menu;
  - the spotlight falls from the volume piece;
  - unfold every section and scroll;
  - the wheel over a slider moves the slider;
  - Escape and a click outside close it;
  - dragging a slider shows no OSD;
  - opening it on a second output closes the first.
- [ ] Once `CEL-TRACKS-DOCS-3` has landed, `git merge origin/main` and turn the row's code-span paths of the design and plan into links (until then they stay code spans: the suite documents are not on the branch).
- [ ] Run the three guards. Commit: `celestina-maintenance: Add the CEL-D-002 evidence, validation row and ledger entry`.

---

## Part B — `CEL-D-003`: every piece's menu becomes a sheet

**Worktree:** `sh scripts/worktree.sh open celestina CEL-D-003 --from unit/celestina/CEL-D-002` (stacked: B rewrites A's code). If `CEL-D-002` has landed by then, open it from `main` instead.
**Smoke phases for its landing:** `bar,osd,glance,connectivity,energy,clipboard,media,phone,notify`.

### Decisions (controller's calls, final)

1. **Kinds.**
   - `SheetKind` gains `Brightness`, `Session` and `Page(PageId)`.
   - The Brightness piece's Primary gives `Sheet(Brightness)`; `BrightnessAction::Popup` is removed.
   - The Session piece's Primary gives `Sheet(Session)`; `PieceAction::SessionMenu` is removed.
   - A bar piece's `PieceAction::OpenPage(page)` is read by the Island as `Sheet(Page(page))`. The pieces do not change.
   - The Tray piece is unchanged and opens no sheet.
2. **Level path.**
   - `Controls::open(kind, piece: Key, ..) -> Opened` loses `Place`.
   - Remove:
     - in `osd/popup.rs`: `LevelPopup`, its `PopupShell` impl, `placement`, `POPUP_W` (a brightness body is 320), `CardFrame::Popup` (so `CardFrame` goes), `resize_wanted` and `popup_size`;
     - in `osd/control.rs`: `LevelRedraw::Popup` (`refresh` returns `Option<SurfaceId>`);
     - in `island/mod.rs`: `IslandAction::OpenLevelPopup`, the field `level_popup`, `open_level_popup`, `level_popup_open` and `close_level_popup`.
   - `monitors_card` and `monitors_size` lose their haze and fixed card: width 320, the level padding of A's decision 5.
3. **Session.**
   - `SessionCore` loses its haze and its stored `fit`: `new(theme, reduced, now, fit: &mut dyn FnMut(&str, f32, f32) -> f32)` and `input(ev, now, fit)`.
   - Width 240 (`MENU_W`), the session padding of A's decision 5.
   - `SheetBody::Session(SessionCore)`; `BodyOutcome::Session(SessionAction)`.
   - The Island builds the session body itself in `Island::input`, where the measure is in hand: no app round trip.
   - A choice gives `IslandAction::Session(SessionAction)` and closes the sheet. The app posts `Message::Command(action.verb())` at once: no popup grab exists any more, and the lock surface maps over a closing sheet.
   - Remove: `SessionMenu` and its `PopupShell` impl, `SessionSlot`, the app's `session_choice` and its handling at `PopupClosed` and at output removal, `IslandAction::OpenSessionMenu`, the Island's `session_menu` field, `open_session_menu` and `session_menu_open`.
4. **Page-backed sheets: the Island page and a page sheet do not share a container.**
   - `SheetBody::Page(PageBody)` with `PageBody { page: PageId, tree: Tree, input: Interaction }`. Its view, built in `src/island/sheets.rs` (where `view::button` is visible), is a col (width 420, the page padding):
     - a header row of 32: the title (`type_scale.title`, Semibold, TextHi), growing, then the overflow button, `button(theme, sheet_more_key(), "sheet-more-label", copy::MORE)` with `sheet_more_key() = Key::named("sheet-more")`;
     - a gap of `spacing.sm`;
     - the page's `PageView::view(&cx)` in a clipped col of 388 × `content_h`.

     It has no clock, no "Cerrar" and no tabs.
   - **Size.** `content_h = clamp(page.wants(&cx)).1 − chrome_height(theme)`, with the same `PageCx` as the Island page (`max_size = state.max_page_size()`), so the `action_for` helpers find the same keys. The body box is `(420, xs + 32 + sm + content_h + lg)`.
   - **One `PageCx`.** `Island::page_cx(&self) -> PageCx<'_>` is extracted from `apply_views`. `apply_views` rebuilds a shown page sheet's body when `stale`; `frame`'s `seen` marks stale while a page sheet is shown.
   - **The overflow button is shown on every page sheet:** today every page has the same overflow menu. Its click gives `IslandAction::OpenMenu { more, keyboard }`, with `more` the button's rect in surface coordinates.
     - Its `ReopenPage` row reopens whatever is open: a sheet closes at once and grows again from its capsule.
     - A compositor dismissal of the menu closes the sheet, as it closes the page today.
     - The sheet closing closes the menu (`close_page_menus`).
   - **Actions.** `Island::activate` splits into `activate_chrome(key)` (the capsule, the close button, the overflow button, the tabs) and `page_action(&mut self, page: PageId, key: Key, now: Instant) -> Option<IslandAction>`. `page_action` holds everything page-specific (clipboard, month step, the System monitor's open, connectivity, media, notifications, phone, activity). `connectivity_action(page, key)` takes the page instead of reading `state`. A page sheet's `BodyOutcome::Activate { key, keyboard }` goes to `page_action(sheet_page, key)`.
   - **The shown page.** `Island::page_shown(&self) -> Option<PageId>` is the Island page or the page sheet. These ask it instead of `state`: `sysmon_page_open`, `shows_clipboard_page`, `shows_notifications`, `delete_key`, the clipboard confirmation and `pending_focus`. `Island::shown_page_input(&mut self) -> Option<(&mut Tree, &mut Interaction)>` gives the matching tree. `Island::page()` keeps meaning the Island page (the notifications toggle verb reads it).
   - The phone duty going off closes a phone sheet.
   - The Island keeps its pages, its tab row, `island open` and the notification paths.
5. **Tests that move or change:**
   - Removed with their code: `the_level_popup_is_one_of_the_islands_popups` and `the_session_menu_is_one_of_the_islands_popups`.
   - **`osd/popup.rs` tests of the removed popup API.** Every test that asserts `resize_wanted()` asserts instead the change of `size()` (the new body size, or "unchanged" where it asserted `None`); `popup_size`/`POPUP_W` become `body_size`/`SHEET_W` (320, the brightness single body included); `the_card_fills_the_popup_in_every_shape` becomes `the_card_fills_the_body_in_every_shape`. Their other assertions, CEL-I-004's folds included, stay.
   - `the_session_piece_asks_for_its_menu_anchored_at_its_node` becomes `the_session_piece_opens_its_sheet`.
   - `a_dangerous_row_asks_first_and_grows_keeping_its_top` is renamed `a_dangerous_row_asks_first_and_the_sheet_grows_down` and asserts `size()` and the sheet's y 8 instead of `resize_wanted` and the placement.
   - `a_bar_piece_while_open_switches_the_page_or_acts_and_closes_it` and `the_glance_pieces_read_the_bars_slices_and_a_click_opens_their_pages` expect page sheets.
   - The popup tests that build `LevelCore` keep their assertions, with the brightness at 320.

### Task B1: Brightness moves into a sheet; the level popup path goes — **Model: Sonnet**

**Files:** `src/osd/popup.rs`, `src/osd/monitor_rows.rs`, `src/osd/control.rs`, `src/pieces/{mod.rs, brightness.rs}`, `src/island/{mod.rs, sheets.rs}`, `src/sheet/body.rs`, `src/app.rs`; the `CEL-D-003` ledger row set to `active`.

**Interfaces — Produces:** `Controls::open(&mut self, kind: Kind, piece: Key, island: &mut Island, ctl: &mut SurfaceCtl<'_>, with: &Opening<'_>) -> Opened`; `Controls::refresh(&self) -> Option<SurfaceId>`; `SheetKind::Brightness`.

- [ ] **Write the failing tests first:**
  - `the_brightness_piece_opens_its_sheet`;
  - `the_brightness_body_is_320_wide_single_or_listed`;
  - `switching_volume_to_brightness_morphs_and_swaps_the_quieted_kinds`: `Opened.released` is the volume's `Closed`, the brightness `Open` follows, the Island switched without closing.

  Every existing CEL-I-003/007/008/009 test keeps its assertions.
- [ ] Run `cargo test -p celestina osd:: island::`. Expected: failures.
- [ ] Implement and remove decision 2's items. Run the workspace tests, the clippy pair and the guards. All pass.
- [ ] Commit: `celestina-maintenance: Replace the brightness popup with its sheet and remove the level popup path`.

### Task B2: The session menu moves into a sheet — **Model: Sonnet**

**Files:** `src/session_menu/{mod.rs, view.rs}`, `src/sheet/body.rs`, `src/island/{mod.rs, sheets.rs}`, `src/pieces/{mod.rs, session.rs}`, `src/app.rs`.

**Interfaces — Produces:** `SheetKind::Session`; `SheetBody::Session(SessionCore)`; `BodyOutcome::Session(SessionAction)`; `IslandAction::Session(SessionAction)`.

- [ ] **Write the failing tests first:**
  - `the_session_piece_opens_its_sheet`;
  - `a_session_choice_is_one_action_and_the_sheet_closes`: Lock gives `[IslandAction::Session(SessionAction::Lock)]` and the sheet is closing;
  - `a_dangerous_row_asks_first_and_the_sheet_grows_down` (the renamed test): the body height grows and the sheet keeps y 8 on the resize motion; Cancel shrinks it back;
  - `the_confirm_card_measures_its_question_with_the_measure_in_hand` (Review Focus 4): a counting measure through `feed`; `SessionCore` has no measuring field;
  - `escape_in_the_confirm_card_cancels_and_escape_in_the_list_closes_the_sheet`.

  The kept session tests (`the_rows_are_lock_suspend_divider_log_out_reboot_power_off_in_order`, `each_action_is_the_verb_its_bus_word_names`, `the_keyboard_moves_through_the_rows_and_enter_chooses`) keep their assertions.
- [ ] Run `cargo test -p celestina session_menu:: island::`. Expected: failures.
- [ ] Implement and remove decision 3's items. Run the workspace tests, the clippy pair and the guards. All pass.
- [ ] Commit: `celestina-maintenance: Replace the session popup with its sheet and post the chosen verb at once`.

### Task B3: Page-backed sheets — **Model: Opus**

**Files:** `src/sheet/body.rs`, `src/island/{mod.rs, sheets.rs, view.rs}`, `src/island/pages/mod.rs` (only if `page_cx` needs a helper there), `src/app.rs` (only if `anything_open` or `shows_notifications` needs it).

**Interfaces:**
- **Consumes:** `PageView`, `PageCx`, `chrome_height`, `button`, `menu::placement`.
- **Produces:** `SheetKind::Page(PageId)`; `SheetBody::Page(PageBody)`; `BodyOutcome::Activate { key: Key, keyboard: bool }` and `BodyOutcome::More { rect: Rect, keyboard: bool }`; `Island::{page_cx, page_action, page_shown, shown_page_input}` (decision 4).

- [ ] **Write the failing tests first:**
  - `a_bar_piece_with_a_page_opens_its_page_as_a_sheet`: for each of network, bluetooth, notifications, clipboard, power, sysmon, media, phone and clock, the sheet is 420 wide, with no tab, clock or Cerrar node, and the title is the page's;
  - `the_page_sheets_body_is_the_islands_page_content` (the same structure as `page.view(&cx)`);
  - `a_row_of_a_page_sheet_asks_for_its_typed_action` (network and bluetooth rows by click and Enter, as on the page);
  - `the_more_button_of_a_sheet_asks_for_the_menu_below_itself_in_surface_coordinates`;
  - `a_dismissed_menu_closes_the_sheet_and_reopen_grows_it_again`;
  - `the_clipboard_sheet_takes_delete_and_the_clear_confirmation_like_the_page`;
  - `the_notifications_sheet_counts_as_viewed`;
  - `the_system_sheet_takes_every_sample` (the `sysmon_page_open` cadence);
  - `island_open_still_opens_the_island_page_with_its_tabs`;
  - `the_phone_sheet_closes_when_the_duty_goes_off`.
- [ ] Run `cargo test -p celestina island::`. Expected: failures.
- [ ] Implement. Run the workspace tests, the clippy pair and the guards. All pass.
- [ ] Commit: `celestina-maintenance: Add page-backed sheets for every piece that opened an Island page`.

### Task B4: Nested check, evidence and documents — **Model: Sonnet**

**Files:** `celestina/docs/evidence/YYYY-MM-DD-cel-d-003-menus-as-sheets.md` and its directory; `VAL-CEL-D-003`; the `CEL-D-003` ledger row.

- [ ] Run the smoke with `--only bar,osd,glance,connectivity,energy,clipboard,media,phone,notify`; it must print `smoke: OK`.
- [ ] Take nested shots:
  - brightness with three fake monitors if the kit allows, otherwise one;
  - the session list and its confirm card;
  - the System sheet (narrow);
  - the Notifications sheet from the start zone (the wide case: the capsule grows and its row stays);
  - frames of a volume → brightness → session → System tour (the preview's tour button).
- [ ] **Evidence:** as A6.
- [ ] **The ledger row.** Intended change: "Replace the brightness and session popups and the Island pages the bar's pieces opened with capsule sheets; the level and session `xdg_popup` paths are removed, and the Island keeps its pages for `island open`, its tabs and notifications ([design], [plan])".
- [ ] **`VAL-CEL-D-003`:**
  - each piece opens its own sheet;
  - switching between two pieces of one capsule is one motion;
  - Lock locks and the confirm card still asks;
  - the overflow button opens its menu;
  - a click on another capsule closes, then opens.
- [ ] Commit: `celestina-maintenance: Add the CEL-D-003 evidence, validation row and ledger entry`.

---

## Part C — `CEL-D-004`: one centre capsule with time, date and weather

**Worktree:** `sh scripts/worktree.sh open celestina CEL-D-004` from `main` once `CEL-D-003` has landed. If it has not, stack it with `--from unit/celestina/CEL-D-003`.
**Smoke phases for its landing:** `bar,glance`.

### Decisions (controller's calls, final)

1. **The capsule view.** `view::capsule_with(theme, clock, glance: Glance<'_>, rows, surface_w)`, with `pub struct Glance<'a> { pub date: Option<&'a str>, pub weather: Option<(Option<&'static str>, &'a str)> }`. Its children, gap `spacing.md` (12), in this order:
   1. the time (`type_scale.row_title`, Semibold, tabular, TextHi);
   2. the date `copy::short_date(..)` (`type_scale.body`, Regular, TextLo);
   3. the weather row (gap `spacing.xs`): `weather::condition_icon(..)` at 16 tinted TextLo, when the condition has one, and `copy::bar_weather(..)` (body, TextLo, tabular);
   4. the workspace dots, only as today (`capsule_shows_workspaces`).

   No date leaves the date out; no reading leaves the weather out.
2. **Rebuild.** A change of `today` or `weather` sets `stale`, so the capsule rebuilds. The rare change costs one frame, never a frame at rest.
3. **Click.** `activate_chrome(capsule_key())` opens `PageId::CLOCK`, through the one-open rule. `island toggle` still reopens the last page.
4. **Layout.** `BarLayout::default().centre` is already empty on `main`, so `settings.rs` does not change. Add the assertion that neither `clock` nor `weather` is in a default zone to `the_bar_defaults_are_todays_layout`. The evidence gives the author's line to change in `~/.config/celestina/shell.json`: `"centre": []`.
5. **Test that changes:** `rows_activate_focus_requests_and_the_capsule_opens_the_last_page` becomes `rows_activate_focus_requests_and_the_capsule_opens_the_clock_page`. `island/view.rs` capsule tests gain the date and weather nodes.

### Task C1: The capsule and its click — **Model: Sonnet**

**Files:** `src/island/{view.rs, mod.rs}`, `celestina-rs/crates/celestina-shell/src/settings.rs` (test only); the `CEL-D-004` ledger row set to `active`.

- [ ] **Write the failing tests first:**
  - `the_capsule_reads_time_date_and_weather_in_that_order`: node x order clock < date < weather; gap 12; the weather icon is 16; text sizes 15/13/13;
  - `without_weather_the_capsule_shows_time_and_date`;
  - `a_clear_night_shows_the_degrees_without_an_icon`;
  - `the_capsule_keeps_its_dots_only_without_a_workspaces_zone`;
  - `a_click_on_the_capsule_opens_the_clock_page`;
  - `a_new_day_or_reading_rebuilds_the_capsule_once`;
  - `the_bar_defaults_place_neither_clock_nor_weather`.
- [ ] Run `cargo test -p celestina island::` and `cargo test -p celestina-shell settings` (from `celestina/`). Expected: failures.
- [ ] Implement. Run the workspace tests, the clippy pair and the guards. All pass.
- [ ] Commit: `celestina-maintenance: Add the date and the weather to the Island capsule and open the clock page from it`.

### Task C2: Nested check, evidence and documents — **Model: Haiku**

- [ ] Run the smoke with `--only bar,glance`; it must print `smoke: OK`.
- [ ] Take nested shots of the capsule with a weather reading, made by putting the glance phase's fake `curl` body in the kit and a location in `cfg/`, and of the clock page after a click.
- [ ] Write `celestina/docs/evidence/YYYY-MM-DD-cel-d-004-centre-capsule.md`, including the author's `"centre": []` line.
- [ ] **The ledger row.** Intended change: "Add the date and the weather to the Island capsule beside the time; a click on it opens the clock page, and the clock and weather pieces leave the default centre ([design], [plan])".
- [ ] **`VAL-CEL-D-004`:** after setting `"centre": []`, the centre reads `HH:MM`, the date and the weather; a click opens the calendar.
- [ ] Commit: `celestina-maintenance: Add the CEL-D-004 evidence, validation row and ledger entry`.

---

## Part D — `CEL-D-005`: the workspaces list

**Worktree:** `sh scripts/worktree.sh open celestina CEL-D-005` from `main` once `CEL-D-004` has landed. If it has not, stack it with `--from unit/celestina/CEL-D-004`.
**Smoke:** the last unit of the series lands with the full smoke.

### Decisions (controller's calls, final)

1. **Roles and copy.**
   - Add `Role::AccentSoft` and `Role::AccentSoftBorder` to `celestina-paint` (`Theme.accent_soft` and `Theme.accent_soft_border`, from `Scheme`), to `ALL` and to the per-role tests.
   - Copy: rename the overview's existing `copy::OVERVIEW_WORKSPACE_EMPTY` to `copy::WORKSPACE_EMPTY` (its one overview use follows), and add `copy::more_windows(n: usize) -> String` (`+N`), pinned by the copy test.
2. **The piece.**
   - One hit target: root `Key::named("workspaces")`, padding 5, radius `radius_md`, interactive and focusable. The dots become plain children, not interactive (the dot sizes are unchanged).
   - Primary gives `PieceAction::Sheet(SheetKind::Workspaces)`.
   - New behaviour (today the piece ignores the wheel): a vertical `Gesture::Scroll` of n steps gives `FocusWorkspace(id)` of the workspace n steps away in niri's order, clamped, with no wrap (the preview's rule). A step that would not move gives `PieceAction::None`. This works open or not, and the wheel never closes a sheet.
   - `Workspaces::label` names the piece with the current workspace.
   - The piece keeps its Activity page.
3. **The body.** `SheetBody::Workspaces(WorkspacesBody)`, width 240, the session padding, gap 2, built by the Island from `rows_for(model, output)` and the model's windows. One row per workspace, `Key::indexed("ws-row", id)`:
   - **Row:** height 40, padding x 10, gap 10, radius `radius_button`, interactive and focusable.
   - **Label:** 18 wide for a number; a named workspace keeps its natural width. `type_scale.body`, Semibold, TextLo; Accent on the current row.
   - **Icons:** the windows' icons at 20, gap 6, ordered by `(column, row)` with floating windows last, then by id. Each is `IconKey::desktop_icon` or the `app-window` fallback tinted TextLo, as the Activity rows do. At most 8 icons, then `+N` (body, TextLo). A workspace with no window shows `copy::WORKSPACE_EMPTY` (body, TextFaint).
   - **Current row:** `AccentSoft` fill with a `View::stroke(AccentSoftBorder)`.
   - **Hover and press:** as today. The coloured fill is kept under hover, CEL-I-006's rule.
4. **Acting.** A click or Enter on a row gives `BodyOutcome::FocusWorkspace(id)`, then `IslandAction::FocusWorkspace(id)`, and closes the sheet. A model change while open rebuilds the body; the height follows on the resize motion.
5. **Tests that change:** `a_primary_input_on_a_dot_focuses_its_workspace` becomes `a_primary_input_anywhere_opens_the_workspaces_sheet`. `the_dots_are_this_outputs_workspaces_with_the_active_one_a_pill` keeps its marks, with no interactive dot.

### Task D1: Roles, copy and the list body — **Model: Sonnet**

**Files:** `crates/celestina-paint/src/theme.rs`, `src/copy.rs`, `src/sheet/body.rs` (a `workspaces` part, or `src/sheet/workspaces.rs` if `body.rs` passes about 600 lines); the `CEL-D-005` ledger row set to `active`.

**Interfaces — Produces:** `Role::{AccentSoft, AccentSoftBorder}`; `pub fn workspaces_view(theme: &Theme, rows: &[Row], windows: &[Window]) -> View`; `pub fn window_order(windows: &[Window], workspace: u64) -> Vec<&Window>`.

- [ ] **Write the failing tests first:**
  - `the_accent_soft_roles_are_the_tokens`;
  - `vacio_is_the_empty_workspaces_copy`;
  - `one_row_per_workspace_in_niris_order_40_tall`;
  - `icons_follow_the_column_order_and_floating_windows_come_last`;
  - `more_than_eight_windows_show_eight_icons_and_plus_n`;
  - `an_empty_workspace_says_vacio_in_the_faint_role`;
  - `the_current_row_has_the_accent_soft_fill_border_and_label`.
- [ ] Run `cargo test -p celestina-paint theme && cargo test -p celestina sheet:: copy`. Expected: failures.
- [ ] Implement. Run them again; all pass.
- [ ] Commit: `celestina-maintenance: Add the workspaces list body and the accent soft roles`.

### Task D2: The piece, the sheet and the wheel — **Model: Sonnet**

**Files:** `src/pieces/{mod.rs, workspaces.rs}`, `src/sheet/body.rs`, `src/island/{mod.rs, sheets.rs}`.

**Interfaces — Produces:** `SheetKind::Workspaces`; `SheetBody::Workspaces(WorkspacesBody)`; `BodyOutcome::FocusWorkspace(u64)`; `pub fn step(rows: &[Row], steps: i32) -> Option<u64>` in `pieces/workspaces.rs`.

- [ ] **Write the failing tests first:**
  - `a_primary_input_anywhere_opens_the_workspaces_sheet`;
  - `the_piece_is_one_hit_target_with_one_pill`;
  - `the_wheel_steps_to_the_neighbour_and_stops_at_the_ends`: active 1 of 6, +1 → 2, −1 → None, +9 → 6;
  - `the_wheel_steps_with_the_sheet_open_and_keeps_it_open`;
  - `a_row_click_focuses_its_workspace_and_closes_the_sheet`;
  - `the_workspaces_sheet_is_wider_than_the_start_capsule_so_it_grows` (the wide case);
  - `a_model_change_rebuilds_the_open_list`.
- [ ] Run `cargo test -p celestina pieces::workspaces island::`. Expected: failures.
- [ ] Implement. Run the workspace tests, the clippy pair and the guards. All pass.
- [ ] Commit: `celestina-maintenance: Add the workspaces sheet and the wheel to the workspaces piece`.

### Task D3: Full smoke, evidence and documents — **Model: Sonnet**

- [ ] Run the full smoke: `timeout -k 60 2700 sh celestina/scripts/smoke.sh --binary <tgt>/debug/celestina`; it must print `smoke: OK`.
- [ ] Take nested shots of the workspaces sheet with the author's six workspaces and some fake windows, and of the wheel stepping.
- [ ] Write `celestina/docs/evidence/YYYY-MM-DD-cel-d-005-workspaces-list.md`.
- [ ] Update the CEL-TRACKS "In progress" lines of `celestina/STATUS.md` and `celestina/ROADMAP.md` that A6 wrote: the series is done. Parts B and C leave those lines alone.
- [ ] **The ledger row.** Intended change: "Add the workspaces list sheet: one row per workspace with its windows' icons, the current one marked, a click focusing it; the wheel over the piece steps workspaces ([design], [plan])".
- [ ] **`VAL-CEL-D-005`:** the list matches the output's workspaces and windows; a click focuses and closes; the wheel steps, open or not.
- [ ] Commit: `celestina-maintenance: Add the CEL-D-005 evidence, validation row and ledger entry`.

## Landing order

`CEL-D-002` (`--smoke bar,osd`), then `CEL-D-003` (`--smoke bar,osd,glance,connectivity,energy,clipboard,media,phone,notify`), then `CEL-D-004` (`--smoke bar,glance`), then `CEL-D-005` (the full smoke). Each lands with `python3 scripts/land-unit.py unit/celestina/CEL-D-00N --kind milestone` from the canonical checkout, after the author's direct authorisation, run by the controller. After each landing the author restarts the shell and runs its `VAL-CEL-D-00N` row.
