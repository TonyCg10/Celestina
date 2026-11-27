# CEL-D-014 Implementation Plan: the piece stays in its slot

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The piece a menu opens on stays in its slot in the bar. It shows the grey open pill behind its content, its content at 80 % opacity and no hover or press fill. The pill slides to the new piece on a switch and fades out on the close. The piece takes its own gestures again, and the window's header row is only a title.

**Architecture:**
- The lift goes, with everything that hangs from it: `BarTree::set_lifted`, the hidden node, the slot collapse and the `piece_fade` crossfade.
- A pure clock model, `island::open_piece::OpenPiece`, owns the pill and the content opacities. `BarTree` hands its look to the painted tree each frame through two narrow `celestina-ui` hooks: a plate painted inside a capsule's box, under its pieces, and a held node that gets no pointer lift and draws its content at an opacity.
- The slide and glide machinery stays. The tray-hide changes and the chevron entering or leaving still use it.
- The subtitle's gesture path goes. A press on the open piece now reaches the bar, and the bar already closes a menu when its own piece asks again.

**Tech Stack:** Rust; `celestina/crates/celestina-ui` (`Tree`, paint walk, `Animated`, `EXIT`, `ONE_UI`), `celestina/crates/celestina-paint` (`Theme`), `celestina/src/island`, `celestina/src/sheet`, `celestina/src/pieces/tray.rs`.

**Spec:** `docs/superpowers/specs/2026-11-27-celestina-menu-drop-design.md` §5.4, §5.7 (the pill and the opacity part) and §6. The approved preview is "Celestina · la gota", version 3, approved by the author on 2026-11-27. It is a private artifact; its source is in the sessions' notes as `cel-d/preview-approved/src16.html` (`.ind`, `.piece.dim`, `S.setPiece`, `S.indOff`). The code map used for this plan is `cel-d/d014-015-map.md`, part A, in the same notes.

**Branch:** `unit/celestina/CEL-D-014`, opened with `scripts/worktree.sh open celestina CEL-D-014` from `main` (celestina 1.35.0, `6ff5a018`, or later). Kind `milestone`. Size: small, executed in the session that runs it. `CEL-D-015` stacks on this branch (`--from unit/celestina/CEL-D-014`).

## Global Constraints

- **No lift.** The open piece never leaves its row: no hidden node, no slot collapse, no crossfade, no flight. The capsule's extent does not change when a menu opens, switches or closes.
- **Pill.**
  - `Role::ControlHover`, radius `theme.radius_md` (12), at the open piece's painted rect.
  - It is painted inside the capsule's box: after the capsule's haze, under its pieces, and clipped to the capsule.
  - On a fresh open it shows at once at the piece, taking over the hover the press already showed (ruling of 2026-11-27 during execution: a 150 ms fade-in left the piece with neither hover nor pill for about 80 ms). The spec names only the fade out.
- **Switch on the same capsule.** The pill slides from where it is painted to the new piece over 350 ms (`Motion::Slow`) on `OutBack(theme.overshoot)` (1.15), and keeps following the new piece's live rect.
- **Other capsule.** The old pill fades out at its piece and the new one fades in at its own. Nothing slides between capsules.
- **Close.** At the restore moment the pill fades out over 150 ms (`Motion::Exit`, `EXIT` curve).
- **Content opacity.** The open piece's content goes to `theme.muted_content_opacity`, a new field holding `celestina_tokens::opacity::MUTED_CONTENT` = 0.8, over 200 ms (`Motion::Normal`, `ONE_UI`). It goes back to 1 the same way at the restore moment, and a piece left by a switch goes back to 1 at once.
- **Restore moment.** It is one constant, `sheet::motion::PIECE_RESTORE_AT: f32 = 0.0`: the fraction of the close at which the piece gives back its pill and opacity. Today that is the first frame of the close, as the `CEL-D-010` piece did. `CEL-D-013` changes only this constant, to 0.78 (§5.3).
- **Hover.** The open piece's own box shows no pointer lift and no wash (no `ControlHover` or `ControlPressed` from the highlight); the pill is its fill. The neighbours keep theirs.
- **Gestures (§5.4, §6).**
  - A primary press on the open piece closes its menu: the existing `sheet_request` rule, "asking for the open piece again closes".
  - Wheel, secondary and middle reach the piece through the bar's ordinary input, under the bar's rules: a non-scroll action closes an open menu, as it does on every other piece.
  - The header row (subtitle) stays as the window's title and takes no gestures.
- **Chevron.** The tray's chevron is an open piece like the others: it gets the pill and the 80 % content, and keeps its 180° turn. Its own `ControlHover` fill when `tray_open` goes, so the two fills never stack.
- **Reduced motion (§5.7).** The pill appears, moves and leaves at once, and the opacity changes at once (`Reduced::Spatial` on every clock here).
- **Unchanged.** The window's open and close motion (`CEL-D-013` replaces them), the switch's two windows and the queue between capsules (`CEL-D-015` replaces those), and the window's anchor rules: the chevron keeps `Edge::Centre`.
- **At rest.** Once the pill and the opacities settle, no frames are requested.
- **Repository rules.**
  - No `unwrap`/`expect` in production; no `unsafe` outside `egl.rs`/`pam.rs`.
  - Product copy only in `src/copy.rs`; English everywhere else.
  - Never touch `src/lock`, `src/polkit`, `celestina/scripts/`, `.githooks`, any `AGENTS.md`, or `docs/contracts|governance|standards`.
