# CEL-I-010 Implementation Plan: clearing notifications

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:**
- A seen notifications window marks everything read.
- Every row can be removed.
- `NOTIFICATIONS_CLEAR_ALL` ("clear all") empties the list.

**Architecture:**
- The pure domain (`celestina-shell`) gains `remove(id)` and a `mark_read` that covers the history.
- The provider gains `NotificationsCommand::Remove`.
- The page's close button and its clear-all bar send `Remove` and `ClearHistory`.
- The row fold and the window narrowing are the page's own motion.

**Tech Stack:** Rust; `celestina-rs/crates/celestina-shell`, `celestina/src/providers/notifications.rs`, `celestina/src/island/pages/notifications.rs`, `celestina-ui`.

**Spec:** `docs/superpowers/specs/2026-11-26-celestina-notifications-tray-design.md` §4.1 and §5. Approved preview: variant A, 2026-11-26.

**Branch:** `unit/celestina/CEL-I-010`, opened with `scripts/worktree.sh open celestina CEL-I-010` from `main`. Size: small, light-review exempt (more than 150 lines with tests).

## Global Constraints

- Exact Spanish strings: the copy table of the approved preview (`/home/toni/CODIGO/previews/2026-11-26-cel-i-010-012.html`); they go only into `src/copy.rs`.
- **Close button:**
  - 24 px, radius 8 (`radius_sm`), `x` glyph 16 px in `textMuted`;
  - hover `controlHover`, pressed `controlPressed` at scale 0.96 (`pressScale`);
  - focus ring 2 px `focusRing`;
  - accessible name `copy::BANNER_DISMISS`.
- **Clear-all bar:** same place, height (28) and button style as today's dismiss-all bar. Label: new `copy::NOTIFICATIONS_CLEAR_ALL` ("clear all"). It shows whenever the list has a row. `copy::NOTIFICATIONS_DISMISS_ALL` is removed.
- **Motion:**
  - a removed row folds its height in 200 ms (`motion.normal`, standard curve) while its opacity falls in 100 ms (`motion.fast`);
  - clear-all folds the rows with a 30 ms stagger, then the empty label fades in in 100 ms, and the window narrows to `PAGE_SHEET_MIN_W` (240) on the 350 ms spring;
  - reduced motion: no fold, the rows go at once.
- **Removing a live notification** closes it with `CloseReason::Dismissed`, so `NotificationClosed` is emitted.
- **Copy and code rules:**
  - product copy only in `src/copy.rs`;
  - no `unwrap`/`expect` in production;
  - never touch `src/lock`, `src/polkit`, `celestina/scripts/`, `.githooks`, any `AGENTS.md`, or `docs/contracts|governance|standards`.
- **Nested runs** go only through the guarded kit, with `env -i`, `--own-bus`, `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0` and fake `wpctl`/`pactl`. Never run `celestina` or `niri msg` from the agent's own shell.

## Review Focus

1. A notification posted while the window is open is shown, so it is read at once. The dot must not flash on.
2. Remove the last row: the empty label shows, and the window narrows without a jump.
3. Remove a live row whose banner is on screen: the banner goes too.
4. Clear all while a banner shows: the banner goes, and no stale toast surface stays.
5. A history over 20 rows (the page shows 20): after a removal, the 21st row appears at the end, and its fold does not reuse the removed row's state.

---

### Task 1: Domain and command

**Files:**
- Modify: `celestina-rs/crates/celestina-shell/src/notifications.rs` (`mark_read`, new `remove`) and its tests.
- Modify: `celestina/src/providers/notifications.rs` (`NotificationsCommand::Remove(u32)`, the worker's handling, a snapshot when anything was unread) and its tests.
- Modify: `celestina/src/command.rs`, `celestina/src/app.rs` (verb `notifications clear` → `ClearHistory` plus `DismissAll`).

**Interfaces:**
- `pub fn remove(&mut self, id: u32) -> Option<Closed>`:
  - drops `id` from `live`, closing it as `Dismissed` (the `Closed` is returned);
  - or drops it from `history` (`None`);
  - no effect when unknown.
- `pub fn mark_read(&mut self)` marks `live` and `history`, and returns `bool` (anything changed).

- [ ] Tests:
  - `mark_read_marks_the_history_too`: an expired, unread entry is read after `mark_read`, and `unread() == 0`.
  - `remove_drops_a_history_entry`.
  - `remove_closes_a_live_one_as_dismissed`.
  - `remove_of_an_unknown_id_is_a_no_op`.
  - Provider: `mark_read_publishes_when_only_the_history_was_unread`.
  - Provider: `remove_command_reaches_the_domain`.
  - Command parse: `notifications clear`.
- [ ] Run `cargo test --workspace --locked` in `celestina/` and `cargo test -p celestina-shell` in `celestina-rs/`. Both pass.
- [ ] Commit `celestina-maintenance: Add removing one notification and mark the history read when the page is seen`.

### Task 2: The page

**Files:**
- Modify: `celestina/src/island/pages/notifications.rs` (close button on every row → `Remove(id)`; the bar → clear-all; the fold and narrowing motion), `celestina/src/notifications/history.rs` (rows carry `removable`), `celestina/src/copy.rs`, and their tests.

- [ ] Tests:
  - `every_row_has_a_close_button` (live and finished);
  - `close_on_a_finished_row_sends_remove`;
  - `clear_all_bar_shows_with_any_row` (none when empty);
  - `clear_all_sends_dismiss_all_then_clear_history`;
  - `reduced_motion_removes_rows_at_once`.
- [ ] Nest screenshots, saved under `celestina/docs/evidence/2026-11-26-cel-i-010-notifications/`:
  - 4 rows (2 live, 2 finished, posted with `notify-send` on the nest's bus);
  - the hover and focus states of a close button;
  - the frame mid-fold;
  - the empty state after `NOTIFICATIONS_CLEAR_ALL` ("clear all");
  - the bell without a dot after opening.
- [ ] Evidence page `celestina/docs/evidence/2026-11-26-cel-i-010-notifications.md`, ledger row `CEL-I-010` in `celestina/docs/plans/active/2026-11-25-cel-tracks.md`, and `VALIDATION.md` row `VAL-CEL-I-010`.
- [ ] Commit `celestina-maintenance: Replace dismiss-all with clearing the notifications list and add a close button to every row`.

Landing kind: `milestone` (new feature), smoke `bar,notify`.
