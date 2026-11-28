# Celestina: menus as a drop, the piece in its slot, a travelling switch and app menus born from their icon

- **Date:** 2026-11-27
- **Entries:** `CEL-D-013` to `CEL-D-017`, on the shell's standing plan `CEL-TRACKS`
- **Approved preview:** "Celestina · la gota", version 3, approved by the author on 2026-11-27 (drop G1, the grey card, the open piece at 80 %, the travelling switch); published as a private artifact, its source kept in the sessions' notes as `cel-d/preview-approved/src16.html`. Amended twice on 2026-11-27: the author turned down drop G1 in the nested check of §7 and chose drop D1 from version 4 of the same preview (version 4.2, source `cel-d/preview-approved/src17.html`); then he chose rule B of the menu placement preview (version 1, source `cel-d/preview-pending/d013/src18.html`) for where a menu opens (§5.8)
- **Status:** approved; implementation by the five plans named in §9

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
  sober. The author chose G1. In the nested check (§7) he turned it down
  ("it looks like a ball with odd bumps") and chose D1, a hanging drop, over
  D2, a heavy drop that stretches into a tear (version 4 of the preview).
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
- **Where a menu opens.** Today (`CEL-D-009`) a window lines up with its
  capsule's outer edge, so the menu of a piece far from that edge (CPU · RAM,
  360 px) opens more than 200 px away from it. Rule B, from the icon (chosen):
  the window starts 16 px before its piece and grows towards the outer edge,
  moving inward only as much as it needs to stay within that edge. Rule C,
  centred under the icon within the capsule: symmetric under the drop, but
  wide menus of end pieces still sit at the edge and middle ones cover more of
  the bar on both sides.

## 5. The design

### 5.1 One glass shape (CEL-D-013)

- While a window is joined to its capsule, the capsule's glass, the drop and
  the window are one shape, filled once with the Haze material (tint, grain,
  the compositor's blur behind). No part is drawn twice, so there is no darker
  seam where they meet.
- **The liquid contour.** Under the capsule's bottom edge (y 48) the drop is
  one closed outline: a half-width profile `w(y)` mirrored around the piece's
  centre. The profile is the smooth maximum of three terms, each continuous
  in `y`: a fillet into the edge (7 px on the open, 6 px on the close), a
  thread (half-width `a` at the edge, waist `m` at height `yw`, widening into
  the bulb) and the bulb, an ellipse of radii `rx`, `ry` centred at `cy`. A
  negative waist splits the thread into two cones that retract: the pinch-off.
  The outline has no corners and no steps; `celestina/src/sheet/liquid.rs`
  holds it and `celestina/src/sheet/drop.rs` its frames.
- The shape is a path: the capsule's rounded rect, the window's rounded rect
  once the drop has opened, and the liquid contour. All parts are wound the
  same way, so the fill is their union.
- **Blur region and input region.** The compositor's blur region follows the
  whole shape, drop included, by the same 2 px horizontal bands the panel uses
  today. The window's glass no longer fades: it leaves by changing shape. The
  35 % rule stays only for a path that fades under reduced motion.

### 5.2 Drop D1, open: 460 ms

The bulb's radius is `R` = 12 px and it falls `fall` = 16 px; `t` runs 0 to
1.

- **0 to 28 %, the drop swells** (ONE_UI): a round bulb grows from 3 px to
  `R` while it moves down out of the edge; the thread is as wide as the bulb
  (`a` 3 → 8, waist 3 → 6.5).
- **28 to 52 %, it falls** (One UI sine in-out 80): the bulb's centre goes to
  `48 + fall + R`; the bulb stretches up to 10 % into a tear and rounds off;
  the waist thins to 1.4 px by 55 % of the fall, to 0 at 78 %, then goes
  negative: the thread pinches near the bulb and its upper cone retracts into
  the capsule.
- **52 to 100 %, it opens:** the rect goes from the round bulb to the window
  with OutBack 1.15, the radius from 12 to 20; the window settles at y 56. The
  last of the thread's root (`a` 1.5 → 0) goes in the first 20 % of the phase.
- **Content:** opacity `(t - 0.6) / 0.34`, blur `12 · (1 - ONE_UI(u))` px with
  `u` the open phase's own fraction.

### 5.3 Drop D1, close: 400 ms

- **0 to 42 %, it gathers** (ONE_UI): the rect goes from the window to the
  round bulb 16 px under the edge, the radius from 20 to 12. Content opacity
  `1 - t / 0.28`, blur `12 · min(1, t / 0.3)^0.7` px.
- **42 to 100 %, it rises** (ease-in, Bezier 0.4, 0, 0.9, 0.6): the bulb
  rises until its centre is `0.7·R` above the edge, shrinking to half its
  radius as it enters. When its gap to the edge is under 12 px the capsule
  reaches down with a cone; under 2 px the two meet, the waist grows and the
  bulb merges; the capsule absorbs it and the root fades in the last 8 %.
- The close targets the piece's live slot, wherever the piece is that frame.
- The open pill and the piece's opacity return at 80 % of the close
  (`drop::MERGE_AT`).

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

### 5.8 Where a menu opens (CEL-D-017)

- **End capsule.** The window's left edge is 16 px before its piece's left
  edge, then moved inward (left) only as much as it needs to keep its right
  edge within the capsule's right edge; a window wider than its capsule grows
  inward from that edge, as today.
- **Start capsule.** The mirror: the window's right edge is 16 px after its
  piece's right edge, then moved inward (right) only as much as it needs to
  keep its left edge within the capsule's left edge.
- **Centre capsule.** Unchanged (`Edge::Centre`).
- The screen's 16 px side margins hold as today. The drop of §5.2 opens from
  the piece into this rect, and the travel of §5.5 goes between two such
  rects.
- App menus (§5.6) from a tray icon in the bar follow the same rule.

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
  He did turn G1 down, and chose D1 (§4); `CEL-D-013` landed with D1.
- Rule B mirrors on the start capsule and leaves the centre capsule's windows
  (the Island's capsule and the hidden-apps window under the chevron) as they
  are (§5.8).
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
| `CEL-D-017` | Where a menu opens (§5.8) | small | `docs/superpowers/plans/2026-11-27-celestina-cel-d-017-plan.md` |
| `CEL-D-016` | App menus as capsule menu windows (§5.6) | large | `docs/superpowers/plans/2026-11-27-celestina-cel-d-016-plan.md` |

Order, by the tracks' rule (small and safe first): `CEL-D-014`, then
`CEL-D-015` stacked on it, then `CEL-D-013`, then `CEL-D-017`, then
`CEL-D-016`, which opens the bar's tray menus with the drop of `CEL-D-013`
where `CEL-D-017` places them. The close overlap (request 3)
is fixed by `CEL-D-013`: the window leaves by changing shape into the
capsule's own glass.