- **Nested runs.**
  - Only through the guarded kit (`nest_guard`), under `env -i`, with `--own-bus`, `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0` and fake `wpctl`/`pactl`.
  - One nest at a time, a private Cargo target on disk (`~/.cache/cel-d-014-tgt`, not `/tmp`), and `timeout -k 60 2700`. Never `-s KILL`, never `pkill` by name.
  - Never run a `celestina` binary or `niri msg` from the agent's own shell.

## Review Focus

1. **Reopening the same piece during the pill's fade out.** The pill's alpha turns back up from where it is, with no slide and no flash, and the content returns to 0.8 from where it is. Test: Task 1, `reopening_during_the_fade_out_turns_back_from_where_it_is`.
2. **The open piece changes width while open** (volume 9 % to 100 %, a network name, a Wi-Fi that drops). The pill keeps to the piece's live painted rect, also in the middle of a slide (the slide's target is the live rect, not a frozen one). Tests: Task 1, `a_slide_ends_on_the_piece_where_it_is_now`; Task 2, `the_pill_follows_the_piece_when_it_changes_width`.
3. **The open piece vanishes from the bar while its menu is open** (the piece returns no view, or a layout change removes it). Its pill and held state go with it, no plate is left on the capsule, and the window still closes towards the last pill. Tests: Task 1, `a_vanished_piece_drops_its_pill_and_opacity`; Task 2, `a_vanished_open_piece_leaves_no_plate` and the rewritten `a_vanished_piece_closes_towards_its_last_slot`.
4. **A tray app hidden, or the chevron entering, while a menu is open on the same capsule.** The capsule still glides on the spring, the pill rides on its piece's painted rect through the glide, and the pill stays inside the capsule's clip. Test: Task 2, `a_tray_slide_while_open_carries_the_pill_with_its_piece`.
5. **Pointer and keyboard on the open piece.** Hover and press add no fill over the pill. A primary release closes the menu, the neighbours keep their hover, and the keyboard focus ring is still drawn. Tests: Task 1, `a_held_box_takes_no_pointer_lift`; Task 2, `the_open_piece_shows_no_hover_or_press_fill` and `a_primary_press_on_the_open_piece_closes_its_menu`.

---

### Before Task 1 (controller)

- [ ] Open the worktree: `scripts/worktree.sh open celestina CEL-D-014`. Run `git config core.hooksPath .githooks` there if the clone lacks it.
- [ ] Add the ledger row to `celestina/docs/plans/active/2026-11-25-cel-tracks.md`, after the `CEL-D-012` row and in the same columns:
  `| CEL-D-014 | \`celestina:\` | active | pending | pending | Replace the open piece's lift and crossfade with the piece staying in its slot under the grey open pill at 80 % opacity, and give its gestures back to the piece (the author's request of 2026-11-27; menu drop preview version 3 approved 2026-11-27; [design](../../../../docs/superpowers/specs/2026-11-27-celestina-menu-drop-design.md), [plan](../../../../docs/superpowers/plans/2026-11-27-celestina-cel-d-014-plan.md)) | [evidence](../../evidence/2026-11-27-cel-d-014-piece-in-slot.md) | VAL-CEL-D-014 |`
- [ ] Create the evidence stub `celestina/docs/evidence/2026-11-27-cel-d-014-piece-in-slot.md`: a title, the unit and "in progress"; Task 3 fills it in.
- [ ] Keep the SDD ledger and the task reports in the notes folder (`cel-d/sdd/`), not in the worktree.
- [ ] Commit: `celestina-maintenance: Add the CEL-D-014 ledger row and evidence stub`.

---

### Task 1: The open piece's clocks and the tree's plate

**Files:**
- Modify: `celestina/crates/celestina-paint/src/theme.rs`. Add the field `pub muted_content_opacity: f32` (doc: "The alpha of a control's content while it is held open, as the open piece in the bar."), set from `opacity::MUTED_CONTENT`. Extend the `disabled_opacity` tuple assert in `dark_matches_design_md` with it (0.8).
- Modify: `celestina/crates/celestina-ui/src/tree.rs`, `paint.rs`, `lib.rs` (the two hooks below and their export).
- Create: `celestina/src/island/open_piece.rs`, declared in `celestina/src/island/mod.rs` as `pub(crate) mod open_piece;`.
- Modify: `celestina/src/sheet/motion.rs`. Add `PIECE_RESTORE_AT`, add `SheetMotion::close_fraction`, and make `lerp_rect` `pub(crate)`.
- Modify: `celestina/src/sheet/mod.rs` (`Sheets::held_piece`).

