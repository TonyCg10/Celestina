# Celestina capsule sheets: menus that grow out of the bar

- **Date:** 2026-11-25
- **Status:** approved by the author on 2026-11-25; §4.2 to §4.4 amended
  after the author's live tests (CEL-D-007 and CEL-D-008 on 2026-11-26;
  CEL-D-009, the menu as its own window, and CEL-D-010, its close, on
  2026-11-26)
- **Project:** `celestina` (1.19.x on `main` at `c0052728`)
- **Entries:** `CEL-D-002` to `CEL-D-005` of the standing tracks
- **Supersedes:** §6.2, §6.2.1, §6.3, §6.4 and §6.5 of
  [the standing-tracks design](2026-11-24-celestina-standing-tracks-design.md),
  as the author redirected them on 2026-11-25
- **Approved preview:** the author approved version 7 of the preview
  of the capsule sheets on 2026-11-25. It is an artifact private to
  the author; its screenshots and the throwaway prototype's frames go under
  `celestina/docs/evidence/` with each entry. It is the reference for the
  implementation and the final review. Until those frames are committed, a
  copy of the approved page and its source lives outside the repository in
  the sessions' shared notes (`cel-d/preview-approved/`), with the progressive
  blur spike's diff (`cel-d/spike-progressive-blur.diff`); `CEL-D-002`'s
  evidence commits the preview's frames.

## 1. Intent

The author wants the bar's menus to feel like one connected thing that
moves, not windows that pop up beside an icon. After seeing a first preview
of panels hanging under a piece, the author asked for menus that open like
the iPhone's Dynamic Island: **the bar capsule itself grows into the menu**,
and the other pieces of that capsule stay on top so the author can jump from
one menu to another without closing it.

The same batch merges the clock, the date and the weather into the centre
Island and turns the workspaces piece into a small list of workspaces with
their windows' icons.

## 2. Goals, non-goals, anti-goals

- **Goals:** every piece that opens something opens it as the sheet of its
  own capsule; switching between pieces of an open capsule is one click and
  one motion; it is clear which piece's menu is open; the centre reads the
  time, the date and the weather at a glance.
- **Non-goals:** new compositor features; changing the hover and press
  language (it stays as today: instant grey fill, 0.96 recoil); theming
  beyond the existing tokens; the tray's application menus (they stay
  `xdg_popup`, they are the applications' own).
- **Anti-goals:** a menu that feels like a separate window; two things half
  open at once on one output; any motion that delays input; a bar that is
  slower or busier at rest than today; regressing a CEL-2 live row.

## 3. Approaches considered

1. **One open sheet per output (chosen by the author).** On one output only
   one thing is open at a time: the Island's centred page or the sheet of one
   bar capsule. One owner decides what is open and derives from it the input
   region, the blur region, the keyboard and "a press outside closes".
   Opening one closes the other.
2. **Independent state machines for the Island and each capsule**, which
   close each other. Duplicates the regions, the input and the keyboard per
   capsule and lets two animations overlap; rejected.

The earlier design's change of the hover and press language (a hover tween
shell-wide, a press that lifts toward the panel, no recoil elsewhere) is
dropped: the author keeps today's hover and press, and may tint them with the
wallpaper's colours in a later entry.

Earlier rejected designs, for the record: panels hanging under the piece with
a neck, flush or floating (preview versions 1 and 2, approved then replaced
by the author's redirect); keeping `xdg_popup` menus (they feel detached and
cannot animate).

## 4. The design

### 4.1 One owner of what is open

- Each output's `celestina-panel` surface (unchanged: output width × 560,
  layer Top) has at most one **open sheet**: the Island page (today's
  behaviour) or the sheet of one bar zone capsule (`start`, `centre_left`,
  `centre_right`, `end`).
- Opening a capsule sheet closes the Island page and any other sheet, and the
  other way round. While anything is open the input region is the whole
  surface (a press outside closes, as pages do today), keyboard
  interactivity is OnDemand, and Escape closes.
