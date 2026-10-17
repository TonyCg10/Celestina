# Author validation — Celestina

This queue contains no implementation work and never blocks `ROADMAP.md`.

Statuses: `pending`, `passed`, `failed`, `deferred`, `obsolete`. The
`VAL-CEL-*` rows are the live checks of the design's §8 for the CEL-1 release;
each is run on
the binary the landing of its related unit deploys. CEL-1-H checked that all
nine `VAL-CEL-*` rows are queued and pending. The `VAL-CEL2-*` rows are queued
by the CEL-2 units, each unit adding its own (as the CEL-2 ledger states), and
are run the same way; among them CEL-2-E queues VAL-CEL2-WALLPAPER and
CEL-2-B queues VAL-CEL2-IDLE, VAL-CEL2-CAFFEINE, VAL-CEL2-NIGHTLIGHT and
VAL-CEL2-SESSION; CEL-2-K queues VAL-CEL2-SYSMON, VAL-CEL2-WEATHER,
VAL-CEL2-CALENDAR and VAL-CEL2-GLANCE-TABS. None has been run and none is closed by an agent.

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

## VAL-CEL2-BAR-LIVE — The bar beside Noctalia, its blur, its popups, focus and two scales

- **Status:** pending
- **Related implementation:** CEL-2-A (the bar surface, the piece table and
  the bar duty)
- **Requires:** the deployed shell started by hand in the author's niri
  session with `duties.bar` set to `on` in `shell.json`, Noctalia still running
  with its bar hidden by `noctalia msg bar-hide`, and two outputs of different
  scale for check 5
- **Procedure:** (1) with Noctalia's bar hidden, look at the bar beside
  Noctalia's other surfaces: the Island in the centre zone, the Workspaces and
  Notifications pieces at the start, the Tray piece at the end. (2) Check the
  blur over a busy window under each capsule and over the Island while it is
  open. (3) Click each tray icon, right click it and scroll over it, and open
  its DBusMenu popup: it anchors to the icon. Check that nm-applet's and
  Blueman's icons are not on the bar and are listed on the Tray page (the
  default `tray_hidden` assumes the item `Id`s `nm-applet`, `blueman` and
  `blueman-tray`; one that shows on the bar has another `Id`). (4) Open the
  Island page and the overflow popup, click outside them, press keys in the
  launcher and move the pointer across the wide surface: clicks outside must
  close what is open and must reach the windows below, and keyboard focus
  must follow ADR 0007. (5)
  On the two outputs of different scale, check that the bar's text and icons
  are sharp on each and that its zones are placed on each output
- **Pass condition:** (1) the bar shows its three zones on the output with the
  Island centred and nothing of Noctalia's bar visible; (2) the blur region
  follows each capsule and the Island, with no blur outside them; (3) every
  tray popup opens at its icon and closes on click-outside, and nm-applet and
  Blueman are absent from the bar and present on the Tray page; (4) clicks and
  focus behave as ADR 0007 describes on the wider surface, and clicks outside
  reach the windows below; (5) text is sharp on both outputs
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

## VAL-CEL2-IDLE — The idle chain: screen off, and no suspend without a lock

- **Status:** pending
- **Related implementation:** CEL-2-B (the idle worker on `ext-idle-notify`
  and the pure chain)
- **Requires:** the deployed shell started by hand in the author's niri
  session; `duties.idle` set to `"on"` in `~/.config/celestina/shell.json`
  before starting it; Noctalia's idle screen-off switched off first in its
  own idle settings, so the two never act at once; caffeine off
- **Procedure:** start the shell and leave keyboard and pointer untouched. The
  shell's journal line `idle: armed` appears at start. After the screen-off
  delay (20 s) every output must power off; move the mouse and check that the
  outputs wake at once. Leave the session idle past the suspend delay (240 s)
  and check that nothing suspends and the journal says the step was refused
  for want of a locker. Then set `duties.idle` back to `"off"`, wait for the
  shell to follow the file, and check that idling no longer powers the
  outputs off. Switch Noctalia's idle behaviour back on if you want it