**Interfaces:**
- Consumes: `celestina_ui::{Animated, Reduced, Motion, Easing, EXIT, ONE_UI, Key}`, `crate::sheet::motion::lerp_rect`.
- Produces:
  - `celestina_ui::Plate { pub rect: Rect, pub radius: f32, pub role: Role, pub alpha: f32 }` (`Copy`, `PartialEq`, `Debug`).
    - `Tree::set_plate(&mut self, key: Key, plate: Option<Plate>)` and `Tree::plate_of(&self, key: Key) -> Option<Plate>`.
    - The plate is painted inside the box keyed `key`, after its own fill and haze and after its clip is pushed, before its children.
    - It is drawn at `rect` as given, in painted surface coordinates, with no slide added, at `alpha` times the role colour's own alpha. `alpha <= 0` draws nothing.
    - Stored like `extents`.
  - `celestina_ui::Held { pub quiet: bool, pub inside: f32 }` (`Copy`, `PartialEq`, `Debug`).
    - `Tree::set_held(&mut self, key: Key, held: Option<Held>)` and `Tree::held_of(&self, key: Key) -> Option<Held>`.
    - With `quiet`, the box keyed `key` is painted as if neither hovered nor pressed: `box_fill` and `box_wash` get `false, false`.
    - Everything painted after its own box (children and focus ring) gets `painter.opacity(inside)` inside the node's pushed state. Its own box fill does not.
  - `pub fn Tree::box_roles(&self, id: NodeId) -> (Option<Role>, Option<Role>)` (`#[must_use]`): the fill and wash the walk would draw for box `id` now. It is a plain `pub`, not `#[cfg(test)]`, because Task 2's tests in the `celestina` crate call it. `paint_enter` uses it, so there is one rule.
  - `island::open_piece::PillLook { pub zone: usize, pub piece: Key, pub rect: Rect, pub alpha: f32 }`.
  - `island::open_piece::OpenPiece`:
    - `pub fn new(theme: &Theme, reduced: bool) -> OpenPiece` and `pub fn set_theme(&mut self, theme: &Theme)`. It keeps `motion`, `overshoot` and `muted_content_opacity`.
    - `pub fn set_reduced(&mut self, on: bool)`.
    - `pub fn set(&mut self, open: Option<(Key, usize)>, now: Instant, live: &dyn Fn(Key) -> Option<Rect>)`: the piece and zone open from `now`, `None` when none is.
    - `pub fn open(&self) -> Option<Key>`.
    - `pub fn pills(&self, now: Instant, live: &dyn Fn(Key) -> Option<Rect>) -> Vec<PillLook>`: at most one per zone. A pill whose piece has no live rect is left out.
    - `pub fn content(&self, key: Key, now: Instant) -> Option<f32>`: `None` means full opacity and not held.
    - `pub fn settle(&mut self, now: Instant, holds: &dyn Fn(Key) -> bool)`: drops finished clocks and those of pieces the bar no longer holds.
    - `pub fn is_animating(&self, now: Instant) -> bool`.
  - `pub const PIECE_RESTORE_AT: f32 = 0.0;` in `sheet/motion.rs`, and `SheetMotion::close_fraction(&self, now: Instant) -> Option<f32>`: the close's time fraction in 0..=1, `None` while not closing.
  - `Sheets::held_piece(&self, now: Instant) -> Option<Key>`: the open sheet's piece, and a closing sheet's piece while its `close_fraction < PIECE_RESTORE_AT`.

**Model rules (`OpenPiece`):**
- Each pill has an alpha `Animated` and an optional slide `(from: Rect, start: Instant)`. Its rect is `lerp_rect(from, live(piece), OutBack(overshoot).eval(t))` with `t` over `Motion::Slow`, then `live(piece)` once the slide ends.
- `set` with a piece in the zone of a pill still shown (at any alpha): that pill slides from its current painted rect to the new piece, and its alpha retargets to 1 from where it is. A slide to the same piece is no slide.
- `set` with a piece in another zone: the old pill's alpha goes to 0 and a new pill starts at alpha 0 and goes to 1.
- `set(None)`: the alpha goes to 0.
- Alpha changes run over `Motion::Exit` on `EXIT`. Content changes run over `Motion::Normal` on `ONE_UI`: the open piece goes to the muted opacity, and the piece left goes back to 1.
- Every clock uses `Reduced::spatial(reduced)`.

- [ ] **Step 1: Write the failing tests**

