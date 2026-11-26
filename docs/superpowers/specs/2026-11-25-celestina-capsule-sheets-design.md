# Celestina capsule sheets: menus that grow out of the bar

- **Date:** 2026-11-25
- **Status:** approved by the author on 2026-11-25
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

### 4.2 The capsule sheet (Dynamic Island)

- **Shape.** The zone capsule (y 8, h 40, radius 20) morphs into one rounded
  rectangle, radius 20: its top 40 px keep the capsule's row of pieces, the
  rest is the menu body. Glass is the Haze of the capsules (one silhouette;
  the blur region follows it in 2 px bands as today).
- **Width and position.** Let `W` be the menu's width, `[L, R]` the
  capsule's extent and `c` the centre of the clicked piece.
  - `W ≥ R − L` (the menu is at least as wide as the capsule): the sheet
    grows to `W` and must contain the whole capsule:
    `x = clamp(c − W/2, R − W, L)`. The row stays exactly where it is.
  - `W < R − L` (narrower): the sheet shrinks to `W` inside the capsule:
    `x = clamp(c − W/2, L, R − W)`. The row does **not** move: the sheet is
    a window over it. Pieces outside the window are not drawn; pieces cut by
    a window edge are drawn with the progressive blur of §4.3 and stay
    clickable while visible.
  - Finally `x` is clamped to 16 px from the surface's sides.
- **Height.** `40 + body`, capped by the surface's room (560 − 8 − 8 = 544).
  The open spring may overshoot that cap for a moment; the painted sheet and
  its regions are clipped to the surface.
  A taller body scrolls inside the sheet with the wheel; a wheel over a
  slider still adjusts the slider (CEL-I-008).
- **Motion.** Open: 350 ms (`motion.slow`) with the emphasized OutBack
  easing, overshoot 1.15 (`motion.overshoot`), from the capsule's rect to the
  target rect; the body fades in over the last 65 %. Close: 150 ms
  (`motion.exit`) with the exit curve back to the capsule; the body fades out
  over the first 60 %. Switching to another piece of the same capsule while
  open morphs from the current rect to the new target with the open curve;
  the old body fades out over the first 30 %, the new one in over the last
  65 %. Reduced motion: geometry jumps, opacities still fade.
- **Bodies (widths).** Volume 320 and brightness 320 (their popup contents
  today, without their own card), session menu 240, workspaces 240 (§4.6),
  and a page-backed sheet 420 (the Island page width).
- **Page-backed sheets.** A piece that opens an Island page today
  (network, Bluetooth, notifications, clipboard, energy, system, media,
  phone, clock) opens a sheet whose body is that page's `PageView` body, under
  a header row of 32: the page title (17, semibold) and its overflow button (`copy::MORE`) when
  the page has a menu (that menu stays an `xdg_popup`). No clock, no
  close button and no tab row in the sheet. These sheets add no tabs and no
  `island open` words; the Island keeps its pages for `celestina island open`,
  the tab row and notifications.
- **Tray.** Unchanged: primary activates, middle is the secondary activate,
  right click opens the application's menu (`xdg_popup`). It opens no sheet.

### 4.3 The cut edge: progressive blur

- On each side where the sheet's window cuts the row (never at a capsule's
  own end), a band of `min(96, W × 2/5)` px blurs the row progressively:
  four levels (σ 0, 1.5, 3 and 6 logical px) cross-faded by distance to the
  edge with triangular weights at t = 0, 1/3, 2/3 and 1 (t = 1 at the edge),
  and the band's alpha falls linearly to 40 % at the edge.
- This is the WWDC25 "variable blur with a soft gradient" of macOS 26's soft
  scroll edge, measured against the alternatives in the preview (macOS 27's
  plain fade, and its uniform blurred veil); the author chose it.
- **Feasibility (spike, 2026-11-25).** In femtovg 0.27: the row is rendered
  into an offscreen image, blurred with `filter_image(GaussianBlur)` at the
  three sigmas, each level masked by its weight gradient (`DestinationIn`),
  the levels summed (`Lighter`) and drawn under the window's rounded clip.
  Cost in the nest: about +0.19 ms of GPU per frame (median) on a moving
  window, no seams, banding or halo. The row image must extend past the
  window so the blur has pixels at a cut (the spike dimmed slightly where
  the window met the image's edge). It only runs while a window cuts the row:
  at rest nothing changes.

### 4.4 Which menu is open

- While a sheet is open, the open piece keeps today's grey pill
  (`controlHover`, radius 12), which slides with the open spring when the
  sheet switches pieces, and every other piece of that capsule's row is
  painted at 40 % opacity: the approved "focus" indicator (I5 of the preview).
  The dim comes in over 200 ms (One UI curve) and goes over 150 ms (exit
  curve) on close; a switch keeps it. Reduced motion: it jumps.
- Correction of 2026-11-25: this section first described a white spotlight
  under the open piece, from a misreading of the author's choice; CEL-D-002
  shipped it and CEL-D-003 (Task B5) replaced it with the focus indicator.

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
  over the piece steps to the previous or next workspace, open or not.
- Copy: the overview's existing empty-workspace word, renamed
  `copy::WORKSPACE_EMPTY` and shared by both.

## 5. Delivery

Four entries, one landing each, in order; each needs its predecessor.

| Entry | Delivers |
|---|---|
| `CEL-D-002` | The open-sheet owner, the capsule sheet (shape, rules, motion, switching), the progressive blur, the open-piece spotlight, scrolling, the regions and keyboard; the volume menu moves into it and its `xdg_popup` path is removed |
| `CEL-D-003` | Brightness, the session menu and every page-backed piece move into sheets; the level and session `xdg_popup` paths are removed |
| `CEL-D-004` | The centre capsule with time, date and weather; the default layout's centre empties |
| `CEL-D-005` | The workspaces list sheet and the wheel on the workspaces piece |

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
- How the spotlight is painted (gradients, box gradient, or one blurred
  image), as long as it matches §4.4.
- Caching of the blurred row while the window moves.
- Whether the Island page and a page-backed sheet share one container.
