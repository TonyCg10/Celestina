# CEL-D-011 Implementation Plan: notifications and the level display come out of the Island

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A notification grows the Island into the notice and shrinks it back to the clock; a burst waits under it as sheets that fan out on hover; a level change widens the Island in place (with `osd.position: "island"`); right click and wheel up close a notice, middle click closes them all.

**Architecture:** A pure model (`IslandNotice`) turns the toasts, the level display, the hover and the clock into one look per frame: the Island's rect, three content faces (clock, notice, level) with opacity, blur and offset, and the sheets under it. The `celestina-toasts` surface stays on the Overlay layer, spans the output's width and paints that look; while it exists, the panel's Island on that output lends its capsule (no paint, no regions, no input). With the Island page open, the notice is a card inside the page instead.

**Tech Stack:** Rust, `celestina-ui` (`Tree`, `Animated`, `Interaction`), `celestina-paint` (`Painter::haze`, `Painter::blurred`), `celestina-wayland` layer surfaces, the shell's notifications provider and OSD duty.

**Spec:** `docs/superpowers/specs/2026-11-26-celestina-island-notices-design.md` (all of §5). Approved preview: variant A of the Island notices preview, version 1, chosen by the author on 2026-11-26 (private artifact; source in the sessions' notes `cel-d/preview-approved/src15.html`, stage `st-a`). The code map used to write this plan is `cel-d/d011-code-map.md` in the same notes.

**Branch:** `unit/celestina/CEL-D-011`, opened with `scripts/worktree.sh open celestina CEL-D-011` from `main`.

## Global Constraints

- Island at rest: centred on the output, y 8, h 40, radius 20 (`theme.radius_lg`), its width the panel Island's capsule width (`IslandState` rest rect).
- Notice: 420 wide, centred, top y 8, height 88 (120 with buttons), radius 20; padding 16 across, 12 down; icon 32, summary rowTitle 15 semibold one line, close button 24, body 13 two lines, buttons 28 (`BUTTON_H`), as today's banner.
- Grow and move: 350 ms OutBack 1.15 (`motion.slow`, `motion.overshoot`). Shrink to rest: 300 ms ONE_UI.
- Face out: 160 ms ONE_UI, opacity 1 → 0, blur 0 → 4 px, offset 0 → +10 px. Face in: delay 120 ms (140 ms for the clock coming back), 200 ms ONE_UI, opacity 0 → 1, blur 4 → 0, offset −10 → 0.
- Sheets: at most two; sheet `i` (1 or 2) is `420 − 24·i` wide, centred, 24 high, radius 12 (`theme.radius_md`), bottom `7·i` px under the Island's bottom; glass opacity 0.92; enters from 10 px higher over 200 ms; leaves by fading over 150 ms. Fanned: 420 wide, radius 20, 8 px apart under the Island, on the 350 ms spring; folding waits 250 ms after the pointer leaves.
- Level display (`osd.position: "island"`): 274 × 40, centred, radius 20, icon 18, track 160 × 4 (`theme.linear_track_height`), label `NN %` 40 wide right-aligned, padding 16, gap 12; shown for `osd.timeout_ms`; while shown, a notice's timeout is held (`NotificationsCommand::Hover(true)`).
- Page card (§5.5): 336 wide, 80 or 112 high, `Role::ControlFill`, radius 12, padding 12 × 8, 8 px from the page's top and 12 px from its sides; the page grows by card height + 8.
- Gestures on any notice or sheet: left click as today (`banners::actions_for`); right click → `NotificationsCommand::Close(id)`; vertical wheel towards the top → `Close(id)`; middle click → `NotificationsCommand::DismissAll`; hover pauses (`Hover`). The level display takes no input.
- Reduced motion: geometry jumps; faces fade 150 ms, no blur, no offset.
- Surfaces: `celestina-toasts`, `Layer::Overlay`, anchored top on both sides, height 440, exclusive zone -1 (over the bar's zone, from the top edge), keyboard none, regions = painted shapes. It exists only while something shows; at rest nothing asks for frames. Hand-over: its first frame is the rest Island, then the panel lends its capsule; at the end the panel takes it back, then the surface is destroyed.
- The log lines `banners surface requested` (with `created=`) and `banners surface released` stay, with the same fields: the smoke's `notify` phase reads them. `osd.position` keeps `bottom` as its default; the bottom and bottom-right display is unchanged.
- No `unwrap`/`expect` in production; no `unsafe` outside `egl.rs`/`pam.rs`; product copy only in `src/copy.rs`; never touch `src/lock`, `src/polkit`, `celestina/scripts/`, `.githooks`, any `AGENTS.md`, `docs/contracts|governance|standards`.
- Nested runs only through the guarded kit (`nest_guard`), `env -i`, `--own-bus`, `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`, fake `wpctl`/`pactl`, one nest at a time, a private Cargo target. Never run a `celestina` binary or `niri msg` from the agent's own shell.

## Review Focus

1. A toast that arrives while the Island is shrinking back to rest: the shrink turns into the grow without the surface being destroyed and re-created, and the panel never paints its capsule under the grown Island.
2. The banner output changes (focus moves to another monitor) while a notice is out: the old output's panel takes its capsule back, the new output's Island lends its own, and no output is left without a capsule.
3. The output with the notices surface goes away: its Island and surface go with it, the hover is released (the server's clock runs again), as today in `banners_gone`.
4. A level change and a notice at the same time, with the level ending first and with the notice closed by its program while held: the Island ends on the right face and the server's clock is not left paused.
5. Do-not-disturb on with a critical notification: it shows, never times out, and right click closes it.

---

### Task D11.1: the Island notice model

**Files:**
- Create: `celestina/src/notifications/island_notice.rs`
- Modify: `celestina/src/notifications/mod.rs` (declare the module)
- Test: in the same file (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `celestina_shell::notifications::Notification`; `celestina_paint::{Rect, Theme}`; `celestina_ui::Animated`; `banners::{actions_for, BannerAction, DEFAULT_ACTION}` (kept for the left click).
- Produces:
  - `pub struct IslandNotice` with `pub fn new(theme: &Theme, reduced: bool) -> IslandNotice`;
  - `pub fn set_rest(&mut self, rest: Rect)`: the panel Island's rest rect on this output;
  - `pub fn set_toasts(&mut self, toasts: &[Notification], now: Instant) -> bool` (newest first; keeps at most three; whether the look changed);
  - `pub fn set_level(&mut self, level: Option<Level>, now: Instant)` with `pub struct Level { pub kind: celestina_shell::osd::Kind, pub percent: u8, pub muted: bool }`;
  - `pub fn set_hovered(&mut self, over: bool, now: Instant)` (applies the 250 ms grace on leave);
  - `pub fn look(&self, now: Instant) -> NoticeLook` with `pub struct NoticeLook { pub island: (Rect, f32 /*radius*/), pub faces: Vec<Face>, pub sheets: Vec<SheetLook> }`, `pub struct Face { pub kind: FaceKind, pub opacity: f32, pub blur: f32, pub dy: f32 }`, `pub enum FaceKind { Clock, Notice(u32), Level }`, `pub struct SheetLook { pub id: u32, pub rect: Rect, pub radius: f32, pub glass: f32, pub content: f32 }`;
  - `pub fn at_rest(&self, now: Instant) -> bool` (nothing shown and every motion settled: the surface may go);
  - `pub fn is_animating(&self, now: Instant) -> bool`;
  - `pub fn holds_timeouts(&self) -> bool` (hover or a level shown);
  - `pub fn gesture(&self, target: u32, gesture: Gesture) -> Vec<BannerAction>` with `pub enum Gesture { Left(celestina_ui::Key), Right, Middle, WheelUp }`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn a_toast_grows_the_island_to_the_notice()            // rest (870,8,180,40) on a 1920 output: target (750,8,420,88), r 20; at 350 ms settled; clock face opacity 0 at 160 ms; notice face opacity 0 until 120 ms, 1 at 320 ms
#[test] fn buttons_make_the_notice_120_high()
#[test] fn the_last_toast_leaving_shrinks_to_rest_in_300_ms_without_overshoot() // h never below 40, w never below rest.w
#[test] fn the_next_toast_takes_the_island_without_going_to_rest()               // look never equals rest between the two
#[test] fn a_burst_shows_two_sheets_under_the_island()                           // rects (762,79,396,24) and (774,86,372,24) for an 88-high notice on a 1920 output; glass 0.92; content 0
#[test] fn hover_fans_the_sheets_and_folding_waits_250_ms()
#[test] fn a_level_widens_the_island_in_place_and_holds_timeouts()              // (823,8,274,40); holds_timeouts() true while shown
#[test] fn the_notice_comes_back_after_the_level()
#[test] fn right_click_and_wheel_up_close_middle_dismisses_all()
#[test] fn reduced_motion_jumps_geometry_and_fades_150_ms()
#[test] fn at_rest_only_after_every_motion_settled()
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p celestina island_notice`. Expected: FAIL (module missing).

- [ ] **Step 3: Implement `IslandNotice`**

Rect and radius on one set of `Animated` values per coordinate, retargeted from the current value (a target change during a motion continues from where it is). Faces keep their own opacity, blur and offset clocks. Curves: reuse the OutBack and ONE_UI easings the sheet motion already uses (`src/sheet/motion.rs`); no new easing code.

- [ ] **Step 4: Run the tests**. Expected: PASS; `cargo clippy -p celestina --all-targets -- -D warnings` clean.

- [ ] **Step 5: Commit** `celestina: Add the Island notice model` (subject verbs from `commit_scope`).

---

### Task D11.2: the notices surface paints the Island, and the panel lends its capsule

**Files:**
- Modify: `celestina/src/notifications/banners.rs` (the stack's view and `BannerStack` become the painter of `NoticeLook`; `actions_for`, `hover_command`, the keys and `NAMESPACE` stay)
- Modify: `celestina/src/app.rs` (`sync_banners`, `create_banners`, `banners_gone`, `banners_act`, frame wiring)
- Modify: `celestina/src/island/mod.rs` (lend the capsule), `celestina/src/island/view.rs` only if the clock face needs a constructor of its own
- Test: `banners.rs` tests, `src/island/tests/` (new `lent.rs`), app-level tests where `sync_banners` is covered today

**Interfaces:**
- Consumes: Task D11.1's `IslandNotice`, `NoticeLook`, `Gesture`.
- Produces:
  - `Island::lend_capsule(&mut self, lent: bool)`: while lent, the Island paints no capsule glass or capsule tree, leaves its rect out of `shapes` (blur and input regions) and ignores pointer input over it; the bar's placement keeps `island_w`;
  - `Island::rest_rect(&self) -> Rect` and the capsule's clock and glance for the surface to paint the same face (`capsule_with` with the panel's clock, `Glance` and `surface_w`).

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn the_surface_spans_the_output_and_is_440_high()            // LayerSpec: Overlay, anchor top + both sides, exclusive 0
#[test] fn regions_are_the_painted_island_and_sheets_only()
#[test] fn a_lent_capsule_leaves_paint_shapes_and_input()           // Island::shapes has no Island rect; a click on it reaches nothing
#[test] fn the_capsule_is_lent_after_the_first_frame_and_taken_back_before_release()
#[test] fn a_toast_during_the_shrink_keeps_the_surface()
#[test] fn focus_moving_to_another_output_hands_the_capsule_back()
#[test] fn right_click_wheel_up_and_middle_click_send_their_commands()
#[test] fn hover_and_leave_send_hover_commands_and_release_on_destroy()
```

- [ ] **Step 2: Run them to make sure they fail.**

- [ ] **Step 3: Implement.** The surface is created on the first toast (or level, Task D11.3) for `banner_output`, paints at the Island's rest rect, and asks the app to lend that output's capsule after its first frame is drawn; it stays while `IslandNotice::at_rest` is false. At rest the app takes the capsule back first and releases the surface on the next loop turn. Paint order: sheets, then the Island's glass (`Painter::haze`), then the faces through `Painter::blurred` when their blur is above 0.05. Keep the two log lines (`banners surface requested` with `created`, `toasts`; `banners surface released`). Right, middle and wheel come from the raw `InputEvent` (the `Interaction` only clicks with the left button); a wheel step towards the top is a negative vertical axis value.

- [ ] **Step 4: Run** `cargo test -p celestina` and both clippy runs (`--all-targets`, and the default). Expected: PASS.

- [ ] **Step 5: Commit** `celestina: Replace the banner cards with the Island growing into the notice`.

---

### Task D11.3: the level display in the Island

**Files:**
- Modify: `celestina-rs/crates/celestina-shell/src/settings.rs` (`OsdPosition::Island`, `#[serde(rename = "island")]`; the default stays `Bottom`)
- Modify: `celestina/src/osd/surface.rs` (with `Island`, hand the shown reading to the notices model instead of creating `celestina-osd`; while the Island page is open on the focused output, fall back to `Bottom`), `celestina/src/app.rs`
- Modify: `celestina/src/notifications/island_notice.rs` only through its Task D11.1 interface (`set_level`)
- Test: `settings.rs` tests, `osd/surface.rs` tests (`Fake` host), `banners.rs` tests

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn osd_position_reads_island_and_defaults_to_bottom()
#[test] fn island_position_creates_no_osd_surface_and_sets_the_level()
#[test] fn an_open_island_page_falls_back_to_the_bottom_display()
#[test] fn the_level_leaves_after_timeout_ms_and_the_notice_timeout_resumes() // Hover(false) sent once
#[test] fn an_open_slider_menu_still_suppresses_the_level()
```

- [ ] **Step 2: Run them to make sure they fail.**
- [ ] **Step 3: Implement.** The OSD duty keeps its timer and suppression; only where it shows changes.
- [ ] **Step 4: Run** `cargo test -p celestina -p celestina-shell` and clippy. Expected: PASS.
- [ ] **Step 5: Commit** `celestina: Add the island position of the level display`.

---

### Task D11.4: the notice card inside an open Island page

**Files:**
- Modify: `celestina/src/island/mod.rs`, `celestina/src/island/state.rs` (the page's height takes the card), `celestina/src/island/view.rs` or `celestina/src/notifications/banners.rs` (the card's view), `celestina/src/app.rs` (route the newest toast to the Island when its page is open on the banner output, instead of the notices surface)
- Test: new `celestina/src/island/tests/notice_card.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn an_open_page_takes_the_notice_as_a_card_at_its_top()     // card (12,8,336,80) inside the page; page height grows by 88 on its resize motion
#[test] fn the_notifications_page_shows_no_card()
#[test] fn the_card_takes_the_notice_gestures()                       // left/right/middle/wheel as in D11.1
#[test] fn closing_the_page_with_a_notice_out_hands_it_to_the_notices_surface()
```

- [ ] **Step 2–4:** fail, implement, pass (`cargo test -p celestina`, clippy).
- [ ] **Step 5: Commit** `celestina: Add the notice card at the top of an open Island page`.

---

### Task D11.5: nested shots, validation and docs

**Files:**
- Create: `celestina/docs/evidence/2026-11-26-cel-d-011-island-notices.md` (stub opened by the controller; this task fills it)
- Modify: `celestina/VALIDATION.md` (new `VAL-CEL-D-011`; update `VAL-CEL-BANNERS` steps that name the cards), `celestina/README.md` lines that describe the banners and the OSD position

- [ ] **Step 1:** Through the guarded kit, capture: rest; a notice at 120, 240 and 400 ms; the shrink at 150 ms; a burst at rest and fanned (scripted pointer); a right click closing; `osd.position: "island"` with three volume steps; a notice with the Clock page open. Confirm from the logs that `banners surface released` follows every burst and that no frame at rest is drawn.
- [ ] **Step 2:** Run the smoke phases `notify`, `bar` and `osd` the way the landing does (`--smoke bar,osd,notify` equivalent through the kit) and record the result.
- [ ] **Step 3:** Write `VAL-CEL-D-011` (the author's steps on the real session, three monitors, including the exact `shell.json` line `"position": "island"`), the evidence (procedure, shots, limits) and the README lines.
- [ ] **Step 4: Commit** `celestina: Add the evidence and validation steps of the Island notices`.

---

## Model table (heuristic estimate)

| Chunk | Model | Reasoning | Why |
|---|---|---|---|
| D11.1 model | Sonnet | medium | Pure state and curves with exact numbers and tests in the plan |
| D11.2 surface and lending | Sonnet | medium | Integration across `app.rs` and the Island with a precise hand-over; the reviewer is Opus |
| D11.3 level display | Sonnet | medium | Small setting plus routing, tests given |
| D11.4 page card | Sonnet | medium | View and routing on existing page motion |
| D11.5 nest and docs | Sonnet | medium | Guarded nest captures and evidence |
| `/simplify` | this session | n/a | In session, no subagents |
| Final review | Opus | high | `/code-review`, different model from the implementers; at most 2 rounds |
