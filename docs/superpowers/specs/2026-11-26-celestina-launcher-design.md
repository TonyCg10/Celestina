# Celestina: a launcher with tabs, a grid, scrolling, a better search, a quick clear and a calculator

- **Date:** 2026-11-26
- **Entries:** `CEL-I-014` (quick clear), `STYLE-ICONS-2` (four glyphs), `CEL-I-015` (search), `CEL-I-016` (calculator), `CEL-I-017` (scrolling), `CEL-D-012` (tabs and grid), on the shell's standing plan `CEL-TRACKS` and the style plan `STYLE-T1`
- **Approved preview:** the author approved on 2026-11-26 the launcher preview with the variants marked as recommended (tabs variant A; hold Backspace and the clear button; names in Spanish when the `.desktop` file carries them). It is a private artifact, <https://claude.ai/artifact/QoGCyzhoqoM5P4pH2ASHWB>, and its source, with the extracted index of the author's 68 applications, is kept at `/home/toni/CODIGO/previews/2026-11-26-launcher/`
- **Status:** approved preview and approaches; this document awaits the author's approval. A builder's read raised eight questions; §5 answers them as the designer's calls, listed in §9

## 1. Intent

The author browses the launcher's list more than he types into it. In his words the launcher today has no scrolling, its search is poor or too strict, it has no quick way to clear the query, and it cannot be laid out as a grid or split into tabs. He also wants a calculator: typing a math expression shows its result.

Success: on opening, the applications he uses most are in front of him; every application is reachable by scrolling, in a list or a grid, by tab; a Spanish word, a word out of order or a one-letter typo still finds the application; holding Backspace empties the field; a typed expression shows its result first and Enter copies it.

## 2. Goals, non-goals, anti-goals

