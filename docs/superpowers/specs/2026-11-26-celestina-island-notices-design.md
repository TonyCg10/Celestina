# Celestina: notifications and the level display come out of the Island

- **Date:** 2026-11-26
- **Entry:** `CEL-D-011`, on the shell's standing plan `CEL-TRACKS`
- **Approved preview:** variant A of the Island notices preview (version 1), chosen by the author on 2026-11-26; published as a private artifact, its source kept in the sessions' notes as `cel-d/preview-approved/src15.html`
- **Status:** approved; implementation by `docs/superpowers/plans/2026-11-26-celestina-cel-d-011-plan.md`

## 1. Intent

After trying celestina 1.29.0 the author asked for better transient surfaces: a
notification appears with no motion at all, and it should come out inside the
Dynamic Island and close with a right click. The level display (volume and
brightness) is part of the same request.

Success: a notification and a level change both read as the Island
changing shape and going back to the clock, with no surface popping in or out;
a right click puts a notification away.

## 2. Goals, non-goals, anti-goals

- **Goals:**
  - the Island capsule grows into the newest notification and shrinks back to
    the clock;
  - older notifications of a burst wait under it and fan out on hover;
  - the level display widens the Island in place;
  - new gestures: right click and wheel up dismiss one notification, middle
    click dismisses all of them.
- **Non-goals:**
  - grouping notifications by app;
  - inline replies or any text field in a notification;
  - a new look for the Notifications page or the bell.
- **Anti-goals:**
  - a notification that pops in or out in one frame;
  - a notification that hides a window under it longer than today (the
    timeouts stay as they are);
  - a level display that moves the clock to another place on screen;
  - a dismissed notification that leaves no trace: right click and wheel act
    like the close button, so the notification stays in the page's history.

## 3. Today (1.29.0), from the code and a nested capture

- **Notifications.** `src/notifications/banners.rs` draws up to three Haze
  cards, 420 px wide, 80 px tall (112 with buttons), 8 px apart, newest first,
  8 px under the bar's zone. They live on their own `celestina-toasts` layer
  surface (Overlay layer, anchored to the top edge) on the focused output
  (`banner_output` in `src/app.rs`). `sync_banners` creates that surface with
  the first toast and destroys it with the last, so nothing can animate: in a
  nested capture of 2026-11-26 nothing was on screen 120 ms after the
  notification was posted and the whole card was there at 200 ms; it leaves the
  same way.
- **Gestures.** Left click on the body runs the default action, or opens the
  Notifications page and closes the notification; a button runs its action;
  the close button closes it; hovering pauses the server's clock
  (`NotificationsCommand::Hover`). Right click, middle click and the wheel do
  nothing.
- **Do not disturb.** The server withholds every toast but critical ones
  (`Notifications::toasts`); history records everything.
