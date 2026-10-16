# Author validation — Celestina

This queue contains no implementation work and never blocks `ROADMAP.md`.

Statuses: `pending`, `passed`, `failed`, `deferred`, `obsolete`. The rows below
are the live checks of the design's §8 for the CEL-1 release; each is run on
the binary the landing of its related unit deploys.

## VAL-CEL-GLASS — Haze and blur on the Island, a banner and a popup

- **Status:** pending
- **Related implementation:** CEL-1-B, CEL-1-F
- **Requires:** the deployed shell started by hand in the author's niri
  session with blur armed
- **Procedure:** look at the Island capsule, an open page, a banner and an
  overflow popup over a busy window and over the wallpaper
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
  its submenu, then press Escape, click outside and choose an item
- **Pass condition:** the overflow menu hangs directly under the More
  button, takes the grab, closes on Escape (the page stays open), on a click
  outside (the page closes with it when the click lands outside the whole
  Island) and on a choice. The reopen row makes the page shrink to the
  capsule and grow again. The do-not-disturb row flips its switch and the
  setting survives a restart of the shell. The quit row stops the shell. While
  another program holds the notifications name the do-not-disturb row is
  dimmed, says that another program manages it, and cannot be chosen by pointer
  or keyboard. Tray menus (CEL-1-G): nested submenus close with their parent
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
  Notifications and Tray pages; stop Noctalia, restart the shell, send a
  notification and open the tray
- **Pass condition:** beside Noctalia the pages say who owns each name and
  nothing is claimed; without it the shell serves notifications and the tray
- **Result:** not run
- **Evidence:** none