- **Pass condition:** the outputs power off after the screen-off delay and
  wake on input; the session never suspends and the refusal is logged once
  per idle period; with the duty off the shell does nothing on idle; Noctalia
  and the shell were never both enabled
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-CAFFEINE — Caffeine as a logind inhibitor

- **Status:** pending
- **Related implementation:** CEL-2-B (the logind worker and the caffeine
  verbs)
- **Requires:** the deployed shell started by hand in the author's niri
  session; `duties.caffeine` set to `"on"` in `~/.config/celestina/shell.json`
  before starting it; Noctalia's caffeine switched off first with
  `noctalia msg caffeine-disable`, so the two never hold it at once (the
  niri startup line `caffeine-enable` is the one this replaces)
- **Procedure:** run `celestina caffeine on` and then
  `systemd-inhibit --list`: a `Celestina` lock for `idle:sleep` in mode
  `block` must be listed. Run `celestina caffeine toggle` and check that it is
  gone. Turn it on again, set `duties.idle` to `"on"` and wait past the
  screen-off delay: the outputs must stay on while caffeine is held. Run
  `celestina caffeine off`, stop the shell with `celestina quit` and check
  that no `Celestina` lock remains. Verbs print nothing; the result is read
  in `systemd-inhibit --list` and in the shell's journal
- **Pass condition:** the lock exists exactly while caffeine is on, blocks the
  idle chain, and is released by the verb, by switching the duty off and by
  the shell stopping; `energy.caffeine` in `shell.json` follows the verb;
  with the duty off the verb does nothing to the system and says so in the
  journal
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-NIGHTLIGHT — Forced night light on the three outputs

- **Status:** pending
- **Related implementation:** CEL-2-B (the gamma worker and the night-light
  verbs)
- **Requires:** the deployed shell started by hand in the author's niri
  session; `duties.nightlight` set to `"on"` and `energy.night_light` to
  `true` in `~/.config/celestina/shell.json` before starting it; Noctalia's
  night light switched off first with `noctalia msg nightlight-disable`
  (and its startup `nightlight-force-toggle` not run), so the two never own
  the gamma at once
- **Procedure:** start the shell and look at all three outputs: each must turn
  warm (2700 K by default) with a short transition, not a jump. Run
  `celestina nightlight toggle` and check that every output returns to
  neutral; toggle again and check the warm return. Unplug or disable one
  output and plug it back, and check that it takes the colour again. Set
  `energy.night_light_kelvin` to a value in 2000..6500 and check that the
  colour follows the file. Stop the shell and check that the outputs return
  to neutral
- **Pass condition:** the warm tint appears and goes on every output, survives
  hotplug, follows the kelvin setting, and is gone when the shell stops; if
  an output refuses the gamma control the others still change and the journal
  says `night light: not applied`
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-SESSION — Session verbs through logind

- **Status:** pending
- **Related implementation:** CEL-2-B (the logind worker and the session
  verbs)
- **Requires:** the deployed shell started by hand in the author's niri
  session; no unsaved work, since two of the verbs below would end the
  session if the lock existed; no duty needs to be on for these verbs
- **Procedure:** run `celestina session suspend` and check that the machine
  does not suspend: until the lock of a later delivery exists the shell
  refuses and the journal says `session Suspend: refused: no locker yet`.
  Do not run `session reboot` or `session power-off` as a check. For a harmless
  look at the same path, run `busctl call org.freedesktop.login1
  /org/freedesktop/login1 org.freedesktop.login1.Manager CanReboot` and
  read the answer (`yes`, `challenge` or `na`) that the shell's probe would
  also see. Run `celestina session log-out` only if you are ready to end the
  session: niri asks its own confirmation before it does
