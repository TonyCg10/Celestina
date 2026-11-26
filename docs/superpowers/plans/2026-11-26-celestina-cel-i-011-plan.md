# CEL-I-011 Implementation Plan: a tray that shows every icon and opens every menu

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every app that registers a tray item shows an icon and opens its menu.

**Architecture:**
- The item reader learns the properties it ignores today: `IconThemePath`, `ItemIsMenu`, `AttentionIconName` and `AttentionIconPixmap`.
- Icons resolve through the item's own theme path first. An absolute `IconName` is accepted only under bounded roots.
- The fallback becomes the bundled Lucide glyph.
- The right click falls back to `ContextMenu` when there is no usable dbusmenu.
- A pending menu waits briefly for a late layout.
- Everything stays on the tray host's thread, within today's 1 s call timeout and 900 ms read budget.

**Tech Stack:** Rust, zbus (blocking, on the host thread), `freedesktop-icons` 0.4, `celestina-paint` icons.

**Spec:** `docs/superpowers/specs/2026-11-26-celestina-notifications-tray-design.md` §4.2. No new look except the fallback glyph, which the approved preview of 2026-11-26 shows.

**Branch:** `unit/celestina/CEL-I-011`, opened with `scripts/worktree.sh open celestina CEL-I-011` from `main`. Size: small, two tasks.

## Global Constraints

- **Hostile input.**
  - An absolute `IconName` is used only when it canonicalizes to a regular file under:
    - the item's `IconThemePath`;
    - an `$XDG_DATA_DIRS` entry plus `icons` or `pixmaps`;
    - `$XDG_DATA_HOME/icons`.
  - The extension must be `.png` or `.svg`, and the file at most 1 MiB.
  - `IconThemePath` itself must be absolute, at most 4096 bytes, and an existing directory.
  - Pixmap limits stay as today.
- **The pixmap** is the candidate nearest to `24 × output scale`, preferring the larger on ties.
- **The fallback** is `celestina-style/icons/app-window.svg`, drawn in `textMuted` through the bundled icon path (`IconKey::Bundled`, or whatever the painter calls its Lucide key). It is no longer a theme name.
- **`ContextMenu(x, y)`** gets the pointer's position in output coordinates. The reply is not awaited.
- **Late layout:**
  - an empty root keeps `pending_menu` for 500 ms, or until one `LayoutUpdated` brings entries, whichever comes first;
  - after that the request is dropped with a debug line.
- **Menu events:** opening sends `Event(0, "opened", 0i32, 0u32)`, and closing sends `Event(0, "closed", …)`. Neither is awaited.
- **Code rules:**
  - no `unwrap`/`expect` in production;
  - no `unsafe`;
  - never touch `src/lock`, `src/polkit`, `celestina/scripts/`, `.githooks`, any `AGENTS.md`, or `docs/contracts|governance|standards`.
- **Nested runs** go only through the guarded kit. A fake SNI item (a small Python `dbus-next` or `gdbus` script on the nest's own bus) stands in for real apps. Never run `celestina` or `niri msg` from the agent's own shell.

## Review Focus

1. An `IconName` such as `/etc/shadow`, `../../x.png` or a symlink out of the roots is refused, and no file is opened.
2. An item that changes `IconThemePath` and `IconName` together (`NewIconThemePath` then `NewIcon`) shows the new icon, not a mix.
3. A right click on an item whose `GetLayout` times out ends in `ContextMenu`, and never in two menus at once.
4. Two quick right clicks on a slow item open one menu.
5. On a scale 2 output the pixmap chosen is the 48 px candidate when one exists.

---

### Task 1: Icons

**Files:**
- Modify: `celestina/src/tray/item.rs` (read `IconThemePath`, `ItemIsMenu`, `AttentionIconName`, `AttentionIconPixmap`), `celestina/src/tray/model.rs` (`TrayItem` fields `icon_theme_path: Option<PathBuf>`, `item_is_menu: bool`, `attention_icon_name: String`, `attention_pixmap: Option<Pixmap>`; `choose_pixmap(candidates, want_px)` nearest rule), `celestina/src/island/pages/tray.rs` and `celestina/src/pieces/tray.rs` (resolution order of the spec's §4.2, attention icon, bundled fallback), `celestina/crates/celestina-paint/src/icon.rs` (a lookup in an extra theme directory, and the bounded absolute-path check), and their tests.

**Interfaces:**
- `pub fn resolve_tray_icon(item: &TrayItem, scale: f32) -> RowIcon`, the one place both the piece and the page call.
- `pub(crate) fn bounded_icon_file(path: &Path, roots: &[PathBuf]) -> Option<PathBuf>`.

- [ ] Tests:
  - `icon_from_item_theme_path_wins`;
  - `absolute_icon_name_under_a_root_is_used`;
  - `absolute_icon_name_outside_the_roots_is_refused` (including `..` and a symlink out);
  - `pixmap_nearest_to_scaled_size`;
  - `attention_icon_replaces_the_icon_when_needs_attention`;
  - `missing_icon_draws_the_bundled_fallback`.
- [ ] Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo test --workspace --locked`. All three pass.
- [ ] Commit `celestina-maintenance: Fix tray icons that never resolved: the item's theme path, bounded absolute names, the scaled pixmap and a visible fallback`.

### Task 2: Menus

**Files:**
- Modify: `celestina/src/tray/item.rs` (`menu_path` treats `/NO_DBUSMENU` as none; `ItemCall::ContextMenu(i32, i32)`), `celestina/src/tray/model.rs` (`TrayCommand::ContextMenu(ItemId, i32, i32)`), `celestina/src/tray/dbusmenu.rs` (opened and closed events; empty root → wait), `celestina/src/providers/tray.rs` (fall back to `ContextMenu` when the layout fails or there is no menu), `celestina/src/pieces/tray.rs` and `celestina/src/app.rs` (`ItemIsMenu` → menu on the left click; the pending menu kept for one late layout), and their tests (the `test_bus.rs` fake item).

- [ ] Tests:
  - `no_dbusmenu_path_is_no_menu`;
  - `right_click_without_menu_calls_context_menu`;
  - `failed_layout_falls_back_to_context_menu`;
  - `item_is_menu_opens_the_menu_on_left_click`;
  - `empty_root_waits_for_one_layout_update`;
  - `empty_root_gives_up_after_500_ms`;
  - `opened_and_closed_events_are_sent`.
- [ ] Nest check with a fake SNI item (`IconThemePath` set, `Menu = "/NO_DBUSMENU"`, `ItemIsMenu` toggled):
  - screenshots of the icon and of the fallback;
  - a log line proving `ContextMenu` was called.
- [ ] Evidence page `celestina/docs/evidence/2026-11-26-cel-i-011-tray-fixes.md`, ledger row `CEL-I-011`, and `VALIDATION.md` row `VAL-CEL-I-011`: the author tries his real apps (Steam, Discord, nm-applet, blueman, solaar, shelly).
- [ ] Commit `celestina-maintenance: Fix tray menus that never opened: no-dbusmenu items, ItemIsMenu, late layouts and the ContextMenu fallback`.

Landing kind: `bug`, smoke `bar`.
