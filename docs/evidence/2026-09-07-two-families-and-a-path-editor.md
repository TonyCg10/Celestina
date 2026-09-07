# Evidence: 2026-09-07 two families of interaction, and a path pill that edits

- **Date:** 2026-09-07
- **Scope:** `FEEDBACK-4` — `celestina-style` 1.8.6, `siderita` 1.5.8,
  `fluorita` 1.3.5, `grafita` 1.2.4. Follows `FEEDBACK-1`
  ([record](2026-09-03-apps-feedback-and-icon-first.md)) after the author
  reviewed the delivered bytes
- **Environment:** the author's Arch-derived Linux, Qt 6.11, `cargo` stable,
  offscreen QPA for tests. No real pointer was driven; the interaction tests
  synthesise presses, sweeps and keys on the real components
- **Artifact:** the registered production artifacts of the four owners

## What the author saw

Three things, in one sentence each.

1. The click still read as "the same light background". Buttons already sank
   (`FEEDBACK-1`), but the plates behind rows and cells — the sidebar, the
   files in the middle, the tabs — only changed colour, and that is most of
   what is clicked in a file manager.
2. Under the pointer everything went the same grey, and on the click it went
   blue: `surfaceHover` → `badgeAccentFill`, a hue change on the release that
   read as a flicker. The author asked for a softer blue first and the final
   blue after, and for the middle of the window to behave differently from the
   sidebar and the buttons.
3. The path pill: clicking empty pill edited the path, clicking the last crumb
   — the folder you are in — did nothing, and there was no way to select by
   sweeping, so the I-beam cursor promised an editor the pill was not.

## Inventory of what is clicked

The whole suite, from `rg` over every `MouseArea`, `TapHandler`,
`HoverHandler`, `CelestinaButton`, `CelestinaIconButton`, `FloatingButton`,
`CelestinaRowHighlight`, `GlassMenuItem`, `CelestinaSwitch`, `CelestinaSlider`
and `CelestinaTextField`, sorted into the two families the author named. The
family decides the hover; the press sinks in both.

| Family | What | Hover | Press | Selected / current |
|---|---|---|---|---|
| **Control** — behaves like a button | `CelestinaButton`, `CelestinaIconButton`, `FloatingButton` and every pill built on them | role fill (`surfaceHover`, `accentHover`, …) | fill darkens, **sinks 0.96** | `Selected` role / `checked` |
| | Sidebar rows: places, devices, phone, favourites, bookmarks (`Sidebar.qml`, `SidebarFavoriteRow`, `SidebarBookmarkRow`, `PickerSidebar`) | `surfaceHover` | `pressedWash`, **plate sinks 0.985** | `badgeAccentFill` |
| | Sidebar section headers and their trailing counts | text tone only | — | fold state |
| | Tabs (`siderita/…/TabStrip`, `grafita/…/TabStrip`) | `surfaceHover` | `pressedWash`, sinks | `badgeAccentFill` / `surfaceSelected` |
| | Column titles (`DetailsHeader`) | `surfaceHover` | `pressedWash`, sinks | sort glyph |
| | Breadcrumb ancestors (`TopBar`) | `surfaceHover` | `pressedWash`, sinks | — |
| | Plugin rows (`magnetita/…/PluginRow`) | `surfaceHover` | `pressedWash`, sinks | switch |
| | Menu items (`GlassMenuItem`) | `surfaceHover` | — (menu closes) | `badgeAccentFill` |
| | Switches, sliders | own anatomy | own anatomy | own anatomy |
| **Content** — the thing you came to look at | Files as rows (`FolderRowDelegate`) | **`contentHover`** — the accent at 7 % | **`pressedWash`** — the accent at 26 %, **plate sinks** | `badgeAccentFill` — 15 % |
| | Files as cells (`FolderCellDelegate`) | `contentHover` | `pressedWash`, **plate and content sink together** | `surfaceSelected` — 20 % |
| | Picker cells (`PickerCellDelegate`) | `contentHover` | `pressedWash`, sinks | `surfaceSelected` |
| | "Open with" and icon-picker rows | `contentHover` | `pressedWash`, sinks | `badgeAccentFill` |
| | Gallery cards, music rows, filmstrip frames (`fluorita`) | `contentHover` | `pressedWash`, sinks | own card surface |
| | Encoding rows (`grafita`) | `contentHover` | `pressedWash`, sinks | `badgeAccentFill` |
| | Recent-document rows (`grafita`) | Ghost button | sinks 0.96 | — |
| **Editor** — text under the pointer | The path pill (`TopBar`) | I-beam | caret lands, sweep selects | the field's selection |
| | Search field, rename field, dialog fields | I-beam | Qt's text field | Qt's selection |

