# Author validation — Celestina

This queue contains no implementation work and never blocks `ROADMAP.md`.

Statuses: `pending`, `passed`, `failed`, `deferred`, `obsolete`. The
`VAL-CEL-*` rows are the live checks of the design's §8 for the CEL-1 release;
each is run on
the binary the landing of its related unit deploys. CEL-1-H checked that all
nine `VAL-CEL-*` rows are queued and pending. The `VAL-CEL2-*` rows are queued
by the CEL-2 units, each unit adding its own (as the CEL-2 ledger states), and
are run the same way. None has been run and none is closed by an agent.

## VAL-CEL-GLASS — Haze and blur on the Island, a banner and a popup

- **Status:** pending
- **Related implementation:** CEL-1-B, CEL-1-F
- **Requires:** the deployed shell started by hand in the author's niri
  session with blur armed
- **Procedure:** look at the Island capsule, an open page, a banner and an
  overflow popup over a busy window and over the wallpaper. For the banner,
  stop Noctalia, then send one normal notification with
  `notify-send "Celestina" "Glass check"` so the shell claims the name, and
  look at that banner over the same windows (the banner's other urgencies are
  in VAL-CEL-BANNERS)
- **Pass condition:** each surface shows the Haze material over the
  compositor's blur of the real windows below, with no outline, no lit edge
  and no opaque plate
- **Result:** not run
- **Evidence:** none

## VAL-CEL-ISLAND — Capsule to page at 165 Hz without jank

- **Status:** pending
- **Related implementation:** CEL-1-B (the growth into a page), CEL-1-D (the
  pages and their switching)
- **Requires:** the deployed shell on DP-1 at 3840x2160@165 Hz, scale 1.5,
  started by hand with `celestina --metrics` from a terminal
- **Procedure:** from CEL-1-B: click the capsule and close the page with
  Escape, a click outside and `celestina island close`, ten times each, then
  run `celestina island toggle` ten times from a key binding; stop the shell
  with `celestina quit` and read the `DP-1` line it prints (`late` and
  `comp_late` should stay 0). From CEL-1-D: open each page in turn with
  `celestina island open activity`, `celestina island open notifications` and
  `celestina island open tray`; with one open, switch to each other page by
  clicking its tab and by running `celestina island open <page>` again, and
  switch again before a slide has finished; with several windows and more than
  ten workspaces open, read the Activity page and open it on a long list
- **Pass condition:** the growth takes 350 ms on the One UI curve with no
  visible jump, no stutter and no flash of the old content. Escape closes a page
  only after it has been clicked: niri gives an `OnDemand` layer surface the
  keyboard on a click inside it, not when the page opens; a click outside
  closes it regardless. From CEL-1-D: a switch slides the incoming page
  horizontally in the direction of the tab order (Activity, Notifications,
  Tray), the page resizes to the new page's size without a jump, a switch
  during a slide continues from where the content is, and Activity grows with
  its rows up to the Island surface's height (560 logical px, less the top
  gap and the page margin) and never lists more than 24 rows
- **Result:** not run
- **Evidence:** none

## VAL-CEL-REST — Zero frames in rest and memory under budget

- **Status:** pending
- **Related implementation:** CEL-1-B
- **Requires:** the deployed shell with two outputs connected
- **Procedure:** start the shell by hand with `celestina --metrics`, leave the
  session idle for five minutes with no page open, then stop it with
  `celestina quit` and read the report it prints: one line per output
  (`frames=`, `rest_frames=`) and `rss_mib=`
