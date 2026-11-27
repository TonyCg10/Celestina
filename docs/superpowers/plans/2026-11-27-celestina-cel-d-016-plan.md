# CEL-D-016 Implementation Plan: app menus as capsule menu windows

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A tray app's menu stops being an `xdg_popup`. A bar icon's menu comes out of the icon as a capsule menu window, with the drop of `CEL-D-013` and the pill of `CEL-D-014`. A hidden app's menu unfolds as a card under its row inside the hidden-apps window while the other rows blur, and the Island's Tray page uses the same card. Submenus open inside the window, and a tall menu scrolls.

**Architecture:**
- One pure **menu pane** (`tray/pane.rs`) shows a dbusmenu tree. It owns the drill-in stack, the slide, the back row, its own scroll with the thin bar, and keyboard focus. It answers input with a `MenuOutcome`.
- Three holders use it:
  - a new sheet body `SheetBody::TrayMenu` for a bar icon;
  - a `MenuCard` inside `TrayOverflowBody` for the hidden-apps window;
  - a `MenuCard` on the Island's Tray page.
- The Island opens the holder at the press, so the drop runs at once. It asks the app for the menu with `IslandAction::AskTrayMenu`. The app hands each layout to the Island with `Island::tray_menu_arrived`.
- `CloseMenu` comes from one place. After each input, frame and output removal, the app compares which items' menus the Islands hold. Any item that is no longer held gets one `CloseMenu`.
- The tray's `PopupShell` path, `ChoiceSlot`, `ShellChoiceSlot`, `Chain`, `AskSink`, `MenuContext` and the `PopupClosed` choice hook all go.

**Tech Stack:** Rust; `celestina/src/{tray,sheet,island,app.rs}`, `celestina-ui` (`Tree`, `Interaction`, `Animated`, `Scroll`, `curve::ONE_UI`), `celestina-paint` (`Painter::blurred`), `celestina-wayland` input events.

