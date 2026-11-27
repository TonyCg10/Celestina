# Celestina: menus as a drop, the piece in its slot, a travelling switch and app menus born from their icon

- **Date:** 2026-11-27
- **Entries:** `CEL-D-013` to `CEL-D-016`, on the shell's standing plan `CEL-TRACKS`
- **Approved preview:** "Celestina · la gota", version 3, approved by the author on 2026-11-27 (drop G1, the grey card, the open piece at 80 %, the travelling switch); published as a private artifact, its source kept in the sessions' notes as `cel-d/preview-approved/src16.html`
- **Status:** approved; implementation by the four plans named in §9

## 1. Intent

The author recorded celestina 1.35.0 and asked for five changes to the menus:

1. The tray's app menus have no motion, unlike the capsule menus, and the one
   opened from the hidden-apps window lands beside that window, over other
   applications. An app's menu should come out of the app.
2. Moving from one piece's menu to another's has a delay; it should be
   instant.
3. The close ends with the window lying over the piece's pill for a few
   milliseconds.
4. The open piece goes back to its slot in the bar, keeps the focus pill and
   shows a little less opacity.
5. Opening and closing a menu read as a drop that detaches from the bar and
   merges back into it, with the content blurred while it moves.

Success: the author sees each menu, the shell's and the apps', leave the bar
and return to it as one liquid shape. A switch shows one box. The bar keeps
every piece in place, and an app's menu hangs from its app.

## 2. Goals, non-goals, anti-goals

- **Goals:**
  - drop G1 for every capsule menu window, open and close;
  - the open piece stays in its slot with the grey pill and 80 % content
    opacity;
  - a switch on the same capsule morphs the open window; a switch to another
    capsule never waits;
  - tray app menus as capsule menu windows: from a bar icon with the drop,
    from the hidden-apps window as a card that unfolds under the app's row;
  - a menu taller than its room scrolls.
- **Non-goals:**
  - the Island page (the clock page and the other pages of the Island) keeps
    its own motion;
  - the notifications of `CEL-D-011` keep theirs;
  - no change to which pieces open which menus, or to the menus' content.
- **Anti-goals:**
  - a gap in the bar while a menu is open;
  - two windows visible at once on one capsule;
  - a closing window that lingers as a grey veil over the bar;
  - a tray menu that appears beside the hidden-apps window or away from its
    app.

## 3. Today (1.35.0), from the code and the author's recording

- **Open.** A capsule menu window grows out of its piece's pill with OutBack
  1.15 over 350 ms (`sheet/motion.rs`, `SheetMotion::open`). It is drawn on
  the panel's layer surface (`celestina-panel`, 560 px tall), right-aligned to
  the capsule's outer edge, 8 px under the bar (y 56), at most 496 px tall.
- **The piece.** Since `CEL-D-008` the piece leaves the bar: its slot
  collapses and its icon and text become the window's subtitle; since
  `CEL-D-010` the piece fades out blurring and sinking 10 px over 160 ms, and
  fades back in over 200 ms on the close.
- **Close.** 260 ms: the window blurs to 12 px, fades (glass `1 - t^1.6`,
  content `1 - (t/0.8)^1.4`) and shrinks 70 % of the way to the pill. Its
  glass is a separate shape from the capsule's, and the compositor's blur
  leaves it below 35 % glass opacity, so the end of the close reads as a grey
  box over the pill for about 150 ms (measured in the recording).
- **Switch.** On the same capsule the new window grows from its pill while the
  old one closes over 260 ms: two boxes, and the new content is readable about
  165 ms after the click. A menu on another capsule waits for the close to end
  (`Sheets::queue`).
