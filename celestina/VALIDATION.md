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
VAL-CEL2-CALENDAR and VAL-CEL2-GLANCE-TABS; CEL-2-G queues VAL-CEL2-CLIPBOARD-LIVE,
VAL-CEL2-CLIPBOARD-SECRET, VAL-CEL2-CLIPBOARD-IMAGE and VAL-CEL2-CLIPBOARD-LOOK;
CEL-2-F queues VAL-CEL2-NETWORK, VAL-CEL2-BLUETOOTH and VAL-CEL2-POWER;
CEL-2-C queues VAL-CEL2-OSD-AUDIO, VAL-CEL2-OSD-BRIGHTNESS and
VAL-CEL2-OSD-PIECES; CEL-2-D queues VAL-CEL2-MEDIA and VAL-CEL2-MEDIA-COVER.
None has been run and none is closed by an agent.

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

## VAL-CEL2-CLIPBOARD-LIVE — The clipboard history with real applications

- **Status:** pending
- **Related implementation:** CEL-2-G (`duties.clipboard`, the bar piece and
  the Island page)
- **Requires:** the deployed shell started by hand in the author's niri
  session; `duties.clipboard` set to `"on"` in `~/.config/celestina/shell.json`
- **Procedure:** copy text in a browser and in a terminal, and check the
  piece and the page `celestina island open clipboard` list them newest first.
  Copy something from a real password manager and check it is not listed. Pick
  an older entry by click and by Enter, paste it in another application, and
  check the page does not gain a duplicate. Remove one entry with Delete, use
  pause and resume, and clear with its confirmation. Look at the piece (and its
  paused mark) and the page at both scales
- **Pass condition:** entries appear and paste back unchanged; the password
  manager's copy never appears; no duplicate after a re-selection; the piece
  and page look like the rest of the shell and work from the keyboard
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-CLIPBOARD-SECRET — The history file and the journal hold no secret

- **Status:** pending
- **Related implementation:** CEL-2-G (persistence and logging rules)
- **Requires:** as above
- **Procedure:** with the duty on and a few texts copied, run
  `stat -c '%a' ~/.local/state/celestina/clipboard.json` and the same on the
  directory. Set `duties.clipboard` to `"off"` and check the file is gone. Run
  `journalctl --user -b | grep -F` for a distinctive string you copied
- **Pass condition:** file mode `600`, directory `700`; the file is absent
  with the duty off; the string appears in no journal line
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-CLIPBOARD-IMAGE — A screenshot copy round trip

- **Status:** pending
- **Related implementation:** CEL-2-G (PNG entries, thumbnails)
- **Requires:** as above
- **Procedure:** copy a screenshot to the clipboard with the screenshot tool,
  check the page shows its thumbnail, copy something else, select the image
  entry again and paste it in an image-aware application
- **Pass condition:** the thumbnail shows; the pasted image is the original;
  no image bytes are written under `~/.local/state/celestina/`
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-CLIPBOARD-LOOK — The page's tabs, rows line and thumbnails

- **Status:** pending
- **Related implementation:** CEL-2-G round B (tab strip, the "N more" line,
  thumbnail alpha)
- **Requires:** as above; try the text scales Compact, Normal, Large and Larger
  and a light and a dark wallpaper behind the Island
- **Procedure:** open the Island and look at the six tabs (about 58 px each,
  at the caption size; the Notifications and Clipboard tabs show their short
  forms "Notific." and "Portap.") at each text scale; fill the history past what the page shows and check the last line says how
  many entries are left out; copy a screenshot with transparent corners and
  look at its thumbnail
- **Pass condition:** every tab label is whole at Compact and Normal and
  clipped, never overlapping its neighbour, at Large and Larger; the "N more"
  line is complete and counts right; a transparent thumbnail is acceptable on
  both (it is composited over black, a known limit)
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-NETWORK — The network piece and its page beside nm-applet

- **Status:** pending
- **Related implementation:** CEL-2-F (the network worker over
  `cuprita-core`'s NetworkManager client, the network piece and the `network`
  page)
- **Requires:** the deployed shell started by hand in the author's niri
  session; `duties.bar` set to `"on"` and `network` placed in `bar.end` of
  `~/.config/celestina/shell.json` (the piece has no duty of its own);
  `nm-applet` running as it does today. Noctalia's network widget may stay on:
  the shell only reads NetworkManager. Have the password of one saved network
  and one protected network that is not saved ready, but type it only into
  `nm-applet`, never into the shell