```rust
// celestina-paint theme.rs
#[test] fn dark_matches_design_md()   // existing; the tuple gains muted_content_opacity == 0.8

// celestina-ui paint.rs / tree.rs, on the existing fixtures (`lifted_capsule`, `card_with_rows`)
#[test] fn a_plate_is_kept_per_key_until_cleared()             // plate_of returns it; None clears; other keys untouched
#[test] fn a_held_box_takes_no_pointer_lift()                   // hovered + pressed interactive box with no fill: box_roles == (None, None) while quiet; (Some(ControlPressed), None) after set_held(key, None)
#[test] fn a_held_box_with_its_own_fill_keeps_it_and_gets_no_wash() // fill Accent: (Some(Accent), None) while quiet

// celestina/src/island/open_piece.rs (theme = Theme::dark(); live = fixed rects A (100,10,60,28), B (170,10,40,28) in zone 3, C (20,10,50,28) in zone 0)
#[test] fn opening_lights_the_pill_in_150_ms_and_mutes_the_content_in_200_ms() // alpha 0 at +0, EXIT.eval(0.5) at +75, 1 at +150; content 1 at +0, 1-0.2*ONE_UI.eval(0.5) at +100, 0.8 at +200; rect == A throughout
#[test] fn a_switch_in_the_capsule_slides_the_pill_on_the_spring()     // open A, settle, set B at t1: rect at +175 == lerp_rect(A,B,OutBack(1.15).eval(0.5)); == B at +350; alpha stays 1; content(A) back to 1 by +200, content(B) 0.8 by +200
#[test] fn a_slide_ends_on_the_piece_where_it_is_now()                  // live(B) widens mid-slide: rect at +350 == the new B
#[test] fn another_capsule_fades_the_old_pill_and_lights_a_new_one()   // open A, then C: two PillLooks at +75 (zone 3 falling, zone 0 rising), none slides; one at +150
#[test] fn closing_fades_the_pill_in_150_ms_and_restores_the_content() // set(None): alpha 1-EXIT.eval(0.5) at +75, gone at +150; content 1 at +200; is_animating false at +200
#[test] fn reopening_during_the_fade_out_turns_back_from_where_it_is() // Review Focus 1: close, reopen A at +50: alpha at +50 equals alpha just before; rises to 1; no slide (rect == A)
#[test] fn a_vanished_piece_drops_its_pill_and_opacity()               // Review Focus 3: settle with holds(A)==false: pills empty, content(A) None, not animating
#[test] fn reduced_motion_lights_slides_and_restores_at_once()         // every value at its end on the first frame; is_animating false

// celestina/src/sheet/motion.rs and mod.rs
#[test] fn the_close_fraction_runs_over_the_close_only()               // None while open; 0 at the close's first frame; 1 at its end
#[test] fn the_piece_is_held_until_its_restore_moment()                // written against PIECE_RESTORE_AT: held before close + CLOSE*PIECE_RESTORE_AT (when > 0), released from there; with today's 0.0, held_piece is None from the close's first frame
```

- [ ] **Step 2: Run them and see them fail.** From `celestina/`: `CARGO_TARGET_DIR=~/.cache/cel-d-014-tgt cargo test --workspace --locked open_piece close_fraction held box_roles plate muted dark_matches`. Expected: they fail to compile (missing items).
- [ ] **Step 3: Implement** the items listed under Interfaces. In `paint_enter`:
  - for a quiet box, pass `false, false` to `box_fill` and `box_wash`;
  - draw the plate after `push_clip`;
  - call `painter.opacity(inside)` after the box's own draws.