- **Tray menus.** A dbusmenu card is an `xdg_popup` per menu level
  (`tray/menu.rs`), 300 px wide, anchored under the row that opened it, with
  no motion. From the hidden-apps window (`CEL-I-012`, 240 px wide, centred
  under the chevron) the card hangs under the row and niri flips or slides it,
  so it lands beside the window over other applications. A parked bug lives
  on this path: niri sometimes dismisses the card at once (log "tray menu:
  dismissed by the compositor", from `xdg_popup.done`).
- **Unused.** The progressive blur of `CEL-D-002` (E1: 1.5, 3 and 6 px, down
  to 40 % opacity) is still in `celestina-paint` (`Painter::cut_row`) with no
  caller.

## 4. Approaches considered

- **The drop.** G1, a neck (chosen): a drop swells from the capsule under the
  piece, stretches a neck that breaks, then opens into the window. G2, a
  falling drop: an 18 px round drop falls into the window's place and splashes
  into it; more playful, slower (520 ms). G3, a meniscus: the window grows
  attached to the capsule by a wide bridge that breaks at the end; the most
  sober. The author chose G1.
- **The switch.** The window travels to its new shape and only the content
  changes (chosen), or a hard cut: the old window vanishes and the new one
  opens with the full drop. The cut shows the drop more often but reads as a
  jump.
- **Menus from the hidden-apps window.** The window turning into the app's
  menu (the chosen row rising to a header), a second drop under the window,
  and a card unfolding under the app's row inside the window (chosen). The
  author's correction made the third the design: the progressive blur belongs
  to the other apps in the list, only while a menu is open.
- **Contrast between the list, the menu and the open app.** A grey card
  (chosen): the menu on ControlFill, the open app's row on ControlHover in
  semibold, the same language as the open piece in the bar. An accent tint:
  the card and the row on AccentSoft with its border.

## 5. The design

### 5.1 One glass shape (CEL-D-013)

- While a window is joined to its capsule, the capsule's glass, the neck and
  the window are one shape, filled once with the Haze material (tint, grain,
  the compositor's blur behind). No part is drawn twice, so there is no darker
  seam where they meet.
- The shape is a path: the capsule's rounded rect, the window's rounded rect
  (or the drop's), the neck between them, and the small tips left when the
  neck breaks. All parts are wound the same way, so the fill is their union.
- **The neck.** Between the capsule's bottom edge (y 48) and the drop's top
  edge. Top half-width `a`, bottom half-width `b`, waist half-width `m`,
  fillet `f` = 7 px into both edges. Each side is two cubic curves through
  the waist at mid height, tangent to the edges, so the joins are concave and
  smooth. The neck's top is centred on the piece; its bottom is centred on the
  piece but kept inside the window's straight top edge (for a round drop, up
  to 45 % into its corner radius).
- **Merging.** When a round drop overlaps the capsule's edge, two concave
  fillets of 6 px smooth the crease.
- **Blur region and input region.** The compositor's blur region and the
  input region follow the whole shape, neck included, by the same 2 px
  horizontal bands the panel uses today. The window's glass no longer fades:
  it leaves by changing shape, so the rule that dropped a window below 35 %
  glass from the blur region goes.

### 5.2 Drop G1, open: 460 ms

`pw` is the piece's width; `dw = max(pw, 28) + 6` and `dh = 26` are the
drop's size; `t` runs 0 to 1.

- **0 to 28 %, the drop swells** (ONE_UI): width `0.6·pw → dw`, height
  6 → 26, gap under the capsule 0 → 3 px; the neck is as wide as the drop and
  narrows to 0.8 of it.
- **28 to 52 %, it falls** (One UI sine in-out 80): gap 3 → 16 px; the waist
  narrows with the gap and the neck breaks at a gap of 13 px. The capsule keeps
  a 6 px tip that retracts over the next 10 % of the flight.
- **52 to 100 %, it opens:** the rect goes from the drop (at a gap of 16) to
  the window with OutBack 1.15, the radius from 13 to 20; the window settles at
  y 56.
- **Content:** opacity `(t - 0.6) / 0.34`, blur `12 · (1 - ONE_UI(u))` px with
  `u` the open phase's own fraction.

### 5.3 Drop G1, close: 400 ms

- **0 to 42 %, it gathers** (ONE_UI): the rect goes from the window to the
  drop at a gap of 14 px under the piece, the radius from 20 to 13. Content
  opacity `1 - t / 0.28`, blur `12 · min(1, t / 0.3)^0.7` px.
- **42 to 100 %, it rises** (ease-in, Bezier 0.4, 0, 0.9, 0.6): the gap goes
  from 14 to `-(dh + 2)`, the width from `dw` to `0.6·pw`. Below a gap of
  12 px the neck forms again, its waist growing as the gap closes; once the
  drop overlaps the capsule the merge fillets of §5.1 apply, and the capsule
  absorbs it.
- The close targets the piece's live slot, wherever the piece is that frame.
- The open pill and the piece's opacity return at 78 % of the close.

### 5.4 The piece in its slot (CEL-D-014)

- The piece no longer leaves the bar: no slot collapse, no crossfade, no
  flight.
- **The open pill.** The open piece shows the grey pill (ControlHover, radius
  12, the piece's rect) behind its content. A switch on the same capsule
  slides the pill to the new piece on the bar's spring (350 ms, OutBack); a
  close fades it out over 150 ms with the exit curve.
- **Opacity.** The open piece's content goes to `opacity.mutedContent` (0.8)
  over 200 ms (ONE_UI) and back on the close.
- **Hover.** The open piece shows no hover or press fill of its own: the pill
  is its fill, and a second fill would stack on it.
- **Header.** The window keeps its first row (icon, title, the More button where it
  has one) as its title.
- **Gestures.** They belong to the piece again, as before `CEL-D-008`: a
  primary press on the open piece closes its menu; wheel, secondary and middle
  act on the piece. The window's header stops taking the piece's gestures.

### 5.5 The switch (CEL-D-015)

- **Same capsule.** The window stays. Its rect morphs from the shown rect to
  the new menu's rect over 300 ms (ONE_UI); no drop, no neck. The old content
  fades `1 - t / 0.3` and blurs to `8 · t / 0.3` px; the new content fades in
  `(t - 0.2) / 0.5` and its blur goes from 8 px to 0 by `t = 0.8`. The pill
  slides (§5.4).
- **Another capsule.** The new menu opens at once with its drop while the old
  one closes with its own: two capsules, two drops, no wait. The queue stays
  only between a capsule menu and the Island page.
- A switch during a drop starts from the shape shown at that frame.
- The new menu takes input from the moment of the switch; the old content
  takes none.

### 5.6 App menus (CEL-D-016)

- **One kind of window.** Tray app menus become capsule menu windows on the
  panel's surface; the `xdg_popup` path for tray menus goes. The content is
  today's dbusmenu card: rows of 40 px, padding 8, separators of 17 px, a
  16 px check column, a chevron for a submenu, disabled entries in TextFaint,
  the shell's own entry (eye or eye-off, `copy::TRAY_SHOW` or
  `copy::TRAY_HIDE`), labels cut at 36 graphemes. Live updates (AboutToShow, layout and
  property changes) refresh the body in place.
- **From a tray icon in the bar.** A window anchored to the icon's piece like
  any other: 300 px wide, drop G1, the piece in its slot with the pill.
- **Submenus.** A submenu opens inside the same window: the content slides
  left (200 ms, ONE_UI), and a first row «‹ label of the parent» goes back;
  Left and Escape go back one level.
- **Gestures stay as today.** The button that opens an app's menu today
  (`pieces/tray.rs`, `row_action`) opens it; primary still activates the app
  and middle still sends the secondary activation. With an app's menu open, a
  primary press on its bar icon only closes the menu.
- **Waiting for the menu.** The drop starts at the press. The menu's height is
  known only when dbusmenu answers (up to 500 ms today, `MENU_LATE_LAYOUT`):
  the drop runs its first two phases and holds as a drop under the icon until
  the layout arrives, then opens to the menu's size. In the hidden-apps window
  the row shows its press fill and the card unfolds when the layout arrives.
  If nothing arrives in 500 ms, the menu holds only the shell's entry, as
  today.
- **No dbusmenu.** An app without one, or whose layout fails, keeps today's
  fallback: the shell asks the app to draw its own menu (`ContextMenu`).
- **From the hidden-apps window.** The window is 280 px wide (it was 240).
  Opening an app's menu unfolds it as a card under the row, inside the
  window:
  - the card's height grows from 0 with OutBack 1.15 over 350 ms; its content
    arrives blurred (12 px to 0) and fades in `(t - 0.12) / 0.5`;
  - the window's glass follows the body's height every frame;
  - opening it again from the row folds the card (ONE_UI, 350 ms, content blur
    `12·t`, opacity `1 - t / 0.6`); a press on another app's row folds the
    first card at once and unfolds the new one;
  - while a card is open, the other rows blur by their distance from the open
    row: 1.5 px for the next row, 3 px for the one after, 6 px beyond, with
    opacity `1 - 0.6 · level / 6` (down to 40 %), over 250 ms (ONE_UI); they
    are sharp again when the card folds;
  - **contrast:** the card is ControlFill, radius 12, 2 px above and 4 px
    below; the open app's row is ControlHover with its name in semibold.
- **Scrolling.** A menu whose window would pass 496 px scrolls: the bar's menu
  window, or the card inside the hidden-apps window. The wheel scrolls; the
  thin bar of the launcher (`launcher/scrolling.rs`: the launcher's thumb
  width, at least 24 px long, lingers 800 ms) shows while it moves. The keyboard keeps the selected
  row in view (`reveal`).
- **Choosing an entry** sends the event and closes the whole window. Escape
  folds an open card first, then closes the window.
- **The Tray page.** The Island's Tray page opens an app's menu the same way
  as the hidden-apps window: a card unfolding under the app's row, the other
  rows blurred by distance, scrolling inside the page when it does not fit.
- **The parked bug.** The dismissal by the compositor came from the
  `xdg_popup` grab; with no popup it has no path left. The live check confirms
  it.

### 5.7 Reduced motion

No drop and no travel: a window appears and disappears with a 150 ms fade at
its place, the card unfolds and folds at once, rows blur at once, and the
pill and the piece's opacity change at once.

## 6. Calls made after the preview

- The header stays in the window (§5.4); the author approved the preview with
  it and did not choose otherwise.
- Gestures go back to the piece (§5.4): the subtitle took them only because
  the piece had left the bar.
- The queue between a capsule menu and the Island page stays (§5.5).
- Submenus drill in (§5.6); the preview described it without showing it.
- Escape folds the card before closing the window (§5.6).
- The Tray page of the Island uses the card too (§5.6): it opened its menus as
  `xdg_popup` cards, and that path goes.
- The new menu takes input at once during a switch (§5.5).
- App menus keep today's buttons; the preview opened them with a left click
  only to keep the demo short (§5.6).
- The drop of an app menu starts at the press and holds until dbusmenu
  answers; the `ContextMenu` fallback stays (§5.6).
- If the author turns down the nested screenshots of the neck (§7), the
  geometry is written so the meniscus (G3) uses the same neck with other
  numbers, and `CEL-D-013` stops at its first task until the author chooses.
- The open piece has no hover fill over its pill (§5.4).
- The fidelity check of §7 is the first step of `CEL-D-013`, before the rest
  of the unit.

## 7. Constraints

- **The compositor's blur.** HTML cannot show how niri blurs a thin neck. The
  first task of `CEL-D-013` paints the drop in a nested session and shows the
  author nested screenshots of the neck, the break and the merge before the
  other tasks start.
- **One surface.** The panel's surface is 560 px tall and never resizes; a
  window is at most 496 px tall.
- **The painter** draws with femtovg. A path fill with the Haze material is
  new; rounded rects alone cannot make the neck.
- **Live-session safety.** No unit runs `celestina` or `niri msg` against the
  author's session; nested runs follow the repository's nest rules.
- **Language.** Spanish strings live only in `celestina/src/copy.rs`.

## 8. Left to the builder

- Where the path fill lives in `celestina-paint` and how the band slicing
  reads a path.
- How the window's header drops the piece's gestures without breaking the
  pages that use the subtitle's text.
- How a dbusmenu body fits the sheet body interface (input, keyboard, live
  refresh) and how the card nests inside the hidden-apps body.
- The exact easing of the submenu slide, within ONE_UI.

## 9. Units and order

| Entry | What | Size | Plan |
| --- | --- | --- | --- |
| `CEL-D-014` | The piece in its slot (§5.4) | small | `docs/superpowers/plans/2026-11-27-celestina-cel-d-014-plan.md` |
| `CEL-D-015` | The travelling switch (§5.5) | small | `docs/superpowers/plans/2026-11-27-celestina-cel-d-015-plan.md` |
| `CEL-D-013` | The drop and the one glass shape (§5.1 to §5.3, §5.7) | large | `docs/superpowers/plans/2026-11-27-celestina-cel-d-013-plan.md` |
| `CEL-D-016` | App menus as capsule menu windows (§5.6) | large | `docs/superpowers/plans/2026-11-27-celestina-cel-d-016-plan.md` |

Order, by the tracks' rule (small and safe first): `CEL-D-014`, then
`CEL-D-015` stacked on it, then `CEL-D-013`, then `CEL-D-016`, which opens the
bar's tray menus with the drop of `CEL-D-013`. The close overlap (request 3)
is fixed by `CEL-D-013`: the window leaves by changing shape into the
capsule's own glass.