- **Procedure:** start the shell and compare the piece and the `network` page
  with `nm-applet` and `nmcli device wifi list`: the connected network, its
  signal bars, its saved or unknown state and the Wi-Fi switch must match.
  Join a saved network from the page and check that it becomes connected
  within the page's pending window and that `nm-applet` shows the same
  network. Click an unsaved or protected network and check that nothing
  opens, no password is asked by the shell, and the row says it is configured
  in `nm-applet`. Activate `Editar…` and check that `nm-connection-editor`
  opens and the shell opens nothing else. Switch the Wi-Fi switch off and on
  from the page and check that `nm-applet` follows. Check that the piece and
  the page read correctly at the real text scale and that the icons are
  legible
- **Pass condition:** the state matches `nm-applet` and `nmcli` at all times
  the page is open; a join of a saved network converges to the same state as
  `nm-applet`; an unsaved or protected network never opens a shell dialog and
  is answered with the `nm-applet` sentence; the shell never asks for or shows
  a password; `Editar…` opens `nm-connection-editor` only
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-BLUETOOTH — The Bluetooth piece and its page beside blueman

- **Status:** pending
- **Related implementation:** CEL-2-F (the Bluetooth worker over
  `cuprita-core`'s BlueZ client, the Bluetooth piece and the `bluetooth` page)
- **Requires:** the deployed shell started by hand in the author's niri
  session; `duties.bar` set to `"on"` and `bluetooth` placed in `bar.end` of
  `~/.config/celestina/shell.json`; a paired device within reach that the
  author is willing to connect and disconnect; `blueman` running as it does
  today. Noctalia's Bluetooth widget may stay on, and a connect from either
  side is fine as long as the two converge
- **Procedure:** start the shell and compare the piece and the `bluetooth`
  page with `bluetoothctl show` and `blueman-manager`: the adapter power, the
  paired and connected devices, their aliases and the battery of a device that
  reports one must match. Turn the adapter off and on from the page and check
  that `blueman-manager` follows. Connect the paired device from the page and
  check it becomes connected within the pending window; disconnect it the same
  way. Activate `Editar…` and check that `blueman-manager` opens. Check that
  the shell never starts a scan, never pairs or removes a device, and never
  asks for a PIN
- **Pass condition:** the state matches `blueman-manager` and `bluetoothctl`
  at all times the page is open; each toggle and connect converges to the same
  state in `blueman`; no scan, pairing, trust or removal is ever started by the
  shell; `Editar…` opens `blueman-manager` only; the icons and the device names
  read correctly at the real scale
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-POWER — The battery and power-profile pieces beside UPower and the profile daemon

- **Status:** pending
- **Related implementation:** CEL-2-F (the power worker over UPower and
  power-profiles-daemon, the battery and power-profile pieces, the `power`
  page, and the six-tab strip of the Island)
- **Requires:** the deployed shell started by hand in the author's niri
  session; `duties.bar` set to `"on"` and `battery` and `power-profile` placed
  in `bar.end` of `~/.config/celestina/shell.json`; a machine with a battery
  for the battery row (the piece is absent without one); the profile daemon
  running; Noctalia's power profile widget, if any, may stay on since the
  shell changes the profile only from the page or by a middle click that the
  author makes
- **Procedure:** start the shell and compare the battery piece and the
  `power` page with `upower -i` on the display device: the percentage, the
  state, and the time to empty or full must match. Compare the active profile
  and the list of profiles with `powerprofilesctl list`. Change the profile
  from the page and check that `powerprofilesctl get` reports it within the
  pending window; middle-click the power-profile piece and check that the
  profile cycles in the same order. Then open the Island and check the six
  page tabs (Activity, Notifications, Tray, Network, Bluetooth, Power) for
  fit, overlap and icons at the real scale, with the text scale at its default
  and at "Larger"; note any clipped title
- **Pass condition:** the battery and profile values match `upower` and
  `powerprofilesctl` while the page is open; a profile change from the page or
  the piece converges to the same profile; every tab fits the body without
  overlap at the default scale; the icons are legible; no profile is set that
  the daemon does not list
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-OSD-AUDIO — Volume and microphone OSD beside Noctalia's

- **Status:** pending
- **Related implementation:** CEL-2-C (the `osd` duty, the audio worker on
  `pactl subscribe` and `wpctl`, the `celestina-osd` surface)
- **Requires:** the deployed shell started by hand in the author's niri
  session; Noctalia's OSD switched off first with `noctalia msg osd-disable`
  (or its own setting), so the two never draw at once; `duties.osd` set to
  `"on"` in `~/.config/celestina/shell.json` before starting the shell
- **Procedure:** start the shell and leave audio alone: no OSD may appear at
  start. Press the volume keys (they still run `wpctl` directly): an OSD must
  appear at the bottom centre of the focused output with the level and
  `NN %`, replace itself while the keys repeat, and fade after about 1.5 s.
  Mute with the mute key and check the muted icon with the remembered level.
  Change the volume from another program (a media player slider, `pavucontrol`)
  and check the OSD follows. Use the microphone mute key and check the
  microphone OSD. Run `celestina volume up`, `celestina volume down`,
  `celestina volume set 30`, `celestina volume mute` and `celestina
  microphone set 20`, and check the real volume follows. Set `duties.osd` to
  `"off"`, wait for the shell to follow the file, and check that no OSD
  appears and that `celestina volume up` is rejected in the journal. Check
  with `niri msg layers` that a `celestina-osd` surface exists only while an
  OSD is on screen. Switch Noctalia's OSD back on if you want it
- **Pass condition:** no OSD at start or at rest; the OSD follows keys and
  other programs, shows muted state, replaces in place and is destroyed
  after its exit; the verbs move the real volume with the duty on and are
  rejected with it off; the two OSDs were never on at once
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-OSD-BRIGHTNESS — Brightness OSD over DDC, beside Noctalia's

- **Status:** pending
- **Related implementation:** CEL-2-C (the brightness worker: sysfs where
  present, DDC through one `ddcutil` thread under a lease)
- **Requires:** the deployed shell started by hand in the author's niri
  session; Noctalia's OSD switched off first with `noctalia msg osd-disable`;
  Noctalia's brightness widget not open, and no other `ddcutil` running, since
  two `ddcutil` processes on one bus lost the GPU twice in the halted shell
  and the shell's lease cannot cover Noctalia's processes; `duties.osd` set to
  `"on"`, the `brightness` piece placed in the bar layout; no
  `CELESTINA_DDC=0` in the shell's environment. With the piece placed, the
  shell has a brightness consumer from the start, so it runs **one
  `ddcutil detect --brief` at start** (when the first output is announced),
  plus at most one follow-up detection no sooner than 5 s after it if
  outputs were announced while it ran; it reads and writes nothing until the
  first use. Without the piece, DDC stays untouched until the first
  brightness verb or popup, which detects first
- **Procedure:** start the shell and leave the brightness alone: check in the
  journal (`brightness: ddcutil starts`, `operation="detect"`) and with
  `pgrep -a ddcutil` (read-only, run by you) that only the detection(s)
  described above run, and no `getvcp` or `setvcp`. The brightness keys still
  run `brightnessctl --class=backlight`, which has no device on this machine,
  so they change nothing until CEL-2-M rebinds them: use the verbs. Run
  `celestina brightness up` once and wait for the answer: the first change
  after start must show an OSD (the level before the write is read first and
  is the baseline). Run it again slowly, then a fast burst, which must
  collapse to few writes. Run `celestina brightness down` and `celestina
  brightness set 40 DP-1`, and check the monitor follows and the OSD shows
  the read-back value. Change the brightness with the monitor's own buttons:
  the shell is not expected to notice it (documented gap). Watch the screen
  for black flashes or a frozen picture during every step, and stop at the
  first one. Check the journal for `brightness: ddcutil starts` lines and that
  none overlaps another
- **Pass condition:** with the piece placed, only the start detection (and at
  most one follow-up at least 5 s later) before the first use, and no read
  or write; one `ddcutil` child at a time; the first change after start
  shows an OSD; the OSD shows the read-back value, never 0 for an unread
  monitor; no GPU loss, flicker or freeze; the monitor's own buttons are not
  followed, as documented
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-OSD-PIECES — The volume and brightness pieces and their popup

- **Status:** pending
- **Related implementation:** CEL-2-C (the `volume` and `brightness` pieces,
  the slider popup, the verbs)
- **Requires:** the deployed shell started by hand in the author's niri
  session with `duties.bar` and `duties.osd` `"on"`; the pieces `volume` and
  `brightness` in the `bar` end zone of `shell.json` (they are absent from a
  layout until a reading exists); Noctalia's OSD and its bar off or hidden;
  Noctalia's brightness widget closed
- **Procedure:** check that the volume capsule shows the level and the muted
  state, and the brightness capsule the read-back brightness. The start
  detection reads no level, so the brightness capsule appears only after a
  first `celestina brightness` verb has read the monitor. Scroll over each
  capsule: the level moves by one step. Middle-click the volume capsule:
  mute toggles. Click each capsule: a popup opens at the capsule with a
  slider; drag it, press the arrow keys, Home and End on it (visible focus
  ring), toggle the mute button, and for volume use the microphone row. The
  OSD must not appear for a change the popup is driving, nor when the popup
  closes and the last level it set is read back; with the volume popup open,
  `celestina brightness up` must still show the brightness OSD. Close the
  popup with Escape and by clicking outside. Check the capsules on both
  outputs at their
  scales, and that the Brightness piece of an output with no DDC reading
  shows no capsule
- **Pass condition:** capsules show the right state; scroll, click and
  middle-click do what is described; the slider works by pointer and
  keyboard with a visible focus; the popup opens and closes cleanly; no OSD
  for the change the popup drives, during it or after it closes; another
  kind's OSD still shows while it is open
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-PHONE — The phone piece beside Noctalia's phone widget

- **Status:** pending
- **Related implementation:** CEL-2-J (the `phone` duty, off by default, the
  Magnetita worker on the session bus, the phone piece, the Phone page of the
  Island and the `phone media` verbs)
- **Requires:** the deployed shell started by hand in the author's niri
  session; the author's phone paired with Magnetita, which runs on the session
  bus and owns `org.celestina.Devices1`; `duties.phone` set to `"on"` in
  `~/.config/celestina/shell.json` before starting the shell. The piece lives
  on the bar, so the same file also sets `"duties": {"bar": "on", "phone":
  "on"}` and puts the id `phone` in the bar's end zone, for example
  `"bar": {"end": ["bubbles", "phone", "tray"]}` (the ids `bubbles` and
  `phone` belong to the piece table; the zones are `start`, `centre` and
  `end`). Noctalia's phone widget stays active: this row checks that the two show the phone
  without interfering, and it does not switch Noctalia off
- **Procedure:** start the shell and run `celestina island open phone`. Compare
  the battery percentage and the charging state on the Phone page and on the
  phone itself. Plug the phone in and out and check that the charging state
  follows. Take the phone out of reach until Magnetita reports it disconnected,
  and check that the phone piece leaves the bar (a phone that is not
  connected is simply not listed, and the Phone page says there is no phone)
  and nothing else breaks. With a
  track playing on the phone, check that its title and state appear on the
  Phone page, then run `celestina phone media PlayPause`, `celestina phone
  media Next` and `celestina phone media Previous` and check that the phone's
  player reacts to each. Stop Magnetita and check that the piece is no longer
  drawn, with no error in the journal. Set `duties.phone` back to `"off"`,
  wait for the shell to follow the file, and check that the piece disappears
  without a restart. Switch the duty on again if you want it kept
- **Pass condition:** the battery, the charging state, the connection (a
  disconnected phone leaves no piece) and the
  media match the phone; each `phone media` verb reaches the phone's player;
  when Magnetita is absent or the duty is off the shell draws no phone piece
  and reports nothing as an error; Noctalia's widget and the shell's piece do
  not fight over the phone
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-BUBBLES — Minimised windows as bubbles, on the author's niri

- **Status:** pending
- **Related implementation:** CEL-2-J (the `bubbles` duty, off by default, the
  Melibea worker over its socket with the v1 default and the v2 transition
  hint, the bubbles piece and the `bubbles minimize` verb)
- **Requires:** the deployed shell started by hand in the author's niri
  session, with the patched niri that accepts the anchored transition (the
  one `~/.local/lib/celestina/niri` the nested smoke also prefers); Melibea
  running as the author's own service; `duties.bubbles` set to `"on"` in
  `~/.config/celestina/shell.json` before starting the shell. The bubbles
  live on the bar, so the same file also sets `"duties": {"bar": "on",
  "bubbles": "on"}` and puts the id `bubbles` in the bar's end zone, for
  example `"bar": {"end": ["bubbles", "phone", "tray"]}`. The author binds
  a key of their own to `celestina bubbles minimize` (the shell binds no key)
  and, for the last step, uses a Melibea that speaks only v1 if one is at
  hand; if none is, that step stays not run
- **Procedure:** with the duty on, minimise a window with the key bound to
  `celestina bubbles minimize` and check that a bubble appears in the bubbles
  group with the window's icon. Minimise more windows than the group shows and
  check that the overflow bubble reads `+N` with the number of hidden windows
  and that clicking it opens the selector. Click a bubble and check that the
  window comes back on its own output at the place it left, with the
  transition anchored to the bubble. Watch the same minimise and restore
  again with reduced motion on in the author's appearance settings and check
  that the transition is disabled and the window still changes state. Then,
  with a Melibea that speaks only v1, minimise and restore once: the windows
  must still change state, the anchor is dropped, and the journal says once
  that Melibea speaks v1 only. Stop Melibea and check that the bubbles
  disappear and that the journal gets one line per change of state, not one
  per reconnection attempt: the worker's `melibea: unavailable: ...` (or
  `melibea: absent: ...` once the socket is gone) with the loop's matching
  `bubbles: Melibea is unavailable` (or `... is absent`), and then, when
  Melibea is back, `melibea: subscribed` with the bubbles drawn again
