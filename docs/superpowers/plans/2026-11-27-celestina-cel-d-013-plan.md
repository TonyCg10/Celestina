# CEL-D-013 Implementation Plan: the drop and the one glass shape

> **Amended during execution (2026-11-27).** The author turned down drop G1 at the gate of Task 1.5 and chose drop D1; the section "Amendment" at the end replaces Tasks 1.2 and 1.3 and the G1 numbers below. `CEL-D-013` landed with D1 as `be89dd0a`.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every capsule menu window leaves the bar as drop G1 (it swells under the piece, stretches a neck that breaks, then opens into the window) and goes back the same way. The capsule's glass, the neck and the window are one Haze shape, filled once, and the compositor's blur follows that shape.

**Architecture:**
- `celestina-paint` gains a pure path type (`shape.rs`: move, line, cubic, close), the band slicing of a path, and a Haze fill on a path.
- The shell gains two pure modules:
  - `sheet/liquid.rs`: the neck, the tip and the merge fillet as path builders with free numbers, so G3 can reuse them;
  - `sheet/drop.rs`: G1's open and close phases as functions of `t`.
- `SheetMotion` hands out one `DropFrame` per instant.
- The Island paints each window's glass in one path with its capsule's glass, before the bar's content. The bar's tree leaves out the glass of a joined capsule. The blur region is the path cut into 2 px bands.

**Tech Stack:** Rust; `celestina-paint` (femtovg 0.27), `celestina-ui` (`Tree`, curves), `celestina/src/sheet`, `celestina/src/island`, `celestina-wayland` regions.

**Spec:** `docs/superpowers/specs/2026-11-27-celestina-menu-drop-design.md` §5.1, §5.2, §5.3, §5.7, §6 and §7. Approved preview: "Celestina · la gota", version 3, approved by the author on 2026-11-27. It is a private artifact; its source is `cel-d/preview-approved/src16.html` in the sessions' notes (`/home/toni/.config/Claude/side-session-notes/local_306a3d35-bdfe-402d-9860-465ab8c52ddd/`), and this plan ports its functions `dropOpen`, `dropClose`, `neck`, `neckPath`, `fillet` and `tip`, for mode `g1` only. Code map: `cel-d/d013-map-drop.md` in the same notes; its line numbers are of `main` at 1.35.0 and drift after `CEL-D-014` and `CEL-D-015`.

**Branch:** `unit/celestina/CEL-D-013`. Kind `milestone`. Size: large, executed in its own session.
- Once `CEL-D-014` and `CEL-D-015` have landed: `scripts/worktree.sh open celestina CEL-D-013` from `main`.
- Otherwise, stacked: `scripts/worktree.sh open celestina CEL-D-013 --from unit/celestina/CEL-D-015`, and land after it.

**Depends on:**
- `CEL-D-014`: the piece stays in its slot, `Anchor::pill` is always the piece's rect, and the open pill and the 0.8 opacity live on the bar.
- `CEL-D-015`: a same-capsule switch morphs the window over 300 ms, and the queue only waits for the Island page.

This plan names their behaviour, not their identifiers. Each task that touches their code starts by reading the base (`rg -n` commands given in the task).

## Global Constraints

- **Coordinates.** Surface `celestina-panel`, 560 px tall. Capsule bottom edge `y0 = TOP_GAP + CAPSULE_H = 48`. Window top `WINDOW_TOP = 56`. Window height at most 496. Radius 20 (`theme.radius_lg`).
- **Drop size.** `pw` and `cx` are the live piece's width and centre (`Anchor::pill`). `dw = max(pw, 28) + 6`, `dh = 26`, drop radius `dh / 2 = 13`.
- **Curves.**
  - `ONE_UI` = `CubicBezier(0.22, 0.25, 0.0, 1.0)`.
  - "One UI sine in-out 80" = `SINE_IN_OUT_80` = `(0.33, 0.0, 0.2, 1.0)`; both exist in `celestina-ui`.
  - OutBack with `theme.overshoot` (1.15).
  - The close's rise: a new `CubicBezier(0.4, 0.0, 0.9, 0.6)`, local to `drop.rs` as `RISE`.
- **Open, 460 ms** (§5.2). `t` runs from 0 to 1.
  - `t < 0.28`, swell: `u = ONE_UI(t / 0.28)`.
    - Width `lerp(0.6·pw, dw, u)`, height `lerp(6, 26, u)`, gap `lerp(0, 3, u)`, radius `min(h/2, 13)`.
    - Neck `a = w/2`, `b = w/2 − 2`, `m = (w/2)·lerp(1, 0.8, u)`, `f = 7`.
  - `0.28 ≤ t < 0.52`, fall: `u = SINE_IN_OUT_80((t − 0.28) / 0.24)`.
    - Gap `lerp(3, 16, u)`, rect `dw × 26`, radius 13, `pin = clamp((gap − 3) / 10)`.
    - While `pin < 1`, neck `a = dw/2`, `b = dw/2 − 3`, `m = (dw/2)·0.8·(1 − pin)^0.8`, `f = 7`.
    - Once `pin = 1` (gap 13), the neck has broken: a hanging tip at (`cx`, 48), half-width 5, height `6·(1 − clamp((t − 0.47) / 0.1))`.
  - `t ≥ 0.52`, open: `u = clamp((t − 0.52) / 0.48)`, `k = OutBack(u)`.
    - Rect `lerp(drop at gap 16, window, k)`, radius `lerp(13, 20, clamp(k))`.
    - The same tip while its height is at least 0.4.
  - Content: opacity 0 and blur 12 before 0.52. From 0.52, opacity `clamp((t − 0.6) / 0.34)` and blur `12·(1 − ONE_UI(u))`.
  - At `t ≥ 1`: the window, radius 20, opacity 1, blur 0.
- **Close, 400 ms** (§5.3). `R0` is the shown rect at the close's start; `D` is the drop rect `dw × 26` centred on `cx` at gap 14.
  - `t < 0.42`, gather: `u = ONE_UI(t / 0.42)`.
    - Rect `lerp(R0, D, u)`, radius `lerp(20, 13, u)`, no neck.
    - Content opacity `1 − clamp(t / 0.28)`, blur `12·clamp(t / 0.3)^0.7`.
  - `t ≥ 0.42`, rise: `u = RISE((t − 0.42) / 0.58)`.
    - Gap `lerp(14, −28, u)`, width `lerp(dw, 0.6·pw, u)`, height 26, radius 13. Content 0, blur 12.
    - `pin = clamp((gap − 1) / 11)`.
    - `pin < 1` and gap > 0: neck `a = w/2 + 1`, `b = w/2 − 2`, `m = (w/2)·0.8·(1 − pin)^0.8`, `f = 7`.
    - Gap ≤ 0: merge fillets.
    - Gap > 12: a tip of half-width 4 and height `4·clamp((14 − gap) / 2)`.
  - The close always reads the live `cx` and `pw`.
- **The neck** (`neck` in the preview), between the upper edge (`c0`, `y0`, half-width `a`) and the lower one (`c1`, `y1`, half-width `b`). If `y1 < y0`, then `y1 = y0`.
  - Constants: `ym = (y0 + y1)/2`, `k = max(y1 − y0, 1)/2`, `cm = (c0 + c1)/2`, tab `o = 2`.
  - The contour, clockwise on screen:
    1. M (`c0−a−f`, `y0−o`), L (`c0+a+f`, `y0−o`), L (`c0+a+f`, `y0`);
    2. C (`c0+a`, `y0`) (`cm+m`, `ym−k/2`) (`cm+m`, `ym`);
    3. C (`cm+m`, `ym+k/2`) (`c1+b`, `y1`) (`c1+b+fb`, `y1`);
    4. L (`c1+b+fb`, `y1+o`), L (`c1−b−fb`, `y1+o`), L (`c1−b−fb`, `y1`);
    5. C (`c1−b`, `y1`) (`cm−m`, `ym+k/2`) (`cm−m`, `ym`);
    6. C (`cm−m`, `ym−k/2`) (`c0−a`, `y0`) (`c0−a−f`, `y0`), Z.
- **The neck into a drop** (`neckPath`). With `r` the drop's radius and `rc` its rect:
  - `lo = rc.x + 0.45·r` and `hi = rc.right − 0.45·r`; if `hi − lo < 2`, there is no neck.
  - If `hi − lo ≤ 2(b + f)`, scale `b` and `f` by `(hi − lo) / (2(b + f))` and set `c1 = (lo + hi)/2`. Otherwise `c1 = clamp(cx, lo + b + f, hi − b − f)`.
  - `y1 = rc.y + 0.12·r` when `r > 0.4·rc.h`, else `rc.y`.
  - The waist is `min(m, b + f, a + f)`, and the scaled `f` is used at both ends.