- **Pass condition:** suspend is refused with the stated reason and the
  machine stays awake; the logind probe answers; log out goes to niri's
  confirmation and not to logind; reboot and power off are never exercised
  by this row
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-SYSMON — The system monitor piece and page

- **Status:** pending
- **Related implementation:** CEL-2-K (the `sysmon` piece and the Sistema
  page)
- **Requires:** the author's niri session with the deployed shell; `"sysmon"`
  added to a zone of `bar` in `~/.config/celestina/shell.json`; Hematita or any
  other monitor to compare against; a way to load the CPU (a stress run)
- **Procedure:** place `sysmon`, check that `CPU n% · RAM m%` appears within a
  few seconds and agrees with the monitor to a few points. Run `celestina island
  open sysmon`, and check the CPU and memory rows (used and total), the
  temperature row when the machine has a CPU sensor, the two history graphs, and
  that the row that opens the monitor starts Hematita. Load the CPU above 80 %
  and above 90 % and check the colour changes to the elevated and then the
  critical tone. Remove `sysmon` from the file and check the piece disappears
  and the shell stops sampling (`top -H -p $(pidof celestina)`)
- **Pass condition:** the numbers agree with the monitor; the colour follows the
  thresholds; the page and its monitor row work; with the piece removed there is
  no sysmon thread and no periodic wakeup
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-WEATHER — The weather piece and the network rule

- **Status:** pending
- **Related implementation:** CEL-2-K (the `weather` piece, the Reloj page's
  reading and ADR 0016)
- **Requires:** the author's niri session with the deployed shell; network
  access; `ss` to watch connections
- **Procedure:** start the shell with `weather` placed and no `weather` key, and
  check with `ss -tnp` and a short capture that the shell makes no connection and
  starts no `curl`; the piece draws nothing. Add `"weather": {"latitude": ...,
  "longitude": ..., "label": "..."}` to `shell.json` and check that the
  temperature and the condition appear within a minute and that the Reloj page
  shows the reading, the place and the update time. Compare the temperature with
  another source. Block the network and check that the reading disappears after
  15 minutes rather than showing a stale value. Remove the `weather` key and
  check that the piece disappears at once and no further connection is opened. Try
  an out-of-range latitude and a label over 64 characters and check that no
  request is made
- **Pass condition:** no request without a valid manual location; with one, a
  correct reading; stale readings are absent; removing the key stops requests
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-CALENDAR — The clock piece and the Reloj page

- **Status:** pending
- **Related implementation:** CEL-2-K (the `clock` piece, the Reloj page)
- **Requires:** the author's niri session with the deployed shell; `"clock"`
  placed in a zone of `bar`
- **Procedure:** check that the short date in the bar matches today. Run
  `celestina island open clock` and check the title, the seconds clock, the
  month grid (Monday first, today highlighted, six rows), and Previous, Next and
  Today moving between months. Close the page, open it again and check that it
  shows the current month. Leave the shell running across midnight, or change
  the clock, and check that the date and the highlight move to the new day. Check
  the bar still rests at about one frame a minute with `clock` placed
- **Pass condition:** the date, the grid and the highlight are right; navigation
  works and resets on close; the day change is picked up
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-GLANCE-TABS — Five tabs on the Island page

- **Status:** pending
- **Related implementation:** CEL-2-K (the Sistema and Reloj tabs beside
  Activity, Notifications and Tray)
- **Requires:** the author's niri session with the deployed shell, at scale 1.5
  on the 4K output and at scale 1 on the other
- **Procedure:** open the Island and look at the tab row with the five pages. Each
  tab is about 71 px wide, so check whether "Notificaciones" is cut off or
  overflows its tab, whether the tabs are readable and hit-testable, and whether
  the overflow popup behaves
- **Pass condition:** every tab label is fully visible and every tab is easy to
  hit; otherwise record the failure and open a corrective unit
- **Result:** not run
- **Evidence:** none
