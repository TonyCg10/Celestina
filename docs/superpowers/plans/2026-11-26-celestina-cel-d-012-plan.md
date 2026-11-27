# CEL-D-012 Implementation Plan: the launcher's tabs and grid

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The launcher opens on a Recents tab, offers All (A to Z) and up to seven category tabs, and shows any of them, and any search, as a list or as a six-column grid the author's choice remembers.

**Architecture:**
- `LauncherState` gains a tab and a view, and answers an empty query from the tab.
- The card grows a tab row (variant A of the approved preview) and a grid layout inside `CEL-I-017`'s scrolled area.
- `shell.json` gains `launcher.view`, written the way `bar.tray_hidden` is.

**Tech Stack:** Rust; `celestina/src/launcher`, `celestina-ui`, `celestina-rs/crates/celestina-shell/src/settings.rs`, `celestina/src/providers/settings.rs`, `celestina/src/app.rs`.

**Spec:** `docs/superpowers/specs/2026-11-26-celestina-launcher-design.md` §5.1, §5.2, §5.5, §5.8 and §9 calls 6 and 7. Approved preview (tabs variant A): `/home/toni/CODIGO/previews/2026-11-26-launcher/index.html`, artifact <https://claude.ai/artifact/QoGCyzhoqoM5P4pH2ASHWB>; the implementation and the final review compare against it.

**Branch:** `unit/celestina/CEL-D-012`, from `main` after `STYLE-ICONS-2`, `CEL-I-015` and `CEL-I-017` have landed. Kind `milestone`. Size: large; executed in its own session.

**Overlap:** `CEL-D-011` (another session) changes `celestina-shell/src/settings.rs` (`OsdSettings`) and `src/app.rs`. Rebase onto it if it lands first; never edit its parts.

## Global Constraints

- **Card.** 560 × 444 for the launcher: padding 12, pill 48, gap 8, tab row 36, gap 8, area 320, padding 12. The selector keeps 400 and has no tab row; `card_size` takes the source.
- **Tab row.** Height 36. The tabs sit in a flex row with a 4 px gap, then a growing spacer, then the view button.
- **A tab.**
  - a 32 px pill, radius 16, an 18 px glyph, padding 0 by 9;
  - the open tab: `Role::ControlFill`, its glyph and label in `Role::TextHi`, the label at body size (13, `Weight::Regular`; the toolkit has no medium weight) 6 px after the glyph;
  - the other tabs: the glyph only, in `Role::TextLo`, with the pointer lift;
  - the label appears by fading in while the neighbours slide, over normal (200 ms); `View` animates no width.
- **Tabs and glyphs.**
  - Recents `rotate-ccw-clock`, All `arrow-down-a-z`;
  - the groups use `groups::Group::icon` from `CEL-I-015`;
  - labels: new `LAUNCHER_TAB_RECENT` and `LAUNCHER_TAB_ALL`, plus the `LAUNCHER_GROUP_*` constants `CEL-I-015` added;
  - the Spanish text is the preview's `TABS[].lbl`.
