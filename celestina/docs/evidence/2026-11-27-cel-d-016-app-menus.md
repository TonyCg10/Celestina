# Evidence: app menus as capsule menu windows (CEL-D-016)

- **Date:** 2026-11-27
- **Scope:** `CEL-D-016`: `celestina/src/tray`, `celestina/src/sheet`, `celestina/src/island`, `celestina/src/app.rs`, their tests, the ledger row and `VAL-CEL-D-016`
- **Environment:** worktree `celestina-CEL-D-016` from `main` at `7c400adc` (rebased); the first nest ran at `24732ae0` (Tasks 1 to 5); the sheets s07, s10, s14 and s17 were shot again after the review's fixes, at `f0763345`, `d289c952` and `69b8f2df`
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** the tray stage of "Celestina · la gota", version 3, approved by the author on 2026-11-27 (an artifact private to the author)

## Procedure

```sh
CARGO_TARGET_DIR=$HOME/.cache/cel-d016-tgt cargo build --locked --workspace --bins --examples
timeout -k 60 2700 sh scripts/smoke.sh --binary $HOME/.cache/cel-d016-tgt/debug/celestina --only bar,osd,notify
```

The nested look ran through the guarded kit of `CEL-D-013` (`env -i`, `--own-bus`, `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`, fake `wpctl`, `pactl` and `brightnessctl`, the private fake system bus, nested niri at 2560 by 1440 logical and scale 1.5, one `timeout -k 60 2700`, one nest at a time under the host nest lock). Every helper that talks to the nest calls `nest_guard` first. The binary is the branch build as committed. Frames are `grim` loops on the nest's output as in `CEL-D-013`; each tile is labelled `+N ms #shot` from the press (the nest's input and frame latency is about 100 ms, so offsets are from the press, not from the animation's start; a large crop makes each shot take about 28 ms, so tiles are one frame apart at best).

The fake items are a StatusNotifierItem process the probe starts inside the nest, on the nest's own bus, with a `com.canonical.dbusmenu` object each (`GetLayout`, `GetGroupProperties`, `AboutToShow`, `Event`, `LayoutUpdated`). The watcher takes four items per connection, so two processes ran:

| Item (`Id`) | Menu |
|---|---|
| `fake-menu` | 4 entries, a separator, a disabled entry, a checked toggle, a submenu of 3, a submenu that is empty until `AboutToShow` and fills 250 ms later with a `LayoutUpdated`, a 40-grapheme label |
| `fake-long` | 64 entries |
| `fake-slow` | the same menu as `fake-menu`, its layout 300 ms late |
| `fake-empty` | an empty root that never fills |
| `fake-h1` to `fake-h4` | the `fake-menu` menu again (to give the hidden-apps window six rows) |

Layouts: the bar-menu runs used `bar.tray_hidden` empty (all four icons on the bar); the hidden-apps window and Tray page runs used `tray_hidden = [fake-long, fake-slow, fake-h1..h4]` (icons `fake-menu` at x 2168 and `fake-empty` at x 2203 stay on the bar, chevron at x 2134). The item logs every `Event` and `AboutToShow` it gets.

Runs, one nest each: bar menus of `fake-menu` and `fake-empty`; hidden-apps window; bar `fake-slow`, switches and `fake-long`; Tray page (`island open tray`, sent by the probe inside the nest); reduced motion (`appearance.toml`); the stress run (20 bar and 20 window cycles, shell started with `--metrics`); the smoke.

## Result