- **Goals:** the six requests above, as the approved preview shows them.
- **Non-goals:**
  - favourites, or any ordering by hand (the author chose the tab set without favourites, variant C of the brainstorm);
  - a generic scroll container in `celestina-ui` (the scrolling is the launcher's own; a toolkit container is later work);
  - any change to the `Mod+K` selector of minimised windows beyond what sharing the card forces: it keeps today's matcher, rows and size;
  - unit or currency conversion in the calculator;
  - the suite identity direction approved on 2026-11-26: the launcher keeps today's tokens and moves with the rest of the shell when the tokens change.
- **Anti-goals:**
  - a card that changes size while it is open (on a tab switch, a view switch or a query);
  - a query that matches an application by a keyword and puts it above one whose name equals, starts with, or has a word starting with the query (a name that matches only by a typo or by letters in order may rank below a keyword: typing "web" should put the browsers above an application whose name only contains w, e and b in order);
  - a calculator row for text that is not an expression (a plain number, a word, an expression cut short);
  - a second `.desktop` reader;
  - a hostile `.desktop` file or a pathological query that costs more than a bounded amount of work per keystroke.

## 3. Today (1.29.0), from the code

- `celestina/src/launcher/state.rs` answers a query with `celestina_shell::launcher::rank` and keeps at most `MAX_ROWS` (8) rows; nothing scrolls and the rest are dropped. With an empty query every entry answers loosely, so the list is the recent launches and then the alphabet, cut at 8.
- `celestina-rs/crates/celestina-shell/src/launcher.rs` (the salvaged matcher):
  - case and accents already fold (`fold`);
  - the tiers are exact name, name prefix, a name word's start, and letters in order (subsequence);
  - the generic name, comment, keywords and command answer only in the loose tier;
  - the query is one string, so two words out of order do not match, and there is no typo tolerance.
- `celestina-rs/crates/celestina-core/src/desktop_entry.rs` keeps the unlocalized `Name` only and drops every `Key[locale]`, by design ("the caller's business"). So a Spanish word the file carries only in `GenericName[es]` or `Keywords[es]` never matches. In the author's index 9 of 68 entries carry `Name[es]` and 2 carry `Keywords[es]`.
- `celestina/src/launcher/recent.rs` keeps the last 50 launched ids in `recent.json`, most recent first; that order is the only use the ranking knows.
- `celestina-ui`'s text field (`crates/celestina-ui/src/field.rs`, `input.rs`): Backspace deletes one grapheme, and Ctrl+Backspace does nothing (`plain` excludes Ctrl).
- The shell has no key repeat: `celestina-wayland` asks for its keyboard with `get_keyboard(.., None)`, never with repeat, so a held key acts once (found in a nest while building `CEL-I-014`; `repeat_key` exists but never fires). Holding Backspace today deletes one letter.
- The toolkit has clipping (`View::clip`), wheel events (`UiEvent::Scroll`) and a pure scroll offset, `celestina_ui::Scroll` (`crates/celestina-ui/src/scroll.rs`: `set_extent`, `wheel`, `offset`, clamped), which the sheets use; it has no scroll container that lays out and clips children. The launcher ignores `Scroll`.
- The clipboard worker (`celestina/src/clipboard/worker.rs`) can re-offer a history entry (`ClipboardHandle::select`) but cannot offer a new text.
- `celestina-style/icons/` has `view-grid`, `view-list`, `x`, `film`, `gamepad-2`, `file-code`, `file-text`, `settings` and `toolbox`, but no history, A–Z, globe or calculator glyph.

## 4. Approaches

The author chose the first option in each case on 2026-11-26.

| Question | Chosen | Rejected, and why |
|---|---|---|
| Fuzzy matching and the expression parser | Written by hand in the pure domain, with tests: about 150 and 250 lines, no new crate | `nucleo-matcher` and `evalexpr`: two dependencies, each needing a manifest justification, both doing far more than the launcher needs |
| Scrolling | The launcher's own layout over the toolkit's `celestina_ui::Scroll` offset: it builds the rows that intersect a clipped area and offsets them by that scroll | A generic scroll container in `celestina-ui`: reusable by the Island's pages, but a larger change to a shared crate; left for later |
| Localized names and keywords | `celestina-core`'s reader keeps the localized values of `Name`, `GenericName`, `Comment` and `Keywords` beside the unlocalized ones; the launcher picks the locale | The launcher reading the files again: a second reader, which the architecture standard forbids |
| Ranking by use | A new `usage.json` (launch count and last launch per id), seeded once from `recent.json` | Keeping `recent.json` only: it knows order, not how often |
| Copying the result | A new command of the clipboard worker that offers a text; the result enters the history like any copy | Spawning `wl-copy`: an external program and a path the history never sees |

## 5. Design

### 5.1 The card

- The launcher's card stays 560 px wide and grows from 400 to 444 px high: padding 12, the field's pill 48, gap 8, the tab row 36, gap 8, the rows' area 320, padding 12. It never changes size while open.
- The selector (`Mod+K`) is unchanged: no tab row, no clear button, no sweep, no grid, no calculator and no scrolling, its 8 rows and its 400 px. The card's size therefore depends on the source. Only the toolkit's Ctrl+Backspace and Ctrl+Delete reach it, as every field.
- The pill keeps today's search glyph, field and placeholder (`copy::LAUNCHER_PLACEHOLDER`), and gains the clear button at its right end (§5.3).

### 5.2 Tabs (CEL-D-012)

- The tab row holds, left to right:
  - Recents (`rotate-ccw-clock` glyph);
  - All, A to Z (`arrow-down-a-z` glyph);
  - seven groups: Internet (`globe`), Multimedia (`film`), Games (`gamepad-2`), Development (`file-code`), Office (`file-text`), System (`settings`) and Utilities (`toolbox`);
  - at the far right, the view button (§5.5).
  
  The visible labels are new copy constants (for example `LAUNCHER_TAB_RECENT`, `LAUNCHER_TAB_ALL`, `LAUNCHER_GROUP_INTERNET`) whose Spanish text is the preview's.
- Variant A: each tab is a 32 px pill of radius 16 with an 18 px glyph. The open tab has the control fill, its glyph and label in `TextHi`, and its label (body size, regular weight) unrolls beside the glyph over the normal duration (200 ms) on the standard curve. The other tabs show the glyph alone in `TextLo` and take the control hover under the pointer. The preview's open tab uses the control fill, not the accent of the Island's tabs; the preview rules.
- Each application belongs to one group, the first of these whose categories its `Categories=` names:
  1. Games: `Game`;
  2. Development: `Development`;
  3. Multimedia: `AudioVideo`, `Audio`, `Video`, `Graphics`;
  4. Internet: `Network`;
  5. Office: `Office`, `Education`, `Science`;
  6. System: `System`, `Settings`;
  7. Utilities: `Utility`, and every application that names none of the above.
  
  A group with no application has no tab. Recents and All always show.
- Recents lists the applications with a recorded use, best first by the use score (§5.6), at most 50. With none it shows one line, a new constant `LAUNCHER_RECENT_EMPTY`.
- All and the groups list their applications by display name, folded.
- The launcher always opens on Recents with an empty query.
- Switching tabs:
  - a click on a tab;
  - Ctrl+Tab and Ctrl+Shift+Tab, which wrap;
  - Alt+1 to Alt+9, the visible tabs in order.
  
  A switch empties the query, puts the selection on the first row and the scroll at the top. The new content slides 12 px in from the side of the move while it fades in, over 200 ms on the standard curve.
- While the query is not empty the search covers every application whatever the tab, and the tab row is drawn at 45 % opacity. Its tabs still work, and a click on one empties the query.

### 5.3 Quick clear (CEL-I-014)

- **Ctrl+Backspace and Ctrl+Delete** in `celestina-ui`'s field delete back to the previous word start and forward to the next word end. They use the Unicode word boundaries the field's Ctrl+arrow motions already use; a selection is deleted instead. This applies to every field of the shell.
- **Holding Backspace** in the launcher:
  - the first press deletes one grapheme, as today;
  - the launcher sees every key event before the field does (`launcher/mod.rs` `handle`), so it takes every Backspace repeat (a Backspace press with no release since the last one) and the field never sees it: no repeat deletes a grapheme;
  - once Backspace has stayed down for 600 ms (niri's default repeat delay; the launcher times the hold itself, since the shell has no key repeat), or at the first repeat if one ever arrives sooner, the sweep starts: a wash of the selected-wash role (`surfaceSelected`, the accent at 20 %) grows from the text's end to its start over 450 ms, linear;
  - when the sweep ends, the field and the query empty;
  - further repeats during the hold do nothing;
  - releasing Backspace before the end cancels the sweep and leaves the text as it was when the sweep started (one grapheme shorter than before the press).
  
  The whole hold takes about one second, as in the approved preview, where the sweep also starts at the repeat delay. A Backspace pressed during a composition never sweeps until it is released. A Backspace with Ctrl, Alt or Logo held is not a hold.
  
  The sweep sits behind the text, inside the pill, between the pill's vertical insets of 10 px.
- **The clear button**:
  - a 28 px round button with the 16 px `x` glyph in `TextLo`, at the pill's right end, shown only while the query is not empty;
  - it fades and scales in from 0.8 over the fast duration (100 ms) and takes the control hover;
  - a click empties the query and leaves the focus in the field.
- Escape keeps closing the launcher; the "Escape clears first" variant was not chosen.

### 5.4 Scrolling (CEL-I-017)

- The rows' area is 320 px high and clipped to its rect. Rows (40 px) or grid cells (80 px) are laid out at their full positions, offset by the scroll; only those that intersect the area are built.
- The offset is a `celestina_ui::Scroll` (one owner; no second offset type). The wheel moves it by the vertical amount `UiEvent::Scroll` carries, in logical px as the sheets already use it, clamped to the content.
- A keyboard move that puts the selected row or cell outside the area scrolls it into view, animated over 200 ms on the standard curve, or at once under reduced motion.
- A thin scroll bar shows on the area's right edge while it moves and for 800 ms after, then fades over the slow duration (350 ms):
  - 4 px wide, 2 px from the edge, radius 2, in the strong-hairline role;
  - its length is the area's share of the content, at least 24 px.
- The selection still wraps at both ends; a wrap scrolls to the other end.
- `MAX_ROWS` stops bounding the answer. The ranked answer is bounded by the index's own bound (`MAX_INDEXED_ENTRIES`). The selector keeps its own cap of 8 rows.

### 5.5 Grid (CEL-D-012)

- The view button at the tab row's right end (36 by 32 px, the `view-grid` glyph in the list view and `view-list` in the grid view), or Ctrl+G, switches between the list and the grid. The view crossfades with a scale from 0.985 over 200 ms.
- The choice is written to `shell.json` as `launcher.view` (`"list"` or `"grid"`, default `"list"`), the way `bar.tray_hidden` is written (`providers/settings.rs`). An unknown value reads as the list.
- The grid has 6 columns across the 536 px content width. Each cell is 80 px high, with radius 12, a 40 px icon, a 6 px gap and the name below in the secondary size (12) in `TextHi`, on one line, cut with an ellipsis.
- A cell's states are a row's: the control hover under the pointer, the control fill when selected, and the focus ring once the keyboard moved the selection.
- Arrows in the grid:
  - Up and Down move by one row of 6;
  - Left and Right move by one cell, only while the field is empty (with text they move the caret);
  - every arrow wraps at the ends.
- Search results use the chosen view.

### 5.6 Search (CEL-I-015)

- **Localized fields.** `celestina-core`'s `DesktopEntry` gains the localized values of `Name`, `GenericName`, `Comment` and `Keywords`:
  - every `Key[locale]` of those four keys is kept, bounded;
  - a method returns the best value for a locale by the Desktop Entry Specification's matching order (`lang_COUNTRY@MODIFIER`, `lang_COUNTRY`, `lang@MODIFIER`, `lang`, then the unlocalized key).
  
  The existing fields keep their meaning, so Siderita and every other caller is unchanged. The launcher takes the locale from `LC_ALL`, then `LC_MESSAGES`, then `LANG`.
- **Display name.** The localized `Name` when the file has one, else `Name`: the author chose names in Spanish, so Bottles shows as its translators named it. The detail text is the localized `GenericName`, else `GenericName`.
- **Terms.** The query, folded, splits on whitespace into terms. Every term must match the entry; terms match in any order.
- **One term's score** is the best of these tiers:

  | Score | Tier | Matched against |
  |---|---|---|
  | 60 | equals a name | localized or unlocalized `Name` |
  | 50 | a name starts with the term | either name |
  | 40 | a word of a name starts with the term | either name |
  | 30 | a word starts with the term | the secondary texts (below) |
  | 20 | one edit from a name word | either name; terms of 4 characters or more |
  | 15 | one edit from a secondary word | secondary texts; terms of 4 characters or more |
  | 10 | the term's letters in order | either name |
  | 5 | the term's letters in order | secondary texts and `Exec` |

  The secondary texts are:
  - `GenericName` and `Keywords`, localized and not;
  - the Spanish aliases (below).
  
  `Comment` is not searched (today it answers loosely; it made results noisy).
- **Edit distance.** One edit is an insertion, a deletion, a substitution or a swap of two adjacent letters, measured against the word's prefix of the term's length (or of one more or one less), so that `krtia` finds Krita and a typo in a prefix finds the word.
- **An entry's score** is the mean of its terms' scores. A query that equals a whole display name scores 60, whatever the words.
- **Aliases.** Spanish words for the categories, as product copy in `copy.rs`, count as secondary texts:
  - each group's label;
  - a short table of freedesktop categories, for example `WebBrowser` gives the alias that means "web browser", `TerminalEmulator` "terminal", `TextEditor` "text editor", `FileManager` "files", `Player` "player", `InstantMessaging` "chat", `Emulator` "emulator".
  
  Typing the Spanish word for browser therefore finds all three of the author's browsers, although only one says so in its file.
- **Order.** Score, then the use score, then the display name.
- **Use score.** `usage.json` under the shell's configuration directory holds, per desktop-file id, the launch count and the last launch time:
  - at most 200 ids, bounded and validated as `recent.json` is today (`recent.rs`), written whole and atomically after each launch;
  - the score is the count times a weight by the age of the last launch: 100 under 4 days, 70 under 14, 50 under 31, 30 under 90, 10 beyond;
  - on the first start with no `usage.json` and a `recent.json`, each recent id gets a count of 1 and a last launch one minute apart, most recent first. `recent.json` is then no longer written and is left on disk.
- **Bounds.** The matcher runs per keystroke over the bounded index:
  - at most 8 terms are read, and a term longer than 64 characters is cut;
  - the edit check is linear in the word.
- **Unchanged.** The `/` switches (a `/` query lists them as list rows whatever the view, under the dimmed tab row; Ctrl+Tab, Alt+digit and Ctrl+G still act and a tab switch empties the query), and the `Mod+K` selector, which keeps `celestina_shell::launcher::score`. The new matcher lives beside it in the same pure module.
- **`recent.rs`** becomes a read-only seeder: it keeps its bounded parse of `recent.json` and loses its writer. The new use record lives in its own module, and `LauncherState::new` takes the use records instead of the recent ids.

### 5.7 Calculator (CEL-I-016)

- **The parser** is pure, in the domain crate. Its grammar:
  - numbers with a comma or a point as the decimal separator;
  - `+ - * / × ÷ ^ %` (postfix percent divides by 100) and parentheses;
  - implicit multiplication before a parenthesis or a function;
  - unary minus and plus;
  - the functions `sqrt`, `√`, `sin`, `cos`, `tan` (radians), `ln` and `log` (base 10);
  - the constants `pi`, `π` and `e`.
  
  `^` is right-associative and binds tighter than unary minus on its left (`-2^2` is -4). Nesting is bounded at 32 levels and the input at the field's own bound; past either the query is not an expression.
- **The row's condition.** The query is an expression only when it parses whole, holds at least one binary operator, a postfix percent or a function, and gives a finite result. So `-5`, `pi` and `2` alone show no row. Implicit multiplication applies only before a parenthesis or a function, never before a constant or a number, so `2e` and `7-zip` do not parse. Then a row comes first, above the applications that still answer the same text, and it is the selected row, so Enter copies rather than launches.
- **The row** has the `calculator` glyph in `TextLo` and the result in the row-title size (15), tabular, after an `=` in `TextLo`. The detail reads a new constant, `LAUNCHER_CALC_COPY` (the preview's "Copy"), with an Enter key cap. The row grows in from 0 to 40 px with a fade over 200 ms.
- **The result** has up to 12 significant digits and a comma decimal separator, with no grouping. Beyond 1e15, or below 1e-9 and not zero, it is written with an exponent in the same style. A division by zero or a non-finite result shows no row.
- **Enter or a click** on the row:
  - the result is offered to the clipboard as shown, through a new `ClipboardHandle::offer_text`, and enters the history;
  - the glyph becomes `check` and the detail a new constant, `LAUNCHER_CALC_COPIED`, in the success role;
  - after 350 ms the launcher closes.
  
  With the clipboard duty off, the row shows the result with no detail, stays selected, and Enter does nothing; the app tells the launcher whether it can copy when it opens it.
- **`offer_text`** is a clipboard worker change with its own tests. It sets the selection through the path `Select` already uses for the worker's own sources, so the worker does not record its own offer twice. The text enters the history as a copy would: not while reading is paused, and deduplicated as any copy is.
- **In the grid** the row spans the full width above the cells.

### 5.8 Motion

All durations and the curve are the motion tokens (fast 100, normal 200, slow 350, `CURVE_STANDARD`). Under reduced motion every change above is immediate.

### 5.9 Icons (STYLE-ICONS-2)

`rotate-ccw-clock`, `arrow-down-a-z`, `globe` and `calculator` come from Lucide 1.27.0, the release `STYLE-ICONS-1` vendored (Lucide renamed `history` to `rotate-ccw-clock`, and the catalogue keeps the upstream name), with the catalogue's colour and stroke recipe (`#fff`, 2.5). They are registered as that unit did: `CMakeLists.txt`, `icons.qrc`, `CelestinaIcons.qml` and `tests/tst_iconcatalog.qml`.

## 6. Order and dependencies

Small and safe first, as the tracks require:

| Order | Entry | Depends on |
|---|---|---|
| 1 | `CEL-I-014` quick clear | none |
| 2 | `STYLE-ICONS-2` four glyphs | none |
| 3 | `CEL-I-015` search and use ranking | none |
| 4 | `CEL-I-016` calculator | `STYLE-ICONS-2` |
| 5 | `CEL-I-017` scrolling | none |
| 6 | `CEL-D-012` tabs and grid | `STYLE-ICONS-2`, `CEL-I-015` (Recents), `CEL-I-017` |

Each is a milestone landing of its own, tried live by the author. The shell's entries run the launcher's smoke phases in a nest.

## 7. Verification

- Unit tests in the pure modules:
  - the matcher's tiers, terms, typos, aliases and bounds;
  - locale matching;
  - the parser's grammar, bounds and formatting;
  - use scoring and the seeding from `recent.json`;
  - group assignment;
  - the scroll offset that keeps the selection visible.
- Tree tests in `view.rs`:
  - the card's size per source;
  - tabs, cells and the calculator row in place;
  - the clear button only with text.
- Input tests in `launcher/mod.rs`:
  - the hold sequence (press, repeat, release before and after the sweep);
  - Ctrl+Tab, Alt+digit and Ctrl+G;
  - grid arrows with and without text.
- Nest screenshots of each visible state against the approved preview, under `docs/evidence/`.

## 8. Left to the builder

- Where in `LauncherState` the tab, the view and the scroll live, and how the tree keeps the scroll between applies.
- How the area clips: a clipped box with absolute children, or a translated column.
- The exact copy constant names, and the Spanish alias table beyond the categories listed in §5.6.
- How the launcher learns a Backspace release (the field already sees key releases as `pressed: false`).

## 9. Calls made after the builder's read

The author did not decide these; they follow the approved preview or today's code, and the author may overturn any.

1. The anti-goal protects only the strong name tiers (equal, prefix, word start); a keyword word start ranks above a name typo or a name subsequence.
2. The scroll offset reuses `celestina_ui::Scroll`.
3. Backspace repeats never reach the field; the sweep starts 600 ms into the hold (or at a first repeat); a cancel keeps the text as the sweep found it.
4. The calculator row needs a binary operator, percent or function; implicit multiplication only before a parenthesis or function; the row is selected; with no clipboard it stays and Enter is inert.
5. `offer_text` follows the worker's own-source path, pause and dedupe rules.
6. The selector keeps today's look and size entirely.
7. A `/` query lists switches as rows in either view, under the dimmed tabs.
8. `recent.rs` keeps only its reader, as the seeder of `usage.json`.