- [ ] **Step 4: Run all checks**:
  - `cargo fmt --all -- --check`;
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`, then the same with `cargo +1.100.0`;
  - `cargo test --workspace --locked`, summing every `test result` line;
  - from the repo root, `bash scripts/check-architecture-contract.sh`, `bash scripts/check-documentation-contract.sh` and `python3 scripts/check-language-contract.py`.

  Expected: all green, and nothing outside the new items changed behaviour.
- [ ] **Step 5: Commit**: `celestina-maintenance: Add the open piece's pill and opacity clocks and the tree's plate and held hooks`.

---

### Task 2: The piece stays in its slot

**Files:**
- Modify: `celestina/src/island/bar_tree.rs`.
  - Remove `lifted`, `set_lifted`, `lifted()`, `fades`, `fade_widths`, `live_slot`, `paint_fading`, `slot_rect`, `fade_slot`, `fade_of`, `LAYER_GRID` and the `hide` call in `update`.
  - Keep `lift_changed` (the tray-hide and chevron path), `glides`, `capsule_extent`, `is_sliding`, `sync_extents` and `slide_with`.
  - Add the `OpenPiece` field, `set_open`, `pill_of` (simplified), `sync_open`, and the test accessors below.
  - Rewrite the module doc (lines 1-24).
- Delete: `celestina/src/island/piece_fade.rs` and its `mod` line in `island/mod.rs`.
- Modify: `celestina/src/notifications/island_notice.rs`. Its `FACE_BLUR`, `FACE_DY`, `FACE_OUT_MS` and `FACE_IN_MS` become literals, 4.0, 10.0, 160 and 200, with the doc citing the `CEL-D-011` design. Drop the `piece_fade` import; values unchanged.
- Modify: `celestina/src/bar/view.rs`. Remove `ZoneViews::hide` and `hide_in`: their only caller goes.
- Modify: `celestina/crates/celestina-ui/src/paint.rs` and `tree.rs`. Remove the two dead `paint_except_hiding` and `paint_children_hiding`, and `Scope::ChildrenOf` with its walk test. Keep `Tree::paint_hiding`, the `hidden` parameter, `Scope::Except` and the test `a_hidden_key_takes_its_subtree_out_of_the_walk_only`: `CEL-D-016` paints a row's card with them, even though this unit leaves them without a caller in `src/` (allow the dead-code lint on `paint_hiding` with a comment naming `CEL-D-016`). `Scope::Inside` and `paint_inside` stay; `notifications/banners.rs` uses them.
- Modify: `celestina/src/island/mod.rs`.
  - `layout_with`: `bar.set_open(self.sheets.held_piece(now), now)` replaces the lifted filter. The chevron is no longer excluded; `overflow_shown` stays for the turn.
  - Remove the `subtitle_press` field.
  - Update the docs of `bar_input` and `apply_asked` that mention the subtitle.
- Modify: `celestina/src/island/sheets.rs`. Remove the subtitle branch of `sheet_input` (the `pressed` and `on_subtitle` lines) and `subtitle_gesture`, and rewrite the module doc and the `live_anchor` comment ("the pill is the piece's rect").
- Modify: `celestina/src/sheet/mod.rs`. Remove `on_subtitle`; rewrite the module doc, the `Anchor` doc and the `close` doc.
- Modify: `celestina/src/sheet/subtitle.rs`. The module doc changes to: the header row is the window's title and takes no gestures.
- Modify: `celestina/src/island/bar_tree.rs`. Remove `ask_for` and `takes_wheel`; their only caller was `subtitle_gesture`.
- Modify: `celestina/src/pieces/tray.rs`. The chevron keeps `.rotate(180)` and drops its `fill(ControlHover)` when `tray_open`. Update the doc of `chevron` and the doc of `BarCx.tray_open` in `pieces/mod.rs` ("the chevron is turned").
- Tests: the table below, plus a new `celestina/src/island/tests/open_piece.rs` that replaces `crossfade.rs` and `subtitle_gestures.rs`. Update the declarations at `island/mod.rs:6826-6830`.

**Interfaces:**
- Consumes: Task 1's `OpenPiece`, `PillLook`, `Plate`, `Held`, `Tree::{set_plate, set_held}`, `Sheets::held_piece` and `Theme::muted_content_opacity`.
- Produces:
  - `BarTree::set_open(&mut self, piece: Option<Key>, now: Instant)`. It resolves the zone with `zone_of` and the live rect with `pill_of`. It changes no layout and does not mark the views stale.
  - `BarTree::pill_of(&self, key: Key, now: Instant) -> Option<Rect>`: the piece node's `painted_rect`, with height `PILL_H` when it has none. `None` once the bar does not hold the piece.
  - `fn sync_open(&mut self, now: Instant)`, called first in `paint` after `sync_extents`, then `tree.paint`. It sets:
    - `set_plate(zone_key(z), Some(Plate { rect, radius: theme.radius_md, role: ControlHover, alpha }))` for each `PillLook`, and `None` for the other zones;
    - `set_held(open, Some(Held { quiet: true, inside }))` on the open piece;
    - `Held { quiet: false, inside }` on a piece still returning to 1;
    - `None` once a piece is done.
  - `BarTree::is_animating` adds `open.is_animating(now)`; `update` calls `open.settle` after a rebuild; `set_theme` and `set_reduced_motion` forward to `OpenPiece::set_theme` and `OpenPiece::set_reduced`.
  - `#[cfg(test)]` accessors: `BarTree::pill(&self, now) -> Vec<PillLook>`, `BarTree::content_opacity(&self, key, now) -> f32` (1.0 when not held), `BarTree::open_piece(&self) -> Option<Key>`.

**Tests that change** (names as on `main` at `6ff5a018`):