- **Tests:** `cargo test --workspace --locked` 3126 passed, 0 failed, 2 ignored at `69b8f2df`; `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` on the pinned toolchain and on 1.100.0, and the three root contracts pass
- **Smoke:** at `24732ae0`, `smoke: OK (only: notify,bar,osd)`, exit 0. notify `rest_frames=2` (230 winit frames, `late=0`), bar `rest_frames=0` (133 frames, `late=0`), osd 189 frames. No smoke phase opens a tray menu; the landing runs the full smoke
- **Bar menu opens as a drop** ([sheet](2026-11-27-cel-d-016-app-menus/s01-bar-open-drop.jpg), `fake-menu`, right click on its icon): bulb and thread from +130 ms, the bulb falls and holds (+176 to +350 ms) although the layout came in 5 ms, then a rounded block (+386), the window with its content blurred (+422, +454) and sharp (+496). The window's rows are the approved ones: 40 high, a 16 px check column (the check on "Checked toggle"), chevrons on the two submenus, "Disabled entry" faint, one divider before "Ocultar de la barra" (the shell entry), the 40-grapheme label cut with an ellipsis
- **Submenu drill-in** ([sheet](2026-11-27-cel-d-016-app-menus/s02-bar-drill-in.jpg)): the row is hovered, then the level slides: in full-resolution crops the new level sits at about 140 px from the window's left edge in the first frame, 139, 121 and 58 in the next ones (it ends at 55), and the old level's divider leaves on the left. The new first row is the back row (chevron-left and "Submenu of three"). **Defect:** the window's height is one step in the first drill frame (about 520 physical pixels for the root, about 260 in the first sublevel frame), then 260, 245 and 205: the 200 ms height morph is not seen as a morph in this sampling, only as a step and a short ease
- **Back** (Left key) and **Escape**: the level returns, the window grows back; Escape at the root closes. Read in the settled shots and the item log, not as a burst sheet
- **Delayed submenu** ([sheet](2026-11-27-cel-d-016-app-menus/s03-bar-late-submenu.jpg)): the row stays hovered and the root level stays up for about 300 ms; the new level then slides in with the three late entries. The window does not collapse while it waits
- **`fake-empty`** ([sheet](2026-11-27-cel-d-016-app-menus/s04-bar-empty-shell-entry-alone.jpg)): the drop holds until +620 ms, then the window shows only "Ocultar de la barra" at +671 ms: the shell entry alone after `MENU_LATE_LAYOUT`
- **Held drop on `fake-slow`** ([sheet](2026-11-27-cel-d-016-app-menus/s05-bar-held-drop-slow.jpg)): the bulb hangs under the icon from +96 to +397 ms (the layout takes 300 ms), then the window opens (+450 rounded block, +503 content blurred, +553 sharp). It matches the plan's waiting rule: the drop runs swell and fall and holds
- **Switch between two icons of the same capsule**:
  - `fake-slow` to `fake-menu` ([sheet](2026-11-27-cel-d-016-app-menus/s06-bar-switch-slow-to-menu.jpg)): the window's content blurs out and the new content blurs in, the window travels about 70 physical pixels and keeps its size. No collapse
  - `fake-menu` to `fake-slow` ([sheet](2026-11-27-cel-d-016-app-menus/s07-bar-switch-menu-to-slow.jpg), after review fix I2): the window keeps its size and moves to the slow icon while the old content blurs out. It stays empty from +153 to +450 ms, while the fake item takes 300 ms to answer, and the new content comes in at +501 ms. The first shoot, before the fix, showed the window shrinking to a line and a gap of about 50 ms before a new window grew