- **Tip** (hanging; `tip` with `dir = 1`). Nothing when `h < 0.4`. With `e = y + h`, tab `f = 4`:
  - M (`c−a−f`, `y−2`), L (`c+a+f`, `y−2`), L (`c+a+f`, `y`);
  - C (`c+a`, `y`) (`c+1.2`, `e−0.4h`) (`c`, `e`);
  - C (`c−1.2`, `e−0.4h`) (`c−a`, `y`) (`c−a−f`, `y`), Z.
- **Merge fillet** (`fillet`). With `cy = rc.y + rc.h/2` and `chord(y) = sqrt(r² − (y − cy)²)`, or 0 when `|y − cy| ≥ r`:
  - `a = chord(48)` and `s = min(5, rc.bottom − 48 − 1)`; nothing if `a < 1` or `s < 1`;
  - `b = 0.92·chord(48 + s)`;
  - the fillet is the neck from (`cx`, 48, `a`) to (`cx`, `48 + s`, `b`) with `m = max(a, b)`, `f = 6`, `fb = 0`.
- **One shape** (§5.1).
  - Each sub-path is closed and clockwise on screen (positive shoelace area in y-down coordinates), and the fill is non-zero, so the fill is their union.
  - A capsule's glass, its windows, the neck, the tips and the fillets are one `ShapePath` filled once by `Painter::haze_path`: grain, then tint, as `Painter::haze`.
  - The bar's tree does not paint that capsule's glass in that frame.
  - The window's glass never fades, except under reduced motion.
- **Draw order.** Window glass paths first, then the bar (capsule content), then the Island, then the window bodies.
- **Blur region.**
  - `slices_of_path(path, SILHOUETTE_STEP = 2.0)`: horizontal bands, pixel-aligned outward, that cover every painted pixel. Consecutive equal bands merge into one rect.
  - The `BLUR_REGION_MIN_GLASS` (0.35) rule goes.
  - The input region stays the whole surface while a window shows; the neck needs no input shape.
- **Body.** Laid out and painted at the window's target rect (`R` while opening, `R0` while closing), clipped to the frame's rounded rect, at the content's opacity, through `Painter::blurred` when its blur is over 0. The close's scaling (`follow`) goes.
- **Pill.** The open pill and the piece's 0.8 opacity (`CEL-D-014`) go back at `MERGE_AT = 0.78` of the close, 312 ms after it starts.
- **Reduced motion** (§5.7).
  - No drop and no neck: the window shows at its rect, and its glass and content fade together over 150 ms (`LINEAR`), in on open and out on close, blur 0.
  - The window is then its own path under that opacity, not joined to its capsule.
  - The pill and the piece's opacity change at once, at the close's start.
- **Ladder.** Durations are capped by `theme.motion.ceiling` (500 ms), as `CLOSE_MS` is today.
- **Rules.**
  - No `unwrap`/`expect`/`panic!` in production; no `unsafe` outside `egl.rs`/`pam.rs`.
  - English everywhere; Spanish only in `celestina/src/copy.rs`, and this unit adds no copy.
  - Never touch `src/lock`, `src/polkit`, `celestina/scripts/`, `.githooks`, any `AGENTS.md`, or `docs/contracts|governance|standards`.
  - At rest no frame is requested.
- **Live-session safety.**
  - Never run `celestina` or `niri msg` from a shell or a subagent.
  - Nests only through `celestina/scripts/dev-session.sh --own-bus --once … --probe …`, under `env -i`, with `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`, a fake `wpctl`/`pactl`, and `nest_guard`.
  - The kit is `cel-d/kit-recovered/` in the notes; copy it to the session's scratchpad as `d13/` and point `run.sh` at this worktree.
  - One nest at a time, a private `CARGO_TARGET_DIR`, `timeout -k 60 2700`, never `-s KILL`, never `pkill` by name.
- **Checks.**
  - From `celestina/`: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, the same clippy with `+1.100.0`, and `cargo test --workspace --locked`. Add up every `test result` line.
  - From the root: `bash scripts/check-architecture-contract.sh`, `bash scripts/check-documentation-contract.sh`, `python3 scripts/check-language-contract.py`.
- **Commits.**
  - Branch commits are `celestina-maintenance: <Imperative …>`, with a verb from `scripts/commit_scope.py`: Add, Fix, Update, Replace or Remove.
  - The landing applies `milestone` and the version bump.

## Review Focus

1. **The hand-over frames.** On the first frame of an open and on the frame a close ends (the sheet is reaped), the capsule's glass is painted exactly once: by the path or by the tree, never both, never neither. Task 3 pins it with `the_capsule_glass_is_painted_once_on_every_frame_of_an_open_and_a_close`.
2. **Two capsules at once.** A menu on another capsule opens while the first one closes (`CEL-D-015`: no wait). There are two paths, each with its own capsule and no glass doubled, and the region covers both drops. Task 3 pins it with `two_capsules_drop_at_once_each_in_its_own_path`, Task 4 with `the_region_covers_two_drops_at_once`.
3. **The piece or capsule moves during a drop.**
   - A piece appears in the capsule, so the extent glides, or the piece vanishes.
   - The neck follows the live `cx`, `pw` and capsule; a vanished piece closes towards its last pill.
   - Nothing is NaN, and nothing leaves the surface.
   - Task 2 pins it with `the_close_follows_the_live_piece` and `a_degenerate_piece_never_yields_non_finite_shapes`.
4. **A switch or a reopen during a drop phase.**
   - It starts `CEL-D-015`'s travel from the frame's rect, radius 20, with no neck or tip left behind.
   - A reopen of the same piece during its close does the same; it never restarts a drop from the piece while the old one still shows.
   - Task 5 pins it with `a_switch_mid_drop_travels_from_the_shown_frame` and `reopening_during_the_close_travels_from_the_shown_frame`.
5. **The break and the tips in the blur region.**
   - The 6 px tip after the break, and a drop that is not joined, are covered by the region on every frame.
   - The rect count per frame stays bounded (at most 64 for a capsule, a window and a neck), so `RegionCache` does not resend a flood.
   - Task 1.1 pins it with `slices_cover_a_tip_and_a_detached_drop`, Task 4 with `the_region_never_misses_a_painted_pixel_through_a_whole_open_and_close`.

---

## Setup (the controller, before Task 1)

- [ ] Open the worktree as **Branch** says. Confirm the base has `CEL-D-014`'s pill and `CEL-D-015`'s travel: `rg -n "MUTED_CONTENT|mutedContent|ControlHover" celestina/src/island/bar_tree.rs` and `rg -n "300|travel|morph" celestina/src/sheet/motion.rs`.
- [ ] Add the ledger row `CEL-D-013` to `celestina/docs/plans/active/2026-11-25-cel-tracks.md`, `active`, in the open-row form `docs/governance/documentation.md` requires. Intended change: `Add the drop: a capsule menu window swells out of its piece, stretches a neck that breaks and opens, and goes back the same way, with the capsule, the neck and the window painted as one glass shape whose blur region follows it (preview approved 2026-11-27; [design](../../../../docs/superpowers/specs/2026-11-27-celestina-menu-drop-design.md), [plan](../../../../docs/superpowers/plans/2026-11-27-celestina-cel-d-013-plan.md))`. Add the evidence link to the stub and `VAL-CEL-D-013`. Change only this row.
- [ ] Create the evidence stub `celestina/docs/evidence/YYYY-MM-DD-cel-d-013-drop.md`, dated the day it is opened, from the template the earlier `CEL-D` evidence uses, with a shots folder of the same name.
- [ ] Copy the kit: `cp -r <notes>/cel-d/kit-recovered <scratchpad>/d13`, then point `run.sh` at this worktree, `nest_lock_acquire "cel-d-013"`, and use `timeout -k 60 2700`.
- [ ] Commit `celestina-maintenance: Add the CEL-D-013 ledger row and evidence stub`.

---

## Task 1: the fidelity gate (§7)

The gate stops the unit. Tasks 1.1 to 1.4 build what the drop needs to be painted for real, kept and tested. Task 1.5 paints frozen frames through a throwaway spike in a nest and **stops for the author**. Task 2 does not start until the author approves the nested screenshots of the neck, the break and the merge.

If the author turns G1 down:
- the unit stops here and the controller records the decision in the ledger row's notes;
- the meniscus (G3) then replaces only `drop.rs` (Task 1.3) by a plan amendment, reusing `liquid.rs` with the preview's `g3` numbers (`a = min(0.32·R.w, 80)`, `b = 1.1·a`, `f = 8`);
- Tasks 2 to 6 keep the `DropFrame` interface.

### Task 1.1: the path type and its bands