Content's three depths are one hue: 7 % under the pointer, 26 % under the
finger, 15–20 % when selected. The release is therefore felt as a settle from
the press *down* to the selection, not a jump from grey to blue. Controls keep
the neutral lift because a sidebar row or a tab is furniture, not the thing
being looked at; the press there also takes the accent wash, so even a control
never goes grey → blue in one step.

## What changed

**`CelestinaRowHighlight` has two families.** `family: Control` (default) and
`family: Content`. Both sink on press by the new `rowRecoilScale` (0.985) —
the full button recoil made a wide row lurch — and both take the new
`pressedWash`. Content's hover is the new `contentHover`. Hosts that want the
row's content to sink with the plate bind their `scale` to the plate's
`recoil`; the grid cell does, so an icon and its name go down as one object.

**Two tokens, one recipe.** `accentContentHoverOpacity 0.07` and
`accentPressedWashOpacity 0.26` join the accent recipe in `CelestinaTheme`, so
a change of `ref.accent` still moves every state together. `DESIGN.md` §7
names the two families and forbids the grey-to-blue transition in words.

**Fourteen hosts declare their family.** Siderita's file rows and cells, the
picker cells (whose hand-rolled plate is replaced by the shared one), the
"open with" and icon-picker rows; Fluorita's gallery cards, music rows and
filmstrip frames (the first two were hand-rolled grey plates); Grafita's
encoding rows. Everything else keeps `Control` by default and gains the sink.

**The path pill is an editor with crumbs on top.** The field is now present
under the crumbs at all times, transparent until it is the editor. A left
press on empty pill turns editing on and is then *refused* by the pill's
`MouseArea`, so Qt delivers the very same press to the field underneath: the
caret lands under the pointer and a sweep from there selects, exactly as in
any editor. Nothing is re-dispatched. The current folder's crumb — which has
nowhere to navigate — opens the editor with that one name selected, so typing
replaces it and Home/End reach the rest; its cursor is the I-beam, the
ancestors keep the hand. Ctrl+L and Tab still open the editor with the whole
path selected. Escape, focus loss and a completed navigation put the crumbs
back, as before.

## Procedure

| Check | Result |
|---|---|
| `celestina-style`: `cmake --build`, `ctest` (73 Qt Quick tests) | pass |
| `siderita`: `qml-tests.sh` — 108 pass, 4 new in `tst_path_pill.qml`: a press on empty pill places the caret and a sweep selects; the current crumb selects its own name and navigates nowhere; an ancestor crumb navigates; Escape restores the crumbs | pass |
| `siderita`, `fluorita`, `grafita`: `cargo build --locked` | pass |
| `check-language-contract.py`, `documentation_contract.py` | OK |
| Production `build-production.sh` + `verify-production.sh`, four owners | pass — `celestina-style` 1.8.6, `siderita` 1.5.8, `fluorita` 1.3.5, `grafita` 1.2.4 |
| Deployment to the author's prefix | Siderita, Fluorita and Grafita deployed from the verified artifacts |

## Result

Every plate in the suite sinks on press; the middle of every window hovers,
presses and selects in three depths of the accent; the sidebar, tabs and
titles keep the neutral lift and take the wash only on the press. The path
pill behaves as its I-beam promised.

## Limits

- Whether 7 % reads as "soft blue" and 26 % as "pressed" on the author's panel
  is a perceptual claim and stays with `VAL-STYLE-06` and `VAL-SID-14`.
- A sweep that begins on the last crumb selects that name rather than
  starting a drag-selection from the pointer, by design: the crumb is text
  set in another font and alignment, so a caret placed under the pointer
  would land somewhere unrelated in the monospace path.
- The sidebar's "current" wash still arrives from `surfaceHover`, a grey, on
  the release. The author accepted that controls and buttons may share a
  vocabulary; if the sidebar should also run the accent ramp, it is one
  `family: Content` per row.