- **Pass condition:** `rest_frames` is at most one per output per minute (the
  clock's), and `rss_mib` stays under 100
- **Result:** not run
- **Evidence:** none

## VAL-CEL-POPUP — Popup grab semantics

- **Status:** pending
- **Related implementation:** CEL-1-D, CEL-1-G
- **Requires:** the deployed shell with a tray item that has a nested DBusMenu
- **Procedure:** from CEL-1-D: open a page and click the overflow (More) button
  in its title row; check where the menu appears; press Escape;
  reopen it and click outside it, on the page and on another window; reopen it
  and choose each row in turn (reopen the page, do-not-disturb, quit); with another program holding the notifications name (Noctalia
  running), open the menu again. From CEL-1-G: open a tray item's menu with
  its submenu (a submenu opens on a click, Enter or Right, not on hover),
  then press Escape, click outside and choose an item; in a submenu press Left
  and Escape; right-click an item, and scroll over an item's icon; on the
  Tray page with the row focused, press Enter (the item activates, as on a
  click) and the Menu key or Shift+F10 (its menu opens); with a submenu open,
  make the item change its menu (toggle a check entry from the application)
  and check that the open levels follow; make it add entries to the open
  level (an application whose menu grows with its state) and check that the
  popup grows with them and the new rows take the pointer and the keyboard;
  open a submenu an application fills
  only on demand (a GTK indicator with a lazy submenu, if one is at hand)
- **Pass condition:** the overflow menu hangs directly under the More
  button, takes the grab, closes on Escape (the page stays open), on a click
  outside (the page closes with it when the click lands outside the whole
  Island) and on a choice. The reopen row makes the page shrink to the
  capsule and grow again. The do-not-disturb row flips its switch and the
  setting survives a restart of the shell. The quit row stops the shell. While
  another program holds the notifications name the do-not-disturb row is
  dimmed, says that another program manages it, and cannot be chosen by pointer
  or keyboard. Tray menus (CEL-1-G): nested submenus close with their parent
  on Escape, on a click outside and on a choice, Left closes only the
  submenu, a grab taken by a submenu opened from a click is accepted by niri,
  a menu taller than the screen is cut (it has no scrolling yet) and a
  checked entry shows its mark; Enter on a row activates and the Menu key
  opens the menu; a republished menu updates the open levels without
  closing them; a lazily filled submenu opens once its entries arrive
- **Result:** not run
- **Evidence:** none

## VAL-CEL-KEYBOARD — Keyboard-only operation

- **Status:** pending
- **Related implementation:** CEL-1-D, CEL-1-E
- **Requires:** the deployed shell and the author's niri key bindings for the
  Shell1 verbs
- **Procedure:** without the pointer, open each Island page and the launcher
  by key binding, move with Tab and the arrows, activate with Enter and Space,
  close with Escape. From CEL-1-D: on an open page, Tab to the page tabs and
  switch pages with Enter or Space, Tab to the More button and open the menu with
  Enter; in the menu press Down and Up (or Tab and Shift+Tab) through the
  rows, choose with Enter and with Space, and close with Escape. The menu
  opened by keyboard starts with its first row focused, so Enter at once
  reopens the page; opened by the pointer it starts with no focused row.
  From CEL-1-E: open the launcher by key binding, type a query, move the
  selection with Down and Up (it wraps), launch with Enter, then reopen it and
  close it with Escape and with the toggle; reopen it and click outside the
  card on another window, on the Island and on an empty part of the output;
  while it is open, try to click a window behind it. Open it, paste with Ctrl+V a clipboard holding
  several lines and one holding more than 64 KiB
- **Pass condition:** every surface is reachable and operable from the
  keyboard with a visible focus ring, and the focus returns to the author's
  window on close. In the launcher the field shows a caret and no ring, the
  selected row shows its ring only after an arrow key, Escape, Enter and the
  toggle close it with the focus back on the previous window, and a click
  anywhere outside the card closes it with the focus back on the previous
  window. While it is open a window behind it cannot be clicked (the click
  only closes the launcher; this is expected), and nothing outside the card
  is dimmed or blurred: the rest of the output looks as it did before.
  A multi-line paste arrives as one line with spaces, an oversized one is cut
  and nothing hangs. In the menu the focus ring is visible on the focused row
  only after keyboard input, never from the pointer alone; the arrows skip the
  dimmed do-not-disturb row while another program holds the name; Escape
  closes the menu and leaves the page open
- **Result:** not run
- **Evidence:** none

## VAL-CEL-IME — IME in the launcher's field

- **Status:** pending
- **Related implementation:** CEL-1-E
- **Requires:** the deployed shell and an input method running in the session
- **Procedure:** type a query with composed characters and a dead key in the
  launcher; compose with a candidate window and cancel it with Escape; move
  the caret with the arrows after committing and check that the candidate
  window follows the caret
- **Pass condition:** the preedit shows in place, the committed text is the
  one typed, and the ranking follows it; cancelling a composition leaves no
  text behind, the candidate window sits at the caret, and no character is
  lost while typing fast
- **Result:** not run
- **Evidence:** none

## VAL-CEL-SCALE — Sharp text at scale 1.5 and 1.0

- **Status:** pending
- **Related implementation:** CEL-1-B
- **Requires:** the deployed shell on DP-1 (scale 1.5) and HDMI-A-1 (scale 1)
- **Procedure:** compare the Island's clock in the capsule and the open
  Activity page's title, buttons and workspace rows on both outputs at close
  range, at the normal text scale and again with `text_scale = "large"` in
  `~/.config/celestina/appearance.toml` (the shell follows the file live)
- **Pass condition:** text is sharp on both outputs, with tabular figures in
  the clock and no blurred or doubled glyphs
- **Result:** not run
- **Evidence:** none

## VAL-CEL-NOCTALIA — Beside Noctalia and without it

- **Status:** pending
- **Related implementation:** CEL-1-F, CEL-1-G
- **Requires:** the deployed shell; Noctalia running, then stopped
- **Procedure:** start the shell while Noctalia runs and open the
  Notifications and Tray pages; beside Noctalia the Notifications page shows
  the owner line with Noctalia's unique name and its process, and its switch
  is disabled. Stop Noctalia, restart the shell, send a notification and open
  the tray. Tray (CEL-1-G): with Noctalia stopped and
  status icon, and one that publishes only a pixmap), check that its icon is
  listed, left-click it (Activate), right-click it (its menu), scroll over it
  on both axes (an item that reacts to a horizontal scroll, if one is at
  hand: the shell sends it Qt's `angleDelta` sign, positive for a tilt to the
  left, which is unverified against a real item), quit the program and see
  the row leave; also start a libappindicator program whose icon name is
  private to it (Steam, Dropbox) and check that its pixmap is drawn; then
  start Noctalia again first and check that the shell claims nothing and the
  Tray page names the owner in Spanish, with its process name