- **Pass condition:** a minimised window becomes a bubble and a click restores
  it; `+N` counts the hidden windows and opens the selector; the anchored
  transition runs with the patched niri and the window ends where it should;
  reduced motion disables the transition; a v1-only Melibea is served with no
  anchor and no error; a missing Melibea shows no bubbles and no error beyond
  those lines, written once per change of state
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-BUBBLES-KEYS — The bubbles selector on Mod+K

- **Status:** pending
- **Related implementation:** CEL-2-J (the selector: the launcher's overlay
  with a bubbles source, `celestina bubbles select` as a toggle)
- **Requires:** the same session as VAL-CEL2-BUBBLES, with at least three
  minimised windows of different applications; the author binds `Mod+K` to
  `celestina bubbles select` in their own niri configuration (the shell binds
  no key)
- **Procedure:** press `Mod+K` and check that the card opens with the focus in
  its field and the first row selected. Type part of a window's title and
  check that the rows filter by title, then by app id; clear the field and
  check that all windows come back. Press `Down`, `Up`, `Tab` and `Shift+Tab`
  and check that the selection moves and wraps at both ends. Press `Enter` on
  a row and check that the window is restored and the card closes. Press
  `Mod+K` again, select a row and press `Delete`: the window is asked to
  close, and its row leaves only when the window really closes. Press `Mod+K`
  and then `Escape`, then press `Mod+K` and click outside the card, then press
  `Mod+K` and move the focus to another window: each closes without acting.
  Press `Mod+K` while the card is open and check that it closes. Check that
  after every close the keyboard goes back to the window that had it before
  the card opened. With no minimised windows the card shows one note row and
  nothing is selectable