| Where | Test | What happens |
|---|---|---|
| `island/tests/crossfade.rs` | the whole file: `opening_fades_…`, `closing_fades_…`, `the_subtitle_is_never_hidden`, `reopening_during_a_close_hands_over`, `a_switch_back_hands_the_fade_over_…`, `a_vanished_piece_drops_its_fade`, `reduced_motion_fades_the_piece_…`, `the_fading_piece_stays_in_its_slot_…`, `the_fading_piece_is_cut_to_a_slot_…`, `neighbours_that_switch_share_the_slot_room` | Delete the file. Its ideas move to `open_piece.rs` below; `the_subtitle_is_never_hidden` becomes `the_header_row_is_shown_through_open_and_close`. |
| `island/piece_fade.rs` | its unit tests | Deleted with the module. |
| `island/tests/subtitle_gestures.rs` | `a_right_click_on_the_volume_subtitle_mutes_and_a_click_closes`, `a_gesture_the_piece_has_no_action_for_keeps_the_sheet_open`, `a_release_on_the_subtitle_after_a_press_elsewhere_is_no_click`, `only_a_piece_with_a_wheel_action_takes_the_wheel_over_its_subtitle`, helpers `subtitle_point`, `session_open` | Delete. The gestures are the piece's again. |
| same | `the_only_piece_of_a_capsule_keeps_a_row_and_opens` | Move to `open_piece.rs` as `the_only_piece_of_a_capsule_keeps_its_capsule_and_opens`: the capsule's rect is the same before and during the open, and the pill sits on the piece. |
| same | `no_frame_is_asked_at_rest`, `the_leaving_window_keeps_its_own_pill` | Move unchanged to `open_piece.rs` (`CEL-D-015` rewrites the second). |
| same | `the_slide_is_followed_only_for_a_moment` | Rewrite on a tray-hide slide: a volume window open, then a tray app hidden in the same capsule. `follows_slide` is true at +340 ms and false 400 ms later. If no fixture has both, write it in `tray_overflow.rs` with the chevron's window open. |
| `island/mod.rs` | `the_wheel_steps_with_the_sheet_open_and_keeps_it_open` (uses `subtitle_point`) | Wheel over the workspaces piece in the bar (`bar.node_rect(workspaces_piece::key())`). Same assertions. |
| `island/mod.rs` | `a_lifted_only_piece_leaves_an_empty_capsule_and_a_window` | Rename to `an_only_piece_keeps_its_capsule_and_a_window`: the capsule keeps its width and `pill_of(bell)` equals the bell's rect. |
| `island/mod.rs` | `asking_for_the_open_piece_again_closes_its_sheet`, `switching_the_volume_sheet_to_the_brightness_sheet_morphs_in_place`, `a_second_piece_of_the_same_capsule_switches_in_place` | Keep (re-run). |
| `island/bar_tree.rs` | `lift()` helper, `a_lifted_piece_leaves_no_gap`, `landing_the_piece_reopens_its_slot`, `the_slot_keeps_its_centre_while_lifted`, `reduced_motion_jumps_the_slot`, `the_only_piece_of_a_capsule_can_be_lifted`, `a_lifted_piece_that_leaves_the_bar_has_no_slot`, `switching_lands_the_old_piece_and_lifts_the_new_one_on_one_spring`, `the_slot_of_the_last_piece_starts_where_the_piece_was`, `a_piece_at_the_end_of_its_capsule_reopens_inside_the_capsules_edge` | Delete. |
| same | `the_row_slides_with_the_open_spring`, `the_capsule_extent_follows_its_sliding_pieces`, `a_capsule_moved_during_a_glide_keeps_its_glass_and_regions_on_it`, `a_placement_change_during_a_lift_keeps_the_lifted_zones_glide` | Re-base on a `set_layout` whose `tray_hidden` changes (the helper `hiding_a_tray_app_slides_the_icons_on_the_open_spring_and_settles` uses) instead of `lift`. Same assertions; the last one is renamed `…during_a_tray_slide_keeps_the_zones_glide`. |
| same | `the_pill_is_the_piece_at_rest_a_square_while_lifted_and_grows_back` | Rename to `the_pill_is_the_pieces_painted_rect`: equal at rest and while a neighbour slides; `None` for an unknown key. |
| same | `hiding_a_tray_app_slides_…`, `a_zone_change_still_jumps_…` | Keep. |
| `island/tests/page_sheets.rs` | `the_open_piece_is_out_of_the_row_and_the_others_keep_their_hover`, helper `hovered_and_hidden` | Rename to `the_open_piece_stays_in_its_row_and_the_others_keep_their_hover`: `open_piece() == Some(network)`, the network's rect unchanged, the neighbours' hover kept. |
| same | `switching_lands_the_old_piece_and_lifts_the_new_one` | Rename to `switching_moves_the_pill_to_the_new_piece`: `open_piece() == Some(bluetooth)`, network's `content_opacity` back to 1 by +200 ms, the pill on bluetooth at +350 ms. |
| same | `after_close_the_piece_lands_and_shows_its_hover_again` | Rename to `after_close_the_piece_lets_go_of_its_pill_and_shows_its_hover_again`: no pill at +150 ms after the restore moment, `content_opacity == 1` at +200, and a hover lifts it again. |
| `island/tests/window.rs` | `the_bar_paints_every_capsule_while_a_window_is_open` | The `lifted == Some(volume)` assert becomes `open_piece() == Some(volume)` and the capsule's extent is unchanged. |
| same | `a_vanished_piece_closes_towards_its_last_slot` | Drop the `fade_of` assert; the window still closes towards the last pill. |
| same | `a_press_on_the_glass_of_its_own_capsule_closes` | Comment only: the padding is empty glass, not a slot. |
| `island/tests/tray_overflow.rs` | `chevron_opens_the_overflow_window` (lines 100-105) | Replace the `lifted`/`fade_of` asserts with `open_piece() == Some(chevron_key())` and the pill on the chevron. |
| `pieces/tray.rs` | `the_chevron_is_lit_and_turned_while_its_window_is_open` | Rename to `the_chevron_turns_while_its_window_is_open`: rotation 180, no own fill. |
| `crates/celestina-ui` | `a_hidden_key_takes_its_subtree_out_of_the_walk_only`, the `Scope::Except` and `Scope::ChildrenOf` walk tests | Delete with the API. `an_inside_walk_enters_…` stays. |

- [ ] **Step 1: Write the new failing tests** in `celestina/src/island/tests/open_piece.rs`, on `level_island()`, `open_on`, `step` and `ms`. The table's renames and rewrites come in the same step.