**Files:**
- Create: `celestina/crates/celestina-paint/src/shape.rs` (tests in the file)
- Modify: `celestina/crates/celestina-paint/src/lib.rs` (`mod shape; pub use shape::{slices_of_path, Seg, ShapePath};`)

**Interfaces:**
- Produces:
  - `#[derive(Clone, Copy, Debug, PartialEq)] pub enum Seg { Move(f32, f32), Line(f32, f32), Cubic { c1: (f32, f32), c2: (f32, f32), to: (f32, f32) }, Close }`.
  - `#[derive(Clone, Debug, Default, PartialEq)] pub struct ShapePath`, with these methods:
    - `new()`, `move_to(x, y)`, `line_to(x, y)`, `cubic_to(c1: (f32, f32), c2: (f32, f32), to: (f32, f32))`, `close()`;
    - `segs(&self) -> &[Seg]`, `is_empty(&self) -> bool`, `append(&mut self, other: &ShapePath)`;
    - `rounded_rect(&mut self, rect: Rect, radius: f32)`, and `ShapePath::rounded(rect, radius) -> ShapePath`;
    - `bounds(&self) -> Option<Rect>`;
    - `flatten(&self, per_cubic: usize) -> Vec<Vec<(f32, f32)>>`, one closed polyline per sub-path;
    - `contains(&self, x: f32, y: f32) -> bool`.
  - `pub fn slices_of_path(path: &ShapePath, step: f32) -> Vec<Rect>`.

