# CEL-I-012 Implementation Plan: a collapsible tray

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:**
- The bar shows only the tray apps the author keeps there.
- A chevron opens a window with the hidden ones.
- Every app's menu can hide it or show it again.

**Architecture:**
- `bar.tray_hidden` becomes writable from the shell through `ShellSettings`, the way `set_do_not_disturb` is.
- The tray piece draws the chevron when any registered app is hidden.
- The chevron opens a `CEL-D-009` menu window: grown from the chevron (variant B) and aligned to the right capsule's right edge.
- The window's content is the hidden apps as tray-page rows.
- The tray menu card gains one shell-owned entry after a separator.

**Tech Stack:** Rust, `celestina-ui`, the sheet/window modules of `CEL-D-009`, `celestina-shell` settings.

**Spec:** `docs/superpowers/specs/2026-11-26-celestina-notifications-tray-design.md` §4.3 and §5. Approved preview: Windows style, list G2, 2026-11-26.

**Branch:** `unit/celestina/CEL-I-012`, opened from `main` after `CEL-I-011` lands. Size: large, executed in its own session.

## Global Constraints

- Exact Spanish strings: the copy table of the approved preview (`/home/toni/CODIGO/previews/2026-11-26-cel-i-010-012.html`); they go only into `src/copy.rs`.
- **Chevron:**
  - box 32 (24 + 4 pad), radius 12, Lucide `chevron-up` 18 px, the tray icon's states;
  - while its window is open it stays lit, and the glyph turns 180° on the 350 ms spring (`motion.slow`, OutBack 1.15);
  - reduced motion: no turn, the glyph flips;
  - accessible name: new `copy::TRAY_HIDDEN` ("hidden icons") and `copy::TRAY_HIDDEN_ATTENTION` ("hidden icons, one needs attention").
- **Window:**
  - 240 wide, padding 8, radius 20, rows of 40 like `island/pages/tray.rs` (24 px icon, title, `copy::TRAY_NEEDS_ATTENTION` in accent);
  - top at y 56, right edge on the right capsule's right edge, clamped 16 px;
  - motion as `CEL-D-009`.
- **Menu entry:**
  - separator, then `copy::TRAY_HIDE` ("hide from the bar") with Lucide `eye-off`, or `copy::TRAY_SHOW` ("show in the bar") with `eye`, at the end of every app's menu;
  - an app without a menu gets a card holding only that entry.
- **Hiding:** the entry writes `bar.tray_hidden` by the item's `Id` (the key today's filter uses), through `ShellSettings::set_tray_hidden`. The bar icon leaves or enters its slot on `CEL-D-008`'s slot spring.
- **Attention:** a hidden app with `NeedsAttention` puts the 7 px accent dot on the chevron at (21, 2), and marks its row.
- **No hidden app means no chevron.** At rest the bar's frame count does not grow.
- **Code rules:**
  - product copy only in `src/copy.rs`;
  - no `unwrap`/`expect` in production;
  - never touch `src/lock`, `src/polkit`, `celestina/scripts/`, `.githooks`, any `AGENTS.md`, or `docs/contracts|governance|standards`.
- **Nested runs** go only through the guarded kit, with fake SNI items on the nest's bus. Never run `celestina` or `niri msg` from the agent's own shell.

## Review Focus

1. The last hidden app unregisters while the window is open: the window closes, and the chevron folds out of the bar on the slot spring.
2. `shell.json` is edited by hand (or by `celestina-settings`) while the window is open: the window follows the new list, and the shell's own write is not reverted (`adopt_read` generation).
3. `TRAY_SHOW` ("show in the bar") chosen from the window's menu: the menu closes, the row leaves the window, and the icon enters the bar. If it was the last one, the window and the chevron go.
4. A hidden app's menu opened from the window, then the window is closed with Escape: Escape closes the menu first, as menu levels do today.
5. Two outputs: the chevron and its window live on the output whose bar was pressed, and hiding on one output updates the other.

---

### Task 1: Writable hidden list and the menu entry

**Files:**
- `celestina-rs/crates/celestina-shell/src/settings.rs`: `BarLayout` setter, `kept_hidden` reused.
- `celestina/src/providers/settings.rs`: `pub fn set_tray_hidden(&mut self, ids: Vec<String>) -> Result<bool, ShellSettingsError>`.
- `celestina/src/tray/menu.rs`: a shell entry row type, `MenuRow::Shell(ShellEntry::Hide | ShellEntry::Show)`.
- `celestina/src/app.rs`: the entry's action writes the list.
- `celestina/src/pieces/tray.rs`: the slot spring on hide and show.
- `celestina/src/copy.rs`.
- The tests of each.

- [ ] Tests:
  - `set_tray_hidden_writes_the_key_and_keeps_others`;
  - `own_write_is_not_reverted_by_a_stale_read`;
  - `menu_ends_with_hide_entry_for_a_bar_app`;
  - `menu_ends_with_show_entry_for_a_hidden_app`;
  - `app_without_menu_gets_a_card_with_only_the_entry`;
  - `hide_entry_moves_the_id_into_tray_hidden`.
- [ ] Run the fast checks.
- [ ] Commit `celestina-maintenance: Add hiding and showing a tray app from its menu`.

### Task 2: The chevron and its window

**Files:**
- `celestina/src/pieces/tray.rs`: the chevron item, its states, its turn and its attention dot.
- `celestina/src/sheet/` and `celestina/src/island/sheets.rs`: a window kind for the tray overflow, anchored on the chevron, end-zone alignment.
- `celestina/src/island/pages/tray.rs`: the row view reused, filtered to hidden apps.
- `celestina/src/copy.rs`.
- The tests of each.

- [ ] Tests:
  - `chevron_shows_only_with_a_hidden_app`;
  - `chevron_opens_the_overflow_window`;
  - `overflow_window_lists_hidden_apps_only`;
  - `overflow_window_aligns_to_the_capsule_right_edge`;
  - `overflow_row_left_click_activates` (menu when `ItemIsMenu`);
  - `overflow_row_right_click_opens_the_app_menu`;
  - `attention_dot_on_chevron_for_a_hidden_app`;
  - `last_hidden_app_gone_closes_the_window`.
- [ ] Run the fast checks.
- [ ] Commit `celestina-maintenance: Add the tray's hidden-apps chevron and its window`.

### Task 3: Nest evidence

- [ ] Nest screenshots with three fake SNI items, one hidden and asking for attention. Save them under `celestina/docs/evidence/2026-11-26-cel-i-012-collapsible-tray/`:
  - the chevron at rest, with its hover and focus states;
  - the window mid-birth and open;
  - the menu with the hide entry;
  - the slot spring;
  - the attention dot.
- [ ] Evidence page, ledger row `CEL-I-012`, and `VALIDATION.md` row `VAL-CEL-I-012`.
- [ ] Commit `celestina-maintenance: Add the CEL-I-012 evidence and validation row`.

Landing kind: `milestone`, smoke `bar`; the series' last unit runs the full smoke.