- **View button.** 36 × 32, radius 16, glyph `view-grid` in the list view and `view-list` in the grid view, `Role::TextLo`.
- **While a query is typed.** The tab row's opacity is 0.45 and the tabs stay clickable.
- **Grid.**
  - 6 columns over the 536 px content width; each cell is 80 px high, radius 12;
  - a 40 px icon, a 6 px gap, then the name at `type_scale.row_secondary` (12) in `Role::TextHi`, one line, cut with an ellipsis (`truncate_title` or the text's own ellipsis);
  - a cell's states are a row's.
- **Recents.**
  - The entries with a use score above 0, best first, at most 50.
  - With none, one line: the new constant `LAUNCHER_RECENT_EMPTY`. Its Spanish text is yours to write in the preview's tone, meaning "you have not opened any application yet"; show it to the author with the evidence.
- **Motion.**
  - A tab switch: the content crossfades, and slides 12 px in from the move's side if the tree can start a slide from an offset; otherwise crossfade only, and say so in the evidence.
  - A view switch: a crossfade.
  - Reduced motion: none.
- **`shell.json`.** `launcher.view` holds `"list"` or `"grid"`; an absent or unknown value reads as `"list"`. It is written only when the author toggles the view.
- **Keys.**
  - Ctrl+Tab and Ctrl+Shift+Tab (Ctrl with `XKB_KEY_TAB`, and Ctrl+Shift or `XKB_KEY_ISO_LEFT_TAB`) step the visible tabs and wrap.
  - Alt+1 to Alt+9 (keysyms `0x31`–`0x39`) open the n-th visible tab.
  - Ctrl+G (`0x67`/`0x47`) toggles the view.
  - These act in the launcher only, never in the selector.
- **Language and safety.** English only outside `copy.rs`; no new dependency; live-session safety and checks as in `celestina/AGENTS.md`.

## Review Focus

1. A group that empties while the launcher is open (a new index drops its last app) while that tab is open: the launcher falls back to All, and the selection and scroll reset.
2. Alt+digit beyond the visible tabs' count does nothing, and Alt with any other key still reaches the field as today (Alt+letters insert nothing; check `key_to_edit`).
3. A tab clicked while a query is typed empties the field and the query, and shows the tab.
4. Grid arrows with a calculator row on top: Down from the row goes to cell 0, and Up from the first cell line goes to the row; Left and Right with text in the field move the caret, not the selection.
5. `shell.json` unreadable (protected mode): toggling the view works for the open launcher, writes nothing, logs once, and does not crash.

---

### Task 1: tabs and the view in the state

**Files:**
- Modify: `celestina/src/launcher/state.rs` (with tests)

**Interfaces:**
- Consumes: `groups::{Group, group_of}` and the use scores (`CEL-I-015`).
- Produces:
  - `pub enum Tab { Recent, All, Group(Group) }`;
  - `pub enum ListView { List, Grid }`, with `Default` = `List`;
  - `LauncherState` fields `pub tab: Tab` and `pub view: ListView`;
  - `pub fn visible_tabs(&self) -> Vec<Tab>`: Recent, All, then the non-empty groups in `Group::ALL` order;
  - `pub fn set_tab(&mut self, tab: Tab, index: &[DesktopEntry])`: it empties the query and puts the selection on row 0;
  - `pub fn move_grid(&mut self, dx: i32, dy: i32)`.
- **`move_grid`.**
  - The items form lines: a calculator row is a line of one, then lines of 6 cells.
  - `dy` moves to the same column on the next or previous line, clamped to that line's length, and wraps from the last line to the first.
  - `dx` moves ±1 through the items in order and wraps.

- [ ] **Step 1: Write the failing tests:**
  - an empty query answers from the tab: Recents by use score, capped at 50, and empty with no scores; All by folded display name; a group filters by `group_of`;
  - `visible_tabs` hides an empty group;
  - `set_tab` clears the query;
  - each `move_grid` case of Review Focus 4, plus a wrap at both ends;
  - Review Focus 1's fallback in `update`.
- [ ] **Step 2: Run** `cargo test -p celestina --locked launcher::state`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the same tests. Expected: PASS.
- [ ] **Step 5: Commit** `Add tabs and a grid view to the launcher's state`

### Task 2: `launcher.view` in `shell.json`

**Files:**
- Modify: `celestina-rs/crates/celestina-shell/src/settings.rs` (a `launcher` field on `Settings`, lenient, `skip_serializing_if` default; tests)
- Modify: `celestina/src/providers/settings.rs` (`pub fn set_launcher_view(&mut self, grid: bool) -> Result<bool, ShellSettingsError>` through `set_merged`; tests beside `set_tray_hidden_writes_the_key_and_keeps_others`)

**Interfaces:**
- Produces:
  - `pub struct LauncherChoice { pub view: LauncherView }` in `celestina-shell`;
  - `pub enum LauncherView { List, Grid }`, serialized as `"list"`/`"grid"`, with any unknown value read as `List` through the module's `lenient` helpers;
  - `Settings.launcher: LauncherChoice`.

- [ ] **Step 1: Write the failing tests:**
  - a file without `launcher` reads `List` and writes no `launcher` key;
  - `{"launcher":{"view":"grid"}}` reads `Grid`;
  - `{"launcher":{"view":7}}` reads `List`;
  - `set_launcher_view(true)` writes the key and keeps the other keys, as the `tray_hidden` test checks.
- [ ] **Step 2: Run** `cargo test -p celestina-shell --locked settings` and `cargo test -p celestina --locked providers::settings`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the same tests. Expected: PASS.
- [ ] **Step 5: Commit** `Add the launcher's list or grid choice to the shell settings`

### Task 3: the tab row, the grid and the card's size

**Files:**
- Modify: `celestina/src/launcher/view.rs` (with tests), `celestina/src/copy.rs`

**Interfaces:**
- Produces:
  - `pub fn tab_key(i: usize) -> Key` (`launcher-tab`, indexed by the visible position) and `pub fn tab_of(key: Key, count: usize) -> Option<usize>`;
  - `pub fn view_button_key() -> Key` (`launcher-view`);
  - `pub fn cell_key(i: usize) -> Key`, or reuse `row_key(i)` for cells so that `row_of` keeps working (preferred);
  - `card_size(theme, source)`.
- Each tab node is interactive and holds a glyph, plus a label node at opacity 1 when open and 0 otherwise.
- The content column's key includes the tab and the view (for example `Key::indexed("launcher-content", tab_index * 2 + view)`), so a switch is a new node and the tree crossfades.

- [ ] **Step 1: Write the failing tests:**
  - the launcher card is 560 × 444 and the selector's is 560 × 400;
  - the tab row holds one tab per visible tab, then the view button, which holds `view-grid` in the list view;
  - the open tab has `Role::ControlFill` and its label at opacity 1;
  - with a query, the row's opacity is 0.45;
  - the grid lays out 6 cells per line at 80 px, a calculator row spans the width above the cells, and a cell holds a 40 px icon and a name at 12 px.
- [ ] **Step 2: Run** `cargo test -p celestina --locked launcher::view`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the same tests. Expected: PASS.
- [ ] **Step 5: Commit** `Add the launcher's tab row and grid layout`

### Task 4: keys, clicks, persistence and the scroll

**Files:**
- Modify: `celestina/src/launcher/mod.rs`, `celestina/src/app.rs`

**Interfaces:**
- Produces:
  - `LauncherAction::SetView(ListView)`, for which `closes()` returns false;
  - `Launcher::new(.., view: ListView, ..)`.
- **The app.**
  - When it opens the launcher, it passes the view from `settings.current().launcher.view`.
  - On `SetView` it calls `set_launcher_view`; on an error it logs and keeps going (Review Focus 5).

**Behaviour:**
- Tab clicks and the view button arrive as `UiEvent::Click`.
- In the grid view, the arrows route to `move_grid`: Left and Right only while the field is empty.
- The scroll from `CEL-I-017` uses the grid's line heights:
  - a calculator line is 40 px and a cell line 80 px;
  - its extent is `calc + lines × 80`;
  - `reveal` takes the selected item's line top.
- The launcher always opens on `Tab::Recent`.

- [ ] **Step 1: Write the failing tests:**
  - Ctrl+Tab steps the tabs and wraps, and Ctrl+Shift+Tab goes back;
  - Alt+3 opens the third visible tab, and Alt+9 beyond the count does nothing;
  - Ctrl+G returns `SetView(Grid)` and does not close;
  - in the grid, Right with an empty field moves the selection, and with text it does not;
  - a click on a tab while typing empties the query;
  - the selector ignores all of these.
- [ ] **Step 2: Run** `cargo test -p celestina --locked launcher::`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the three workspace checks from `celestina/`. Expected: PASS.
- [ ] **Step 5: Commit** `Switch the launcher's tabs and view by key and click and remember the view`

### Task 5: evidence in a nest against the preview

- [ ] Private target dir and one nest, as in `celestina/AGENTS.md`, with a throwaway `XDG_DATA_HOME` of about 40 `.desktop` files covering every group, a seeded `usage.json`, and `LANG=es_ES.UTF-8`.
- [ ] Take `grim` shots of the preview's ten states, in the list view and the grid view, and put each beside the preview's state in the evidence:
  1. on open;
  2. All, scrolled;
  3. after the arrows;
  4. with the pointer over a row;
  5. the web-browser alias;
  6. a typo;
  7. a sum;
  8. copied;
  9. no results;
  10. clearing.
  
  Name every visible difference.
- [ ] Run the launcher's smoke phases; the series' last unit runs the full smoke at landing.
- [ ] Write the evidence.
