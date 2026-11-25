# Celestina standing tracks: design and improvements

- **Date:** 2026-11-24
- **Status:** approved by the author on 2026-11-24; revised on 2026-11-25 (§2.2)
- **Project:** `celestina` (the Rust shell, 1.14.0 at `44b8cc2a` when designed, 1.15.4 on 2026-11-25; CEL-2 complete,
  the author's session runs Celestina since the cutover of 2026-11-24)

## 1. Intent

CEL-2 gave the shell every function the author needs; its look and feel are a
base, not the finished thing. The author wants two lines of work that never
close:

- **Design (`CEL-D`):** polish how the shell looks, moves and feels. This is
  the "group 2" the CEL-2 design set aside (§8 of
  `2026-10-16-celestina-parity-program-design.md`).
- **Improvements (`CEL-I`):** small new functions, each about the size of a
  plugin, and bugs to fix.

## 2. How the two tracks run

These rules are the author's, as stated on 2026-11-24.

- **The author proposes; Claude orders.** The author states an idea, a bug or
  a dislike in a sentence. Claude files it as an entry in the right track
  (design if it is how the shell looks or feels, improvements if it is
  something it does or fails to do) and decides the order. The rule:
  1. what breaks or blocks the author's live session;
  2. small and safe;
  3. large.
  Claude states the order in one line whenever it changes; the author may
  correct it.
- **Each entry is one unit** under the repository's rules: one session
  worktree, one landing, a ledger row closed `done` with its inventory. An
  entry that qualifies as a light unit (`docs/standards/verification.md`)
  runs as one. A track is not a unit, because a unit must close.
- **Done** means: landed in `main`, and, when the change is visible or
  touchable, tried live by the author. A failed live check opens a corrective
  entry; nothing is reopened.
- **Landings are batched:** Claude asks the author to authorise the landings
  of the entries that are ready, as one group, in the coordinating chat.
  Each landing still needs that direct authorisation.

### 2.1 Repository mechanics (Claude's call; the plan layout is superseded by §2.2)

- Two standing plans under `celestina/docs/plans/active/`:
  `cel-d-design.md` and `cel-i-improvements.md`. Each has the usual ledger
  table and a **backlog** section (entries not yet started, in order). They
  are never archived; `celestina/ROADMAP.md` names both as standing
  checkpoints, and the root roadmap gets one line pointing at them.
- Unit ids: `CEL-D-001`, `CEL-D-002`, … and `CEL-I-001`, … Commit prefix stays
  `celestina:`; kinds are `milestone` for new behaviour, `maintenance` for
  polish without behaviour change, `bug` for fixes.
- A large design entry gets its own short design note under
  `docs/superpowers/specs/` before it starts; a small one is described in its
  ledger row.
- The two sessions the CEL-2-M coordinator started (default flip for `lock`
  and `polkit`; smoke and CI hardening) are recorded as `CEL-I-001` and
  `CEL-I-002`.

### 2.2 Revision of 2026-11-25 (Claude's calls)

- **One plan, not two.** The documentation guard allows one active plan per
  roadmap, so both tracks share
  `celestina/docs/plans/active/2026-11-25-cel-tracks.md` (checkpoint
  `CEL-TRACKS`): one ledger and one backlog per track. A landing changes one
  active plan, so the shell's first entry, `CEL-I-004`, opens that plan as its
  own open row.
- **Suite documents.** Design notes and task plans of the entries live under
  `docs/superpowers/`, outside the shell's commit roots. A standing suite plan,
  `docs/plans/active/2026-11-25-cel-tracks-docs.md` (checkpoint `TRK-1`),
  hosts the documents-only units that commit them, starting with
  `CEL-TRACKS-OPEN`.
- **Light lane.** [ADR 0013](../../decisions/0013-development-speed.md)
  amends §2: an entry that qualifies as a light unit
  (`docs/standards/verification.md`) uses the light lane.
- **Order.** On 2026-11-25 the author made audio urgent. The order is:
  1. `CEL-I-004`, the per-application volume and the output and input
     device selector, in today's level popup; it absorbs `CEL-I-005`
     ([its design](2026-11-25-celestina-cel-i-004-audio-design.md));
  2. then `CEL-D-001` to `CEL-D-005`;
  3. then `CEL-I-003`.

## 3. Goals, non-goals, anti-goals

- **Goals:** the shell feels connected and alive: menus grow out of the thing
  you clicked, everything moves with one motion language, icons say what text
  says today. Each step is small enough to try live the same day.
- **Non-goals:** new compositor features; theming beyond the existing token
  schemes in this first batch; Customize UI (later entry).
- **Anti-goals:** menus that feel like separate windows; animations that delay
  input; any change that makes the bar or Island slower to react than today;
  a polish that regresses a CEL-2 live row.

## 4. The first batch (the author's list of 2026-11-24)

| # | Track | Entry | Kind |
|---|---|---|---|
| 1 | D | Icons instead of text in the Island's tab row | maintenance |
| 2 | D | Anchored panels inside the bar surface, and the new hover/press language | milestone |
| 3 | D | Move every piece's menu (network, Bluetooth, volume, brightness and the rest) into anchored panels | milestone |
| 4 | D | One centre capsule: clock and date merged, and that capsule is the Island | maintenance |
| 5 | D | Workspaces piece opens a small overview panel with each workspace's app icons | milestone |
| 6 | I | Brightness panel shows every monitor | milestone |
| 7 | I | Per-application volume | milestone |
| 8 | I | Output and input device selector | milestone |

Order is as listed, except as §2.2 revises it: 6 waits for 3 because it
lives inside the anchored panels.

## 5. Approaches considered for opening menus

1. **Panels inside the bar surface (chosen by the author).** The panel grows
   down from the piece's capsule, attached to the bar, on the surface that
   already exists. One motion and one glass for every panel; no separate
   window, no compositor round trip.
2. **Separate popups beside each icon** (today's volume/brightness mechanism,
   `xdg_popup`). Cheapest, but it is exactly what feels detached and
   unanimated now.
3. **The Island travels to the icon and morphs.** Most striking, but moving
   and resizing the Island per opening is the riskiest path and the author
   wants the Island kept for other uses.

## 6. Design of each entry

The facts below come from the code at `44b8cc2a` (paths under `celestina/`).

### 6.1 Tab icons (CEL-D-001)

- Today `island/pages/tabs.rs` draws text pills from each page's `title()`.
- Each `PageView` gains an icon (a semantic name from `celestina-style/icons`).
  The tab shows the icon only; the title stays as the accessible name and as a
  tooltip-free hover label is **not** added (anti-goal: no new floating text).
- Missing icons in the set: a clock/calendar and a plain battery. The unit adds
  them as Lucide SVGs to `celestina-style/icons` (same licence as the rest).
- Mapping (builder may refine): activity `gauge`, notifications `bell`, tray
  `system-tray`, system `cpu`, clock `calendar`, clipboard `clipboard-paste`,
  network `wifi`, Bluetooth `bluetooth`, energy `power`.

### 6.2 Anchored panel and motion language (CEL-D-002)

- **One surface, unchanged:** panels draw on the existing `celestina-panel`
  surface (fixed 560 px tall, regions follow content). No new surface.
- **A panel** is a rounded glass rect anchored under a piece's node rect
  (`node_rect(key)`), horizontally centred on it and clamped to the surface
  edges, top attached to the bar capsule it came from. Max height is the
  surface's room (560 − margins); content scrolls past that.
- **Motion:** opening grows the panel from the capsule's width and height to
  its size, with the capsule's shape morphing into the panel's top edge (the
  "connected to the bar" feel); closing reverses it. Uses the existing
  `Animated`/`Motion` tokens; the builder picks the curve and duration within
  the existing token set.
- **State:** a panel state beside `IslandState` (one open panel per output at
  a time; opening a panel closes the Island page and vice versa).
- **Input and regions:** while a panel is open the input region is the whole
  surface (so a click outside closes it, as pages do), the blur region
  includes the panel's silhouette, and keyboard interactivity is OnDemand.
- **Hover/press language:** replaces the instant fill change and the 0.96
  recoil (`celestina-ui` `highlight.rs`, `input.rs`). New behaviour: hover
  fades the fill in over a short tween; press lifts toward the panel it will
  open (a subtle grow of the capsule, not a shrink). Token names stay
  (`control_hover`, `control_pressed`); `press_scale` is retired or reused.
  The builder proposes the exact curves; the author judges them live.
- **Layout room:** the bar tree is laid out in a 48 px room; a panel gets its
  own tree laid out in the panel rect.

### 6.2.1 Calls settled after the builder check (Claude's calls)

- **What grows:** a zone is one capsule holding several pieces, so the panel
  grows out of **the piece's own spot** inside its zone capsule, not out of
  the whole zone. Hover and press draw a pill behind the piece (a new
  per-piece highlight shape); the zone capsule never changes size, so its
  neighbours never move. The panel's top edge joins the bar under that pill.
- **Scope of the new hover/press language:** the hover tween (fill fading in
  and out instead of switching instantly) applies **shell-wide** in
  `celestina-ui`; the press "lift toward the panel" applies **only to bar
  pieces** that open a panel. Other surfaces (page rows, launcher, overview,
  polkit card, banners) lose the 0.96 recoil in favour of a short pressed
  tint, and their CEL-2 live rows are re-checked by the author afterwards.
- **Panel content contract:** a panel body is a new small contract beside
  `PageView` (`PanelView`), so volume, brightness and the workspaces panel do
  **not** become Island pages, add tabs or add `island open` words. Pieces
  that already have a page reuse their `PageView` body through a thin adapter.

### 6.3 Pieces open panels (CEL-D-003)

- Every piece whose click opens a page or a level popup opens an anchored
  panel instead: network, Bluetooth, volume, brightness, battery/energy,
  clock, notifications, clipboard, tray, system, media, phone.
- The panel shows the same content as that piece's Island page (the
  `PageView` builds the body; the panel is a second container for it). Level
  controls (volume, brightness) move from `osd/popup.rs` into their panels;
  the `xdg_popup` level path is removed in the same unit.