- **A press while a sheet is open (Claude's call).** On a piece of the open
  sheet's own row: a piece with a menu switches the sheet to it (§4.2); a
  piece without one (tray, a bubble) acts and the sheet closes, as a bar piece
  does today with a page open. On a piece of another capsule, or on the
  Island capsule: the open sheet closes (150 ms) and then the other opens, in
  sequence, so two things are never open at once. Pressing the open piece
  again closes its sheet; a press on the sheet's glass that is not on a piece
  or in the body does nothing (both are the approved preview's behaviour).
  Anywhere else: it closes.
- **Overlap (Claude's call).** An open sheet paints over and takes the input
  of whatever it covers (a neighbouring capsule, the Island capsule), as an
  open Island page does today.
- The level controls (volume, brightness) keep today's global rule: one open
  level menu across all outputs; opening one on another output closes the
  first (Claude's call). The OSD is quieted for its kinds while a level sheet
  is open, and released on close exactly as `Suppression::Open/Closed` does
  for the popup today.

### 4.2 The menu window (`CEL-D-009`)

- **Shape.** Each menu is its own window: one rounded rectangle, radius 20,
  with the capsules' Haze glass, apart from the bar. The bar keeps its
  capsules as they are, and the open piece still leaves its slot (§4.3).
  The window is painted on the same `celestina-panel` surface as the bar, as
  a glass shape of its own with its own blur and input regions (Claude's
  call: a separate Wayland surface per menu would look the same and would
  add a second keyboard, catcher and region path).
- **Size.** The window is as large as its content and no larger: its width
  is the body's width (§ Bodies) and its height the subtitle plus the
  content as laid out, with no minimum height of the Island page. The
  surface's room caps it (560 − 56 − 8 = 496); a taller body scrolls inside
  the window with the wheel, and a wheel over a slider still adjusts the
  slider (CEL-I-008). The open spring may overshoot for a moment; the
  painted window and its regions are clipped to the surface.
- **Position.** The window's top is at y 56, 8 px under the capsules (y 8,
  h 40). It lines up with the outer edge of its capsule: in the end zone its
  right edge is the capsule's right edge, in the start zone its left edge is
  the capsule's left edge (the author's call on 2026-11-26), and under a
  centre capsule it is centred on that capsule (Claude's call). It never
  comes closer than 16 px to the surface's sides; a window wider than its
  capsule grows towards the inside of the screen. The capsule's outer edge
  does not move while its open piece's slot closes, so the window stays put.
- **Motion (variant B).** Open: a pill the size of the open piece (its rect
  in the bar, radius 14) grows on the open spring (350 ms, `motion.slow`,
  OutBack 1.15, `motion.overshoot`) into the window, radius 20; the body
  fades in over the last 65 %. Close (`CEL-D-010`, 260 ms): the window
  blurs out, fades and shrinks towards its piece, all at once. Its blur
  (glass and content together) reaches 12 px within the first 150 ms
  (`12 · min(1, t/0.6)^0.7`); its content fades as `1 − (t/0.8)^1.4` and its
  glass as `1 − t^1.6`; its rect moves 70 % of the way to the pill at the
  piece's live slot (`0.7 · (1 − (1 − t)²)`), corner radius 20 → 14. It never
  folds whole into the pill, so nothing waits for it. Switching to another
  piece of the same capsule: the window being left closes this way while the
  new window grows out of the new piece. The approved preview of this close
  is version 3 of the menu-close preview (2026-11-26).
  Reduced motion: geometry jumps, opacities still fade.
- **Bodies (widths).** Volume 320 and brightness 320 (their popup contents
  today, without their own card), session menu 240, workspaces 240 (§4.6).
  A page-backed window takes its content's natural width, from 240 to 420
  (the Island page's width is the most), instead of always 420.
- **Page-backed sheets.** A piece that opens an Island page today
  (network, Bluetooth, notifications, clipboard, energy, system, media,
  phone, clock) opens a sheet whose body is that page's `PageView` body, under
  the subtitle of §4.3, with the page's overflow button (`copy::MORE`) at its
  right when the page has a menu (that menu stays an `xdg_popup`). No clock, no
  close button and no tab row in the sheet. These sheets add no tabs and no
  `island open` words; the Island keeps its pages for `celestina island open`,
  the tab row and notifications.
- **Tray.** Unchanged: primary activates, middle is the secondary activate,
  right click opens the application's menu (`xdg_popup`). It opens no sheet.
- **History.** From CEL-D-002 to CEL-D-008 the sheet was one shape with its
  capsule: the capsule morphed into the menu, at least as wide as the
  capsule, and a page-backed sheet took the Island page's width and height.
  In 1.25.0 the author found empty space (the energy menu under its three rows, the
  brightness menu's left half, the capsule left empty while the system load
  flew down) and approved on 2026-11-26 version 2 of the menu-window preview:
  each menu its own window, variant B (the pill that grows out of the
  piece), lined up with its capsule's outer edge. `CEL-D-009` delivers it.

### 4.3 The open piece becomes the menu's subtitle (`CEL-D-008`)

- **Subtitle.** Every sheet body starts with a subtitle row, height 40,
  padding x 4, gap 10: the open piece's icon at 22 px, then its text at
  rowTitle 15 semibold `TextHi`, growing. A piece without text shows the
  menu's name instead, with the page's icon when the piece has none
  (`copy::PIECE_SESSION`, the page's title; Claude's call for the workspaces list, whose
  piece is a row of dots: Lucide `layout-grid` and `copy::WORKSPACES_FALLBACK`).
  A page-backed sheet keeps its overflow button at the
  subtitle's right. The subtitle replaces the level menus' heading row and
  the page sheets' 32 px header, and shows the same live value as the piece
  (the volume's percent, the system load). While its sheet is open the
  subtitle takes the piece's gestures: the wheel and the secondary and
  middle presses do what they do on the piece, and a primary press closes
  the sheet (the author's choice on 2026-11-26, after the final review).
- **Slot.** When a sheet opens on a piece, that piece leaves its capsule: its
  slot closes with the open spring (350 ms, OutBack 1.15), the pieces beside
  it slide together, and the capsule takes its new width in the same motion
  (since `CEL-D-009` the window is apart from the capsule, §4.2). No gap
  stays behind. When the sheet switches to
  another piece or closes, the piece that comes back reopens its slot with
  the same spring.
- **Crossfade (`CEL-D-010`).** The piece no longer flies. On open, the
  piece in the bar fades out, blurs to 4 px and moves 10 px down towards
  the bar's bottom edge (160 ms, ONE_UI) while its slot closes, and the
  subtitle appears with the body. On close, the piece fades in at its slot
  from 10 px below, from a 4 px blur to sharp (200 ms, ONE_UI), from the first
  frame of the close, and the subtitle leaves with the window. Reduced
  motion: no blur and no displacement, the opacities still fade. The author
  asked for it on 2026-11-26 because the window used to close before the
  subtitle came back, two steps out of order.
- **Flight (history, CEL-D-008 to CEL-D-009).** The piece's icon and text fly between its slot and the
  subtitle: down with the open spring when it opens, up with the exit curve
  when it leaves (150 ms on a switch; on close the slot reopens first and the
  flight starts 60 ms later and takes 200 ms). The icon grows from its bar
  size to 22 px and the text from body 13 to rowTitle 15; the text's weight
  goes from regular to semibold by cross-fading the two weights (femtovg
  cannot interpolate a font's weight axis; the HTML preview did). The
  subtitle's own icon and text, and the piece in the bar, are not painted
  while they fly. Reduced motion: no flight, the slot jumps.
- No indicator pill: the open piece is no longer in the row, so the
  subtitle is what says which menu is open. The other pieces keep their
  normal opacity and their hover and press (O1 of CEL-D-007).
- **History.** The first version of this design let a narrower menu shrink
  to a window over the row and blurred the pieces its edges cut (the
  progressive blur E1, shipped by CEL-D-002). Living with it, the author found
  that the narrowed capsule hid the other buttons, and approved on
  2026-11-26 the capsule that never narrows, with the piece flying down as
  the subtitle and the bar closing its slot without a gap (preview
  of the whole bar, version 4). CEL-D-008 delivers it; the sheet no
  longer cuts the row, and the painter keeps the progressive blur for
  later use (the author's call on 2026-11-26).

### 4.4 Which menu is open

- The subtitle names the open menu (§4.3). History: this section first
  described a white spotlight under the open piece (a misreading, shipped by
  CEL-D-002 and replaced by CEL-D-003 Task B5 with the other pieces dimmed to
  40 %); after trying 1.23.0 the author asked for full opacity and a single
  travelling pill (CEL-D-007, variants O1 and P1 approved on 2026-11-26).
  CEL-D-008 removes that pill with the piece itself.

### 4.5 One centre capsule (`CEL-D-004`)

- The Island capsule shows, gap 12: the time `HH:MM` (rowTitle 15, semibold,
  tabular, TextHi), the short date, weekday and day (body 13, TextLo) and the weather
  (its icon 16 and `12°`, body 13, TextLo, gap 4). Without weather data the
  weather is left out. The workspace dots stay where they are today (only
  when no zone holds `workspaces`).
- Clicking the capsule opens the Island at the calendar (clock) page.
- The default layout's centre is already empty on `main`; `clock` and
  `weather` remain pieces a person may still place. The
  author's `shell.json` sets `centre` explicitly, so the evidence gives the
  exact line to change: `"centre": []`.

### 4.6 Workspaces list (`CEL-D-005`)

- A primary click anywhere on the workspaces piece opens the `start`
  capsule's sheet with the workspaces of that output, in niri's order. The
  dots are no longer separate buttons; the piece is one hit target with one
  pill (today's hover).
- The body (width 240, padding 8, gap 2): one row per workspace, height 40,
  padding x 10, gap 10, radius 10: the workspace's name or number (18 wide,
  semibold, TextLo; Accent on the current one), then the icons (20 px, gap 6)
  of its windows in column order, at most 8 and then `+N`, or the empty label `copy::WORKSPACE_EMPTY`
  (TextFaint) when it has none. The current workspace's row has the
  `accentSoft` fill with a 1 px `accentSoftBorder`. Hover and press as today.
- A click on a row focuses that workspace and closes the sheet. The wheel
  over the piece steps to the previous or next workspace, open or not
  (while the list is open, the wheel over its subtitle, §4.3).
- Copy: the overview's existing empty-workspace word, renamed
  `copy::WORKSPACE_EMPTY` and shared by both.

## 5. Delivery

Four entries, one landing each, in order; each needs its predecessor. The
amendments after the live tests follow them: `CEL-D-008` stacked on
`CEL-D-007`, then `CEL-D-009` and `CEL-D-010` from `main`.

| Entry | Delivers |
|---|---|
| `CEL-D-002` | The open-sheet owner, the capsule sheet (shape, rules, motion, switching), the progressive blur, the open-piece spotlight, scrolling, the regions and keyboard; the volume menu moves into it and its `xdg_popup` path is removed |
| `CEL-D-003` | Brightness, the session menu and every page-backed piece move into sheets; the level and session `xdg_popup` paths are removed |
| `CEL-D-004` | The centre capsule with time, date and weather; the default layout's centre empties |
| `CEL-D-005` | The workspaces list sheet and the wheel on the workspaces piece |
| `CEL-D-007` | Full opacity around an open sheet and one travelling pill (amendment of 2026-11-26) |
| `CEL-D-008` | The capsule that never narrows, the slot that closes, the piece that flies down as the subtitle; the sheet stops cutting the row, and the painter keeps its progressive blur for later (amendment of 2026-11-26; [plan](../plans/2026-11-26-celestina-cel-d-008-plan.md)) |
| `CEL-D-009` | Each menu its own window, as large as its content, grown out of its piece and lined up with its capsule's outer edge (amendment of 2026-11-26; [plan](../plans/2026-11-26-celestina-cel-d-009-plan.md)) |
| `CEL-D-010` | The window's close blurs, fades and shrinks towards its piece; the piece crossfades in the bar instead of flying (amendment of 2026-11-26; [plan](../plans/2026-11-26-celestina-cel-d-010-plan.md)) |

Nothing else is in flight in `osd/popup.rs` or `session_menu` (the
coordinating session dropped `CEL-I-010` and `CEL-I-011`), so `CEL-D-002`
starts from `main`. Moving the level controls keeps, with their tests in
`osd/popup.rs` and `osd/monitor_rows.rs`: the hold of an asked level
(`CEL-I-007`, `CEL-I-009`: `MonitorMirror::asked`, `adopt`, `wait`,
`ASK_HOLD`, `brightness_asked`, `hold_brightness`) and the slider wheel
(`CEL-I-008`: `wheel`, `ask_level`, `shown_level`, `WHEEL_BURST`). `CEL-D-002` carries its own painter
work (offscreen image, Gaussian blur, compositing) on the pattern of the
spike's diff.

## 6. Constraints

- Never run the shell against the author's live session: nested niri with
  `--own-bus`, `env -i`, `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`, fake
  `wpctl`/`pactl`, `timeout`.
- CI green on the runner's stable Rust (1.100) and the pinned toolchain.
- No `unsafe` outside its two allowed files; the spike's `glFinish` helper is
  measurement only and is not carried over.
- At rest the bar's frame count does not grow (the smoke's rest checks).
- English in the repository; product copy in `src/copy.rs`.
- The painter is borrowed during `Island::input`: content that measures text
  takes the frame's measure, never a stored closure that borrows the painter.

## 7. Left to the builder

- The module layout of the sheet owner and how `LevelCore` and
  `SessionCore` lose their own card (a variant without `.haze`/size, or a
  wrapper).
- The scroll container's API in `celestina-ui`.
- How the open piece leaves its capsule (a node out of the view or one
  that keeps a zero-width slot), as long as no gap stays and the sheet's
  anchor keeps the slot's centre.
- Where a piece declares what its subtitle shows (its icon and text).
- Whether the Island page and a page-backed sheet share one container.
- How a page-backed window measures its content's natural width and height
  (a layout pass of the page's view, or a size each page declares), as long
  as no row is cut and no empty area stays under the last row.