- **Pass condition:** beside Noctalia the pages say who owns each name and
  nothing is claimed; without it the shell serves notifications and the tray;
  tray items appear, activate, open their menus and disappear with their
  program
- **Result:** not run
- **Evidence:** none

## VAL-CEL-BANNERS — Banners, actions, hover and do-not-disturb

- **Status:** pending
- **Related implementation:** CEL-1-F (the banners, the history page and the
  do-not-disturb switch)
- **Requires:** the deployed shell started by hand in the author's niri
  session, Noctalia stopped so the shell owns the notifications name
- **Procedure:** send one low, one normal and one critical notification with
  `notify-send -u low`, `-u normal` and `-u critical`, and check that the low
  banner leaves after 5 s, the normal one after 10 s and the critical one
  stays. Send a notification with an action (for example
  `notify-send --action=open=Open "Celestina" "Actions"`), click its button and
  check that the action runs. Click a banner body without a default action:
  the Notifications page opens on that output. Send four notifications and
  check that three banners show. Point at the banner stack and check that
  the expiry pauses; move the pointer away and check that it resumes. Dismiss
  one banner with its icon button. Turn on do-not-disturb from the overflow
  menu, send a normal notification and check that no banner shows, while the
  Notifications page keeps it in its history
- **Pass condition:** each step does what is described: 5 s, 10 s and a
  staying critical banner; the action button and the body click behave as
  described; four notifications show three banners; hover pauses the expiry
  and leaving resumes it; the dismiss button closes the banner; do-not-disturb
  hides banners and keeps history
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-BAR-LIVE — The bar beside Noctalia

- **Status:** pending
- **Related implementation:** CEL-2-A
- **Requires:** the deployed shell with the `bar` duty switched on in
  `shell.json`; Noctalia running with its bar hidden by `noctalia msg bar-hide`
- **Procedure:** look at the bar on each output; check that the blur regions
  follow every capsule; open the tray icons' popups; click outside an open
  page and check the focus policy with the wider surface; hover the bar's idle
  stretches with `focus-follows-mouse`; repeat on two outputs of different
  scale and check the edge pixels of the input regions. The queued detail is
  completed by CEL-2-A's own unit
- **Pass condition:** every capsule is blurred and clickable, nothing outside
  a capsule steals a click or the focus, and windows do not move when the bar
  is switched on
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-WALLPAPER — The wallpaper duty beside Noctalia's

- **Status:** pending
- **Related implementation:** CEL-2-E (the `wallpaper` duty, off by default,
  one `celestina-wallpaper` surface per output on the Background layer)
- **Requires:** the author's niri session with the deployed shell; Noctalia
  running with its wallpaper first disabled through Noctalia's own verb, then
  enabled again for the last step; a folder with one image per output
- **Procedure:** with Noctalia's wallpaper disabled through its own verb, set
  `duties.wallpaper` to `"on"` and `wallpaper.folder` to the folder in
  `~/.config/celestina/shell.json` (the shell follows the file live). Check
  that each output shows its image. On the 4K output at scale 1.5, check the
  image is sharp, with no resample blur. Run `celestina wallpaper next DP-1`
  and check that DP-1 changes picture and the other output keeps its own.
  Change the mode with `celestina wallpaper mode fit`, then `stretch`, then
  `center`, and check each one on both outputs. Unplug and replug an output
  (hotplug) and check that it gets its image back. Restart the shell and check
  that the saved choices are restored. Then turn Noctalia's wallpaper on again
  while the duty stays on and run `celestina wallpaper retry`, and check that
  the shell refuses the output that Noctalia holds: no `celestina-wallpaper`
  surface on it, and the log names the holder. The shell checks the holder
  only when the duty turns on, when an output appears and on `wallpaper
  retry`, so without the retry a holder that arrived later is not noticed
- **Pass condition:** each output shows its own image at the chosen mode, sharp
  at scale 1.5; `next`, `mode`, hotplug and restart behave as described; after
  `celestina wallpaper retry` with Noctalia's wallpaper still on an output, the
  shell draws nothing on that output and names the holder
- **Result:** not run
- **Evidence:** none