- **`fake-long` from the bar** ([sheet](2026-11-27-cel-d-016-app-menus/s08-bar-long-open.jpg)): a drop, then a window at the 496 cap with rows 01 to 12 in view. The wheel (vertical 60) moves the rows at once and shows the thumb on the right edge ([settled shots](2026-11-27-cel-d-016-app-menus/s09-bar-long-keyboard-shell-entry.jpg): tiles in order: 3 Down presses, 63, 64, 65, 66, 67). After 65 Down presses the focus ring is on "Ocultar de la barra" with the divider above it, the last entry in view and the thumb at the bottom; the 66th Down wraps to Entry 01 and the list returns to the top
- **Hidden-apps window with a card** ([unfold](2026-11-27-cel-d-016-app-menus/s10-window-slow-card-unfold.jpg), `fake-slow` row, right click): the row takes the press fill at +46 ms and the other rows stay sharp while the card waits (after review fix I3); the card comes at about +400 ms (300 ms late layout) as a block that grows and sharpens; the window's glass grows with it and the last frames hold still; the card is 264 wide under the row with the card fill. The window is 280 wide with six rows. Other rows blur at the distances of the plan: [states](2026-11-27-cel-d-016-app-menus/s13-window-states.jpg) (card on `fake-h2`, fourth row): `fake-h1` and `fake-h3` lightly blurred, `fake-slow` and `fake-h4` more, `fake-long` most. **All three levels show in one frame and each looks blurred**, the three different from each other
- **Fold** ([sheet](2026-11-27-cel-d-016-app-menus/s11-window-card-fold.jpg)): Escape turns the card's content to a blur, the card and the window shrink, the rows sharpen; at +280 and +314 ms a hairline stays under the open row for two frames, then it is gone; Escape again closes the window (item log, below)
- **Card submenu and scroll** ([states](2026-11-27-cel-d-016-app-menus/s13-window-states.jpg): third tile, and [sheet](2026-11-27-cel-d-016-app-menus/s12-window-card-scroll-thumb.jpg)): the back row and three entries show inside the card and the card shrinks to them; a card of 9 rows scrolls at once with the wheel, the thumb shows at its right edge from +103 to +793 ms and is gone at +891 ms (800 ms after the last move)
- **Another row's card:** the card of `fake-h2` goes at once (no fold frames) when `fake-slow`'s row is pressed, and the new card waits and unfolds ([states](2026-11-27-cel-d-016-app-menus/s13-window-states.jpg), seventh tile). Escape then folds it and a second Escape closes the window
- **The Tray page card** ([sheet](2026-11-27-cel-d-016-app-menus/s14-tray-page-card.jpg), `island open tray`, after review fixes I4, I5 and the last wave): a card on the first row unfolds with `fake-long` and the page only grows. The rows past the card leave one by one as the box reaches them, and none is painted over the box. A switch from row 4 to row 2 shows no frame of the old box. Escape folds the card and the page stays; while it folds, a returning row shows faintly over the box's lower edge for about two frames. The card has the room under its own row (`cap = room - lead - (row + 1) * 40 - 6`). A row with less than 48 px under it opens no card and asks the app for its own menu (`ContextMenu`): with eight rows every row has room, so a unit test covers that case, not the nest. In the first shoot, with the show entry of a hidden app focused, Return folded the card, removed `fake-slow` from `bar.tray_hidden` and its icon showed on the bar; no `Event` reached the app for the shell entry
- **Reduced motion** ([bar](2026-11-27-cel-d-016-app-menus/s15-reduced-bar-open.jpg), [card](2026-11-27-cel-d-016-app-menus/s16-reduced-window-card.jpg)): the bar window has no drop; it fades in as one rectangle from +114 to +250 ms. The card appears whole in one frame (+398 none, +454 complete), with no blur and no height growth; rows blur at once. On the Tray page ([sheet](2026-11-27-cel-d-016-app-menus/s17-reduced-round1.jpg), after `69b8f2df`) the card and the switch show no overlap: the rows the card has no room for go in the same frame
- **Fake item log** (`calls.log`, every run):
  - every menu shown got one `opened` and one `closed`; the stress run counted 20 and 20 for the bar icon (`fake-menu`) and 20 and 20 for the window row (`fake-h1`)
  - a choice reaches the app as `Event(id,clicked)` and then `Event(0,closed)` in the same millisecond: "Reload settings" (id 2), "Checked toggle" (id 5), "Entry 01" from a card (id 1)
  - a click on the disabled entry sent nothing; a click on the checked toggle closed the menu with its `clicked`
  - a switch sends the new menu's `opened` before the old one's `closed` (both directions); a card removed by pressing another row sends its `closed` after the new menu's layout answer, because the item's single worker answers its calls in order and `fake-slow`'s layout holds it for 300 ms
  - the late submenu: `AboutToShow(7)`, `LayoutUpdated`, then one `GetLayout`
  - a wheel over a row of the window (not over a card) sent `Scroll(-6, vertical)` to that app, as before
- **At rest** (the stress run, shell with `--metrics`): last close of the run at 04:00:11.955 UTC, then 12 s untouched with the pointer parked. The only rest frame after it was at 04:00:14.890, on the 20 s grid of the earlier ticks (03:58:34.874, 03:58:54.876, 03:59:14.882, ...). Nothing was left running by a linger, card, slide or row clock. During the cycles the rest frames pair with the probe's pointer moves (two per window cycle, one in 20 bar cycles that pointed at one icon)

## The parked bug: "tray menu: dismissed by the compositor"