**Spec:** `docs/superpowers/specs/2026-11-27-celestina-menu-drop-design.md` §5.6 (app menus), §5.7 (reduced motion) and §6 (calls made after the preview); §5.1 to §5.5 are consumed as landed by `CEL-D-013`, `CEL-D-014` and `CEL-D-015`. Approved preview: "Celestina · la gota", version 3, approved by the author on 2026-11-27 (private artifact; source in the sessions' notes `cel-d/preview-approved/src16.html`, the tray IIFE at the end: `blurRows`, `toggleApp`, `shut`, `tick`, the `.ocard`/`.orow`/`.sbar` styles). Code map used to write this plan: `cel-d/d016-map-tray.md` in the same notes.

**Branch:** `unit/celestina/CEL-D-016`, opened with `scripts/worktree.sh open celestina CEL-D-016` from `main` **after `CEL-D-013` has landed** (`CEL-D-014` and `CEL-D-015` land before it). Kind `milestone`. Size: large; executed in its own session.

**Before Task 1, the controller reads the landed `CEL-D-013`, `CEL-D-014` and `CEL-D-015` code.** This plan names their parts by behaviour. Their real names replace the placeholders below in each task's brief:
- the drop's open and its phase boundaries;
- the content blur wrapper;
- the open pill;
- the travel switch.

## Global Constraints

- **Pane content (§5.6).**
  - Rows 40 (`theme.row_height`); separators 17 (`2 · spacing.sm + 1`, a 1 px `Role::Divider` line with `spacing.md` side padding).
  - A 16 px check column (`MARK`): `check` when `toggle == Some(true)`, else an empty 16 × 16 spacer. A `chevron-right` 16 px for a submenu.
  - Label `type_scale.body`, `Weight::Regular`, `Role::TextHi`; disabled entries `Role::TextFaint`, not interactive, not focusable.
  - Row padding `spacing.md` across, gap `spacing.md`, radius `radius_sm`.
  - Labels cut at 36 graphemes with U+2026 (35 kept + `…`, `short`). In a card, a label still wider than its room is cut by measure as well (Review Focus 5).
- **Shell entry.** Last row of the root level only, after a separator unless the level is empty or already ends in one. `ShellEntry::Hide` = `copy::TRAY_HIDE` with `eye-off`; `ShellEntry::Show` = `copy::TRAY_SHOW` with `eye`. With no `Id` there is no shell entry. A menu is never cut: past its room it scrolls.
- **Bar window.**
  - 300 wide (`MENU_W`) and 8 px padding all round (`PaneFrame::WINDOW`).
  - Height `min(content, geometry::max_height(surface))`, which is 496 on the 560 surface. The pane scrolls itself, so the sheet's own scroll never engages for this body.
  - Anchored to the icon's piece like any capsule menu: the capsule's edge by `Edge::of_zone`, y 56.
- **Submenu drill-in.**
  - Slide 200 ms `ONE_UI`. Going in, the old level moves 0 → −w and the new one +w → 0. Going back reverses it.
  - The window's height follows on the sheet's resize morph (`Motion::Normal`, 200 ms, `ONE_UI`).
  - First row of a sublevel: the back row, 40 high, `chevron-left` 16 in the check column, the parent entry's label (`short`) in `Role::TextHi`.
  - Left and Escape go back one level. At the root, Escape closes (bar window) or folds (card), and Left does nothing.
  - Back restores the parent level's focused row.
- **Scroll.**
  - The wheel moves the offset by the event's vertical px (no slide).
  - The thumb is the launcher's (`launcher/view.rs` `THUMB_W`, moved with `scrolling.rs`), 3 px from the right edge, radius half its width, `Role::ControlHover`, its height and y from `scrolling::thumb` (`THUMB_MIN` 24). It shows until `THUMB_LINGER` (800 ms) after the offset last moved.
  - A keyboard focus move reveals the focused row (`scrolling::reveal`). Drilling in or back starts the new level at offset 0, then reveals its focus.
- **Waiting (§5.6).**
  - The holder opens at the press.
  - Bar: the drop runs its swell and fall (0 → 0.52 of the 460 ms open) and holds there until the layout arrives. It then runs the open phase (0.52 → 1) to the menu's size. A layout that arrives before 0.52 · 460 ms does not stop the drop.
  - Hidden-apps window and Tray page: the row shows `Role::ControlHover` (its press fill) until the card unfolds.
  - With no layout after `MENU_LATE_LAYOUT` (500 ms), the holder gets the shell entry alone, as today. With no shell entry, the holder closes or cancels.
  - `NoMenu` (no dbusmenu, or the layout failed) keeps the host's `ContextMenu` fallback. The holder then gets the shell entry alone, or closes when there is none.
- **Hidden-apps window (§5.6).**
  - 280 wide (`LIST_W`, was 240), padding 8, row gap 2, rows 40. The card's width is 264.
  - The card sits right under the open row:
    - fill `Role::ControlFill`, radius 12 (`radius_md`);
    - 2 px above and 4 px below while its height is above 1 px, 0 otherwise;
    - inner padding 4 down, 0 across (`PaneFrame::card(264)`).
  - Card height cap `min(content, max_height(surface) − list_h − 6)`, where `list_h` is the window body's height without the card. Past the cap the card scrolls.
  - **Unfold:** height 0 → cap with `OutBack(1.15)` over 350 ms. Content blur `12 · (1 − ONE_UI(t))` px, content opacity `clamp((t − 0.12) / 0.5, 0, 1)`.
  - **Fold:** height current → 0 with `ONE_UI` over 350 ms. Content blur `12 · t`, opacity `1 − t / 0.6` (clamped). The card is gone at t = 1.
  - Opening the same row again folds the card. A press on another app's row removes the first card at once, with no fold, and the new one waits or unfolds.
  - A drill-in inside a card retargets the card's height on `ONE_UI` over 200 ms.
  - The window's glass follows the body's height every frame while the card moves, with no window morph.
- **Row blur (§5.6).**
  - While a card is open, a list row at distance `d` from the open row gets sigma 1.5 (`d = 1`), 3 (`d = 2`) or 6 (`d ≥ 3`) px. Its opacity is `1 − 0.6 · sigma / 6` (0.85, 0.7, 0.4).
  - Rows change over 250 ms `ONE_UI` from where they are, and go back to sharp from the start of the fold.
  - The open row is `Role::ControlHover` with its title in `Weight::Semibold`.
- **Gestures (§5.6).**
  - Unchanged: `tray_piece::row_action` decides what each button does, on the bar icon, on a window row and on a page row.
  - With an app's menu open from its bar icon, a primary press on that icon only closes the menu. The gesture that opens the menu closes it too. Middle and wheel act on the app as usual.
  - Choosing an entry or the shell entry sends it and closes the whole window: the bar window, or the whole hidden-apps window from a card. The Tray page card folds at once.
  - In the hidden-apps window, Escape works in this order: back one level in the card, then fold the card, then close the window.
- **Reduced motion (§5.7).**
  - No drop: a waiting bar holder paints nothing until the layout arrives, then fades in with `CEL-D-013`'s 150 ms fade.
  - No slide.
  - The card unfolds and folds at once, and rows blur and sharpen at once.
- **`CloseMenu`** is sent exactly once per item, when no Island holds that item's menu any more, whatever closed it. A choice's `MenuEvent` is sent before that `CloseMenu`.
- **Painter.** A `Painter::blurred` call made inside another `blurred` layer draws plainly, because femtovg offscreen targets do not nest (`Backend::end_offscreen` sets the target back to the screen). So rows paint sharp, with only their opacity, while the window's drop or close blurs its content.
- **Copy.** Spanish only in `src/copy.rs`; no new strings, since the back row shows the app's own label. Fixture labels in tests and the nest's fake item are English.
- **Repository rules.**
  - No `unwrap`/`expect` in production; no `unsafe` outside `egl.rs`/`pam.rs`.
  - Never touch `src/lock`, `src/polkit`, `celestina/scripts/`, `.githooks`, any `AGENTS.md` or `docs/contracts|governance|standards`.
  - At rest, no frame is asked for: no linger, card, slide or row clock is left running.
- **Live-session safety.**
  - Never run `celestina` or `niri msg` from the agent's own shell.
  - Nests only through the guarded kit: `celestina/scripts/dev-session.sh --own-bus --once … --probe …` under `env -i`, with `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`, fake `wpctl`/`pactl` and a private Cargo target dir.
  - One nest at a time, under `timeout -k 60 2700`. Never `-s KILL`; never `pkill` by name.

## Review Focus

1. **A late answer after the holder is gone.** The person presses an icon and presses Escape, or opens another icon's menu, before dbusmenu answers. No window or card reappears, the late `Menu` is dropped as unasked, and the first item gets one `CloseMenu`. Test in Task 3: `a_late_layout_after_the_drop_closed_opens_nothing_and_closes_the_menu_once`.
2. **A republish that removes the open submenu, during or after the slide.** The pane falls back to the deepest level still in the tree. The focus never points at a vanished row, and nothing panics. Test in Task 1: `a_republish_that_removes_the_open_submenu_falls_back_to_the_deepest_level_left`.
3. **The app quits while its menu is open.** With a bar icon, the piece vanishes and the window closes through the usual path. With a card, the row leaves the list and the card goes at once. Either way, one `CloseMenu`. Tests in Task 3 (`the_icon_leaving_closes_its_menu_window_and_sends_close_menu_once`) and Task 4 (`a_card_whose_app_leaves_goes_at_once_and_the_window_stays`).
4. **A 64-entry level plus the shell entry.** The menu scrolls, Down from the top reaches the shell entry with it in view, and nothing is cut. Test in Task 1: `a_long_level_scrolls_and_the_keyboard_reaches_the_shell_entry_in_view`.
5. **A 36-grapheme wide label (W, CJK) in the 264 px card.** The label stays inside its row, before the chevron. Test in Task 4: `a_wide_label_in_a_card_ends_inside_its_row`.

---

### Task 0 (controller): worktree, ledger row and evidence stub

- [ ] `scripts/worktree.sh open celestina CEL-D-016` from `main` with `CEL-D-013` landed. Run `python3 scripts/agent-context.py celestina/src/tray` and read what it lists.
- [ ] Add the row to `celestina/docs/plans/active/2026-11-25-cel-tracks.md`, after the last `CEL-D` row:
  `| CEL-D-016 | \`celestina:\` | active | pending | pending | Replace the tray's app menu popups with capsule menu windows: a bar icon's menu comes out of its icon with the drop, a hidden app's menu unfolds as a card under its row while the other rows blur, submenus open inside the window and a tall menu scrolls (the author's request of 2026-11-27; la gota preview version 3 approved 2026-11-27; [design](../../../../docs/superpowers/specs/2026-11-27-celestina-menu-drop-design.md), [plan](../../../../docs/superpowers/plans/2026-11-27-celestina-cel-d-016-plan.md)) | [evidence](../../evidence/<date>-cel-d-016-app-menus.md) | VAL-CEL-D-016 |`
- [ ] Create the stub `celestina/docs/evidence/<date>-cel-d-016-app-menus.md`: title, unit, "filled by Task 6".
- [ ] Keep the SDD ledger and task reports in the notes folder (`cel-d/sdd/`), not in the worktree.
- [ ] Commit `celestina-maintenance: Add the CEL-D-016 ledger row and evidence stub`.

---

### Task 1: the menu pane

**Files:**
- Move: `celestina/src/launcher/scrolling.rs` → `celestina/src/scrolling.rs` (`git mv`; `mod scrolling;` in `main.rs`/`lib.rs` wherever `launcher` is declared; launcher imports `crate::scrolling`; its tests move with it unchanged).
- Create: `celestina/src/tray/pane.rs` (declare in `tray/mod.rs`).
- Modify: `celestina/src/tray/menu.rs`: make `ShellEntry`, `MenuRow`, `row_at`, `rows`, `row_count`, `level_at`, `short`, `MENU_W`, `MAX_LABEL_GRAPHEMES`, `MARK`, `entry_key`, `separator`/`entry_row`/`shell_row` builders `pub(crate)` for the pane. The popup code stays until Task 3.
- Test: `tray/pane.rs` `#[cfg(test)] mod tests`, using `tray/menu.rs`'s `fake_measure` style (copy the helper).

**Interfaces:**
- Consumes: `MenuTree`, `MenuEntry`, `ItemId` (`tray/model.rs`); `crate::scrolling::{reveal, thumb, THUMB_MIN, THUMB_LINGER}`; `celestina_ui::{Tree, Interaction, Scroll, Key, view}`; `curve::ONE_UI`.
- Produces (all `pub(crate)`):
  - `struct PaneFrame { width: f32, pad_x: f32, pad_y: f32 }` with `const WINDOW: PaneFrame = { 300, 8, 8 }` and `fn card(width: f32) -> PaneFrame` = `{ width, 0, 4 }`;
  - `enum MenuOutcome { Keep { changed: bool }, Choose(i32), Shell(ShellEntry), Submenu(i32), Back, Close }` (`Copy`): `Submenu(id)` asks the item's `AboutToShow(id)`; `Back` went up one level; `Close` is Escape at the root;
  - `struct MenuPane` with:
    - `fn new(theme: &Theme, tree: MenuTree, shell: Option<ShellEntry>, frame: PaneFrame, max_h: f32, reduced: bool, now: Instant) -> MenuPane`;
    - `fn replace(&mut self, tree: MenuTree, shell: Option<ShellEntry>, now: Instant) -> bool`: re-resolves the stack's path of entry ids with `level_at`, pops to the deepest level still there, keeps the focus by row key, opens an awaited submenu once it has entries; whether the look changed;
    - `fn size(&self) -> (f32, f32)`: `(frame.width, min(content, max_h))` of the current level, content = `2 · pad_y + Σ row heights` (+ 40 for the back row below the root);
    - `fn content_h(&self) -> f32`, `fn depth(&self) -> usize`, `fn focus_first(&mut self)`, `fn focused_rect(&self) -> Option<Rect>` (pane coordinates, scroll taken off);
    - `fn input(&mut self, ev: &InputEvent, now: Instant) -> MenuOutcome`;
    - `fn wheel(&mut self, vertical: f32, now: Instant) -> bool`, `fn can_scroll(&self) -> bool`;
    - `fn set_theme(&mut self, theme: &Theme, now: Instant)`, `fn layout(&mut self, …)` (the body's layout signature), `fn paint(&mut self, painter: &mut Painter, now: Instant)` (its own clip at `(0, 0, size)`; both levels while sliding, translated; the thumb), `fn is_animating(&self, now: Instant) -> bool`, `fn tree(&self) -> &Tree`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn rows_map_entries_separators_toggles_and_disabled_state()   // heights 40/17; check only for Some(true); disabled not focusable; port of menu.rs :1073
#[test] fn the_root_ends_with_the_shell_entry_after_one_separator()   // Hide → eye-off/TRAY_HIDE, Show → eye/TRAY_SHOW; none when shell is None; no double separator
#[test] fn an_app_without_entries_gets_the_shell_entry_alone()
#[test] fn labels_are_cut_at_36_graphemes()                            // 35 kept + '…'
#[test] fn the_window_frame_is_300_wide_with_8_of_padding()           // 3 entries → (300, 136)
#[test] fn arrows_skip_disabled_entries_and_separators_and_enter_chooses() // Down/Up = Tab/Shift+Tab, KeyRepeat too; Enter → Choose(id)
#[test] fn right_or_enter_on_a_submenu_drills_in_and_asks_about_to_show() // → Submenu(id); depth 1; first row is the back row with the parent's label
#[test] fn a_submenu_without_entries_waits_and_opens_when_a_republish_fills_it() // depth stays 0 until replace() brings entries; then depth 1
#[test] fn the_drill_in_slides_200_ms_and_back_reverses()              // at 100 ms old level x = -w·ONE_UI(0.5), new x = w·(1-ONE_UI(0.5)); settled at 200 ms
#[test] fn left_and_escape_go_back_and_restore_the_parents_focus()    // → Back; at depth 0 Escape → Close, Left → Keep{changed:false}
#[test] fn a_click_on_the_back_row_goes_back()
#[test] fn a_republish_keeps_the_focus_on_its_row()
#[test] fn a_republish_that_removes_the_open_submenu_falls_back_to_the_deepest_level_left() // Review Focus 2: also mid-slide; focused row exists
#[test] fn a_long_level_scrolls_and_the_keyboard_reaches_the_shell_entry_in_view() // Review Focus 4: 64 entries + shell; size().1 == max_h (496); after enough Down presses focused_rect within [0, 496)
#[test] fn the_wheel_scrolls_and_the_thumb_lingers_800_ms()            // thumb w 3, x = width - 6; gone at +800 ms; is_animating false after
#[test] fn drilling_in_starts_the_new_level_at_the_top()
#[test] fn reduced_motion_drills_in_without_a_slide()
#[test] fn nan_and_huge_inputs_stay_finite()                           // max_h NaN/0, wheel NaN
```

- [ ] **Step 2: Run them to make sure they fail.** `cargo test -p celestina --locked tray::pane` from `celestina/`. Expected: FAIL (module missing).
- [ ] **Step 3: Implement `MenuPane`.**
  - Keep a stack of `Level { path: Vec<i32>, tree: Tree, focus: Option<Key>, back_label: Option<String> }` and a `Slide { from: Level, dir, start }` painted under the pane's clip.
  - The pane owns a `celestina_ui::Scroll` whose extent is `(content_h, size().1)`.
  - Reuse `tray/menu.rs`'s row builders without its `.haze`, since the holder has the glass.
  - The Up/Down → Tab remap is `LevelCore::input`'s; move it to a shared `fn arrows_as_tab(ev: &InputEvent) -> Option<InputEvent>` in `pane.rs`, and have `menu.rs` call it until Task 3 deletes `LevelCore`.
- [ ] **Step 4: Run** `cargo test -p celestina --locked` and `cargo clippy --workspace --all-targets --locked -- -D warnings`. Expected: PASS; the launcher's scrolling tests still pass at their new path.
- [ ] **Step 5: Commit** `celestina-maintenance: Add the tray menu pane with drill-in submenus and scrolling`.

---

### Task 2: the drop holds while the menu is asked for

**Files:**
- Modify: `celestina/src/sheet/motion.rs`, where `CEL-D-013`'s Task 2 makes `SheetMotion` hand out one `drop::DropFrame` per instant. If it landed under other names, use the real ones.
- Test: `motion.rs` tests, in its helpers' style (`ms(t0, n)`, `near`).

**Interfaces:**
- Consumes: `CEL-D-013`'s `sheet/drop.rs`: `OPEN_MS` (460), `CLOSE_MS` (400), `OPEN_HOLD` (0.52, which its plan exports for this unit) and `open_frame`/`close_frame`; its reduced 150 ms fade.
- Produces:
  - `SheetMotion::hold(&mut self, now: Instant)`: an open in progress stops at `drop::OPEN_HOLD` when it gets there (before that it runs on);
  - `SheetMotion::release(&mut self, now: Instant)`: if the open had stopped, it continues from `OPEN_HOLD` at `now` (the open phase takes `0.48 · 460` ms from `now`); else it just drops the hold;
  - `SheetMotion::is_held(&self, now: Instant) -> bool`: stopped at `OPEN_HOLD`, which keeps `wants_frame` false while still.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn a_held_open_stops_as_a_drop_at_52_percent()        // shape at +239 ms == shape at +900 ms; wants_frame false while held
#[test] fn a_release_after_the_hold_opens_in_the_remaining_221_ms() // release at +900: window rect reached at +900+221; content opacity (t-0.6)/0.34 on the resumed clock
#[test] fn a_release_before_52_percent_never_stops_the_drop() // release at +100: same shapes as an unheld open at every 20 ms
#[test] fn closing_a_held_drop_rises_back_into_the_capsule()  // close from the drop's shape; no frame grows the rect; done within 400 ms
#[test] fn reduced_motion_holds_with_nothing_shown_and_fades_in_on_release() // content and glass 0 while held; 150 ms fade from release
```

- [ ] **Step 2: Run them to make sure they fail.** `cargo test -p celestina --locked sheet::`. Expected: FAIL.
- [ ] **Step 3: Implement** `hold`/`release` as a pause of the open's clock: shift its start by the time held. No new curve.
- [ ] **Step 4: Run** the tests and clippy. Expected: PASS; every `CEL-D-013` test unchanged.
- [ ] **Step 5: Commit** `celestina-maintenance: Add a hold to the drop for a menu that is still being asked for`.

Tasks 1 and 2 touch different files and may run in parallel worktrees.

---

### Task 3: the bar icon's menu window, one close path, and the popup path removed

**Files:**
- Modify: `celestina/src/pieces/mod.rs` (`SheetKind::TrayMenu`).
- Create: `celestina/src/sheet/tray_menu.rs` (`TrayMenuBody`).
- Modify: `celestina/src/sheet/body.rs` (variant, every `match`, new outcome) and `celestina/src/sheet/mod.rs` (hold wiring, `takes_wheel`).
- Modify: `celestina/src/island/mod.rs`:
  - the `PieceAction::TrayMenu` arm at ~1860;
  - the primary-on-open rule;
  - remove `tray_menu`, `tray_menu_on_bar`, `bar_menu_asked`, `overflow_menu_asked`, `tray_menu_from_window`, `open_tray_menu`, `tray_card_orphaned`, and the tray branches of `menu_closed`/`close_page_menus`;
  - `IslandAction::OpenTrayMenu` → `AskTrayMenu`;
  - add `TrayShellEntry`.
- Modify: `celestina/src/island/sheets.rs` (`sheet_request` gate, `apply_outcome`, `body_takes_escape`, `open_request` stale list) and `celestina/src/app.rs`:
  - `PendingMenu`, `open_tray_menu`, `expire_pending_menu`, the `NoMenu` arm;
  - remove `OpenTrayMenu`, `open_tray_card`, `give_up_menu_or_card`, `give_up_menu`, `tray_menu_gone`, `tray_choice`, `tray_shell_choice`, and the `PopupClosed` choice block at ~3811-3848 and its imports at ~85-95;
  - add `sync_tray_menus`.
- Modify: `celestina/src/tray/menu.rs`: delete `TrayMenu` (`PopupShell`), `LevelCore`, `Outcome`, `ChildClosed`, `child_closed`, `placement`, `submenu_placement`, `drawn_rows`, `drawn_len`, `level_size`, `outgrown`, `MAX_MENU_H`, `ChoiceSlot`, `ShellChoiceSlot`, `MenuModel`, `SharedMenu`, `AskSink`, `Chain`, `MenuContext` and their tests. Rewrite the module doc: rows only. The log line `tray menu: dismissed by the compositor` goes with `TrayMenu::dismissed`.
- Overflow rows and Tray page rows: until Tasks 4 and 5, their menu gesture sends `AskTrayMenu` and holds nothing visible.
- Tests:
  - `celestina/src/sheet/tray_menu.rs` tests;
  - new `celestina/src/island/tests/tray_menu.rs` (declare in `island/tests/mod.rs`);
  - `app.rs` tests;
  - rewrite the bar-icon tests at `island/mod.rs` ~3033 and ~3569 to the new action;
  - delete the popup-only tests: `a_dismissed_menu_closes_the_page_but_an_island_press_does_not`, `a_parent_or_shell_close_of_the_menu_leaves_the_page_open`, `the_trays_first_level_closes_like_the_overflow_menu`, and in `island/tests/tray_overflow.rs` `escape_closes_the_apps_menu_first_then_the_window`, `a_card_hung_from_a_window_row_goes_with_the_window` and `a_card_goes_when_its_app_is_shown_while_the_window_stays`. Tasks 4 and 5 replace them.

**Interfaces:**
- Consumes: Task 1's `MenuPane`, `MenuOutcome`, `PaneFrame`; Task 2's `hold`/`release`; `CEL-D-014`'s open pill (the tray icon gets it as any sheet's piece).
- Produces:
  - `SheetKind::TrayMenu` (no payload; the body holds the item);
  - `TrayMenuBody` (`pub(crate)`):
    - `fn waiting(item: ItemId, keyboard: bool, max_h: f32) -> TrayMenuBody`, sized `(300, 0)` while it waits;
    - `fn arrive(&mut self, theme: &Theme, tree: MenuTree, shell: Option<ShellEntry>, reduced: bool, now: Instant) -> bool`: builds the pane, or `replace`s it when shown, and calls `focus_first` when `keyboard`;
    - `fn item(&self) -> &ItemId`, `fn is_waiting(&self) -> bool`, `fn depth(&self) -> usize`;
  - `SheetBody::TrayMenu(Box<TrayMenuBody>)`. Its `takes_wheel` is true when the pane can scroll and `takes_escape` while `depth() > 0`; `SheetBody::focused_rect(&self) -> Option<Rect>` returns `None` for the other bodies;
  - `BodyOutcome::TrayMenu(MenuOutcome)`;
  - `IslandAction::AskTrayMenu { item: ItemId, at: (i32, i32) }` and `IslandAction::TrayShellEntry(ItemId, ShellEntry)`;
  - Island:
    - `fn tray_menu_arrived(&mut self, item: &ItemId, tree: MenuTree, shell: Option<ShellEntry>, now: Instant) -> bool` (`false` when nothing on this Island holds `item`);
    - `fn tray_menu_failed(&mut self, item: &ItemId, now: Instant)` (the waiting holder closes);
    - `fn tray_menus_held(&self, now: Instant) -> Vec<ItemId>` (waiting or shown, not closing or folding);
  - `Sheets`: a body that `is_waiting()` opens with `motion.hold(now)`; the first `sync`/`input` after it stops waiting calls `motion.release(now)` and retargets to the body's size;
  - app: `PendingMenu { surface, item, until }` (the `row` and `keyboard` fields go); pure `fn closed_tray_menus(before: &[(SurfaceId, ItemId)], after: &[(SurfaceId, ItemId)]) -> Vec<ItemId>` (items in `before` held on no surface in `after`); `App::sync_tray_menus(&mut self)`.
- The flow, decided here:
  1. The bar icon's `TrayMenu` action. If the open sheet is this piece's `TrayMenu`, the press closes it. Otherwise `open_sheet_at(key, SheetKind::TrayMenu, waiting body)` runs and the Island pushes `AskTrayMenu { item, at: output_point(pointer) }`.
  2. Any primary press on the icon of the open `TrayMenu` sheet only closes the menu (no `Activate`). If `CEL-D-014` already closes on a primary press on the open piece, only add the test.
  3. `AskTrayMenu` in the app: `Pending::replace` (the replaced request is *not* given up here), then `TrayCommand::OpenMenu(item, x, y)`.
  4. `TrayEvent::Menu(item, tree)`: an Island holding `item` gets `tray_menu_arrived` (a live refresh). Otherwise `Pending::of` decides:
     - `Open`: `tray_menu_arrived` on `pending.surface`, with `tray_shell_entry_for(item)`;
     - `Wait`: `redraw_after`, as today;
     - `GiveUp`: deliver the shell entry alone, or `tray_menu_failed` when there is none.
  5. `NoMenu`: shell entry alone, or `tray_menu_failed`. The expiry (`Pending::due`) does the same, and the item stays followed, so a late layout refreshes the shell-only menu in place.
  6. `MenuOutcome` in `apply_outcome`:
     - `Choose(id)`: `IslandAction::Tray(MenuEvent(item, id))`, then the sheet closes;
     - `Shell(e)`: `TrayShellEntry(item, e)`, then the sheet closes;
     - `Submenu(id)`: `Tray(AboutToShow(item, id))`;
     - `Back` and `Keep`: nothing;
     - `Close`: the sheet closes.
  7. `App::sync_tray_menus` runs after every Island's input actions have been handled, after every Island frame, and after an output's Island is removed. It sends `CloseMenu` for each `closed_tray_menus` item and drops `pending_menu` when its item is held nowhere. The app sends `CloseMenu` nowhere else.

- [ ] **Step 1: Write the failing tests**

```rust
// sheet/tray_menu.rs
#[test] fn a_waiting_body_is_300_wide_and_arrives_at_its_menus_height()
#[test] fn a_body_taller_than_496_stays_496_and_takes_the_wheel()
#[test] fn escape_belongs_to_the_body_only_below_the_root()
// island/tests/tray_menu.rs
#[test] fn a_press_on_a_tray_icon_opens_a_held_drop_and_asks_for_the_menu() // AskTrayMenu{item, at}; kind TrayMenu; motion held at 0.52 after 239 ms
#[test] fn the_layout_opens_the_drop_to_the_menus_size()                   // window (x per Edge::of_zone, 56, 300, h)
#[test] fn a_primary_press_on_the_open_icon_only_closes_its_menu()        // no IslandAction::Tray(Activate)
#[test] fn the_opening_gesture_again_closes_the_menu()
#[test] fn choosing_sends_the_event_and_closes_the_window()               // [Tray(MenuEvent(item, id))]; then tray_menus_held empty
#[test] fn the_shell_entry_sends_its_action_and_closes_the_window()
#[test] fn a_submenu_asks_about_to_show_and_the_window_follows_its_height()
#[test] fn escape_goes_back_then_closes()
#[test] fn every_close_path_leaves_nothing_held()   // Escape, outside press, FocusLost, choose, another capsule's menu, Island page open, close_everything, piece gone
#[test] fn the_icon_leaving_closes_its_menu_window_and_sends_close_menu_once() // Review Focus 3 (Island half)
#[test] fn without_a_shell_entry_a_failed_menu_closes_the_drop()
// app.rs
#[test] fn closed_tray_menus_names_each_item_once()           // before [(s1,a),(s1,b)], after [(s1,b)] → [a]; same item on s2 kept → []
#[test] fn a_late_layout_after_the_drop_closed_opens_nothing_and_closes_the_menu_once() // Review Focus 1: pending dropped by sync; Pending::of → Unasked
#[test] fn an_empty_root_past_500_ms_gets_the_shell_entry_alone()
#[test] fn a_request_for_another_item_replaces_the_old_without_giving_it_up() // replaces the old a_request_for_another_item_gives_up_the_old_one
#[test] fn the_choice_event_is_sent_before_close_menu()        // sync after actions: [MenuEvent, CloseMenu]
```

Keep, adjusted to the new `PendingMenu`: `empty_root_waits_for_one_layout_update`, `a_due_request_stays_for_its_card_until_the_message_takes_it`, `empty_root_gives_up_after_500_ms`, `a_menu_nobody_asked_for_is_unasked`, `hide_entry_moves_the_id_into_tray_hidden`. Keep every `tray/dbusmenu.rs`, `tray/model.rs`, `tray/item.rs` and `providers/tray.rs` test unchanged.

- [ ] **Step 2: Run them to make sure they fail.** `cargo test -p celestina --locked tray_menu` and `app::`. Expected: FAIL.
- [ ] **Step 3: Implement** the flow above. `TrayMenuBody` owns only a `MenuPane` once it arrives: no `Tree` of its own, and no haze, since the window's glass is the sheet's. `rg -n "OpenTrayMenu|tray_choice|ChoiceSlot|PopupShell for TrayMenu|bar_menu_asked|overflow_menu_asked" celestina/src` must return nothing.
- [ ] **Step 4: Run all checks** from `celestina/`: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo +1.100.0 clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` (sum every `test result` line). From the repo root: `bash scripts/check-architecture-contract.sh`, `bash scripts/check-documentation-contract.sh`, `python3 scripts/check-language-contract.py`. Expected: PASS.
- [ ] **Step 5: Commit** `celestina-maintenance: Replace the tray's menu popups from the bar with a capsule menu window`.

---

### Task 4: the card in the hidden-apps window

**Files:**
- Create: `celestina/src/tray/card.rs` (`MenuCard`, `RowBlurs`, `paint_with_row_blur`).
- Modify: `celestina/src/sheet/tray_overflow.rs`:
  - `LIST_W` 280;
  - `fit_titles` and `overflow_view` take the card and the row looks;
  - change the `rows_are_40_tall_in_a_240_wide_list_with_8_of_padding` test to 280 → `(280.0, 98.0)`.
- Modify: `celestina/src/sheet/body.rs` (`TrayOverflowBody`: card, takes_wheel over the card, takes_escape while a card is held, `follows_height`, `focused_rect`).
- Modify: `celestina/src/sheet/mod.rs`: `retarget` treats `body.follows_height(now)` like the bar's slide, so the window rect is the body's target with no morph.
- Modify: `celestina/src/island/sheets.rs`:
  - `overflow_row_gesture`'s `TrayMenu` arm toggles the card and pushes `AskTrayMenu`;
  - `overflow_menu_key`;
  - `overflow_pointer` sends the wheel over the card to the card;
  - Escape.
- Modify: `celestina/src/island/mod.rs`: `tray_menu_arrived`, `tray_menu_failed` and `tray_menus_held` reach the overflow body.
- Modify: `celestina/crates/celestina-paint/src/painter.rs`: nested-blur guard.
- Test: `tray/card.rs` tests, `sheet/tray_overflow.rs` tests, `island/tests/tray_overflow.rs`, and `painter.rs` tests (pure).

**Interfaces:**
- Consumes: Task 1's `MenuPane`, `PaneFrame::card`; Task 3's Island entry points and `AskTrayMenu`; `Painter::blurred`; `Tree::paint_inside`, and `Tree::paint_hiding`. `CEL-D-014` keeps `paint_hiding` for this task (with an allowed dead-code lint naming `CEL-D-016`); this task gives it its caller and removes that allowance.
- Produces (`pub(crate)`):
  - `fn blur_level(distance: usize) -> f32`: 0 for 0, then 1.5, 3, 6;
  - `fn blurred_opacity(sigma: f32) -> f32`: `1 − 0.6 · sigma / 6`;
  - `struct RowBlurs` with:
    - `fn set_open(&mut self, open: Option<usize>, count: usize, reduced: bool, now: Instant)`;
    - `fn look(&self, index: usize, now: Instant) -> (f32 /*sigma*/, f32 /*opacity*/)`;
    - `fn is_animating(&self, now: Instant) -> bool`;
  - `struct MenuCard` with:
    - `fn waiting(row: usize, item: ItemId, keyboard: bool) -> MenuCard`;
    - `fn arrive(&mut self, theme: &Theme, tree: MenuTree, shell: Option<ShellEntry>, width: f32, cap: f32, reduced: bool, now: Instant)`: unfolds, or replaces when already open;
    - `fn fold(&mut self, now: Instant)`, `fn is_folding(&self) -> bool`, `fn gone(&self, now: Instant) -> bool`;
    - `fn height(&self, now: Instant) -> f32` (margins included);
    - `fn content(&self, now: Instant) -> (f32 /*opacity*/, f32 /*blur*/)`;
    - `fn pane_mut(&mut self) -> Option<&mut MenuPane>`, `fn item(&self) -> &ItemId`, `fn row(&self) -> usize`, `fn is_waiting(&self) -> bool`;
  - `fn paint_with_row_blur(tree: &mut Tree, painter: &mut Painter, now: Instant, rows: &[(Key, f32, f32)])`: paints `tree` hiding the blurred rows, then each one through `Painter::blurred(row rect grown by 3·sigma, sigma, …)` at its opacity;
  - `SheetBody::follows_height(&self, now: Instant) -> bool`: true while a card's height moves;
  - painter: a `blur_depth` counter. A pure `fn blur_nested(depth: u32) -> bool` decides when `blurred` and `cut_row` draw plainly.

- [ ] **Step 1: Write the failing tests**

```rust
// tray/card.rs
#[test] fn rows_blur_by_distance_with_their_opacity()          // levels 0,1.5,3,6,6; opacities 1,0.85,0.7,0.4
#[test] fn row_blur_moves_over_250_ms_and_clears_on_fold()      // at 125 ms sigma = level·ONE_UI(0.5)
#[test] fn the_card_unfolds_on_the_spring_with_its_content_blurred_in() // h(t)=cap·OutBack1.15(t); blur 12 at 0, 0 at 350 ms; opacity 0 at t≤0.12, 1 at t≥0.62
#[test] fn the_card_folds_in_350_ms()                           // blur 12·t; opacity 0 by 210 ms; gone at 350 ms
#[test] fn margins_are_2_above_and_4_below_only_while_open()    // height(…) includes 6 when h>1, 0 at rest
#[test] fn the_card_is_capped_by_the_room_the_list_leaves()     // list_h 230 on 560: cap = 496-230-6 = 260; content 600 → pane scrolls
#[test] fn reduced_motion_unfolds_and_folds_at_once()
// sheet/tray_overflow.rs
#[test] fn rows_are_40_tall_in_a_280_wide_list_with_8_of_padding() // (280.0, 98.0)
#[test] fn a_wide_label_in_a_card_ends_inside_its_row()         // Review Focus 5: 36 'W' / 36 CJK; label right ≤ chevron x - gap
// island/tests/tray_overflow.rs
#[test] fn a_right_click_on_a_row_presses_it_and_asks_for_the_menu() // row ControlHover; AskTrayMenu; window unchanged until arrival
#[test] fn the_layout_unfolds_the_card_under_the_row_and_blurs_the_others()
#[test] fn the_window_glass_follows_the_card_every_frame()      // window h == body h at 0, 100, 200, 350 ms; no resize morph
#[test] fn the_open_row_is_grey_and_semibold()
#[test] fn the_same_row_again_folds_the_card()
#[test] fn another_rows_menu_removes_the_first_card_at_once()   // also while the first is still waiting: tray_menus_held == [second]
#[test] fn escape_goes_back_then_folds_then_closes_the_window()
#[test] fn choosing_in_a_card_closes_the_whole_window()
#[test] fn the_shell_entry_in_a_card_shows_the_app_and_closes_the_window()
#[test] fn a_card_whose_app_leaves_goes_at_once_and_the_window_stays() // Review Focus 3
#[test] fn the_wheel_over_the_card_scrolls_the_card_and_over_a_row_scrolls_the_app()
#[test] fn the_menu_key_toggles_the_card_of_the_focused_row_and_focuses_its_first_entry()
// painter.rs
#[test] fn a_blur_inside_a_blur_draws_plain()
```

Keep passing, at 280: `overflow_window_lists_hidden_apps_only`, `overflow_window_centres_under_the_chevron`, `overflow_row_left_click_activates`, `overflow_row_middle_click_secondary_activates`, `overflow_row_wheel_scrolls_the_app`, `a_hand_edit_of_the_hidden_list_updates_the_open_window`, `last_hidden_app_gone_closes_the_window`, `showing_the_last_hidden_app_closes_the_window_and_the_chevron_goes`, `the_chevron_leaving_and_entering_slides_the_bar_on_its_spring`.

- [ ] **Step 2: Run them to make sure they fail.** `cargo test -p celestina --locked tray::card` and `tray_overflow`. Expected: FAIL.
- [ ] **Step 3: Implement.**
  - The card is a child node of the list column right after the open row, sized `(264, height(now))`. Its pane paints inside it, through `Painter::blurred` when `content().1 > 0.05`, at the content opacity.
  - The rows' looks come from `RowBlurs` and are painted with `paint_with_row_blur`.
  - `fit_titles` takes the open row's `Weight::Semibold` into account.
- [ ] **Step 4: Run all checks** as in Task 3. Expected: PASS.
- [ ] **Step 5: Commit** `celestina-maintenance: Add the app menu card that unfolds under its row in the hidden-apps window`.

---

### Task 5: the card on the Island's Tray page

**Files:**
- Modify: `celestina/src/island/mod.rs`:
  - an Island field `tray_card: Option<MenuCard>` and `tray_rows_blur: RowBlurs`;
  - `menu_for`, `open_menu_action` and `menu_key` toggle the card and push `AskTrayMenu`;
  - the page's input reaches the card first while it is held;
  - the card goes when the page closes or its row leaves;
  - the three Island entry points of Task 3 cover it.
- Modify: `celestina/src/island/pages/tray.rs`: the rows view takes the card node after the open row and the open row's look.
- Modify: the page's paint path in `island/mod.rs` (`Island::paint`): `paint_with_row_blur`.
- Test: new `celestina/src/island/tests/tray_page_card.rs`.

**Interfaces:**
- Consumes: Task 4's `MenuCard`, `RowBlurs`, `paint_with_row_blur`, `PaneFrame::card`; `PAGE_SIZE` and the page's content room.
- Produces: no new public names. The card's width is the page's row width. Its cap is the page's content room − the rows' height − 6, so a menu that does not fit scrolls inside the card. The page's rows themselves do not scroll (`MAX_ROWS` 24 stays).

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn a_page_rows_menu_unfolds_a_card_under_the_row_and_blurs_the_others()
#[test] fn middle_click_activates_secondarily_and_right_click_unfolds_the_card() // replaces middle_click_activates_secondarily_and_right_click_asks_for_the_menu
#[test] fn the_menu_key_unfolds_the_card_of_the_focused_row()                  // replaces the_menu_key_opens_the_menu_of_the_focused_row
#[test] fn item_is_menu_unfolds_the_card_on_left_click()                       // replaces item_is_menu_opens_the_menu_on_left_click
#[test] fn choosing_in_the_page_card_folds_it_at_once_and_the_page_stays()
#[test] fn escape_goes_back_then_folds_then_closes_the_page()
#[test] fn closing_the_page_drops_its_card_and_nothing_stays_held()
#[test] fn a_tall_menu_scrolls_inside_the_page_card()
```

- [ ] **Step 2: Run them to make sure they fail.** `cargo test -p celestina --locked tray_page_card`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run all checks** as in Task 3. Expected: PASS. Then `rg -n "open_popup" celestina/src` lists only the page's overflow menu (`island/mod.rs`, `island/menu.rs`).
- [ ] **Step 5: Commit** `celestina-maintenance: Add the app menu card to the Island's Tray page`.

---

### Task 6: nest evidence, the parked bug's live check, validation and docs

**Files:**
- Fill: `celestina/docs/evidence/<date>-cel-d-016-app-menus.md` (+ its shots folder).
- Modify: `celestina/VALIDATION.md`: new `VAL-CEL-D-016`; amend `VAL-CEL-I-011` (~1744), whose "no second menu beside it" now reads "the menu is a window or a card, never beside it".
- Modify: the `celestina/README.md` lines that describe tray menus (`rg -n -i "menu" celestina/README.md | rg -i tray`).
- Kit (outside the repo):
  - copy `cel-d/kit-recovered/` into the session scratchpad and point its `N` and `R` at it and at the worktree;
  - copy `cel-i-012/kit-from-cel-i-011/fake_sni.py`;
  - build with a private `CARGO_TARGET_DIR`.

- [ ] **Step 1: Extend the fake item.** Add a `com.canonical.dbusmenu` object with `GetLayout`, `GetGroupProperties`, `AboutToShow`, `Event` and the `LayoutUpdated` signal, with English labels, for four items:
  - **`fake-menu`:** 4 entries; a separator; a disabled entry; a checked toggle; a submenu of 3; a submenu empty until `AboutToShow`, then `LayoutUpdated`; a 40-grapheme label.
  - **`fake-long`:** 64 entries.
  - **`fake-slow`:** its layout comes after 300 ms.
  - **`fake-empty`:** an empty root that never fills.

  `bar.tray_hidden` holds `fake-long` and `fake-slow`. The item logs every `Event` and `AboutToShow` it gets.
- [ ] **Step 2: Shoot in one nest at a time** (`grim` bursts after each press of the virtual pointer):
  - **Bar `fake-menu`:** press, +120, +240 (held drop on `fake-slow`), +460; the submenu drill-in at +100 ms and settled; back; the delayed submenu opening.
  - **Bar `fake-long`:** opened from the window and from the bar, scrolled with the wheel (the thumb shown), then Down to the shell entry (in view).
  - **Hidden-apps window:**
    - press fill on `fake-slow` while it waits;
    - unfold at 0, 120 and 350 ms;
    - the row blur levels;
    - a card scrolled;
    - another row's card;
    - Escape fold, then Escape close.
  - **`fake-empty`:** the shell entry alone at 500 ms.
  - **The Tray page card**, through the verb `island open tray` sent inside the nest by the probe.
  - **Reduced motion:** all of the above, once.
  - **The fake item's log:** each choice reaches it as `clicked` before `closed`, and there is one `closed` per menu shown.
- [ ] **Step 3: Live check of the parked bug.**
  - Open and close each menu 20 times from the bar and 20 from the window.
  - `journalctl` of the nest's run shows no `tray menu: dismissed by the compositor`.
  - `rg -n "dismissed by the compositor" celestina/src` finds only `island/menu.rs`, the page's overflow menu, which still uses `xdg_popup`.
  - Record that the bug's path is gone, and that this was checked in the nest only, not on the author's session.
- [ ] **Step 4: Run the smoke phases** `bar,osd,notify` through the kit, as the landing does, and record the result. No smoke phase opens a tray menu. This unit is the series' last, so the landing runs the full smoke.
- [ ] **Step 5: Write the evidence** (procedure, shots beside the preview's tray stage, limits: fake items only, no Steam/nm-applet/blueman) and `VAL-CEL-D-016`. The author's steps on the real session:
  - Steam's menu from the bar;
  - a hidden app's card;
  - a submenu;
  - the show entry (`copy::TRAY_SHOW`) from a card;
  - the parked bug watched for a day.
- [ ] **Step 6: Commit** `celestina-maintenance: Add the CEL-D-016 evidence and VAL-CEL-D-016`.

---

## Model table (heuristic estimate)

| Chunk | Model | Reasoning | Why |
|---|---|---|---|
| Task 0 ledger and stub | this session | n/a | Controller's bookkeeping |
| Task 1 menu pane | Sonnet | high | State machine with slide, stack, live republish, focus memory and scroll; the core every holder shares |
| Task 2 drop hold | Sonnet | medium | A clock pause on landed motion code, numbers and tests given |
| Task 3 bar window and close path | Sonnet | high | Cross-module flow (Island, sheets, app, host) with an exactly-once rule and the popup path's removal |
| Task 4 hidden-apps card | Sonnet | medium | Pinned numbers and tests; the painter guard is spelled out; the final review checks the paint |
| Task 5 Tray page card | Sonnet | medium | Reuses Task 4's card on the page |
| Task 6 nest and docs | Sonnet | medium | Guarded nest with a fake dbusmenu item, shots and evidence |
| `/simplify` | this session | n/a | In session, no subagents |
| `stop-slop` | this session | n/a | On the evidence, VALIDATION and README lines |
| Final review | Opus | high | `/code-review`; a different model from the Sonnet implementers (Opus stays free for this review, so Tasks 1 and 3 use Sonnet at high). At most 2 rounds (the second only re-reads the first round's fixes) |

Order: Tasks 1 ‖ 2 (separate worktrees), then 3, 4, 5, 6 in turn. The controller reads each diff; there is no reviewer per task.

## Finish and landing

- [ ] `/simplify` on the branch's code, in the session.
- [ ] `stop-slop` on the evidence, `VAL-CEL-D-016` and the README lines.
- [ ] Final review `CEL-D-016.R` (final review): Opus, high, `/code-review` on `main...unit/celestina/CEL-D-016`, with this plan, the design §5.6 and §5.7, and the preview's tray stage. After round 2, the controller fixes and checks any defect of the unit's goal; everything else goes to the evidence's parked list.
- [ ] Update the ledger row (status, inventory, diffstat) as `land-unit.py` expects.
- [ ] Landing, only on the author's word in this chat. The series' last unit runs the full smoke:
  `python3 scripts/land-unit.py unit/celestina/CEL-D-016 --kind milestone --summary "Replace the tray's app menu popups with capsule menu windows: a bar icon's menu comes out of its icon with the drop, a hidden app's menu unfolds as a card under its row while the other rows blur, submenus open inside the window and a tall menu scrolls"`
  The author may name `--smoke bar,osd,notify` instead. Before landing, `cargo +1.100.0 clippy --workspace --all-targets --locked -- -D warnings`.

## What this plan did not read or verify

- **Sibling plans were only skimmed.** The `CEL-D-013` and `CEL-D-014` plans appeared while this was being written, and were read only for `drop::OPEN_HOLD`, `SheetMotion` and `paint_hiding`'s removal. The `CEL-D-015` plan was not read. None of their code has landed. `CEL-D-014`'s primary-press rule goes through `sheet_request`, so it does not cover a tray icon, whose primary press is `Tray(Activate)`: Task 3 adds the tray's rule.
- **Not read:**
  - the bodies of `island/pages/tray.rs` `item_row` and the page's paint path;
  - `island/bar_tree.rs` keyboard focus on tray icons (the bar path keeps `keyboard: false` as today);
  - `celestina-ui` `Tree` internals beyond `paint_hiding`/`paint_inside`;
  - the smoke scripts.
- **Not verified:**
  - that femtovg executes two same-size `blurred` layers in one frame correctly (the backend caches layers by size). Task 4's nest shots of three blurred rows check it;
  - that `chevron-left` is in the shell's bundled icon set (it is in `celestina-style/icons`).
- **Nothing was compiled or run.** Line numbers are from `main` 6ff5a018 and will drift after `CEL-D-013`/`014`/`015`.
- **Plan decisions the design leaves open:**
  - the Tray page's card scrolls inside itself, and the page's rows do not scroll;
  - the back row's style;
  - Left at the root does nothing;
  - a late layout after the 500 ms shell-only fallback refreshes the menu in place.