- **The Island keeps its pages** for `celestina island open <word>`, the
  bottom tab row, and anything opened for another reason (a notification,
  a verb).
- Tray item menus (DBusMenu) stay `xdg_popup`: they are the application's own
  menus. The Tray piece keeps its CEL-2 clicks (primary activates, middle is
  the secondary activate, right opens the app's menu) and opens no panel; its
  Island page stays reachable from the tab row (Claude's call).

### 6.4 One centre capsule (CEL-D-004)

- Today the Island capsule shows HH:MM (+ workspace dots) and the author's
  layout puts `weather` and `clock` (the date) in a separate capsule beside it.
- The Island capsule shows time and date together; the `clock` piece no longer
  draws a separate date capsule. Clicking the capsule opens the Island (the
  calendar page first). `weather` stays a piece; the author's `shell.json`
  layout is updated to drop `clock` from `centre` (a default-layout change
  applies to everyone; the user file is the author's to edit, with the exact
  line given).

### 6.5 Workspaces panel (CEL-D-005)

- Today a click on a workspace dot focuses that workspace.
- As the author asked, a primary click anywhere on the Workspaces piece opens
  the panel instead of focusing a dot. The mouse wheel over the piece still
  steps to the previous or next workspace, so quick switching survives
  (Claude's call).
- The panel: one tile per
  workspace of that output, in order, each showing the icons of its open
  windows (`NiriModel.windows[].icon`, already resolved by `AppIcons::fill`),
  active one highlighted; a click on a tile focuses it and closes the panel.
- No screenshots (unlike the full overview): it is a light menu.
- The full overview (`celestina overview`) stays as it is.

### 6.6 Brightness of every monitor (CEL-I-003)

- The brightness panel lists every monitor (DDC and backlight) with its own
  slider, named by output, the one of the bar's output first.
- Writes keep the existing one-`ddcutil`-at-a-time rule and throttle.

### 6.7 Per-application volume (CEL-I-004) and device selector (CEL-I-005)

- The volume panel shows: output level and mute, input level and mute, a
  list of applications playing audio with their own slider and mute, and
  output/input device pickers.
- **One audio model (Claude's call).** Celestina's own provider
  (`src/providers/audio.rs`) stays the single source for the volume piece,
  the wheel, the OSD, the level verbs and the new panel, and keeps
  `pactl subscribe` as its trigger. It grows to list streams and devices;
  where `cuprita-core`'s pure parsing and model types (`AudioSnapshot`,
  `wpctl status` parsing) can be reused without its `pw-mon` watcher or
  per-card `pw-cli` calls, they are reused; otherwise the provider parses
  `wpctl status` itself. No second audio model runs beside it (CEL-2-F
  declined cuprita's `wpctl` feature on purpose; that boundary holds).

## 7. Constraints

- Never run the shell or the lock against the author's live session in
  development: nested niri with `--own-bus`, clean environment, `timeout`;
  `CELESTINA_DDC=0` in nests.
- Every unit keeps the CI green on the runner's stable Rust (currently 1.100),
  not only the pinned toolchain.
- English in the repository; the author's conversation is Spanish.

## 8. Left to the builder

- Exact curves, durations and easing of the panel and hover/press motion,
  within the existing `Motion` tokens.
- Panel widths per piece.
- Icon choices where the mapping in §6.1 is a suggestion.
- Whether the panel container and the Island page share one component or two
  thin wrappers around the same `PageView`.
