# CEL-D-015 Implementation Plan: the travelling switch

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A switch between two menus of one capsule keeps one window whose rect travels to the new menu's shape in 300 ms while the old content fades and blurs out and the new one fades in sharpening; a menu on another capsule opens at once while the old one closes, with no queue.

**Architecture:** The window's clocks gain a third flight, `Travel`, in `SheetMotion`: the rect, radius and glass go from what was shown to the new target on ONE_UI, and the motion owns both contents' opacity and blur. `Sheets` keeps the content being left on the open window (`OldBody`) instead of pushing a second window into `leaving`; `leaving` stays only for a window closing on another capsule, and a request on a capsule whose window is still in `leaving` takes that window back and travels from it. `Island::open_sheet_at` queues only behind the Island page.

**Tech Stack:** Rust, `celestina-ui` (`ONE_UI`, `Easing`, `Tree`), `celestina-paint` (`Painter::haze`, `Painter::blurred`), Celestina's sheet and Island modules.

**Spec:** `docs/superpowers/specs/2026-11-27-celestina-menu-drop-design.md` §5.5 (the switch), §5.7 (reduced motion), §6 (calls: the queue between a capsule menu and the Island page stays; the new menu takes input at once). Approved preview: "Celestina · la gota", version 3, approved by the author on 2026-11-27 (private artifact; source `cel-d/preview-approved/src16.html` in the sessions' notes, `DropStage` `travel` frame, line 342). Code map: `cel-d/d014-015-map.md`, part B (§2 and §3).

**Branch:** `unit/celestina/CEL-D-015`, opened with `scripts/worktree.sh open celestina CEL-D-015 --from unit/celestina/CEL-D-014` (stacked: CEL-D-014 removes the slot collapse and the piece's crossfade and adds the open pill that slides on the bar's spring; this plan assumes them).

## Global Constraints

- Same capsule: the window stays. Rect `lerp(shown, live target, ONE_UI(t))` over 300 ms (`TRAVEL_MS = 300`, new constant in `sheet/motion.rs`, cut to `ladder.ceiling` like `CLOSE_MS`; `tokens.toml` has no 300 ms step, so no token is added). Radius from the shown radius to `theme.radius_lg` and glass opacity from the shown glass to 1, both on ONE_UI(t). No drop, no neck (CEL-D-013 brings those later).
- Old content: opacity `a0 · (1 − t / 0.3)`, blur `max(b0, 8 · t / 0.3)` px; gone at `t = 0.3` (90 ms), then released. `(a0, b0)` is what that content showed at the switch frame: `(1, 0)` from rest, which gives the spec's `1 − t/0.3` and `8 · t/0.3` exactly.
- New content: opacity `clamp((t − 0.2) / 0.5)`; blur `8 · (1 − clamp((t − 0.2) / 0.6))` px, so 8 px until `t = 0.2` and 0 at `t = 0.8` (the preview's `travel` frame).
- Each content sits at its own target's origin minus its own scroll, clipped to the travelling rect and radius: the rect reveals it, never scales or carries it.
- Input: the new body takes input from the switch frame, at its target's coordinates, hit-tested on the travelling rect; the old content takes none.
- Another capsule: the new menu opens at once (today's open: 350 ms OutBack 1.15 from its piece; the drop comes with CEL-D-013) while the old one closes in `leaving` (today's 260 ms close towards its own piece). `Sheets::queue` is used only between a capsule menu and the Island page (both directions).
- A switch during any flight (the open, the close, a travel, a window in `leaving` on the requested capsule) starts from the rect, radius, glass and content shown at that frame.
- Two travels in a row keep one old content: whichever of the two contents shows the higher opacity at the second switch's frame (ties: the newer); the other is flushed and dropped.
- Reduced motion (§5.7): instant swap. At the switch frame the rect, radius and glass are at their ends, the new content is at opacity 1 and blur 0, the old content is dropped. Another capsule under reduced motion: today's reduced open and close.
- The open pill moves to the new piece on CEL-D-014's slide (350 ms OutBack); this unit adds no bar code.
- At rest no frames are requested; no `unwrap`/`expect` in production; no `unsafe` outside `egl.rs`/`pam.rs`; product copy only in `src/copy.rs`; never touch `src/lock`, `src/polkit`, `celestina/scripts/`, `.githooks`, any `AGENTS.md`, `docs/contracts|governance|standards`, or the root `docs/` (the suite docs track owns the specs).
- Nested runs only through the guarded kit (`nest_guard`), `env -i`, `--own-bus`, `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`, fake `wpctl`/`pactl`, a private Cargo target, one nest at a time, `timeout -k 60 2700` (never `-s KILL`, never `pkill` by name). Never run a `celestina` binary or `niri msg` from the agent's own shell.

## Review Focus

1. Back to a capsule whose window is still closing in `leaving` (volume, then the bell, then volume again within 260 ms): one window on that capsule, travelling from the closing one's shape; never two. Test in Task D15.2.
2. The new body changes size during the travel (a page sheet rebuilt, a workspaces list that grows): the travel carries the live target and ends on it at 300 ms, without restarting on the 200 ms resize morph. Tests in D15.1 and D15.2.
3. A press during the travel where only the old content had a row: no action of the old body, the window stays open; a press on a new row inside the travelling rect acts at once. Test in D15.2.
4. The old content released at 90 ms (its tree flushed, no hover left behind) and no frame asked after 300 ms. Tests in D15.1 and D15.2.
5. Reopening the same piece during its close: the window turns back from the shape shown, its glass returns to 1 on the travel, nothing waits. Test in D15.2.

---

### Step 0 (controller, before Task D15.1)

- [ ] Open the worktree: `scripts/worktree.sh open celestina CEL-D-015 --from unit/celestina/CEL-D-014` (from the canonical checkout).
- [ ] Add the ledger row in `celestina/docs/plans/active/2026-11-25-cel-tracks.md` after `CEL-D-014`'s: `| CEL-D-015 | \`celestina:\` | active | pending | pending | Replace the switch between two menus of one capsule with one window that travels to the new menu's shape while its content changes, and open a menu on another capsule at once while the old one closes (the author's request of 2026-11-27; la gota preview version 3 approved 2026-11-27) | [evidence](../../evidence/2026-11-27-cel-d-015-travelling-switch.md) | VAL-CEL-D-015 |`.
- [ ] Create the evidence stub `celestina/docs/evidence/2026-11-27-cel-d-015-travelling-switch.md` (title, links to the spec and this plan, empty sections Procedure, Results, Limits; Task D15.3 fills it).
- [ ] Commit: `celestina-maintenance: Add the CEL-D-015 ledger row and evidence stub`.

---

### Task D15.1: the travel clocks

**Files:**
- Modify: `celestina/src/sheet/motion.rs` (a `Kind::Travel` flight and its getters)
- Test: the same file's `#[cfg(test)] mod tests`, in its style (sentence names, `ms(t0, n)`, `near`, `near_rect`, `opened`, `closing`)

**Interfaces:**
- Consumes: `celestina_ui::ONE_UI`, `celestina_paint::Theme` (`radius_lg`, `motion`, `overshoot`).
- Produces:
  - `pub const TRAVEL_MS: u64 = 300;`
  - `#[derive(Clone, Copy, Debug, PartialEq)] pub struct Shown { pub alpha: f32, pub blur: f32 }`: a content as it was shown.
  - `pub fn travel(&mut self, shown: Rect, old: Shown, theme: &Theme, now: Instant)`: from any kind (open flight, at rest, a resize, a close, a travel) the motion becomes a `Travel` flight from `shown`, the radius `self.radius(now)` → `theme.radius_lg`, the glass `self.glass_opacity(now)` → 1, the old content from `old`. Under reduced motion the flight has zero duration.
  - `pub fn content_shown(&self, now: Instant) -> Shown`: the open body as painted now: `(body_opacity, blur_px)` while closing, `(body_opacity, content_blur_px)` otherwise.
  - `pub fn content_blur_px(&self, now: Instant) -> f32`: the new content's own blur over a travel (formula above); 0 for every other kind and under reduced motion.
  - `pub fn old_content(&self, now: Instant) -> Option<Shown>`: `Some` while a travel runs and `t < 0.3`, else `None`.
  - Changed for `Travel`: `rect` (ONE_UI flight to the live `to`, then the resize rule as for an open at rest), `radius`, `glass_opacity`, `body_opacity` (`clamp((t − 0.2)/0.5)`), `blur_px` (0), `blur_region` (true), `is_closing` (false), `resize` (ignored while the travel flies: the flight carries the target), `wants_frame` (the flight).

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn a_travel_morphs_the_rect_on_one_ui_in_300_ms()
// opened, at rest; travel(shown = target(), Shown{alpha:1,blur:0}, …) towards to = r(1400,56,360,300):
// +0: rect == target(); +150: near_rect(rect, lerp_rect(target(), to, ONE_UI.eval(0.5))); +300: rect == to;
// radius == theme.radius_lg throughout; glass_opacity == 1; wants_frame(+299) true, (+300) false
#[test] fn the_old_content_fades_and_blurs_out_by_90_ms()
// old_content: +0 (1.0, 0.0); +45 (0.5, 4.0); +89 Some; +90 None
#[test] fn the_new_content_fades_in_from_60_ms_and_is_sharp_at_240_ms()
// body_opacity: +0 0, +60 0, +90 0.2, +150 0.6, +210 1.0; content_blur_px: +0 8, +60 8, +90 6.667, +150 4, +210 1.333, +240 0, +300 0
#[test] fn a_travel_during_the_open_starts_from_the_shape_shown()
// open from pill(), travel at +100: rect(+100) == rect the open showed at +100; radius(+100) == open radius at +100 (below radius_lg); radius_lg at +400
#[test] fn a_travel_during_a_close_turns_the_window_back()
// close at done, travel at done+130 with old = content_shown(done+130) of the close:
// old_content(+0) == (1 − 0.625^1.4, 12·(0.5/0.6)^0.7); glass(+0) ≈ 1 − 0.5^1.6; glass(+300) == 1;
// is_closing() false; rect(+0) == the close's rect at done+130; blur_region true
#[test] fn the_old_content_keeps_a_blur_it_already_had()
// travel with old = Shown{alpha:0.4, blur:10}: old_content(+0) == (0.4, 10); (+45) == (0.2, 10)
#[test] fn a_resize_during_a_travel_is_carried_by_the_travel()
// resize(…) at +100 changes nothing; with a new `to` the rect at +300 is that `to`; no frame asked after +300
#[test] fn reduced_motion_swaps_at_once()
// reduced: travel → rect == to, radius_lg, glass 1, body_opacity 1, content_blur_px 0, old_content None, wants_frame false, all at +0
```

Existing tests that stay as they are: every test in `motion.rs` today, including `a_resize_at_rest_morphs_on_the_normal_motion` (200 ms is still the resize at rest) and `a_switch_closes_the_old_window_the_same_way` (a window left on another capsule still closes with `close`; rename it `a_window_left_on_another_capsule_closes_the_same_way` and its comment).

- [ ] **Step 2: Run them to make sure they fail**

Run (from `celestina/`): `cargo test -p celestina sheet::motion`. Expected: FAIL (no `travel`, `Shown`, `content_blur_px`, `old_content`).

- [ ] **Step 3: Implement** `Kind::Travel` and the interface above in `celestina/src/sheet/motion.rs`. `flight` takes the travel's duration `TRAVEL_MS.min(ceiling)` (0 when reduced) and `Easing::Bezier(ONE_UI)`; the motion stores the glass and old content it started from (`from_glass: f32`, `old: Shown`). Update the module doc and `body_opacity`'s doc.

- [ ] **Step 4: Run the tests**: `cargo test -p celestina sheet::motion`. Expected: PASS.

- [ ] **Step 5: Commit** `celestina-maintenance: Add the travel clocks of a menu window switching on its capsule`.

---

### Task D15.2: one window travels on its capsule, another capsule never waits

**Files:**
- Modify: `celestina/src/sheet/mod.rs` (`OpenSheet.old`, `Leaving.zone`, `Sheets::open`, `reap`, `layout`, `paint`, `drop_open`, `wants_frame`, the layer helper, docs at the top of the module and of `open`/`close`)
- Modify: `celestina/src/island/sheets.rs` (`open_sheet_at`: the `switchable` test and the sheet-over-sheet queue go; module and function docs)
- Test: `celestina/src/island/tests/window.rs`, `celestina/src/island/mod.rs` tests, `celestina/src/island/tests/page_sheets.rs`, `celestina/src/island/tests/subtitle_gestures.rs` (only if CEL-D-014 kept the test below), `sheet/mod.rs` unit tests

**Interfaces:**
- Consumes: Task D15.1's `TRAVEL_MS`, `Shown`, `travel`, `content_shown`, `content_blur_px`, `old_content`; CEL-D-014's open pill (read its plan's Interfaces block on `unit/celestina/CEL-D-014` for the accessor's name).
- Produces:
  - `struct OldBody { body: SheetBody, scroll: f32, target: Rect }` and `OpenSheet.old: Option<OldBody>`; `Leaving.zone: usize`.
  - `Sheets::open(request, anchor, theme, now, reduced)` (signature unchanged), in this order:
    1. an open window on the request's zone (open or closing): `travel` in place. `shown = o.motion.rect(now, live end, surface)`; the content kept as old is the one of `o.old` / `o.body` with the higher opacity now (`old_content` vs `content_shown`; ties: the body); the other is flushed and dropped; the new body takes `o.body`; `zone/piece/kind/anchor/target/size` are the request's; scroll reset (the old keeps its own in `OldBody.scroll`).
    2. a window in `leaving` on the request's zone: take it out of `leaving`, make it the open sheet travelling from its shown rect with its body as old content. If another window is open (other zone), it goes to `leaving` first as in 3.
    3. an open window on another zone: it goes to `leaving` with `close(shown, its pill, now)` (or its running close if it was already closing), its `zone` kept, its `old` flushed and dropped; then a fresh open as today.
    4. nothing open: a fresh open as today.
  - `reap` drops `o.old` (flushed) once `o.motion.old_content(now)` is `None`.
  - `paint`: a closing open window and every `leaving` window keep `paint_window` as today. An open window paints its glass once at the travelling rect (`haze_at`), then the old content (if `old_content` is `Some`) and the new body, each through `fn paint_content(painter: &mut Painter, rect: Rect, radius: f32, at: (f32, f32), alpha: f32, blur: (Rect, f32), body: &mut dyn FnMut(&mut Painter))`: plain `paint_body` below 0.05 px, else opacity, then `push_clip(rect, radius)` around `painter.blurred(layer, sigma, …)` that paints the body at `at` (check in `celestina-paint/src/painter.rs` that the clip applies to the composited layer; if it does not, clip inside the closure and say so in the task report).
  - `fn cover(rects: &[Rect], surface: (f32, f32)) -> Rect`: the union plus `BLUR_REACH`, cut to `LAYER_GRID` and the surface; `blur_layer` uses it for `[from, pill, capsule]` and the travel for `[old.target, new target]`, so the layer keeps its size through a travel.
  - `#[cfg(test)] pub fn is_swapping(&self) -> bool` (an old content is held); `is_leaving`, `leaving_pill`, `leaving_pieces`, `on_leaving`, `follow_leaving` stay for the other-capsule case.
  - `Island::open_sheet_at`: `if page_shown { close the page; close_sheet; queue(Next::Sheet) } else { page_sheet_ends(); sheets.open(…) }`. `Island::open(page)` with a sheet shown still queues `Next::Page`.

- [ ] **Step 1: Write the failing tests** (rewritten ones keep their file; `bars` = capsule shapes, as in `window.rs`)

```rust
// island/tests/window.rs
#[test] fn a_switch_on_the_same_capsule_keeps_one_window_that_travels()
// replaces a_switch_closes_the_old_window_into_its_piece_while_the_new_one_grows (116):
// settled volume, open_level_sheet_at(brightness) at t: !is_leaving(); windows().len() == 1 and shapes == bars + 2 at +0, +50, +150, +299;
// window rect at +0 == the volume window's shown rect; at +300 == the brightness target (shown_rect at +900); radius == radius_lg at +300
#[test] fn only_the_new_body_takes_input_during_a_travel()
// replaces only_the_new_window_takes_input_during_a_switch (155), Review Focus 3: network page sheet, switch to bluetooth (same capsule);
// at +30 a press where the network body had a row and the bluetooth body has none: no network action, is_open();
// a press on a bluetooth row inside the travelling rect acts at +30; a press outside the rect closes
#[test] fn the_old_content_is_released_at_90_ms_and_nothing_runs_at_rest()
// replaces after_the_close_no_region_of_the_old_window_is_left (181), Review Focus 4: is_swapping() at +89 true, after step at +90 false;
// wants_frame(+299) true; after step at +300 false; shapes == bars + 2 throughout
#[test] fn a_second_switch_during_a_travel_travels_from_the_shape_shown()
// replaces a_second_switch_within_260_ms_keeps_the_first_window_closing (334): back to volume at +60:
// rect right after == rect shown at +60 before it; one window; the old content's opacity at +60 == max(first old's, new body's) at +60; volume target at +360
#[test] fn a_press_on_another_capsule_opens_its_menu_at_once_while_the_old_closes()
// replaces a_press_on_another_capsule_closes_then_opens (93): click the bell: is_open(), piece() == Some(bell), page() == Some(NOTIFICATIONS), !has_queued(), is_leaving()
// leaving_pill() == the volume piece's pill; windows().len() == 2 at +50; !is_leaving() after step at +260
#[test] fn going_back_to_a_capsule_whose_window_still_closes_travels_from_it()
// Review Focus 1: volume open; bell at t; volume again at t+100: leaving_pieces(t+100) == [bell]; one window on the volume's capsule whose rect at t+100 == the closing volume window's rect at t+100
#[test] fn reopening_during_a_close_travels_from_the_shape_shown()
// Review Focus 5 (replaces crossfade.rs reopening_during_a_close_hands_over if CEL-D-014 kept it): close_sheet at t; volume asked at +130:
// is_open(), !has_queued(), rect continuous within 0.5 px, shapes == bars + 2 (the window is back in the blur region), glass reaches 1 by +430
#[test] fn the_open_pill_moves_with_a_switch()
// with CEL-D-014's accessor: the pill is on brightness after a volume → brightness switch, and on the bell's piece after the other-capsule open; skip if CEL-D-014 already has this test (cite it in the evidence)
#[test] fn reduced_motion_swaps_at_once()
// set_reduced_motion(true); switch: rect == brightness target at once, !is_swapping(), !wants_frame after one step

// island/mod.rs tests
// a_second_piece_of_the_same_capsule_switches_in_place (6002): is_leaving() is false at every frame; is_swapping() == (t < 90)
// switching_the_volume_sheet_to_the_brightness_sheet_morphs_in_place (5744): stays; add !is_leaving()
#[test] fn a_body_that_changes_size_during_a_travel_ends_on_its_size()
// Review Focus 2: workspaces or a page sheet switched in, its body rebuilt taller at +100 (refit_page_sheet / a model change): rect at +300 == the new target; no frame asked at +301

// island/tests/page_sheets.rs
// a_piece_asking_for_its_page_again_closes_the_sheet_and_another_switches_it (489): line 511 becomes is_swapping() and !is_leaving() ("one window, the old body fading")

// island/tests/subtitle_gestures.rs (if CEL-D-014 left it)
// the_leaving_window_keeps_its_own_pill (64): rebuild on an other-capsule open (volume → bell); leaving_pill() follows the volume piece

// sheet/mod.rs unit tests
#[test] fn the_travel_layer_covers_both_bodies_on_the_grid()
// cover([old target, new target]) contains both grown by 36 px, edges multiples of 32, inside the surface; blur_layer's results unchanged
```

Tests that stay as they are (the queue behind the Island page): `island/mod.rs` `a_queued_workspaces_sheet_opens_with_the_list_of_that_moment`, `a_bar_piece_while_open_switches_the_page_or_acts_and_closes_it`, `the_window_never_covers_the_bar_and_takes_only_its_own_input`, `the_island_capsule_while_a_sheet_is_open_closes_it_then_opens_the_page`, `a_volume_click_while_a_page_is_open_closes_the_page_then_opens_the_sheet`, `under_reduced_motion_what_waited_opens_on_the_same_input`, `escape_and_focus_loss_drop_what_waited_behind_a_closing_sheet`, `the_keyboard_dropping_does_not_drop_the_sheet_that_waits_for_the_page`, `a_sheet_opened_in_the_gap_after_a_close_does_not_swallow_the_waiting_page`; `page_sheets.rs` `island_open_still_opens_the_island_page_with_its_tabs`, `a_page_sheet_that_waited_for_the_island_page_is_built_again_when_it_opens`, `the_catcher_exists_exactly_while_something_is_shown`, `a_phone_sheet_that_waits_is_given_up_when_the_duty_goes_off`; `window.rs` `a_press_on_another_piece_of_the_capsule_switches`, `a_press_on_a_window_still_closing_does_nothing`, `asking_for_the_open_piece_again_closes_its_sheet`. Run them unchanged; any that fails is a defect of this task, not a test to edit.

- [ ] **Step 2: Run them to make sure they fail**: `cargo test -p celestina island::tests::window island::tests::page_sheets sheet::`. Expected: FAIL (no `is_swapping`, two windows on a switch, `has_queued()` true for the bell).

- [ ] **Step 3: Implement** the interface above. Delete the sheet-over-sheet branch of `open_sheet_at` and the `switchable` local; keep `open_request` and `open_what_waited` (the page wait). `layout` lays out `o.old`'s body too; `sync` never syncs it; `drop_open` drops it. Rewrite the docs that say a switch closes the old window into its piece (`sheet/mod.rs` module doc, `Leaving`, `open`, `windows`, `paint`; `island/sheets.rs` module doc and `open_sheet_at`).

- [ ] **Step 4: Run all checks** (from `celestina/`): `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings` and the same with `+1.100`; `cargo test --workspace --locked` (sum every `test result` line); from the repo root `bash scripts/check-architecture-contract.sh`, `bash scripts/check-documentation-contract.sh`, `python3 scripts/check-language-contract.py`. Expected: all green.

- [ ] **Step 5: Commit** `celestina-maintenance: Replace the two-window switch with one window that travels on its capsule and open another capsule's menu at once`.

---

### Task D15.3: nest check, validation and evidence

**Files:**
- Modify: `celestina/docs/evidence/2026-11-27-cel-d-015-travelling-switch.md` (the stub)
- Modify: `celestina/VALIDATION.md` (new `VAL-CEL-D-015`, format of `VAL-CEL-D-010`; in `VAL-CEL-D-009` and `VAL-CEL-D-010` one line each that their switch steps are superseded by `VAL-CEL-D-015`, history kept)
- Modify: `celestina/README.md` only if it describes the switch (grep "switch" first)

- [ ] **Step 1: Nest check** with the guarded kit: copy `cel-d/kit-recovered/` from the sessions' notes to this session's scratchpad, point `N=` and `R=` in `run.sh` (and the `lib.sh` path in each probe) at the copy and this worktree, lock name `cel-d-015`, run under `timeout -k 60 2700`. Layout: the author's end capsule (system load, battery, network, Bluetooth, volume, brightness, clipboard, energy, session). Probe with `vp move`/`vp click`: network → Bluetooth → volume on one capsule with a burst of shots (grim in a loop) across each 300 ms travel; a switch to the bell (another capsule) with a burst across both windows; volume → bell → volume within 260 ms; a reopen during a close; one window at rest after each. Read every crop: one box on a capsule during a travel, the old content blurred out and the new one sharpening, the pill on the new piece, no window left on a capsule after the other-capsule close.
- [ ] **Step 2: Smoke** `--only bar,osd,notify` through the kit, the way the landing does; record `smoke: OK` or the failure.
- [ ] **Step 3: Write** the evidence (procedure, shots under `celestina/docs/evidence/`, test counts, limits: niri's backdrop blur during the travel is seen only in the nest), `VAL-CEL-D-015` (the author's steps in the real session: switch within the end capsule fast and slowly, switch to the bell's capsule and back, reopen during a close, reduced motion on), and the README lines if any. Run `stop-slop` on the prose.
- [ ] **Step 4: Commit** `celestina-maintenance: Add the CEL-D-015 evidence and VAL-CEL-D-015`.

---

## After the tasks

- `/simplify` on the branch's code, in this session, without subagents; `stop-slop` on the evidence and `VALIDATION.md` text.
- Final review `CEL-D-015.R · Final review`: Opus, high reasoning, `/code-review` on `git diff unit/celestina/CEL-D-014...HEAD`, with this plan and the spec; at most 2 rounds (the second rereads only the first round's fixes).
- When CEL-D-014 has landed: `git fetch` and merge `origin/main` into `unit/celestina/CEL-D-015`. Code under `celestina/src` and `celestina/crates`: ours (first confirm `git diff --stat <CEL-D-014 head> origin/main -- celestina/src celestina/crates` is empty); CEL-D-014's evidence, `Cargo.toml`/`Cargo.lock` version and inventory: theirs; the ledger rows by hand. List conflicts with `git diff --name-only --diff-filter=U`.
- Landing, only on the author's direct authorization, from the canonical checkout: `python3 scripts/land-unit.py unit/celestina/CEL-D-015 --kind milestone --smoke bar,osd,notify --summary "Replace the switch between two menus of one capsule with one window that travels to the new menu's shape while its content changes, and open a menu on another capsule at once while the old one closes (the author's request of 2026-11-27; la gota preview version 3 approved 2026-11-27)"`. CEL-D-013 and CEL-D-016 follow; the series' last landing runs the full smoke.

## Model table (heuristic estimate)

| Chunk | Model | Reasoning | Why |
|---|---|---|---|
| Step 0 ledger and stub | this session | n/a | Two lines of docs |
| D15.1 travel clocks | Sonnet | medium | Pure curves with every number and test given |
| D15.2 sheets and Island | Sonnet | high | A state machine across the open window, the old content and `leaving`, with take-back and input rules; every branch has its test here, and Opus is kept for the review |
| D15.3 nest and docs | Sonnet | medium | Guarded nest captures and evidence from a known kit |
| `/simplify` | this session | n/a | In session, no subagents |
| Final review | Opus | high | `/code-review`, a different model from the implementers; at most 2 rounds |

## Not read or not verified while writing this plan

- CEL-D-014's plan did not exist when this was written: the pill accessor's name, and which of `crossfade.rs`, `subtitle_gestures.rs` and the `bar_tree` tests survive it, are assumptions marked "if CEL-D-014 kept it".
- Nothing was built or run. Line numbers come from main at 1.35.0 (`6ff5a018`) and may drift on the CEL-D-014 branch.
- Not checked: whether `push_clip` applies to the image `Painter::blurred` composites (Task D15.2 asks the implementer to check); `Painter::blurred`'s fallback without offscreen images has no unit test, so it is left to the review and the nest.
- Not read: `sheet/geometry.rs` beyond `window`/`hit`, `sheet/body.rs` beyond signatures, the kit's `vptr.py`.
- A decision of this plan, not of the spec: a request on the same capsule while its window closes (or while it is in `leaving`) travels from the shape shown, instead of opening a second window (the preview opened a second one; the spec's "a switch during a drop starts from the shape shown" and its anti-goal of two windows on one capsule decide it). The old content's start values `(a0, b0)` and the "higher opacity wins" rule for two travels in a row are also this plan's; both give the spec's numbers from rest.