```rust
#[test] fn the_open_piece_keeps_its_slot_and_its_capsule_keeps_its_extent() // capsule_extent and every piece's painted rect equal before the open, at +0/+60/+175/+350 and after; is_sliding false throughout
#[test] fn the_pill_lights_under_the_piece_and_its_content_goes_to_80_percent() // sync_open: plate_of(zone) rect == pill_of(volume), radius 12, role ControlHover, alpha 1 at +150; held_of(volume) == Held{quiet:true, inside:0.8} at +200
#[test] fn the_open_piece_shows_no_hover_or_press_fill()                  // Review Focus 5: hover + press over the open piece: box_roles == (None, None); a neighbour hovered: (Some(ControlHover), None)
#[test] fn a_primary_press_on_the_open_piece_closes_its_menu()            // Review Focus 5: press + release at the volume's rect: is_open() false, held_piece None
#[test] fn the_wheel_on_the_open_piece_steps_its_level_and_keeps_the_menu() // Axis over the volume piece: an Audio action, sheet still open
#[test] fn a_right_click_on_the_open_volume_piece_acts_on_it()           // the mute action is emitted; the bar's rule closes the menu (today's rule, unchanged)
#[test] fn the_header_row_is_shown_through_open_and_close()               // subtitle::row_key in the body tree at +100/+400 and during the close; a press on it is the body's (no piece action)
#[test] fn closing_gives_back_the_pill_and_opacity_at_the_restore_moment() // close at t1: pill alpha falls from t1 + CLOSE*PIECE_RESTORE_AT over 150 ms; content 1 200 ms later; bar not animating after
#[test] fn the_pill_follows_the_piece_when_it_changes_width()             // Review Focus 2: change the volume percent (9 → 100) while open: plate rect == pill_of(volume) after the rebuild
#[test] fn a_vanished_open_piece_leaves_no_plate()                        // Review Focus 3: the piece returns no view while open: plate_of(zone) None, held_of None after the next update
#[test] fn a_tray_slide_while_open_carries_the_pill_with_its_piece()      // Review Focus 4: hide a tray app in the open piece's capsule: at +60/+175 the plate rect == the piece's painted rect and inside capsule_extent
#[test] fn the_chevron_gets_the_pill_and_keeps_its_turn()                  // chevron's window open: plate on its zone at the chevron's rect; rotation 180
#[test] fn reduced_motion_lights_and_restores_the_pill_at_once()          // set_reduced_motion(true): alpha 1 and inside 0.8 on the first frame; gone on the close's first frame
```

- [ ] **Step 2: Run them and see them fail**: `CARGO_TARGET_DIR=~/.cache/cel-d-014-tgt cargo test -p celestina --locked open_piece`. Expected: they fail to compile or fail on the lift.
- [ ] **Step 3: Implement** the Files list.
  - `rg "set_lifted|lifted\(\)|fade_of|fade_slot|slot_rect|paint_hiding|piece_fade|Fades|subtitle_press|on_subtitle|subtitle_gesture|ask_for|takes_wheel|hide_in|ZoneViews::hide"` in `celestina/` must find nothing afterwards, apart from history in `docs/`.
  - Rewrite every doc that says the piece leaves its slot, crossfades or answers through the subtitle: the `bar_tree.rs` module doc, `sheet/mod.rs` lines 1-11 and 49-52 and the `close` doc, `island/sheets.rs` lines 1-6, `sheet/subtitle.rs` lines 1-6, and the `chevron` doc.
- [ ] **Step 4: Run all checks** as in Task 1, Step 4. Expected: all green. The test count drops by the deleted tests and rises by the new ones; record both sums.
- [ ] **Step 5: Commit**: `celestina-maintenance: Replace the open piece's lift and crossfade with the piece staying in its slot under the open pill`.

---

### Task 3: Nest shots, validation and documents

**Files:**
- Modify: `celestina/docs/evidence/2026-11-27-cel-d-014-piece-in-slot.md`: procedure, shots, test sums, limits.
- Create: `celestina/docs/evidence/2026-11-27-cel-d-014-piece-in-slot/*.png`.
- Modify: `celestina/VALIDATION.md`:
  - add `VAL-CEL-D-014` after `VAL-CEL-D-012`;
  - add a `**Note:**` line in `VAL-CEL-D-008` and `VAL-CEL-D-010`, in the style of the note at line 573: "since CEL-D-014 the piece stays in its slot; the slot, subtitle-gesture and crossfade parts of this procedure no longer apply".
- Modify: `celestina/README.md`, only if a line describes the piece leaving the bar (`rg -n "subtitle|slot|crossfade" celestina/README.md`; none was found on `6ff5a018`).