**Decisions:**
- **`rounded_rect`.**
  - It adds nothing when `w ≤ 0.3` or `h ≤ 0.3` (the preview's `rr`). It clamps `radius` to `[0, min(w, h)/2]`, and non-finite input adds nothing.
  - It starts at (`x+r`, `y`), runs clockwise, and draws each corner as one cubic with κ = 0.552 284 7, femtovg's `rounded_rect` approximation.
- **`bounds`.** The hull of every point and control point.
- **`contains`.** Non-zero winding on `flatten(64)`.
- **`slices_of_path`.**
  - Flatten with 16 chords per cubic. Bands start at `floor(min y)` and step `step` until `ceil(max y)`.
  - In each band, take one x-interval per sub-path: the min and max x of that polygon's part inside the band (vertices within it, plus the edge crossings at its top and bottom).
  - Pad each interval by 0.5 px, round it outward to whole pixels, and merge the intervals of a band that overlap or touch.
  - Emit one rect per interval, and merge a band into the rect above when their interval lists are equal.
  - An empty path, a non-finite one or `step ≤ 0` yields nothing.

- [ ] **Step 1: Write the failing tests** (sentence names, the helpers' style of `haze.rs`):

```rust
#[test] fn a_rounded_rect_is_one_clockwise_closed_contour()           // shoelace area > 0 in y-down; last seg Close; first Move at (x+r, y)
#[test] fn a_rounded_rect_too_small_or_not_finite_adds_nothing()     // w 0.3, h NaN
#[test] fn contains_follows_the_rounded_corners()                     // (x+1, y+1) outside for r 20; centre inside
#[test] fn overlapping_contours_are_their_union()                      // two overlapping rounded rects: a point in both is inside (non-zero)
#[test] fn slices_of_a_rounded_rect_cover_like_silhouette_slices()     // capsule (1500,8,400,40) r20 and window (1500,56,320,200) r20: every pixel centre inside → covered; rect count ≤ 2·ceil(r/2)+1 per shape
#[test] fn slices_never_miss_a_painted_pixel_and_overshoot_at_most_3_px() // capsule + window + a neck contour from Task 1.2's numbers written as literal segs; every slice pixel centre has a painted pixel centre within 3 px (Chebyshev)
#[test] fn slices_cover_a_tip_and_a_detached_drop()                   // capsule + tip (cx 1800, y 48, a 5, h 6) + drop (1783,64,34,26) r13: tip pixels covered, the 48..64 gap away from the tip not covered
#[test] fn equal_bands_merge_so_a_tall_window_is_few_rects()          // window 320×496 r20 alone: ≤ 21 rects
#[test] fn an_empty_or_broken_path_has_no_slices()
```

- [ ] **Step 2: Run** `cargo test -p celestina-paint --locked shape`. Expected: FAIL (module missing).
- [ ] **Step 3: Implement** `shape.rs` as decided.
- [ ] **Step 4: Run** the same command. Expected: PASS. Then run `cargo clippy -p celestina-paint --all-targets --locked -- -D warnings`.
- [ ] **Step 5: Commit** `celestina-maintenance: Add a path type to the painter and the bands that cover a path`.

### Task 1.2: the liquid shapes (neck, tip, merge fillet)

**Files:**
- Create: `celestina/src/sheet/liquid.rs` (tests in the file)
- Modify: `celestina/src/sheet/mod.rs` (`pub mod liquid;`)

**Interfaces:**
- Consumes: `celestina_paint::{Rect, ShapePath}` (Task 1.1).
- Produces (free numbers, no G1 constant inside):
  - `#[derive(Clone, Copy, Debug, PartialEq)] pub struct Neck { pub a: f32, pub b: f32, pub m: f32, pub f: f32 }`: top half-width, bottom half-width, waist, fillet;
  - `pub fn neck(path: &mut ShapePath, top: (f32, f32), a: f32, bottom: (f32, f32), b: f32, m: f32, f: f32, fb: f32)`, with `top = (c0, y0)` and `bottom = (c1, y1)`, the contour of Global Constraints;
  - `pub fn neck_into(path: &mut ShapePath, cx: f32, y0: f32, n: Neck, drop: Rect, radius: f32)`: the preview's `neckPath`;
  - `pub fn tip(path: &mut ShapePath, cx: f32, y: f32, a: f32, h: f32)`: hanging;
  - `pub fn merge_fillet(path: &mut ShapePath, cx: f32, y0: f32, drop: Rect, radius: f32)`.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test] fn a_neck_is_one_clockwise_contour_with_its_tabs_two_px_into_both_edges()   // top (1800,48) a 17, bottom (1800,51) b 14, m 13.6, f 7: first Move (1776, 46); bottom tab at y 53; area > 0
#[test] fn the_neck_is_narrowest_at_its_waist()                                     // contains(1800 ± (m − 0.5), ym) true; contains(1800 ± (m + 0.5), ym) false
#[test] fn the_fillets_are_tangent_to_the_edges()                                    // first cubic's c1 at (c0+a, y0); bottom cubic's c2 at (c1+b, y1)
#[test] fn neck_into_keeps_the_bottom_inside_the_drops_straight_edge()                // cx 1830 near the right of drop (1783,64,34,26) r13: c1 clamped to hi − b − f
#[test] fn neck_into_a_narrow_drop_scales_b_and_f()                                   // hi − lo ≤ 2(b+f): c1 centred, b and f scaled by (hi−lo)/(2(b+f))
#[test] fn neck_into_a_round_drop_attaches_12_percent_of_the_radius_inside()         // y1 = 64 + 1.56
#[test] fn a_tip_under_0_4_px_adds_nothing()
#[test] fn a_merge_fillet_appears_only_while_the_drop_crosses_the_edge()             // drop (1783,40,34,26) r13 → a = chord(48) > 1; drop (1791.6,20,16.8,26) → nothing
#[test] fn a_meniscus_neck_wider_at_the_bottom_is_still_one_clockwise_contour()       // G3 numbers: a 80, b 88, m 60, f 8 (proves reuse)
#[test] fn non_finite_inputs_add_nothing()
```

- [ ] **Step 2: Run** `cargo test -p celestina --locked sheet::liquid`. Expected: FAIL.
- [ ] **Step 3: Implement** the functions as ports of the preview's `neck`, `neckPath`, `tip` (`dir = 1`) and `fillet`, with the numbers of Global Constraints. Return early on non-finite input.
- [ ] **Step 4: Run** the same command. Expected: PASS.
- [ ] **Step 5: Commit** `celestina-maintenance: Add the neck, tip and merge fillet shapes of the liquid menus`.

### Task 1.3: drop G1 as pure frames

**Files:**
- Create: `celestina/src/sheet/drop.rs` (tests in the file)
- Modify: `celestina/src/sheet/mod.rs` (`pub mod drop;`), `celestina/src/sheet/geometry.rs` (`pub const CAPSULE_BOTTOM: f32 = TOP_GAP + CAPSULE_H;`, 48)

**Interfaces:**
- Consumes: Task 1.2's `liquid::*`; `celestina_ui::{Easing, CubicBezier, ONE_UI, SINE_IN_OUT_80}`.
- Produces:
  - `pub const OPEN_MS: u64 = 460; pub const CLOSE_MS: u64 = 400; pub const MERGE_AT: f32 = 0.78;`
  - `pub const OPEN_HOLD: f32 = 0.52;`: the end of the fall, for `CEL-D-016`, which holds `t` there until its layout arrives;
  - `#[derive(Clone, Copy, Debug, PartialEq)] pub struct Source { pub cx: f32, pub w: f32 }`, with `Source::of(pill: Rect) -> Source`;
  - `#[derive(Clone, Copy, Debug, PartialEq)] pub enum Join { Detached, Neck(liquid::Neck), Tip { a: f32, h: f32 }, Merge }`;
  - `#[derive(Clone, Copy, Debug, PartialEq)] pub struct DropFrame { pub rect: Rect, pub radius: f32, pub join: Join, pub glass: f32, pub content: f32, pub blur: f32 }`, with `DropFrame::window(rect: Rect, radius: f32) -> DropFrame`: detached, glass 1, content 1, blur 0;
  - `pub fn open_frame(t: f32, to: Rect, src: Source, overshoot: f32) -> DropFrame`;
  - `pub fn close_frame(t: f32, from: Rect, src: Source) -> DropFrame`;
  - `pub fn shape(capsule: Option<(Rect, f32)>, frames: &[(DropFrame, Source)]) -> ShapePath`: the capsule's rounded rect, each frame's rounded rect and its join (neck through `neck_into`, tip at (`cx`, 48), merge fillet), all from `CAPSULE_BOTTOM`.

- [ ] **Step 1: Write the failing tests** (piece `cx 1800`, `w 28`, so `dw 34`; window `R = (1500, 56, 320, 200)`):

```rust
#[test] fn the_drop_swells_from_six_px_under_the_piece()             // t 0: rect (1791.6, 48, 16.8, 6), r 3, Neck{a 8.4, b 6.4, m 8.4, f 7}; content 0, blur 12
#[test] fn the_fall_starts_at_a_three_px_gap()                        // t 0.28: rect (1783, 51, 34, 26), r 13, Neck{a 17, b 14, m 13.6, f 7}
#[test] fn the_neck_breaks_at_a_gap_of_13_and_leaves_a_6_px_tip()     // first t with rect.y ≥ 61: Join::Tip{a 5, h 6·(1 − clamp((t−0.47)/0.1))}
#[test] fn the_tip_is_gone_by_57_percent()                           // t 0.57 → Detached
#[test] fn the_open_lands_on_the_window_with_outback()               // t 0.52: rect (1783, 64, 34, 26); some t in (0.52,1): rect.w > 320 (overshoot); t 1: R, r 20, content 1, blur 0
#[test] fn the_open_content_fades_in_from_60_percent()               // t 0.6 → 0; t 0.77 → 0.5; t 0.94 → 1; blur at t 0.52 = 12, at t 1 = 0
#[test] fn the_close_gathers_into_a_drop_14_px_under_the_piece()     // t 0.42: rect (1783, 62, 34, 26), r 13, Detached; content 0 from t 0.28; blur 12 from t 0.3
#[test] fn the_close_neck_forms_again_below_a_gap_of_12()            // first rise frame with gap < 12 → Join::Neck with a = w/2 + 1
#[test] fn the_capsule_absorbs_the_drop()                             // gap ≤ 0 → Merge; t 1: rect (1791.6, 20, 16.8, 26)
#[test] fn the_close_follows_the_live_piece()                          // same t, src.cx 1700 vs 1800: rect.x differs by 100 in the rise
#[test] fn a_degenerate_piece_never_yields_non_finite_shapes()       // w 0, w NaN, cx NaN, t NaN: every number finite; shape() bounds finite
#[test] fn shape_unites_the_capsule_and_the_drop_in_one_path()       // shape(Some((capsule, 20)), [(open_frame(0.4,..), src)]) contains a point in the capsule, the waist and the drop
```

- [ ] **Step 2: Run** `cargo test -p celestina --locked sheet::drop`. Expected: FAIL.
- [ ] **Step 3: Implement** both frame functions as the preview's `dropOpen`/`dropClose` for `g1`. Clamp `t` to 0..1, and a NaN `t` counts as 1, as `CubicBezier::eval` does. A tip shorter than 0.4 px is `Join::Detached`, so the rise's first frame (gap 14, tip height 0) is detached. `RISE` is a local `CubicBezier(0.4, 0.0, 0.9, 0.6)`.
- [ ] **Step 4: Run** the same command. Expected: PASS.
- [ ] **Step 5: Commit** `celestina-maintenance: Add the drop's open and close frames as pure functions`.

### Task 1.4: the Haze on a path

**Files:**
- Modify: `celestina/crates/celestina-paint/src/backend.rs` (`fill_path`, `fill_image_path`, one `fn fv_path(&ShapePath) -> femtovg::Path`)
- Modify: `celestina/crates/celestina-paint/src/painter.rs` (`Painter::haze_path`; `haze` and `haze_path` share one private grain-then-tint routine)

**Interfaces:**
- Produces: `pub fn haze_path(&mut self, path: &ShapePath)`, the same `HazePlan` as `haze`: the grain tile anchored at (0, 0) at `theme.grain_alpha`, then `Role::GlassTint`; without blur, `Role::GlassFallback` alone.
- Backend: `pub(crate) fn fill_path(&mut self, path: &ShapePath, color: Color)` and `pub(crate) fn fill_image_path(&mut self, path: &ShapePath, image: ImageHandle, placement: Rect, tint: Color)`.

- [ ] **Step 1: Verify femtovg 0.27 before writing.** Find its source with `cargo metadata --format-version 1 --locked | jq -r '.packages[] | select(.name=="femtovg") | .manifest_path'`. Read `Paint` (is there a fill rule, and is non-zero the default?) and the path cache (does it give every solid sub-path the same winding, as nanovg does?). Record both answers in the evidence. Set the paint's fill rule to non-zero explicitly if the API has one.
- [ ] **Step 2: Write the failing tests** (pure, no GL): `fv_path` sits behind a pure `fn ops(path: &ShapePath) -> Vec<PathOp>` that the test reads:

```rust
#[test] fn a_path_converts_segment_for_segment()          // Move/Line/Cubic/Close map 1:1, in order
#[test] fn haze_path_uses_the_same_plan_as_haze()          // HazePlan::for_ with and without blur (grain then tint; fallback alone)
```

- [ ] **Step 3: Run** `cargo test -p celestina-paint --locked`. Expected: FAIL.
- [ ] **Step 4: Implement**, then run the tests and `cargo clippy -p celestina-paint --all-targets --locked -- -D warnings`. Expected: PASS.
- [ ] **Step 5: Commit** `celestina-maintenance: Add the Haze material on a path`.

### Task 1.5: the drop in a nest, and the author's approval (STOP)

**Files:** none of the unit's. A throwaway spike, never committed to `unit/celestina/CEL-D-013`. Its diff is kept as `<notes>/cel-d/d013-spike.diff`. Shots go to the evidence's shots folder.

**The spike**, on a scratch copy of this branch:
- With a menu window open, the Island paints `drop::shape(capsule, [frame])` through `haze_path` before `bar.paint`, leaves the bar's haze out for that capsule, and leaves out the window's own glass.
- It sets the blur region to `slices_of_path` of that path plus the other capsules.
- It freezes the frame at the `t` read from a file under the nest's `XDG_STATE_HOME`, `drop-spike`, holding `open 0.40` or `close 0.90`, re-read every frame. The probe writes that file between shots.

- [ ] **Step 1:** Print the gap per `t` at 1 % steps from `drop.rs`, through a test-only helper that is not committed, to choose the `t` values of the shots.
- [ ] **Step 2:** In one guarded nest with the author's layout (the end capsule of the recording), take shots of the energy menu (end capsule) and the tray chevron's window, in dark and light, over the wallpaper and over a busy window:
  1. open swell (`t` 0.14);
  2. the neck mid-fall (gap ≈ 8);
  3. the break (the first frame with the tip);
  4. the tip retracting (`t` 0.52);
  5. mid-open (OutBack);
  6. at rest (window and capsule in one path: no darker seam, no double tint);
  7. close gathered (`t` 0.42);
  8. the neck formed again (gap ≈ 8);
  9. the merge fillets (gap ≈ −4);
  10. nearly absorbed (gap ≈ −20).
  
  Repeat shots 2, 3 and 9 at output scale 1.25 (the nest's `run.kdl`).
- [ ] **Step 3:** Read every crop at full size. Name every visible defect against the preview's frames: a seam, a hairline at the tabs, a blur edge that does not follow the neck, a fringe, or a gap between the neck and the capsule.
- [ ] **Step 4: STOP.** The controller shows the author the shots of the neck, the break and the merge beside the preview, with the defects found. Nothing from Task 2 on starts until the author approves. On a refusal, follow Task 1's refusal note.

---

## Task 2: SheetMotion drives the drop

**Files:**
- Modify: `celestina/src/sheet/motion.rs` and its tests
- Modify: the callers `rg -n "\.rect\(|\.radius\(|body_opacity|glass_opacity|blur_px|blur_region|BLUR_REGION_MIN_GLASS|start_radius|CLOSE_SHRINK" celestina/src` finds (`sheet/mod.rs`, `island/sheets.rs`, tests), only as far as the new signatures need

**Interfaces:**
- Consumes: Task 1.3's `drop::{open_frame, close_frame, DropFrame, Source, OPEN_MS, CLOSE_MS, MERGE_AT}`.
- Produces:
  - `SheetMotion::open(pill, theme, now, reduced)` and `close(shown, pill, now)` keep their signatures. Open runs 460 ms and close 400 ms, both capped by `ladder.ceiling`.
  - `pub fn frame(&self, now: Instant, target: Rect, pill: Rect, surface: (f32, f32)) -> DropFrame`:
    - open: `open_frame` with `Source::of(pill)`;
    - close: `close_frame` from the shown rect it was given;
    - `CEL-D-015`'s travel or resize: `DropFrame::window(lerped rect, 20.0)`, whose content clocks stay `CEL-D-015`'s;
    - reduced motion: `DropFrame::window(target or shown, 20.0)` with `glass = content = LINEAR(t)` over 150 ms (`1 − …` on the close);
    - the rect is cut to the surface by `within`.
  - `pub fn rect(&self, now, target, pill, surface) -> Rect` = `frame(..).rect`.
  - `pub fn merged(&self, now: Instant) -> bool`: true once a close is at `MERGE_AT` or past it, and at the close's start under reduced motion.
  - Removed: `glass_opacity`, `body_opacity`, `blur_px`, `radius`, `blur_region`, `start_radius`, `BLUR_REGION_MIN_GLASS`, `CLOSE_SHRINK`, `CLOSE_BLUR_PX` and the old `CLOSE_MS`, along with every caller's use of them.
  - `closed`, `wants_frame`, `is_closing`, `ladder`, `overshoot` and `reduced` keep their meaning. `closed` is true at the close's end, 400 ms, or 150 ms reduced.

- [ ] **Step 1: Write the failing tests** in `motion.rs`'s style (one `Instant::now()`, `ms(t0, n)`):

```rust
#[test] fn opening_is_drop_g1_over_460_ms()                 // frame at 0, 129 (0.28), 239 (0.52), 460 ms equals drop::open_frame at those t
#[test] fn closing_is_drop_g1_over_400_ms_from_the_shown_rect()
#[test] fn the_close_reads_the_live_pill_every_frame()
#[test] fn merged_turns_true_at_312_ms_of_the_close()       // 311 false, 312 true
#[test] fn reduced_motion_fades_in_place_over_150_ms()      // open: rect = target at 0 ms, glass = content = 0.5 at 75 ms, Detached; close: closed at 150 ms; merged at 0
#[test] fn the_glass_never_fades_outside_reduced_motion()   // every frame of open and close: glass == 1
#[test] fn the_durations_follow_the_ladder_ceiling()        // a ladder with ceiling 300: open and close last 300 ms
#[test] fn nothing_wants_a_frame_once_every_clock_has_settled()   // kept, retargeted to 460/400
```

  Delete the tests of the removed curves: `closing_blurs_fades_and_shrinks_towards_the_slot`, `the_blur_region_leaves_below_35_percent_glass`, and the body fade-in test. Keep `the_overshoot_never_leaves_the_surface` on the new frames.
- [ ] **Step 2: Run** `cargo test -p celestina --locked sheet::motion`. Expected: FAIL.
- [ ] **Step 3: Implement.** The callers compile against `frame` and `rect(.., pill, ..)`. Until Task 3 they paint the frame's rect and radius as today's window (glass at `frame.glass`, body at `frame.content`), so the branch stays green between tasks.
- [ ] **Step 4: Run** `cargo test --workspace --locked` and both clippy runs. Expected: PASS.
- [ ] **Step 5: Commit** `celestina-maintenance: Replace the menu window's grow and close with drop G1`.

---

## Task 3: one glass shape, under the bar's content

**Files:**
- Modify: `celestina/crates/celestina-ui/src/tree.rs`, `celestina/crates/celestina-ui/src/paint.rs` (a glassless capsule)
- Modify: `celestina/src/island/bar_tree.rs` (joined zones, the capsule's opacity)
- Modify: `celestina/src/sheet/mod.rs` (`Sheets::glass`; `paint` paints bodies only: the `haze_at`, `follow` and `Look::glass` parts go)
- Modify: `celestina/src/island/sheets.rs`, `celestina/src/island/mod.rs` (`Island::paint` order)
- Test: `celestina/crates/celestina-ui` paint tests, `celestina/src/sheet/mod.rs` tests, a new `celestina/src/island/tests/drop.rs`

**Interfaces:**
- Consumes: Tasks 1.3, 1.4 and 2.
- Produces:
  - `Tree::set_glassless(&mut self, key: Key, glassless: bool)`: while set, the box keyed `key` skips `painter.haze` (fill, wash and stroke unchanged), like `set_extent`.
  - `BarTree::set_joined(&mut self, zones: &[usize])`: glassless for those zones' keys and back for the others. `BarTree::capsule_opacity(&self, zone: usize, now: Instant) -> f32` is the zone node's effective opacity.
  - `Sheets::glass(&self, anchor: Option<&Anchor>, now: Instant) -> Vec<WindowGlass>`, with `pub struct WindowGlass { pub zone: usize, pub frame: DropFrame, pub source: Source }`, for the open window and every leaving one. `Leaving` gains `zone: usize` if the base does not have it.
  - `Island::glass_paths(&self, now: Instant) -> Vec<GlassPath>`, with `pub(super) struct GlassPath { pub joined: Option<usize>, pub path: ShapePath, pub opacity: f32 }`:
    - frames with `glass == 1` group by zone into `drop::shape(Some((bar.capsule_extent(zone), radius_lg)), frames)`, painted at `capsule_opacity`;
    - a frame with `glass < 1` (reduced motion) is its own path, with `joined: None` and `opacity = glass`.
  - `Island::paint` order: `begin`, then each `GlassPath` through `haze_path` under its opacity, then `bar.set_joined(…)` and `bar.paint`, then the Island, then `paint_sheet` (bodies), then `end`.
  - The body is painted at its target, clipped to `frame.rect`/`frame.radius`, at `frame.content`, through `Painter::blurred` when `frame.blur > 0` (layer from `blur_layer`, for the open too).

- [ ] **Step 1: Read the base.** Find where `CEL-D-014` paints the open pill and where `CEL-D-015` keeps the old body: `rg -n "set_open|open_piece|pill|old_body|swap" celestina/src/island/bar_tree.rs celestina/src/sheet/mod.rs`. The bodies keep their `CEL-D-015` cross-fade; only the glass moves.
- [ ] **Step 2: Write the failing tests:**

```rust
// celestina-ui
#[test] fn a_glassless_box_paints_no_haze_but_keeps_its_fill()
// island/tests/drop.rs (Fake painter or the existing paint-recording helpers)
#[test] fn an_open_window_and_its_capsule_are_one_haze_path()                       // one haze_path call holding both; no haze() for that capsule
#[test] fn the_capsule_glass_is_painted_once_on_every_frame_of_an_open_and_a_close() // every 10 ms from open to reap: exactly one of {tree haze of the zone, a joined path}
#[test] fn the_glass_is_painted_before_the_pieces()                                  // the path precedes the capsule's first glyph or icon
#[test] fn two_capsules_drop_at_once_each_in_its_own_path()                          // another-capsule switch: two joined paths, zones differ
#[test] fn a_reduced_motion_window_is_its_own_path_under_its_glass()
#[test] fn the_body_is_clipped_to_the_drop_and_blurred_while_opening()               // t 0.7: clip = frame rect, blur > 0, content > 0
#[test] fn the_tray_chevron_window_joins_the_capsule_that_holds_the_chevron()
```

- [ ] **Step 3: Run** `cargo test --workspace --locked drop`. Expected: FAIL.
- [ ] **Step 4: Implement** as decided. A zone with no capsule (no extent) gets `shape(None, …)`.
- [ ] **Step 5: Run** `cargo test --workspace --locked` and both clippy runs. Expected: PASS. Rewrite the tests in `sheet/mod.rs` (`a_closing_window_paints_through_one_layer_for_its_whole_close`, `the_body_shrinks_with_its_window`) to the clip without scaling.
- [ ] **Step 6: Commit** `celestina-maintenance: Replace the window's own glass with one Haze path shared with its capsule`.

---

## Task 4: the blur region follows the shape

**Files:**
- Modify: `celestina/src/island/mod.rs` (`shapes` loses `sheet_windows`; `sync_ctl` adds the paths' bands), `celestina/src/island/sheets.rs` (`sheet_windows` goes or stays as a test accessor), `celestina/src/sheet/mod.rs` (`windows` goes; `Sheets::glass` replaces it)
- Test: `celestina/src/island/tests/window.rs`, the `Island::shapes` tests in `island/mod.rs`

**Interfaces:**
- Consumes: Task 1.1's `slices_of_path`, Task 3's `Island::glass_paths`.
- Produces: `Island::blur_bands(&self, now: Instant) -> Vec<Rect>` = `silhouette::union(&self.shapes(now))` followed by `slices_of_path(&p.path, SILHOUETTE_STEP)` for every glass path. `sync_ctl` sets it. The input region is unchanged: the whole surface while anything shows.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test] fn the_region_never_misses_a_painted_pixel_through_a_whole_open_and_close() // every 20 ms: each pixel centre inside the union of glass_paths and bar shapes is in blur_bands
#[test] fn the_region_covers_two_drops_at_once()
#[test] fn the_region_keeps_the_window_until_the_capsule_absorbs_it()              // no 35 % rule: the drop's pixels stay covered until reaped
#[test] fn after_the_close_no_region_of_the_old_window_is_left()                     // kept, retimed to 400 ms
#[test] fn the_region_has_at_most_64_rects_for_a_capsule_a_window_and_a_neck()
```

  Delete `the_blur_region_of_a_closing_window_follows_its_rect_and_leaves_with_its_glass`. Update the `bars + n` shape counts in the existing tests: windows are no longer in `shapes`.
- [ ] **Step 2: Run** `cargo test -p celestina --locked island::tests::window`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test --workspace --locked` and both clippy runs. Expected: PASS.
- [ ] **Step 5: Commit** `celestina-maintenance: Replace the window's blur rects with the bands of its glass shape`.

---

## Task 5: the pill at the merge, switches mid-drop, reduced motion

**Files:**
- Modify: `celestina/src/sheet/mod.rs` (`held_piece`; the switch and reopen paths start the travel from the shown frame), `celestina/src/island/sheets.rs` or `island/mod.rs` (feeding the bar), `celestina/src/island/bar_tree.rs` only through `CEL-D-014`'s API
- Test: `island/tests/drop.rs`, `island/tests/crossfade.rs` (or whatever `CEL-D-014` renamed it to)

**Interfaces:**
- Consumes: `SheetMotion::merged` (Task 2) and `CEL-D-014`'s `Sheets::held_piece(now) -> Option<Key>`, `SheetMotion::close_fraction` and `sheet::motion::PIECE_RESTORE_AT` (0.0 after `CEL-D-014`); `BarTree::set_open` feeds the pill.
- Produces: `PIECE_RESTORE_AT` becomes `drop::MERGE_AT` (0.78), so `Sheets::held_piece` keeps a closing sheet's piece until the merge; `close_fraction` reads the drop close's 400 ms. The bar's pill and opacity follow it.
  - Under reduced motion the pill and the opacity change at once: no 150 ms exit fade and no 200 ms ramp. Gate `CEL-D-014`'s clocks on `reduced`.
  - A same-capsule switch during an open or a close drop, or a reopen of the same piece during its close, starts `CEL-D-015`'s travel from `frame(now).rect` with radius 20 and no join, never a new drop from the piece.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test] fn the_pill_and_the_opacity_return_at_78_percent_of_the_close() // held_piece Some at 311 ms of the close, None at 312
#[test] fn reduced_motion_changes_the_pill_and_the_opacity_at_once()
#[test] fn a_switch_mid_drop_travels_from_the_shown_frame()             // switch at open t 0.4: the next frame's rect starts at the drop rect, Join::Detached, one window
#[test] fn reopening_during_the_close_travels_from_the_shown_frame()
#[test] fn another_capsule_opens_at_once_while_the_first_drops_back()   // no queue; two WindowGlass, two zones
```

- [ ] **Step 2: Run** `cargo test -p celestina --locked island::tests::drop`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test --workspace --locked` and both clippy runs. Expected: PASS.
- [ ] **Step 5: Commit** `celestina-maintenance: Add the open pill's return at the drop's merge and the travel from a drop's shown frame`.

---

## Task 6: the motion in a nest, the smoke and the documents

**Files:**
- Modify: the evidence `celestina/docs/evidence/YYYY-MM-DD-cel-d-013-drop.md` and its shots folder; `celestina/VALIDATION.md` (new `VAL-CEL-D-013`, in the format of `VAL-CEL-D-012`, plus a one-line note on `VAL-CEL-D-010` that `CEL-D-013` replaced its close)
- Modify: the lines of `celestina/README.md` and `celestina/docs` that describe the close blurring out or shrinking towards the piece (`rg -n "blurs? (it )?out|towards its piece|35 %" celestina/README.md celestina/docs --glob '!evidence/**'`)

- [ ] **Step 1:** In the guarded nest (kit `d13/`), use the real build with no spike. Take bursts of frames at 60 Hz through an open and a close of the energy menu, the network menu and the tray chevron's window, in both themes. Add a same-capsule switch during the fall, and a switch to another capsule during an open. Read every crop and compare each phase with the preview's.
- [ ] **Step 2:** Confirm in the logs that no frame is requested at rest after a close. Run the smoke phases the landing will run: `celestina/scripts/smoke.sh --binary <path> --only bar,osd,notify`, the same set as `--smoke` below. Record the result.
- [ ] **Step 3:** Fill in the evidence:
  - procedure and shots;
  - the femtovg answers from Task 1.4;
  - the author's approval of Task 1.5, with its date;
  - limits: the blur of the compositor itself is only seen live.
  
  Write `VAL-CEL-D-013`, the author's steps on the real session: open and close the menus of every capsule, a switch, a switch to another capsule, reduced motion on, and the three monitors.
- [ ] **Step 4:** Run the three root guards. Commit `celestina-maintenance: Add the CEL-D-013 evidence and VAL-CEL-D-013`.

---

## Close of the unit

- [ ] `/simplify` on the branch's code, in this session, without subagents. Then `stop-slop` on the evidence, `VALIDATION.md` and the doc lines. Commit what changes (`celestina-maintenance: Update …`).
- [ ] Final review `CEL-D-013.R · Final review`: Opus, high reasoning, `/code-review` over `main...unit/celestina/CEL-D-013`, with the design, this plan and the preview's source. At most 2 rounds; the second only rereads the first round's fixes. After the last round, the session fixes and checks any defect of the unit's goal; everything else goes to the ledger's parked notes.
- [ ] Run every check of Global Constraints one last time, on the final tree.
- [ ] **Landing**, only with the author's direct word, from the canonical checkout:
  `python3 scripts/land-unit.py unit/celestina/CEL-D-013 --kind milestone --smoke bar,osd,notify --summary "<the row's intended change>"`.
  `CEL-D-016` is the series' last unit and runs the full smoke. If stacked, `CEL-D-015` lands first.

## Model table (a heuristic estimate)

| Chunk | Model | Reasoning | Why |
|---|---|---|---|
| Setup | this session | n/a | Ledger, stub, kit copy |
| 1.1 path and bands | Sonnet | medium | Pure algorithm, fully pinned, with coverage tests |
| 1.2 liquid shapes | Sonnet | medium | A port of pinned JS with exact numbers |
| 1.3 G1 frames | Sonnet | medium | A port of pinned JS with exact numbers |
| 1.4 Haze on a path | Sonnet | medium | Small API, but femtovg must be read first; the nest checks it |
| 1.5 spike and gate | Sonnet | medium | Guarded nest and shots; the session reads every crop before the author does |
| 2 SheetMotion | Sonnet | medium | Clock swap with tests given; callers mechanical |
| 3 one glass shape | Sonnet | high | Draw order and hand-over across three crates, on top of `CEL-D-014` and `CEL-D-015` |
| 4 blur region | Sonnet | medium | Wiring plus coverage tests |
| 5 pill and mid-drop switches | Sonnet | high | State interplay with `CEL-D-014`'s pill and `CEL-D-015`'s travel |
| 6 nest and docs | Sonnet | medium | Guarded bursts, smoke, evidence |
| `/simplify` | this session | n/a | In session, no subagents |
| Final review | Opus | high | `/code-review`, a model other than the implementers'; at most 2 rounds |

## Amendment (2026-11-27): drop D1 replaces G1

### Decision

- 2026-11-27 the author saw G1 in the nest (Task 1.5) and turned it down: "no me gusta el estilo, quiero que parezca mas una gota, parece como una pelota con protuberancias raras".
- Preview the drop preview, version 4 (private artifact https://claude.ai/artifact/1QBmyjzDb7p1gbbQKU6ZXw, version "v4.2"; source `cel-d/preview-approved/src17.html` in the notes) offered D1, a hanging drop, and D2, a heavy drop. **The author chose D1** the same day.
- What changes: the drop is ONE continuous liquid contour, built from a half-width profile mirrored around the piece's centre, instead of a rounded rect + neck + tabs + tips + fillets. The bulb is round (radius 12). On open it hangs, stretches a thin thread that pinches near the bulb, the bulb rounds off and opens into the window. On close the window gathers into the round bulb, which rises, meets a cone the capsule reaches down, merges and is absorbed.
- What stays: everything outside `sheet/liquid.rs` and `sheet/drop.rs`. Task 1.1 (`ShapePath`, `slices_of_path`) and Task 1.4 (`haze_path`) stay as they are. Tasks 2 to 6 keep their text, with these substitutions:
  - `DropFrame.join: Join` becomes `DropFrame.liquid: Option<Liquid>` plus `DropFrame.drawn: bool` (whether `rect`/`radius` is painted as a rounded rect; a liquid-only frame carries the bulb's box in `rect` for clips and travels, with `drawn = false`). "Join::Detached" in later tests reads "`liquid` is `None`".
  - `MERGE_AT` is **0.8** (the approved preview's `MERGE.d1`), so the pill returns at 320 ms of the close (Task 5's test: Some at 319 ms, None at 320 ms; Task 2's `merged`: false at 319, true at 320).
  - `OPEN_MS` 460, `CLOSE_MS` 400 and `OPEN_HOLD` 0.52 are unchanged.
  - `Source { cx, w }` stays; D1 does not read `w` (the bulb is 12 px whatever the piece's width). Keep the field.
- The repository's design (`docs/superpowers/specs/2026-11-27-celestina-menu-drop-design.md` §5.1–§5.3) and plan still describe G1; they are amended by a docs unit after this one (the evidence records this amendment and the approval).

### Task 1.2b + 1.3b: port D1 (replaces the code of Tasks 1.2 and 1.3)

**Files:** rewrite `celestina/src/sheet/liquid.rs` and `celestina/src/sheet/drop.rs` (tests in each file). Keep `geometry::CAPSULE_BOTTOM`.

**The source to port, exactly (mode `d1` only; `V4.d1` values; `y0` = `CAPSULE_BOTTOM`; the window radius 20):** the JS below, from the approved preview. `bez`, `ONEUI`, `INOUT`, `EIN`, `OUTBACK` map to `celestina_ui::{CubicBezier, ONE_UI, SINE_IN_OUT_80, Easing::OutBack(overshoot)}` and the local `RISE = CubicBezier(0.4, 0.0, 0.9, 0.6)`; `OUTBACK` here uses `theme.overshoot` (1.15) passed in as before. `f2` (2-decimal rounding) is SVG only: do not port it.

**Interfaces:**
- `liquid.rs`:
  - `#[derive(Clone, Copy, Debug, PartialEq)] pub struct Bulb { pub cy: f32, pub rx: f32, pub ry: f32 }`
  - `#[derive(Clone, Copy, Debug, PartialEq)] pub struct Liquid { pub cx: f32, pub y0: f32, pub a: f32, pub m: f32, pub yw: f32, pub f: f32, pub bulb: Option<Bulb> }` (`f` is `o.f ?? 7`: 7 on open, 6 on close; `k1` = 3 and `k2` = 4 are constants)
  - `pub fn smax(a: f32, b: f32, k: f32) -> f32`
  - `pub fn half_width(l: &Liquid, y: f32) -> f32` = `profileW`
  - `pub fn contour(path: &mut ShapePath, l: &Liquid)` = `liquidPath`: the same y samples (0.5 px steps from `y0 − 2` to `bot`, plus 65 samples `cy + ry·sin(−π/2 + π·i/64)` when there is a bulb, plus `bot`, sorted), the same runs where `w > 0.05`, each run emitted as one closed polygon: right side down (`cx + w`), left side up (`cx − w`), `close()`. Runs of fewer than 2 samples are skipped. Clockwise on screen.
  - Non-finite input adds nothing.
- `drop.rs`: `OPEN_MS`, `CLOSE_MS`, `OPEN_HOLD`, `MERGE_AT = 0.8`, `Source` (+ `Source::of`), `DropFrame { rect, radius, drawn, liquid: Option<Liquid>, glass, content, blur }`, `DropFrame::window(rect, radius)` (drawn, no liquid, glass 1, content 1, blur 0), `open_frame(t, to, src, overshoot)`, `close_frame(t, from, src)`, `shape(capsule: Option<(Rect, f32)>, frames: &[(DropFrame, Source)]) -> ShapePath`: capsule rounded rect, then per frame its rounded rect when `drawn`, then `liquid::contour` when `liquid` is `Some`. `Join` and the old `neck`, `neck_into`, `tip`, `merge_fillet` go.
  - The liquid-only frames carry `rect` = the bulb's box and `radius` = `min(rx, ry)`, `drawn = false` (the preview's `noRect`).
  - Clamp `t` to 0..1, NaN `t` counts as 1, sanitise a non-finite `Source`/rect as the current `drop.rs` does.

**Tests** (sentence names; piece `cx 1800`, `w 28`; `y0` 48; window `R = (1500, 56, 320, 200)`; overshoot 1.15):
```rust
// liquid.rs
#[test] fn smax_is_max_away_from_k_and_rounds_within_it()          // smax(5,0,3)=5; smax(1,1,4)=2
#[test] fn a_contour_is_closed_clockwise_polygons_one_per_run()     // pendant frame: 1 sub-path, area > 0; pinched frame: 2 sub-paths
#[test] fn the_half_width_has_no_step_anywhere()                    // for open t in 0..0.52 step 0.01 and close t in 0.42..1 step 0.01: |w(y+0.05) − w(y)| ≤ 0.6 for every y from y0−2 to bot (the author's "protuberancias": no jump)
#[test] fn nothing_is_drawn_above_a_bulb_that_has_not_reached_the_edge() // close t 0.55: half_width(y0+0.5) ≤ 0.05 (no nub on the edge)
#[test] fn non_finite_inputs_add_nothing()
// drop.rs
#[test] fn the_drop_swells_from_a_3_px_bulb_on_the_edge()           // t 0: liquid a 3, m 3, bulb {cy 46.5, rx 3, ry 3}; drawn false; content 0, blur 12
#[test] fn the_bulb_hangs_round_at_the_end_of_the_swell()           // t 0.28: bulb {cy 59.4, rx 12, ry 12}, a 8, m 6.5
#[test] fn the_thread_thins_then_pinches_into_two_pieces()          // some t in (0.28,0.52) with m in (0,1.5]; the first t with m < 0 → contour has 2 sub-paths
#[test] fn the_open_starts_from_the_round_bulb_and_lands_on_the_window() // t 0.52: drawn rect (1788, 64, 24, 24) r 12, liquid bulb None, a 1.5; some t with rect.w > 320; t 1: R, r 20, content 1, blur 0, liquid None
#[test] fn the_open_content_fades_in_from_60_percent()               // as before
#[test] fn the_close_gathers_into_a_round_bulb_16_px_under_the_edge() // t 0.42 (rise start): liquid bulb {cy 76, rx 12}, drawn false; t 0.30: drawn rect between R and (1788,64,24,24)
#[test] fn the_capsule_reaches_down_and_the_bulb_merges()           // a rise frame with gap < 12 has a > 0 and m < 0; a later one with gap < 2 has m > 0 and 1 sub-path
#[test] fn the_capsule_absorbs_the_drop()                           // t 1: shape below y0 is empty (half_width(y) ≤ 0.05 for y ≥ y0)
#[test] fn the_close_follows_the_live_piece()                        // same t, cx 1700 vs 1800: bulb box x differs by 100
#[test] fn a_degenerate_piece_never_yields_non_finite_shapes()
#[test] fn shape_unites_the_capsule_and_the_drop_in_one_path()       // contains points in the capsule, the thread and the bulb at open t 0.34
#[test] fn the_open_matches_the_preview_at_its_frames()             // compare open/close frames at t ∈ {0.08,0.18,0.28,0.34,0.39,0.43,0.46,0.49,0.52,0.6} and close {0.3,0.45,0.55,0.63,0.7,0.76,0.82,0.88,0.94,0.99} against the numbers printed by running the preview JS with node (see below): every field within 1e-3
```
For the last test, generate the expected numbers with node from the JS below (a throwaway script in the scratchpad, not committed: `const window={}` stub; print `JSON.stringify` of each frame), and write them as literals in the test.

**Commits:** `celestina-maintenance: Replace the drop's neck shapes with one liquid contour` (liquid.rs) and `celestina-maintenance: Replace drop G1's frames with drop D1` (drop.rs), or one commit with the second subject if they cannot compile apart.

### The JS (verbatim from the approved preview)

```js
function bez(x1,y1,x2,y2){const cx=3*x1,bx=3*(x2-x1)-cx,ax=1-cx-bx,cy=3*y1,by=3*(y2-y1)-cy,ay=1-cy-by;
 const sx=t=>((ax*t+bx)*t+cx)*t,sy=t=>((ay*t+by)*t+cy)*t;
 return x=>{if(x<=0)return 0;if(x>=1)return 1;let lo=0,hi=1,t=x;for(let i=0;i<40;i++){const v=sx(t);if(Math.abs(v-x)<1e-6)break;if(v<x)lo=t;else hi=t;t=(lo+hi)/2}return sy(t)}}
const EXIT=bez(.32,0,.67,0),ONEUI=bez(.22,.25,0,1),INOUT=bez(.33,0,.2,1),EIN=bez(.4,0,.9,.6);
const OUTBACK=t=>{const s=1.15;t-=1;return t*t*((s+1)*t+s)+1};
const lerp=(a,b,t)=>a+(b-a)*t,clamp=(v,a=0,b=1)=>Math.max(a,Math.min(b,v));
const lerpR=(a,b,k)=>({x:lerp(a.x,b.x,k),y:lerp(a.y,b.y,k),w:lerp(a.w,b.w,k),h:lerp(a.h,b.h,k)});
const f2=v=>Math.round(v*100)/100;
// ---------- drop v4: one continuous liquid contour ----------
// The shape under the edge y0 is a half-width profile w(y), mirrored around cx: the edge
// fillet, the thread (a at the edge, waist m at yw, then widening into the bulb) and the
// bulb (an ellipse), joined by smooth maxima so every join is a liquid fillet. Each term is
// continuous in y, so the outline has no step. A negative waist splits the thread into two
// cones that retract: the pinch-off.
const smax=(a,b,k)=>{if(k<.01)return Math.max(a,b);const h=Math.max(k-Math.abs(a-b),0)/k;return Math.max(a,b)+h*h*k/4};
function profileW(o,y){const {y0,a,m,yw}=o,aa=Math.max(a,0),f=Math.min(o.f??7,aa);
 let e;if(aa<.05)e=-1e9;else if(y<=y0)e=aa+f;else if(y<y0+f){const d=y-(y0+f);e=aa+f-Math.sqrt(Math.max(0,f*f-d*d))}else e=aa-6*(y-y0-f);
 let th;const b=o.bulb;
 if(y<=yw){const s=Math.min(1,(yw-y)/Math.max(yw-y0,.5));th=m+(a-m)*s*s}
 else if(b&&y<b.cy){const s=(y-yw)/Math.max(b.cy-yw,.5);th=m+(b.rx*.9-m)*s*s}else th=-1e9;
 let w=smax(e,th,o.k1??3);
 if(b){const d=(y-b.cy)/b.ry,bw=Math.abs(d)<1?b.rx*Math.sqrt(1-d*d):0,kb=(o.k2??4)*clamp((b.cy-y)/(.5*b.ry))*clamp((y-b.cy+b.ry)/(.3*b.ry));w=smax(w,bw,kb)}
 return w}
function liquidPath(o){const top=o.y0-2;let bot=o.y0+Math.max(o.f??7,2);
 if(o.bulb)bot=Math.max(bot,o.bulb.cy+o.bulb.ry);else bot=Math.max(bot,o.yw+1);
 const ys=[];for(let y=top;y<=bot;y+=.5)ys.push(y);
 if(o.bulb)for(let i=0;i<=64;i++)ys.push(o.bulb.cy+o.bulb.ry*Math.sin(-Math.PI/2+Math.PI*i/64));
 ys.push(bot);ys.sort((p,q)=>p-q);
 const runs=[];let cur=null;
 for(const y of ys){if(y<top||y>bot)continue;const w=profileW(o,y);
  if(w>.05){if(!cur){cur=[];runs.push(cur)}cur.push([y,w])}else cur=null}
 let d='';for(const r of runs){if(r.length<2)continue;
  d+=`M${f2(o.cx+r[0][1])},${f2(r[0][0])}`;for(const [y,w] of r.slice(1))d+=`L${f2(o.cx+w)},${f2(y)}`;
  for(const [y,w] of r.slice().reverse())d+=`L${f2(o.cx-w)},${f2(y)}`;d+='Z'}
 return d}
// D1 (gota colgante) and D2 (gota pesada): frames of t; the dropOpen/dropClose contract plus `liquid`.
const V4={
 d1:{R:12,fall:16,thin:.55,pinch:.78,stretch:.10,openAt:.52,thread:.72,open:460,close:400},
 d2:{R:13,fall:22,thin:.6,pinch:.85,stretch:.2,openAt:.58,thread:.62,open:540,close:440}};
function dropOpenV4(mode,t,Rw,P,y0){const c=V4[mode],cx=P.cx,R=c.R,cyEnd=y0+c.fall+R;
 if(t<.28){const u=ONEUI(t/.28),r=lerp(3,R,u),cy=y0+lerp(-.5,.95,u)*r;
  return{liquid:{cx,y0,a:lerp(r,8,u),m:lerp(r,6.5,u),yw:y0+(cy-r-y0)*.5,bulb:{cy,rx:r,ry:r}},rect:{x:cx-r,y:cy-r,w:2*r,h:2*r},r,noRect:true,co:0,blur:12}}
 if(t<c.openAt){const u=INOUT((t-.28)/(c.openAt-.28)),cy=lerp(y0+.95*R,cyEnd,u),el=c.stretch*Math.sin(Math.PI*clamp(u/c.pinch*.9));
  const ry=R*(1+el),rx=R*(1-el*.5),yw=y0+Math.max(0,cy-ry-y0)*c.thread;
  const m=u<c.thin?lerp(6.5,1.4,ONEUI(u/c.thin)):u<c.pinch?lerp(1.4,0,(u-c.thin)/(c.pinch-c.thin)):-lerp(0,10,(u-c.pinch)/(1-c.pinch));
  const a=u<c.pinch?lerp(8,4,u/c.pinch):lerp(4,1.5,(u-c.pinch)/(1-c.pinch));
  return{liquid:{cx,y0,a,m,yw,bulb:{cy,rx,ry}},rect:{x:cx-rx,y:cy-ry,w:2*rx,h:2*ry},r:Math.min(rx,ry),noRect:true,co:0,blur:12}}
 const u=clamp((t-c.openAt)/(1-c.openAt)),k=OUTBACK(u),B={x:cx-R,y:cyEnd-R,w:2*R,h:2*R},yw=y0+c.fall*c.thread;
 const a=lerp(1.5,0,clamp(u/.2)),o={rect:lerpR(B,Rw,k),r:lerp(R,20,clamp(k)),co:clamp((t-.6)/.34),blur:12*(1-ONEUI(u))};
 if(a>.05)o.liquid={cx,y0,a,m:-10-40*u,yw,bulb:null};return o}
function dropCloseV4(mode,t,R0,P,y0){const c=V4[mode],cx=P.cx,R=c.R,cyStart=y0+c.fall+R;
 if(t<.42){const u=ONEUI(t/.42);return{rect:lerpR(R0,{x:cx-R,y:cyStart-R,w:2*R,h:2*R},u),r:lerp(20,R,u),co:1-clamp(t/.28),blur:12*Math.pow(clamp(t/.3),.7)}}
 const u=EIN((t-.42)/.58),cy=lerp(cyStart,y0-R*.7,u),r=lerp(R,R*.5,clamp((y0+R-cy)/(2*R))),g=cy-r-y0;
 const chord=Math.abs(cy-y0)<r?Math.sqrt(r*r-(cy-y0)**2):0;
 let a,m;if(g>2){a=lerp(0,4,clamp((12-g)/9));m=-(g-2)*.9}else{const s=clamp((2-g)/5);a=Math.max(lerp(4,r*.9,s),chord*.95);m=lerp(0,r*.75,s)}
 a*=1-clamp((t-.92)/.08);
 return{liquid:{cx,y0,a,m,yw:y0+Math.max(g,0)*.5,bulb:{cy,rx:r,ry:r},f:6},rect:{x:cx-r,y:cy-r,w:2*r,h:2*r},r,noRect:true,co:0,blur:12}}

```
