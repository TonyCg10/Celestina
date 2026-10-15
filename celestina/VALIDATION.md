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
- **Related implementation:** CEL-1-D
- **Requires:** the deployed shell on DP-1 at 3840x2160@165 Hz, scale 1.5
- **Procedure:** open and close each page from the capsule and from `celestina
  island open <page>` ten times, and switch pages while one is open
- **Pass condition:** the growth takes 350 ms on the One UI curve with no
  visible jump, no stutter and no flash of the old content
- **Result:** not run
- **Evidence:** none

## VAL-CEL-REST — Zero frames in rest and memory under budget

- **Status:** pending
- **Related implementation:** CEL-1-B
- **Requires:** the deployed shell with two outputs connected
- **Procedure:** leave the session idle for five minutes with no page open,
  then read the shell's frame counter and resident memory
- **Pass condition:** no frame is drawn in rest except the clock's once a
  minute, and resident memory stays under 100 MB
- **Result:** not run
- **Evidence:** none

## VAL-CEL-POPUP — Popup grab semantics

- **Status:** pending
- **Related implementation:** CEL-1-D, CEL-1-G
- **Requires:** the deployed shell with a tray item that has a nested DBusMenu
- **Procedure:** open the overflow menu and a tray item's menu with its
  submenu, then press Escape, click outside and choose an item
- **Pass condition:** each popup takes the grab, closes on Escape, on a click
  outside and on a choice, and nested submenus close with their parent
- **Result:** not run
- **Evidence:** none

## VAL-CEL-KEYBOARD — Keyboard-only operation

- **Status:** pending
- **Related implementation:** CEL-1-D, CEL-1-E
- **Requires:** the deployed shell and the author's niri key bindings for the
  Shell1 verbs
- **Procedure:** without the pointer, open each Island page and the launcher
  by key binding, move with Tab and the arrows, activate with Enter and Space,
  close with Escape
- **Pass condition:** every surface is reachable and operable from the
  keyboard with a visible focus ring, and the focus returns to the author's
  window on close
- **Result:** not run
- **Evidence:** none

## VAL-CEL-IME — IME in the launcher's field

- **Status:** pending
- **Related implementation:** CEL-1-E
- **Requires:** the deployed shell and an input method running in the session
- **Procedure:** type a query with composed characters and a dead key in the
  launcher
- **Pass condition:** the preedit shows in place, the committed text is the
  one typed, and the ranking follows it
- **Result:** not run
- **Evidence:** none

## VAL-CEL-SCALE — Sharp text at scale 1.5 and 1.0

- **Status:** pending
- **Related implementation:** CEL-1-B
- **Requires:** the deployed shell on DP-1 (scale 1.5) and HDMI-A-1 (scale 1)
- **Procedure:** compare the Island's clock and a page's text on both outputs
  at close range
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