- [ ] **Step 1: Shots.**
  - Copy the kit from the notes, `cel-d/kit-recovered/`, to `~/.cache/cel-d-014-nest/`.
  - Edit `N=` and `R=` in `run.sh`. `R` is the unit worktree's `celestina/`.
  - Build into `~/.cache/cel-d-014-tgt`.
  - Through `run.sh` with probes (the probes call `nest_guard` first), capture on the volume or energy piece of the end capsule:
    - at rest;
    - the open at +0, +75, +150 and +350 ms (the pill lighting, the content dimming);
    - the hover over the open piece (no second plate);
    - a switch to a neighbour at +175 ms (the pill mid-slide) and +350 ms;
    - the close at +75 and +150 ms;
    - the chevron's window open;
    - the same open with reduced motion.
  - Read every crop. Confirm from the logs that no frame is drawn at rest after a close.
- [ ] **Step 2: Smoke** the phases `bar,osd,notify` through the kit, as `CEL-D-010` did (`scripts/smoke.sh --binary <private target>/debug/celestina --only bar,osd,notify`, under the host-wide nest lock, `timeout -k 60 2700`). Record `rest_frames`.
- [ ] **Step 3: Write `VAL-CEL-D-014`** with these fields:
  - **Status:** pending.
  - **Related implementation:** the open piece stays in its slot with the grey pill and its content at 80 %; the pill slides on a switch and fades on the close; no hover fill on the open piece; its gestures are its own; the window's header row is a title.
  - **Requires:** the deployed shell after the landing, and a layout whose end capsule holds volume, brightness and the tray.
  - **Procedure:**
    1. Open the volume menu, hover and press the open piece, and turn the wheel on it.
    2. Switch to brightness.
    3. Right-click the volume piece.
    4. Open the chevron's window.
    5. Close with Escape and by pressing outside.
    6. Repeat once with reduced motion.
  - **Pass condition:**
    - the bar never shows a gap;
    - the pill sits behind the open piece and slides to the new one;
    - no darker plate appears on hover;
    - a click on the open piece closes the menu;
    - the pill fades and the piece is at full opacity once the window has gone;
    - with reduced motion everything changes at once;
    - at rest nothing moves.
  - **Result:** not run.
  - **Evidence:** the unit's evidence file.

  Fill the evidence file, then run `stop-slop` on both texts.
- [ ] **Step 4: Run the three guards** and `cargo test --workspace --locked` once more.
- [ ] **Step 5: Commit**: `celestina-maintenance: Add the CEL-D-014 evidence and VAL-CEL-D-014`.

---

## After the tasks

- [ ] `/simplify` on the branch's code, in the session, without subagents. Run `stop-slop` on the evidence and `VALIDATION.md` text.
- [ ] Final review `CEL-D-014.R · Final review`: Opus, high reasoning, `/code-review` on `git diff origin/main...unit/celestina/CEL-D-014`, against this plan, the spec's §5.4 and §5.7, and the approved preview. At most two rounds (the second rereads only the fixes of the first).
- [ ] Landing, only with the author's direct authorization in this chat, from the canonical checkout: `python3 scripts/land-unit.py unit/celestina/CEL-D-014 --kind milestone --smoke bar,osd,notify --summary "Replace the open piece's lift and crossfade with the piece staying in its slot under the grey open pill at 80 % opacity, and give its gestures back to the piece (the author's request of 2026-11-27; menu drop preview version 3 approved 2026-11-27)"`. `CEL-D-015` stacks on this branch and merges `origin/main` after this lands.

## Model table (heuristic estimate)

| Chunk | Model | Reasoning | Why |
|---|---|---|---|
| Before Task 1 | this session | n/a | Ledger row and stub |
| Task 1: clocks and tree hooks | Sonnet | medium | Pure clocks and two narrow paint hooks with exact numbers and tests given |
| Task 2: the piece stays in its slot | Sonnet | medium | Removal and wiring with the test table given; the reviewer is Opus |
| Task 3: nest and docs | Sonnet | medium | Guarded nest captures and evidence |
| `/simplify` | this session | n/a | In session, no subagents |
| Final review | Opus | high | `/code-review`, a different model from the implementers; at most two rounds |

## Not read or not verified while writing this plan

- Nothing was built or run.
- The test names come from `grep` on `main` at `6ff5a018`; the bodies of the `bar_tree.rs` tests from 1155 to 1643 and of `crossfade.rs` from 115 to 315 were not read.
- Whether `level_island()` has a tray in the volume's capsule, for `a_tray_slide_while_open_carries_the_pill_with_its_piece` and the rewritten `the_slide_is_followed_only_for_a_moment`, was not checked; the Task 2 table gives the fallback.
- The bar's existing rule closes an open menu on a non-scroll action of any piece, so a right click on the open volume piece mutes and closes. The spec says only that those gestures "act on the piece". This plan keeps today's rule; changing it is the author's call.
- The pill fading in on a fresh open (150 ms, the exit curve) comes from the preview's CSS, not from the spec's text.
- The chevron losing its own lit fill in favour of the pill is this plan's reading of §5.4 ("the open piece"); the author has not been asked.
- The open window still starts from a radius of half the piece's height (14), while the pill has radius 12; `CEL-D-013` replaces the open, so this plan leaves it.
- The "pending" values in the active ledger row's inventory and diffstat columns were not checked against `land-unit.py`'s preflight; every row seen was already `done`. Copy whatever the last active row looked like if the preflight refuses.