- 20 opens and closes of the bar menu (10 closed with Escape, 10 with a press on the icon) and 20 of the window's card (two Escapes each): the shell's stderr of that run, with `RUST_LOG=info`, has 0 lines containing `dismissed by the compositor`, and 0 containing `popup_done` or `popup:`. The reduced-motion run and the long-menu run have 0 as well. The stderr of the first three runs was not kept
- `rg -n "dismissed by the compositor" celestina/src` finds one line, `celestina/src/island/menu.rs:414`, the Tray page's overflow menu, which still uses `xdg_popup`
- The code path that logged this line for a tray menu (`TrayMenu` as a popup, closed by the compositor's `popup_done`) is gone. The nest is niri nested in the author's session, and the check ran there only: it neither reproduces nor rules out the bug as the author saw it with Steam's menu. `VAL-CEL-D-016` step 7 checks it over a day of use
- The log came from the shell's stderr as the kit captures it, not from `journalctl`

## Review

- Final review `CEL-D-016.R`, Opus at high reasoning, `/code-review` and the plan's Review Focus, two rounds:
  - round 1: 0 critical, 5 important, 4 minor, 5 nits. The important ones: the hidden-apps card followed a list index, not its app (I1); a switch to a slow menu collapsed the window (I2); rows blurred while the card waited (I3); the Tray page card had about two rows of room, or none on a full page while it still took the keys (I4); the page's card slot faded in and out with the page (I5). All fixed in `f0763345`, with three nits.
  - round 2 re-read the fixes: 12 fixed, I4 in part. It found that the page fell and grew back at each unfold on a crowded page, and that an app added before the open one dropped the card. Both fixed after the last round in `d289c952`, with the controller's checks in the nest.
- Fixes after the review, checked in the nest: one frame of the old box on a row switch, and rows fading under the card under reduced motion (`d289c952`, `69b8f2df`).
- Decisions the review left to the session (in the session's ledger): the Tray page card takes the room under its own row and the rows past it leave the page, since the plan keeps the page's rows from scrolling. A row with no room opens the app's own menu. A choice in the page card folds it over 350 ms.

## Defects and notes

- A drill-in looked like a height step in the first shoot. The review found the 200 ms morph in the code: the first sampled frame was already 60 to 70 ms in. A test pins the height in the middle of the morph.
- At the end of a fold, a hairline under the open row stays for two frames. The approved preview does the same.
- The bar window with a slow menu stays empty for as long as the app takes to answer.
- On a page switch of the Island, the old and new content fade into each other for about 100 ms with no jump. No earlier evidence of a page switch exists to compare with.
- The late-layout probe, the held drop and the fold were seen at the nest's frame rate (about one frame per 28 ms in the large crops, 25 to 50 ms in the later ones); the 350 ms and 200 ms curves are pinned by the clock tests.
- Escape and Left were sent with `wtype` into the nest; the `wtype` process opens a new virtual keyboard per call, so a loop of single calls lost most of its presses (70 calls reached Entry 05); one call with many keys is reliable.

## Limits

- Fake items only: no Steam, nm-applet or blueman. Their real menus (icons are not read, shortcuts are not read, long-lived `LayoutUpdated` bursts) were not seen.
- The shell hard-codes the dark theme; there is no light theme to shoot.
- The nest's blur is the shell's own over the nest's wallpaper; niri's blur behind the windows is only seen in the author's session.
- One scale (1.5), one output, mouse and keyboard only. A Tray page row with no room for a card (ten or more apps) was not driven. Not driven: middle click and wheel on a bar icon with a menu open, the hide entry from a bar menu (covered by tests), an app that quits while its menu is open, a republish that removes the open submenu (tests), the Tray page card on a second output.
- The parked bug was checked in the nest only (see above).
- The settled shots of the first bar run were cut to a 400 by 300 box by a probe slip; the bursts of the same states are complete and are the ones cited.
- The raw bursts are not kept (about 170 MB); the sheets and montages here are the record.
- `cargo +1.100.0` is not installed here; the earlier tasks ran clippy through `rustup run 1.100.0 cargo clippy`.

## Notes

The author's steps on the real session are in `VAL-CEL-D-016`.

## Landing

- **Base revision:** `7c400adcf33b9559ed21806917985706adc59e33`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:a4fe10ac358c6668b08efe564ac3e2e96d46725d85038628d1092b300e2e7041, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