- **Level display.** `src/osd/` shows a 332 × 56 Haze card on its own
  `celestina-osd` surface (Overlay layer), at the bottom centre or the
  bottom-right corner (`osd.position`, `OsdPosition` in
  `celestina-rs/crates/celestina-shell/src/settings.rs`), for `osd.timeout_ms`
  (1500 ms in the author's settings), and leaves on the 150 ms exit fade. The
  audio and brightness workers' readings trigger it, so keys, verbs and the
  bar's wheel all show it. A DDC brightness change shows on the focused output.
  An open slider menu suppresses it (`Osds::set_suppressed`).
- **The Island.** The centre capsule is the Island's own tree
  (`capsule_with` in `src/island/view.rs`, key `capsule`) on the
  `celestina-panel` surface, which is on the Top layer. Its glass is the
  Island's rect (`IslandState::rect`), centred on the output at y 8, h 40,
  radius 20. Only the growth into the Island page animates
  (`IslandState::progress`, ONE_UI). The capsule takes only the left button.
- **Fullscreen.** Nothing in the shell knows about fullscreen windows. The
  Top layer is under them and the Overlay layer over them, so today's
  notifications and level display show over a fullscreen window and the bar
  does not.

## 4. Approaches considered

The preview showed three, each beside today's screen:

- **A. The Island becomes the notification (chosen).** The centre capsule
  grows from its place into the whole notification and back.
  Trade-off: the Island now changes shape on two surfaces, the bar's and the
  notices' own, which hand it over at the start and at the end (§5.1).
- **B. Windows born from the Island.** Each notification is a window like a
  menu window (`CEL-D-009`), grown out of the Island's pill and closed like a
  menu (`CEL-D-010`); the Island does not change. Closest to the menus, but
  the notification comes out of the Island instead of being it.
- **C. A compact Island.** The notification fits in the bar's row as one line
  (icon, title, start of the text) and grows into the whole notification only
  under the pointer. Hides nothing, but the text and the buttons are only seen
  on hover.

## 5. The design (variant A)

### 5.1 Where it is drawn

- The notices keep their own surface, `celestina-toasts`, on the Overlay
  layer and on the output that gets notifications today. It now spans the
  output's width (anchored to the top edge, both sides), 440 px tall, with no
  keyboard. Its exclusive zone is -1, so the compositor places it over the
  bar's zone, from the output's top edge, and the Island it paints sits
  exactly over the panel's; a zone of 0 puts it under the bar's 48 px, as
  the nested captures of `CEL-D-011` showed. Its blur and input regions are
  the shapes it paints, so everything else on screen keeps its input.
- While that surface shows anything, it paints the Island: the
  capsule's glass, the clock, the date and the weather while the Island is at
  rest, and the shapes of §5.2 to §5.4. The panel on that output stops
  painting its Island capsule and drops it from its own regions for that time
  ("the capsule is lent"). The bar's neighbours keep the rest width, so
  nothing in the bar moves.
- Hand-over: the notices surface draws its first frame at the Island's rest
  rect before the panel stops painting its capsule, and when everything has
  gone back to rest the panel paints its capsule again before the notices
  surface is destroyed. So no frame shows the bar without an Island; at most
  one frame shows both capsules on top of each other (Claude's call: the
  surfaces are not synchronised, and a doubled capsule for one frame is less
  visible than a missing one).
- Claude's call, approved with the preview: drawing on the panel would hide
  the notices under a fullscreen window, which they are not today.

### 5.2 A notification

- **Rest.** The Island as today: the time `HH:MM`, the short date and the
  weather, centred, y 8, h 40, radius 20.
- **Arrival.** The Island's rect grows from its rest rect into the notice:
  420 px wide, centred on the output, top at y 8, 88 px tall (120 with
  buttons), radius 20, on the open spring (350 ms, `motion.slow`, OutBack
  1.15, `motion.overshoot`). The clock's content leaves over 160 ms (ONE_UI):
  opacity 1 → 0, blur 0 → 4 px, offset 0 → +10 px (towards the capsule's
  bottom edge). The notice's content comes in 120 ms after the start, over
  200 ms (ONE_UI): opacity 0 → 1, blur 4 → 0 px, offset −10 → 0 px. These
  are the crossfade values of `CEL-D-010`.
- **Content.** Today's banner, with 4 px more room for the radius of 20:
  padding 16 px across and 12 px down; the icon (32), the summary (rowTitle
  15, semibold, one line) beside the close button (24), the body (body 13,
  two lines), and the buttons (28 high) when the notification has any.
- **Leaving.** The notice's content leaves as the clock did; the rect shrinks
  back to the rest rect over 300 ms (ONE_UI, no overshoot), and the clock
  comes back as the notice came in, 140 ms after the start.
- **Next.** If another notification is waiting when one leaves, the Island
  does not go back to rest: its rect moves on the spring to the next notice's
  height and the contents cross-fade the same way.
- **Timeouts and quiet.** Unchanged: low 5 s, normal 10 s, critical never;
  under do-not-disturb only critical ones come out, and the bell in the bar
  marks the rest as unread.

### 5.3 A burst

- The Island shows the newest notification. Up to two older ones (three in
  all, as today) wait under it as sheets: the first 12 px narrower on each
  side and showing 7 px under the Island, the second 24 px narrower and 14 px
  under it; radius 12 (`radius.md`), the capsule's glass at 92 % opacity, no
  content. A new sheet comes in from 10 px higher, fading in over 200 ms.
- **Fan.** While the pointer is over the Island or a sheet, the sheets move on
  the spring to full notices under the Island, 420 px wide, 8 px apart,
  radius 20, each with its content faded in. Leaving starts a 250 ms grace,
  so crossing the 8 px gaps does not fold them (Claude's call). The server's
  clock is paused while the pointer is over any of them, as today.
- When the newest leaves, the next one becomes the Island's notice (§5.2,
  Next) and its sheet goes away (fade 150 ms).

### 5.4 The level display

- A new value of `osd.position`, `"island"`. With it, a volume, microphone or
  brightness change widens the Island in place: 274 × 40, centred, radius 20,
  on the open spring, with the kind's icon (18), the level's track
  (160 × 4, `linearTrackHeight`) and the level as `NN %` (40 px wide, right
  aligned), padding 16, gap 12. The clock and any notice cross-fade out as in
  §5.2. It stays for `osd.timeout_ms` and goes back the same way.
- A notification that is out when the level display comes waits: its timeout
  is held (the server's clock is paused as for hover) and it comes back when
  the level display leaves.
- The level display takes no input.
- `"bottom"` stays the default, and `"bottom-right"` stays too. The author's
  `shell.json` gets `"osd": { "position": "island", … }` by hand; the
  evidence gives the exact line (Claude's call: the smoke checks the bottom
  display and is not changed by this entry).
- While the Island page is open on that output, the level display shows at
  the position's fallback, the bottom centre, as today (Claude's call: the
  Island is then a page and has no capsule to widen).
- An open slider menu still suppresses it, as today.

### 5.5 With the Island page open

- When the Island page is open on the output that gets the notification, the
  notice does not take the capsule: it enters at the top of the page, inside
  its window, as a card (the capsule's `controlFill`, radius 12, 336 px wide,
  padding 12 × 8, 80 px tall or 112 with buttons), 8 px from the page's top
  and 12 px from its sides. The page grows by the card's height and 8 px on its resize
  motion, and the page's content moves down by the same amount. The card
  leaves the same way and the page shrinks back.
- This is drawn on the panel, like the page; the notices surface is not
  created for it.
- On the Notifications page the card is not shown: the page lists it already
  (Claude's call).

### 5.6 Gestures

On a notice (the Island's, a fanned card, or the card inside the page):

| Gesture | Today | Now |
|---|---|---|
| Left click on the body | Default action, or opens the Notifications page and closes it | Unchanged |
| A button / the close button | Runs its action / closes it | Unchanged |
| Right click | Nothing | Closes it (`NotificationsCommand::Close`) |
| Wheel up (or two fingers up) | Nothing | Closes it |
| Middle click | Nothing | Closes every live one (`NotificationsCommand::DismissAll`) |
| Hover | Pauses the server's clock | Unchanged; also fans the burst out |

Every close keeps the notification in the page's history, as the close button
does today. A collapsed sheet takes the same gestures as the notice it
belongs to.

### 5.7 Outputs

- Notices go to the output that gets them today (`banner_output`: the focused
  one); the other outputs' Islands keep the clock.
- The level display shows on the focused output, as today, including a DDC
  brightness change of another monitor.

### 5.8 Reduced motion

Geometry jumps to its target; contents fade over 150 ms with no blur and no
offset; the sheets appear and leave with the same fade.

## 6. Calls made after the preview

The author chose variant A on 2026-11-26. The preview proposed the gestures of
§5.6 for every variant and the `island` value of `osd.position`; the
following are Claude's calls, made after the choice:

- the notices stay on the Overlay layer and borrow the Island (§5.1);
- `osd.position` keeps `bottom` as its default and the author sets `island`
  by hand (§5.4);
- the bottom fallback for a level change while the Island page is open
  (§5.4);
- no notice card on the Notifications page (§5.5);
- the 250 ms grace before a fanned burst folds (§5.3);
- one frame of two capsules at most during the hand-over (§5.1).

## 7. Constraints

- Motion only from `celestina-style` tokens and the values above; the
  ceiling is 500 ms. At rest the notices surface does not exist and the panel
  asks for no frames.
- No `unwrap`/`expect` in production code, no `unsafe` outside `egl.rs` and
  `pam.rs`, product copy only in `src/copy.rs`.
- Never touched: `src/lock`, `src/polkit`, `celestina/scripts/`, `.githooks`,
  any `AGENTS.md`, `docs/contracts`, `docs/governance`, `docs/standards`.
- The smoke stays as it is: the `notify` phase reads the log lines
  `banners surface requested … created=true` and `banners surface released`
  and at most two frames at rest; the `osd` phase checks the bottom display
  on `celestina-osd`. Both keep passing unchanged.
- Nested runs only through the guarded kit.

## 8. Left to the builder

- How the notices surface gets the Island's rest rect, its clock text and its
  glance (date and weather) from the panel's Island on that output, as long
  as it paints them as the panel does.
- The name and shape of the pure model behind §5.2 to §5.6, as long as it can
  be tested without a surface.
- Whether the card inside the page (§5.5) reuses the notice view with another
  padding or has its own view.