- **Pass condition:** every key does what is described; the card holds the
  keyboard while open and gives it back on close; `Delete` never removes a row
  by itself; an empty selector explains itself in one line and does nothing
  when `Enter` is pressed
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-MEDIA — The media duty: the piece, the page, the buttons and the pin

- **Status:** pending
- **Related implementation:** CEL-2-D (the `media` duty, off by default, the
  MPRIS client, the media piece and the Medios page)
- **Requires:** the deployed shell started by hand in the author's niri session,
  with Noctalia's media off through Noctalia's own control; a browser and mpv
  each able to play a track; two players playing at once for the pin step
- **Procedure:** set `duties.media` to `"on"` in `~/.config/celestina/shell.json`
  (the shell follows the file live). Play something in a browser and something
  in mpv. Place the `media` piece in one of the bar zones through the `bar`
  layout of the same file. Check that the piece shows the glyph and one line of
  the track. Click the piece and check that the page "Medios" opens with the
  shown player's title and artist. Press previous, play/pause and next on the
  page and check that each acts on the shown player. Middle-click the piece and
  check that play/pause acts. With two players, pin the second one from the page
  and check that the page and the piece follow it; press its row again and check
  that the choice goes back to the automatic one. Check that the keys still run
  `playerctl` (for example `playerctl --player=mpv play-pause`), with the shell
  binding no media key of its own
- **Pass condition:** the piece and the page show the shown player; the three
  buttons, the middle click and the pin act on that player; the pin does not
  survive a restart of the shell; the keys run `playerctl` as before
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-MEDIA-COVER — Cover art: a file shows, an https link shows none

- **Status:** pending
- **Related implementation:** CEL-2-D (the cover pipeline, the cover cache of
  eight entries, the bounded decode to 128 by 128)
- **Requires:** the deployed shell with `duties.media` `"on"` as in
  VAL-CEL2-MEDIA; a player that publishes its cover as a file. A PNG, JPEG or
  WebP image of at most 8 MiB, named with its extension, at an absolute path
  that the player can point at
- **Procedure:** make a player publish that file as its cover (for example a
  local image path in the player's metadata). Check that the page shows the
  cover, and that the cover of a non-square image is cropped to its centre.
  Then make the player publish an `https://` cover for a track. Check that the
  page shows no cover for it and that the title and the buttons still work. Do
  not use a file over 8 MiB, which should also show none
- **Pass condition:** the file cover shows on the page; the `https` cover shows
  none, with the rest of the page working; nothing else is fetched from the
  network
- **Result:** not run
- **Evidence:** none
