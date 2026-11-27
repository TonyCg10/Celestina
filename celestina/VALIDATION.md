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
VAL-CEL2-OSD-PIECES; CEL-2-J queues VAL-CEL2-PHONE, VAL-CEL2-BUBBLES and
VAL-CEL2-BUBBLES-KEYS; CEL-2-D queues VAL-CEL2-MEDIA and VAL-CEL2-MEDIA-COVER;
CEL-2-H-a queues VAL-CEL2-LOCK-PAM, VAL-CEL2-LOCK-LIVE, VAL-CEL2-LOCK-SUSPEND,
VAL-CEL2-LOCK-HOTPLUG, VAL-CEL2-LOCK-RECOVERY, VAL-CEL2-LOCK-NOTICES and
VAL-CEL2-LOCK-COEXIST; CEL-2-H-b queues VAL-CEL2-LOCK-BACKDROP and
VAL-CEL2-LOCK-CLOCK; CEL-2-I queues VAL-CEL2-POLKIT-LIVE; CEL-I-004 queues VAL-CEL-I-004.
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
  check that the action runs. Click a notice body without a default action:
  the Notifications page opens on that output. Send four notifications and
  check that the Island shows the newest as a notice with two sheets under it.
  Point at the notice and check that the expiry pauses; move the pointer away
  and check that it resumes. Dismiss one notice with its icon button. Turn on do-not-disturb from the overflow
  menu, send a normal notification and check that no banner shows, while the
  Notifications page keeps it in its history
- **Pass condition:** each step does what is described: 5 s, 10 s and a
  staying critical banner; the action button and the body click behave as
  described; four notifications show one notice and two sheets; hover pauses the expiry
  and leaving resumes it; the dismiss button closes the notice; do-not-disturb
  hides banners and keeps history
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-BAR-LIVE — The bar beside Noctalia, its blur, its popups, focus and two scales

- **Status:** pending
- **Related implementation:** CEL-2-A (the bar surface and the piece table;
  the bar duty it shipped with is gone since CEL-2-M)
- **Requires:** the deployed shell started by hand in the author's niri
  session (since the cutover the bar is always drawn; `duties.bar` no longer exists), Noctalia still running
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
  own idle settings, so the two never act at once; caffeine off; **no idle
  inhibitor held by another program**: the shell's idle notifications honour
  inhibitors, as the design asks, so while one is held `idle: armed` never
  fires (observed on 2026-11-23 with Claude Desktop's "Keep computer awake
  while Claude works" switched on, which holds one while a session works)
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
  `systemd-inhibit --list`: a `Celestina` lock for `idle` in mode
  `block` must be listed, and no `Celestina` lock for `sleep` in mode
  `block` (with the lock duty on, its own `sleep` lock in mode `delay` is
  listed and is right). Run `celestina caffeine toggle` and check that the
  `idle` lock is
  gone. Turn it on again, set `duties.idle` to `"on"` and wait past the
  screen-off delay: the outputs must stay on while caffeine is held. With
  caffeine still on and the lock duty on, run `systemctl suspend` (or
  `celestina lock suspend`, or the power key) with no unsaved work: the
  machine must suspend,
  locked, because caffeine only stops what inactivity does by itself. Run
  `celestina caffeine off`, stop the shell with `celestina quit` and check
  that no `Celestina` lock remains. Verbs print nothing; the result is read
  in `systemd-inhibit --list` and in the shell's journal
- **Pass condition:** the `idle` lock exists exactly while caffeine is on;
  while it is on the idle chain is held off by the shell's own switch and
  logind's idle action by the lock, and a suspend asked for by hand goes
  through, locked first; the lock is released by the verb,
  by switching the duty off and by
  the shell stopping; `energy.caffeine` in `shell.json` follows the verb;
  with the duty off the verb does nothing to the system and says so in the
  journal
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-NIGHTLIGHT — Forced night light on the three outputs

- **Status:** pending
- **Related implementation:** CEL-2-B (the gamma worker and the night-light
  verbs); CEL-2-B-GAMMA (the sweep paced by the compositor and the
  connection opened again after a loss)
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
  to neutral. Run the shell for this row with
  `RUST_LOG=info,celestina::energy::nightlight=debug` and, each time the
  light goes on or off, read the worker's line
  `night light: sweep played N of 18 frames, slowest confirm M ms` in the
  journal; write N and M down in the result
- **Pass condition:** the warm tint appears and goes on every output, survives
  hotplug, follows the kelvin setting, and is gone when the shell stops; if
  an output refuses the gamma control the others still change and the journal
  says `night light: not applied`. A transition the compositor paces slower
  than its 300 ms still ends on the target on every output: the tint stays,
  it does not show for an instant and go, every sweep line reports at least
  one frame played with its slowest confirm well under 1000 ms, and the
  journal does not say `the compositor did not answer a sync`. The return of
  the light after a lost connection is covered by the automated tests only:
  no step here causes a loss (a VT switch does not, niri defers the table).
  If the journal does show one while the light is wanted
  (`night light: not applied: ...`), then `night light: applied` must follow
  by itself within the backoff (1 s, doubling to 30 s), with no verb and no
  restart
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
- **Note:** since CEL-D-001 the tabs show icons; their look is checked by
  `VAL-CEL-D-001`, and the tab part of this procedure no longer applies
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
  session; `network` placed in `bar.end` of
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
  session; `bluetooth` placed in `bar.end` of
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
  session; `battery` and `power-profile` placed
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
  shell has a brightness consumer from the start, so when the duty's workers
  start it runs **one `ddcutil detect --brief`** and then **one `getvcp` per
  monitor whose level is unknown**, one child at a time (a cold read takes
  about ten seconds per monitor on this hardware), plus at most one
  follow-up detection if outputs were announced meanwhile, no sooner than
  5 s after the first and after the last output change, which reads only a
  monitor still unknown; it writes nothing until the first use, and a
  monitor that does not answer is not asked again by itself, except after
  each later detection. Outputs that appear or go later (a resume from
  suspend, the idle duty powering them off and on, a cable) cost one
  detection once they have been still for 5 s, and a monitor it no longer
  finds loses its capsule. A wheel step over the brightness capsule writes
  to the monitor, by design: an accidental scroll over the bar is a
  `setvcp`. Without the piece, DDC stays untouched until the
  first brightness verb or popup, which detects first
- **Procedure:** start the shell and leave the brightness alone: check in the
  journal (`brightness: ddcutil starts`, `operation="detect"`, then
  `operation="read"` once per monitor) and with `pgrep -a ddcutil`
  (read-only, run by you) that only the detection(s) and the reads described
  above run, one after another, and no `setvcp`; no OSD shows for these
  first readings. The brightness keys still
  run `brightnessctl --class=backlight`, which has no device on this machine,
  so they change nothing until CEL-2-M rebinds them: use the verbs. Run
  `celestina brightness up` once and wait for the answer: the first change
  after start must show an OSD (the level read at start is the baseline;
  for a monitor still unread, the level before the write is read first).
  Run it again slowly, then a fast burst, which must
  collapse to few writes. Run `celestina brightness down` and `celestina
  brightness set 40 DP-1`, and check the monitor follows and the OSD shows
  the read-back value. Change the brightness with the monitor's own buttons:
  the shell is not expected to notice it (documented gap). Suspend and
  resume, and let the idle duty power the outputs off and wake them: in the
  journal at most one detection, no sooner than 5 s after the outputs
  settle, then one read per monitor whose level became unknown; none
  overlapping; watch for a black flash. Watch the screen
  for black flashes or a frozen picture during every step, and stop at the
  first one. Check the journal for `brightness: ddcutil starts` lines and that
  none overlaps another
- **Pass condition:** with the piece placed, before the first use only the
  start detection (and at most one follow-up at least 5 s later) and one
  read per monitor whose level is unknown, and no write; after a resume or a
  wake, at most one detection, no sooner than 5 s after the outputs settle,
  and reads only of monitors whose level became unknown; one `ddcutil` child
  at a time; the first change after start
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
  session with `duties.osd` `"on"` (the bar is always drawn); the pieces `volume` and
  `brightness` in the `bar` end zone of `shell.json` (they are absent from a
  layout until a reading exists); Noctalia's OSD and its bar off or hidden;
  Noctalia's brightness widget closed
- **Procedure:** check that the volume capsule shows the level and the muted
  state, and the brightness capsule the read-back brightness. With the
  piece placed and the duty on, the shell reads each monitor once after the
  start detection, so the brightness capsule appears by itself, with no verb,
  once its monitor has answered (a cold read takes about ten seconds per
  monitor, one after another); a monitor that does not answer shows no
  capsule. Take `brightness` out of the layout and put it back while the
  shell runs: the capsule goes and comes back without a restart. Scroll over each
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
  on the bar, so the same file also sets `"duties": {"phone":
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
  live on the bar, so the same file also sets `"duties": {
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

## VAL-CEL2-LOCK-PAM — The PAM service, installed by hand

- **Status:** pending
- **Related implementation:** CEL-2-H-a (`celestina/contrib/pam.d/celestina-lock`,
  `celestina-lock-verify`)
- **Requires:** root on the author's machine; the repository checkout. No agent
  installs, edits or checks anything in `/etc`
- **Procedure:** read `celestina/contrib/pam.d/celestina-lock` (four lines: the
  header, a comment and `auth` and `account` including `system-auth`, with no
  `pam_nologin`), then install it with
  `sudo install -m 0644 -o root -g root celestina/contrib/pam.d/celestina-lock /etc/pam.d/celestina-lock`.
  If `pamtester` is installed, run `pamtester celestina-lock <user> authenticate`
  and type the right password. Do not type a wrong password more than once: the
  service shares `pam_faillock`, so three wrong passwords in 15 minutes lock the
  account for 10 minutes everywhere (`faillock --user <user>` shows the tally)
- **Pass condition:** the right password is accepted; a wrong one is refused;
  with the file removed again the shell, with `duties.lock` on, warns once that
  the service is missing, and the lock refuses everything with the
  missing-service message
- **Result:** partial, 2026-10-23: the author installed the service by hand and ran the shell in a nest (own bus, throwaway `XDG_CONFIG_HOME` with `duties.lock` on, the deployed 1.10.0 binaries): a wrong password was refused and the right one accepted through `celestina-lock`, both logged by `pam_unix(celestina-lock:auth)`, with the `faillock` tally empty afterwards. Not run: `pamtester`, and the file removed again (the missing-service notice on the live shell)
- **Evidence:** [the nest check of 2026-10-23](docs/evidence/2026-10-23-cel-2-h-b.md#the-nest-check-of-2026-10-23-pam-and-live)

## VAL-CEL2-LOCK-LIVE — The lock on the real session

- **Status:** pending
- **Related implementation:** CEL-2-H-a (the lock, the covers, the CPU painter,
  the field, the status words, Caps Lock and the fade)
- **Requires:** the deployed shell started by hand in the author's niri session
  with the PAM service installed (VAL-CEL2-LOCK-PAM passed), `duties.lock` set
  to `"on"` in `~/.config/celestina/shell.json`, Noctalia's idle lock switched
  off first so the two never lock at once, `swaylock` left on `Mod+Shift+A`, and
  a root login (a console or an `ssh` key) at hand for a lockout. The lock is
  started by the shell (`celestina lock`), never by hand: a hand-run
  `celestina-lock` outside a nest is refused with exit 15 (ADR 0022 A4)
- **Procedure:** run `celestina lock`. Check the three outputs: each is covered
  in black at once and the password field appears on the output with the
  keyboard focus, then follows the pointer to another output. Type the right
  password and check that it unlocks. Lock again, type a wrong password and
  check the refusal message and the delay (2 s, then 4 s on the second wrong
  password; do not make a third: `faillock` counts them). Press Caps Lock and
  check the message. Type a password with accents and dead keys and check that
  it is accepted (precomposed characters as `passwd` stored them). Check the
  contrast of the field and the words over black, on the scale 1.5 output and on
  the scale 1.0 output. Leave the lock untouched for the fade delay (30 s) and
  check that every output goes black, prompt included, and that a key or the
  pointer brings the prompt back with the typed bullets still there. There is no
  screen-reader support on the lock (a known gap, not tested)
- **Pass condition:** covered at once on every output; the right password
  unlocks and nothing else does; the delay and the messages are as written; the
  accented password works; the field is readable; the fade and its recovery work
- **Result:** partial, 2026-10-23, in a nest with one output and not on the live session: `celestina lock` covered the output and showed the field, a wrong password was refused and the right one unlocked, the lock ended by itself and left no process. Not run: the three real outputs, the field following the pointer, Caps Lock, accented passwords, contrast at scale 1.0 and 1.5, the fade (30 s) and its recovery
- **Evidence:** [the nest check of 2026-10-23](docs/evidence/2026-10-23-cel-2-h-b.md#the-nest-check-of-2026-10-23-pam-and-live)
- **Rollback:** switch `duties.lock` off and leave `swaylock` on `Mod+Shift+A`.
  A running lock is not touched by switching the duty off; unlock it with the
  password

## VAL-CEL2-LOCK-SUSPEND — Suspend only after the lock covers

- **Status:** pending
- **Related implementation:** CEL-2-H-a (the idle chain's second step, the
  `lock suspend` verb, the logind delay inhibitor)
- **Requires:** as VAL-CEL2-LOCK-LIVE, plus `duties.idle` set to `"on"`; the
  session otherwise idle
- **Procedure:** leave keyboard and pointer untouched for the idle suspend
  delay (240 s) and check that the session locks and then suspends, and that on
  resume the lock is up and asks for the password. Then, from an unlocked
  session, close the lid, press the power button and run `systemctl suspend`
  one at a time; on resume check each time that the lock was up before the
  machine slept and that no part of the session was visible on wake (time how
  long the screen shows before the prompt). With the lock up, let the outputs
  power off and wake them with the pointer
- **Pass condition:** every suspend is preceded by a confirmed lock; on every
  resume the first thing shown is the lock; the screen-off while locked wakes on
  input. A glimpse of the session on resume is a finding (it is logind's cap on
  delay inhibitors, F24 of the design) and is recorded with its duration
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-LOCK-HOTPLUG — An output plugged and unplugged while locked

- **Status:** pending
- **Related implementation:** CEL-2-H-a (one cover per output, the prompt
  moving with the focus)
- **Requires:** as VAL-CEL2-LOCK-LIVE; a second or third output that can be
  plugged and unplugged
- **Procedure:** lock the session. Plug an output and check that it shows black
  at once and then a cover; unplug the output that holds the prompt and check
  that the prompt appears on another output; plug it back. Repeat with the
  lock covering all outputs and then with only one output left
- **Pass condition:** at no moment does any part of the session show on any
  output; the prompt is always on an output that exists
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-LOCK-RECOVERY — Getting back in when the lock does not let you

- **Status:** pending
- **Related implementation:** CEL-2-H-a (the pid file, the relaunch, the
  recovery path of the README and the design's §8)
- **Requires:** as VAL-CEL2-LOCK-LIVE; a console login that works on
  `Ctrl+Alt+F3`
- **Procedure:** lock the session. Press `Ctrl+Alt+F3` and log in on that
  console. Read the pid with
  `cat "$XDG_RUNTIME_DIR/celestina/lock-wayland-1.state"` (the display name is the
  session's; `ls /run/user/<uid>/wayland-*`), then `kill -KILL <pid>` by that
  pid only, never by name. Return to the session's console: the screen shows
  the dark lock colour and the shell starts the lock again within a second.
  Then stop the shell, kill the lock again by its pid, and from the console run
  `WAYLAND_DISPLAY=wayland-1 XDG_RUNTIME_DIR=/run/user/<uid> swaylock -c 000000`
  and unlock it with your password. Finally read the `faillock` lockout
  message once (three wrong passwords on the production service lock the
  account: do this only if you accept 10 minutes of lockout, or skip it) and
  write down which root path you keep for a lockout: a console login as root or
  an `ssh` key to root
- **Pass condition:** the lock is killed by pid and the session stays locked;
  the relaunch and the `swaylock` takeover both work; you know your root path
  to `faillock --user <name> --reset`
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-LOCK-NOTICES — The lock's notices and the fade

- **Status:** pending
- **Related implementation:** CEL-2-H-a (the notices of explicit refusals, the
  missing-PAM-file notice at duty-on, the fade)
- **Requires:** as VAL-CEL2-LOCK-LIVE, with a notification owner running
  (Noctalia during coexistence)
- **Procedure:** with another client's lock holding the session (start
  `swaylock` with `Mod+Shift+A`), run `celestina lock suspend` and check that a
  notification says the lock was refused and that nothing suspends. Move the PAM
  service away (as root, `mv /etc/pam.d/celestina-lock /etc/pam.d/celestina-lock.off`)
  and switch `duties.lock` off and on: check that a notification says the service
  is missing; put the file back. Leave a lock untouched and check the fade to
  black after its delay
- **Pass condition:** a refused `lock suspend` and a missing PAM file at duty-on
  each reach the notification owner once; an idle refusal produces no
  notification; the fade follows the delay
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-LOCK-COEXIST — Beside Noctalia's lock

- **Status:** pending
- **Related implementation:** CEL-2-H-a (the protocol's single lock, exit 10
  and its bounded retry)
- **Requires:** Noctalia running with its lock enabled, `duties.lock` `"on"`
- **Procedure:** lock with Noctalia's own lock control (its session menu), then run
  `celestina lock` and check that it is refused and the Noctalia lock is
  unaffected; unlock it with your password. Then lock with `celestina lock`,
  run Noctalia's lock and check that Noctalia is refused and ours is
  unaffected
- **Pass condition:** the second locker is refused in both orders and neither
  lock is displaced; a refused `celestina lock suspend` never suspends
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-OVERVIEW — The workspace overview and its preview

- **Status:** pending
- **Related implementation:** CEL-2-L (the overview panel, the workspace
  geometry and the one on-demand `wlr-screencopy` preview per opening)
- **Requires:** the author's niri session with the deployed shell, or a
  nested niri started with `scripts/dev-session.sh --own-bus` (the nest's
  own session bus, so the live session is not touched). **Test with a nested
  niri first.** A nested niri is the first run of this row, and the live
  session only after it passes there. Windows the author's config blocks with
  `block-out-from "screen-capture"` must be open on the output, so the check
  can see that they come out black
- **Procedure:** bind `celestina overview toggle` to a key in niri with
  `spawn "celestina" "overview" "toggle"`, or run it from a shell in the nest.
  Open the overview with several workspaces and windows, and check that the
  panel shows the workspaces of the focused output in their arrangement:
  columns left to right, rows down a column, windows at their true relative
  sizes. On the workspace active on the output the panel opens on, check that
  one preview of that output is drawn behind its tiles, dimmed and rounded.
  Check that the preview is taken once: change the view while the panel is
  open and check the preview does not refresh. Check that the blocked windows
  are black in the preview and the other windows are not. Close the panel with
  `celestina overview close`, with Escape, with a click outside it, and by
  moving focus to another window, and check each one closes it and the preview
  is gone (not kept in the next opening). Check that `celestina overview open`
  opens it and that a second `overview toggle` closes it. Check the panel on a
  rotated output, if the author has one, and on a fractional-scale output at
  1.5: the rotated output must show geometry only, and the fractional-scale
  preview must not be blurred or misaligned
- **Pass condition:** the panel opens and closes by each action in the
  procedure; the preview appears only on the workspace active on the output
  the panel opens on and is taken once per opening; blocked windows are black;
  no preview survives a close; a rotated output shows geometry only; the
  panel's own surface never appears in its preview
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-SHORTCUTS — The shortcut sheet beside the author's config

- **Status:** pending
- **Related implementation:** CEL-2-L (the shortcut sheet, read from niri's
  `config.kdl` once per opening of the Shortcuts mode)
- **Requires:** the same niri session or nest as VAL-CEL2-OVERVIEW; the
  author's `config.kdl` in place (`$NIRI_CONFIG`, else
  `$XDG_CONFIG_HOME/niri/config.kdl`). **Test with a nested niri first.**
- **Procedure:** bind `celestina shortcuts toggle` to a key in niri with
  `spawn "celestina" "shortcuts" "toggle"`, or run it from a shell in the nest.
  Check that the sheet opens and lists the binds of `config.kdl` grouped by
  binding mode, with the normal binds under their own group. Compare the list
  with the file: every bind in the file appears once, with its own title and
  its own action, and no bind appears that the file does not declare. Add a
  bind to the config (in the nest's config only) and run `shortcuts toggle`
  again: the new bind appears, because the file is read once per opening. Close
  the sheet with Escape, a click outside it, and by moving focus, and check the
  same way as the overview. Check that `celestina shortcuts open` opens it and
  `celestina shortcuts close` closes it
- **Pass condition:** the sheet matches `config.kdl` bind for bind and group
  for group; a change to the file shows on the next opening and not before;
  each close action works; nothing is read from any other file
- **Result:** not run
- **Evidence:** none

## VAL-CEL2-LOCK-BACKDROP — The dimmed wallpaper under the lock

- **Status:** pending
- **Related implementation:** CEL-2-H-b (the wallpaper thumbnail per output,
  the bundle's pixel blocks, the lock's backdrop worker and the scrim)
- **Requires:** as VAL-CEL2-LOCK-LIVE, plus `duties.wallpaper` set to `"on"`
  with `wallpaper.folder` naming a folder of pictures, and one picture with a
  bright area among them. With Noctalia holding the wallpaper (coexistence)
  the shell paints none, so the second half of the procedure is the one that
  applies then
- **Procedure:** with the wallpaper duty on, run `celestina lock` and check
  that each output shows its own picture, dimmed and blurred, with the field
  and the status words on top. Give an output the brightest picture you have
  and check that the field, the bullets and the words stay readable over it.
  Plug or unplug an output while locked and check that the new output is black
  or shows its picture, and never the session. Then switch `duties.wallpaper`
  off, unlock, lock again and check that the backdrop is black; do the same
  with Noctalia owning the wallpaper
- **Pass condition:** each output shows its own dimmed and blurred picture;
  the field is readable over the brightest wallpaper; with the wallpaper duty
  off (coexistence with Noctalia) the backdrop is black, which is the intended
  result there and not a failure, and the prompt is complete in both cases
- **Result:** partial, 2026-10-23, in a nest with one output (R-10 of the rehearsal, [evidence](docs/evidence/2026-10-23-cel-2-h-b.md)): the dimmed and blurred picture showed under the field with the clock and the date, and the clock redrew at the minute. Not run: the live session, three outputs, hotplug, the brightest real picture, resume from suspend, the unreadable-offset case
- **Evidence:** [2026-10-23-cel-2-h-b.md](docs/evidence/2026-10-23-cel-2-h-b.md), section R-10

## VAL-CEL2-LOCK-CLOCK — The lock's clock and date

- **Status:** pending
- **Related implementation:** CEL-2-H-b (the realtime minute timer, the
  uncached UTC offset read at each spawn)
- **Requires:** as VAL-CEL2-LOCK-LIVE; for the offset case, a way to make
  `date +%z` fail for the shell, such as starting the shell by hand with an
  unusable `PATH` in a nest, never on the live session
- **Procedure:** run `celestina lock` and compare the clock and the date with
  the system's. Leave the lock up across a minute change and check that the
  clock follows it. Suspend with `celestina lock suspend` (after
  VAL-CEL2-LOCK-SUSPEND passed), resume and check that the clock is right at
  once and not up to a minute late. Start a shell whose `date +%z` cannot be
  read (in a nest) and lock: the date and the field show and the clock does
  not. Lock shortly before a daylight-saving change, if one is near, and
  check that the offset the lock shows is the one read when it started
- **Pass condition:** the clock and the date are right at lock time and on
  resume from suspend; the clock is hidden, with the date and the field in
  place, when the offset could not be read; a daylight-saving change during
  one lock keeps the offset read at its spawn
- **Result:** partial, 2026-10-23, in a nest with one output (R-10 of the rehearsal, [evidence](docs/evidence/2026-10-23-cel-2-h-b.md)): the dimmed and blurred picture showed under the field with the clock and the date, and the clock redrew at the minute. Not run: the live session, three outputs, hotplug, the brightest real picture, resume from suspend, the unreadable-offset case
- **Evidence:** [2026-10-23-cel-2-h-b.md](docs/evidence/2026-10-23-cel-2-h-b.md), section R-10

## VAL-CEL2-POLKIT-LIVE — The polkit agent in the author's session

- **Status:** pending
- **Related implementation:** CEL-2-I (the `polkit` duty: the agent, the
  prompt card and the helper conversation)
- **Requires:** the deployed shell started by hand in the author's niri
  session, with Noctalia's polkit agent switched off first and Noctalia's
  notifications understood (see the notes below); `duties.polkit` set to
  `"on"` in `~/.config/celestina/shell.json` (the shell follows the file live);
  a terminal; the shell's log in view (its stderr, or `journalctl --user -b`
  if a unit started it). Do not run this with a work session you cannot
  afford to lose focus in: a prompt takes the keyboard exclusively
- **Procedure:** (1) With the duty on and Noctalia's agent off, run
  `pkexec /usr/bin/true`: the card appears on the focused output with the
  other outputs dimmed, showing polkit's message, the action id and your
  account, and the keyboard is the card's. Type the right password and press
  Enter: the card closes and `pkexec` exits 0. (2) Run it again, type a wrong
  password once and press Enter: the card says the answer was not accepted
  and asks again; type the right one and check it succeeds. Do not fail twice
  in a row more than once in a session (see the faillock note). (3) Run it
  again and press Escape: the card closes and `pkexec` reports that the
  request was dismissed. Then run `pkexec /usr/bin/true` and cancel it with
  Escape (or the Cancel control) three times in a row, without typing
  anything, and after each time, then once more after the third, check with
  `faillock --user <your user>` that the failure counter has not risen and
  that the account is not locked (you run `faillock` yourself; the agent never
  does, and a cancel after typing a password is not part of this check).
  (3b) Run it, type a wrong password and press Enter, then press Escape on the
  card that asks again: `faillock --user` shows one more failure (the wrong
  password) and not two (the Escape adds none). Finish this check with one
  successful `pkexec /usr/bin/true`, which resets the counter.
  (4) Run it, and while the card is up start a second
  overlay client that takes the keyboard (any layer-shell program that asks for
  exclusive keyboard, started from another terminal you opened before): the
  card loses the keyboard, the typed text is wiped, and the request is
  cancelled within about two seconds, with a notice that an authorization
  prompt could not be shown. (5) Run it and, with the card up, run
  `systemctl restart polkit` from a terminal opened before: the request is torn
  down, the card closes, and a new `pkexec` prompts again with no restart of
  the shell. (6) Run it and stop the shell with `celestina quit`, or kill it by
  its own pid (never by name); `pkexec` must fail, never succeed, and a new
  shell must register again. (7) With the duty on, ask Hematita for a service
  action that polkit guards (start or stop a unit), and check that the card
  names Hematita's action and that cancelling it makes Hematita report a
  refusal, not a success. (8) After all of this, read the shell's log and
  `journalctl --user -b`: they must contain no password, no part of one and no
  cookie (search for the password you typed and for the cookie shape
  `3-` followed by hex in the polkit lines). (9) Rollback: set `duties.polkit`
  to `"off"`, check that a pending request, if any, is cancelled and that a new
  `pkexec` gets no card from the shell, then re-enable Noctalia's agent and
  check that `pkexec /usr/bin/true` prompts there. Stopping the shell has the
  same effect, since polkit removes an agent whose connection vanished
- **Pass condition:** every step does what is described: the card on the
  focused output with the exclusive keyboard, ready to take the password as
  soon as it appears; success only after the right
  password; a wrong answer asks again and a second one in the same prompt
  ends it; Escape and focus theft cancel (never grant) and **three cancels
  in a row with nothing typed leave `faillock --user` at no failures**; a
  polkit restart or a
  killed shell ends the request and never grants; Hematita's action shows its
  own id and obeys the answer; the log holds no secret and no cookie; and the
  rollback returns the session to Noctalia's agent
- **Notes for the author, not checks:**
  - **faillock.** This host's `pam_faillock` is `deny = 3`, `unlock_time = 600`.
    A prompt allows two attempts (`MAX_ATTEMPTS = 2`), so one prompt cannot lock
    the account, but two prompts that each fail twice can, for login and `sudo`
    too. Keep the failure count at one per prompt in this row. Before
    CEL-2-I-CANCEL (1.13.2) a cancel also counted, because the agent had the
    helper's socket open while PAM waited for the password: the third cancel
    locked the account. The agent now opens nothing before you submit, so a
    cancel with nothing typed counts for nothing. What can still count: a wrong
    password that was sent, a cancel at a second prompt of the PAM stack (this
    host has none), and the shell exiting in the instant between the
    connection and PAM's first prompt. A cancel after a **correct** password
    was sent ends denied, and `pam_faillock` records a success then.
  - **niri.** The prompt cannot be defended from a compositor that is hostile,
    and has no trusted path on Wayland. Recommended in the niri configuration
    (outside this repository, the author decides): a `layer-rule` for the
    namespace `celestina-polkit-prompt` with a `block-out-from` setting
    (whether niri applies the rule to layer surfaces is unverified, and a
    fixed-length echo is not shown in any case), and withholding the
    virtual-keyboard and input-method protocols from untrusted clients, since a
    virtual keyboard injects into the focused surface, which is the card, with no
    leave event (spec Q7).
  - **Noctalia holds the notifications name.** The notices (the prompt could not
    be shown; the helper could not be used; the prompt cool-down) are posted through the shell's own
    notification server, which does not run while Noctalia owns
    `org.freedesktop.Notifications`: the request is still cancelled, but the
    notice is lost and the shell only writes a log line. Step 4 therefore shows
    its notice only if Noctalia's notifications are off and the shell serves them.
  - **Deadlines and the screen.** The prompt's deadlines (the keyboard proof,
    the focus grace, the idle cap) are evaluated when the compositor sends a
    frame callback for the Island's or the card's surface. With the output
    switched off by the idle chain, or the surface hidden, no callback arrives
    and a prompt ends only when polkitd cancels it or the person acts; nothing
    is granted either way. Check step 4 and the idle cap with the screen on.
  - **The session type.** The agent registers only for a local `wayland` session
    of your user. Before the row, `loginctl show-session
    $(loginctl show-user $USER -p Display --value) -p Id -p Type -p Class -p
    Remote` should say `Type=wayland`, `Class=user`, `Remote=no`; a session
    started from a TTY says `Type=tty` and the duty reports `unavailable`.
  - **The glyph.** The card shows the suite's `key` icon where the design says
    lock: `celestina-style/icons` has no `lock.svg` yet.
  - **Accessibility.** The toolkit has no AT-SPI; the answer field has no
    accessible tree, and nothing is claimed.
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [2026-10-17-cel-2-i.md](docs/evidence/2026-10-17-cel-2-i.md)

## VAL-CEL-I-004 — Devices and per-application volume in the volume popup

- **Status:** pending
- **Related implementation:** CEL-I-004 (the audio graph in the audio worker,
  the collapsible output, input and application sections of the volume popup)
- **Requires:** the deployed shell started by hand in the author's niri
  session, after the landing of CEL-I-004 and with `duties.osd` set to `"on"`
  in `~/.config/celestina/shell.json`; the `volume` piece in the bar; real
  PipeWire with the SNOWSKY as the default output, a second sink (any other
  one), Firefox or a game playing sound, and a second application that can
  play sound (a browser tab and mpv do)
- **Procedure:** click the `volume` piece to open the popup. Check that it
  opens with the outputs, inputs and applications sections folded: the outputs
  section shows only the current output with its check mark, the inputs section
  only the current input with its check mark, and the applications section only
  the applications that are playing now. Unfold each section by its header
  (also with Tab and Enter) and check that the outputs and inputs sections list
  every device and the applications section lists every application, and that
  the popup grows downward with its top edge fixed under the piece. Fold them
  again. Close and reopen the popup and check that it opens folded again. Then,
  with the sections unfolded: (1) click another output and check that the sound
  moves to it and the mark moves with it; click the SNOWSKY and check that the
  sound and the mark come back. Then click another input and check that the
  mark moves; click the first one again. (2) With sound playing in Firefox or a
  game and in a second application, drag the slider of the first one down and
  check that only that application gets quieter and the main volume slider does
  not move. (3) Press the speaker button of one application and check that only
  that one goes silent, then press it again. (4) Do steps 1 to 3 once with the
  popup open and watch the screen edge where the OSD appears: no OSD may flash
  while you do it. Close the popup with Escape or a click outside, and check
  that no OSD appears either on closing
- **Pass condition:** the popup opens folded with the current output, the
  current input and only the playing applications; each header unfolds its full
  list and the popup grows downward with its top edge fixed; the output and the
  input each switch to the chosen device and back, and the mark follows;
  lowering or muting one application leaves the others and the main volume
  alone; no OSD flashes during the steps or on closing the popup
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [2026-11-25-cel-i-004-audio.md](docs/evidence/2026-11-25-cel-i-004-audio.md)

## VAL-CEL-I-006 — The session menu

- **Status:** pending
- **Related implementation:** CEL-I-006 (the `session` piece and its menu)
- **Requires:** the deployed shell started by hand in the author's niri
  session, after the landing of CEL-I-006, with `"session"` added last to
  `bar.end` in `~/.config/celestina/shell.json` (the file sets `bar.end`
  explicitly, so the new default does not reach it); the lock duty on; all
  your work saved first, because the last step reboots the machine
- **Procedure:** click the power glyph at the end of the bar and check that a
  menu opens under it with five rows (lock, suspend, then after a divider log
  out, reboot and power off). Move through the rows with the arrow keys (the
  divider is skipped) and close the menu with Escape, then again with a click
  outside. (1) Choose the lock row and check that the session locks and that
  your password returns you. (2) Choose the suspend row and check that the
  session locks, the machine suspends and, after waking it, the lock is still
  up. (3) For each of the log-out, reboot and power-off rows: choose it and
  check that it turns into a red card with a question and two buttons (cancel
  and the row's own action), that the menu grows downward with its top edge
  fixed and that the keyboard is on the cancel button. Press the cancel button
  and check that the list comes back; open the card again and press Escape and
  check the same. Do not confirm log out or power off. (4) Last, with your
  work saved, choose the reboot row and confirm it
- **Pass condition:** the menu opens under the glyph with the five rows; the
  arrows skip the divider; Escape on the list and a click outside close it;
  the lock row locks; the suspend row locks and suspends; each dangerous row
  asks first in the red card, grows the menu downward, and the cancel button
  and Escape go back to the list; confirming the reboot row reboots the
  machine. (After log out niri shows its own confirmation: that is known and
  not a failure.)
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [2026-11-25-cel-i-006-session-menu.md](docs/evidence/2026-11-25-cel-i-006-session-menu.md)

## VAL-CEL-I-003 — The brightness of every monitor

- **Status:** pending
- **Related implementation:** CEL-I-003 (the brightness popup's monitor list
  and the DDC worker's round-robin of writes)
- **Requires:** the deployed shell after the landing of CEL-I-003, started by
  hand in the author's niri session with the `osd` duty on, the `brightness`
  piece placed, DDC allowed (`CELESTINA_DDC` unset) and at least two monitors
  on DDC (the author's three); `ddcutil` installed
- **Procedure:** open the brightness popup from the bar of each output in turn
  (a click on the piece). (1) Check that the card shows the All heading with
  a percent, a slider, a divider and one row for each monitor: its model (or
  its connector when it has none), a slider and a percent, the name of this
  bar's own monitor first and in bold. (2) Drag one row's slider and check
  that only that monitor changes and the All heading's mean follows. (3) Drag
  the All slider slowly from one end to the other and check that all three
  monitors follow during the drag, one after another, and end level at the
  same value after the release. (4) If one monitor has no DDC, check that its
  row is grey with no percent and takes no input. (5) Close the popup and
  check that no brightness OSD appears while it is open or for six seconds
  after it closes. (6) While dragging All, run `pgrep -c ddcutil` in a
  terminal now and then and check that it never reads more than 1
- **Pass condition:** the list and the order are as described on each bar;
  one row changes one monitor; All moves every monitor with a reading and
  they end level; a monitor without DDC is greyed and inert; no OSD appears
  during the popup or its six-second settle; `ddcutil` never runs twice at
  once
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [2026-11-25-cel-i-003-brightness.md](docs/evidence/2026-11-25-cel-i-003-brightness.md)

## VAL-CEL-D-001 — Icons in the Island's tab row

- **Status:** pending
- **Related implementation:** CEL-D-001 (the Island's tab row)
- **Requires:** the deployed shell after the landing of CEL-D-001
- **Procedure:** open the Island (`celestina island open clock`). Check that
  the row at its foot shows nine equal pills with an icon each and no text.
  Open each tab from the row and check that its icon names its page, that the
  page title above matches, and that the open tab is plainly the selected one
- **Pass condition:** nine icon pills of equal width, none cut or overlapping;
  every icon says its page; the open tab stands out in the accent fill
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-D-001](docs/evidence/2026-11-25-cel-d-001-tab-icons.md)

## VAL-CEL-D-006 — The speaker button that mutes one application

- **Status:** pending
- **Related implementation:** CEL-D-006 (the applications section of the
  volume popup)
- **Requires:** the deployed shell after the landing of CEL-D-006, the
  `volume` piece in the bar, and an application playing sound
- **Procedure:** open the volume popup and unfold Aplicaciones. Press the
  speaker button at the right of the playing application, then press it again.
  Repeat with Tab to reach the button and Enter to press it
- **Pass condition:** the button shows a speaker while the application sounds
  and a struck-through speaker while it is muted; muting silences only that
  application and dims its name; the button lines up with the mute switches
  above it
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-D-006](docs/evidence/2026-11-25-cel-d-006-app-mute-button.md)

## VAL-CEL-I-007 — Brightness rows stay where they are released

- **Status:** pending
- **Related implementation:** CEL-I-007 (the brightness popup's monitor rows)
- **Requires:** the deployed shell after the landing of CEL-I-007 and at
  least two DDC monitors
- **Procedure:** open the brightness popup, drag the All slider quickly from
  one end to a middle value and release; then drag one monitor's row and
  release. Watch each row for the next ten seconds
- **Pass condition:** every row stays at the released value and never jumps
  to an earlier value of the drag; each monitor ends at that value
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-I-007](docs/evidence/2026-11-25-cel-i-007-brightness-hold.md)

## VAL-CEL-I-008 — The wheel on the popups' sliders

- **Status:** pending
- **Related implementation:** CEL-I-008 (the volume and brightness popups)
- **Requires:** the deployed shell after the landing of CEL-I-008
- **Procedure:** open the volume popup and turn the wheel one notch at a time
  over the output slider, the microphone slider and one application's slider;
  then open the brightness popup and do the same over All and over one
  monitor's row
- **Pass condition:** each notch moves the slider under the pointer by one
  percent (up raises, down lowers), the sound or the monitor follows, All
  moves every monitor together, and the wheel over the bar's own pieces still
  steps as before
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-I-008](docs/evidence/2026-11-25-cel-i-008-slider-wheel.md)

## VAL-CEL-I-009 — Brightness gaps after CEL-I-007 and CEL-I-008

- **Status:** pending
- **Related implementation:** CEL-I-009 (the brightness popup)
- **Requires:** the deployed shell after the landing of CEL-I-009; a session
  with one DDC monitor for the first check (or one output's monitor alone),
  two monitors for the second
- **Procedure:** (1) with the single-slider popup, drag the slider quickly and
  release; watch it for ten seconds. (2) Set two monitors to 100 % and 99 %,
  then turn the wheel one notch up over All
- **Pass condition:** (1) the slider stays where it was released; (2) both
  monitors end at 100 %
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-I-009](docs/evidence/2026-11-25-cel-i-009-brightness-gaps.md)

## VAL-CEL-I-010 — Removing notifications and clearing the list

- **Status:** pending
- **Related implementation:** CEL-I-010 (the Notifications page: a close
  button on every row, the clear-all bar, the history marked read when the page
  is seen)
- **Requires:** the deployed shell after the landing of CEL-I-010, with a few
  notifications live and a few ended (for example posted with `notify-send`)
- **Procedure:** (1) post a notification while the page is closed, so the bell
  shows its dot, then open the page. (2) Press the x of a finished row and of
  a live one (its banner, if on screen, goes too). (3) Move over and Tab to an
  x. (4) Press "Borrar todo" with a banner on screen. (5) With reduced motion
  on, remove a row
- **Pass condition:** (1) the dot is gone as the window opens and does not
  flash on again; (2) each row folds away and the rows below rise; (3) the x
  shows its hover plate and its focus ring; (4) every row and the banner go and
  the window narrows to its smallest width on a short spring, showing "Sin
  notificaciones"; (5) the row goes without sliding
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-I-010](docs/evidence/2026-11-26-cel-i-010-notifications.md)

## VAL-CEL-D-002 — The volume menu grows out of the bar's end capsule

- **Status:** pending
- **Related implementation:** CEL-D-002 (the capsule sheet and the volume menu
  as its first content;
  [design](../docs/superpowers/specs/2026-11-25-celestina-capsule-sheets-design.md),
  [plan](../docs/superpowers/plans/2026-11-25-celestina-capsule-sheets-plan.md))
- **Requires:** the deployed shell after the landing of CEL-D-002, the
  `volume` piece in the bar, an application playing sound, and a second
  output for the last step
- **Procedure:** press the volume piece. Look at the opening and at the bar's
  icons cut by the menu's edge. Press a cut icon. Unfold every section of the
  menu and scroll it. Turn the wheel over a slider. Drag a slider. Press
  Escape, then open the menu again and press outside it. Open the menu on one
  output, then on the second
- **Pass condition:** the end capsule grows into the menu with a small bounce
  and the row of icons stays where it was; icons cut by the menu's edge blur
  and fade toward the edge, and a cut icon still opens its menu; the
  spotlight CEL-D-002 first drew under the volume piece was replaced by the
  focus indicator in CEL-D-003 (see VAL-CEL-D-003); with every section
  unfolded the menu scrolls; the wheel over a slider moves the slider and does not scroll the
  menu; Escape and a press outside close it; dragging a slider shows no
  on-screen display; opening the menu on the second output closes the first
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-D-002](docs/evidence/2026-11-25-cel-d-002-capsule-sheets.md)

## VAL-CEL-D-003 — Every piece's menu grows out of its capsule

- **Status:** pending
- **Related implementation:** CEL-D-003 (the brightness, session and page
  menus of the bar's pieces as capsule sheets, the level and session
  `xdg_popup` paths removed;
  [design](../docs/superpowers/specs/2026-11-25-celestina-capsule-sheets-design.md),
  [plan](../docs/superpowers/plans/2026-11-25-celestina-capsule-sheets-plan.md))
- **Requires:** the deployed shell after the landing of CEL-D-003, the
  default bar (workspaces, notifications, sysmon, volume, brightness,
  clipboard, session), a monitor that answers DDC for the brightness piece,
  and a second output for the last step
- **Procedure:** press each piece in turn: brightness, session, sysmon,
  clipboard, the network and bluetooth pieces, the clock, the notifications
  bell. While one sheet is open, press another piece of the same capsule.
  Open the session list and press Lock, then press Restart and read the
  confirm card. Press the overflow button of a page sheet. With a sheet open
  on one capsule, press a piece of the other capsule. With a menu open, look
  at the other pieces of its capsule and at the open piece, then close the
  menu
- **Pass condition:** each piece opens its own sheet out of its capsule and
  the Island pill does not open; switching between two pieces of one capsule
  is one motion of the same panel; Lock locks the session at once and a
  dangerous row (Log out, Restart, Shut down) still asks first; the overflow
  button opens its menu over the sheet, under the button; with a menu open,
  the other pieces of its capsule dim, the open one keeps its grey pill and
  there is no light under it, and the pieces return to full opacity when the
  menu closes; a press on another capsule closes
  the open sheet, then opens that capsule's sheet; tray item menus and the
  Island's own menu still open as popups
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-D-003](docs/evidence/2026-11-25-cel-d-003-menus-as-sheets.md)

## VAL-CEL-D-004 — One centre capsule with time, date and weather

- **Status:** pending
- **Related implementation:** CEL-D-004 (the Island capsule shows the time,
  the date and the weather; a click on it opens the clock page; the clock
  and weather pieces leave the default centre;
  [design](../docs/superpowers/specs/2026-11-25-celestina-capsule-sheets-design.md),
  [plan](../docs/superpowers/plans/2026-11-25-celestina-capsule-sheets-plan.md))
- **Requires:** the deployed shell after the landing of CEL-D-004, the
  author's `~/.config/celestina/shell.json` with `"centre": []` (today
  `["weather","clock"]`), no `weather` or `clock` piece placed in any zone,
  and a location for the weather (the weather is read whenever a location is
  set)
- **Procedure:** set `"centre": []` in the shell's config and restart the
  shell. Read the centre capsule. Press the capsule once
- **Pass condition:** the centre reads `HH:MM`, the date and the weather (an
  icon and the temperature); a click opens the calendar (the clock page)
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-D-004](docs/evidence/2026-11-25-cel-d-004-centre-capsule.md)

## VAL-CEL-D-005 — The workspaces list

- **Status:** pending
- **Related implementation:** CEL-D-005 (a press on the workspaces piece
  opens a list sheet with one row per workspace of the output, its windows'
  icons and the current one marked; a click focuses a workspace and closes
  the list; the wheel over the piece steps workspaces, open or not;
  [design](../docs/superpowers/specs/2026-11-25-celestina-capsule-sheets-design.md),
  [plan](../docs/superpowers/plans/2026-11-25-celestina-capsule-sheets-plan.md))
- **Requires:** the deployed shell after the landing of CEL-D-005, the
  `workspaces` piece in the bar, and windows on at least two workspaces
- **Procedure:** press the workspaces piece at the start of the bar. Read the
  list against the workspaces and windows of that output. Click another row.
  Open the list again and turn the wheel over the piece; close it and turn
  the wheel once more
- **Pass condition:** the list matches the output's workspaces and windows
  (one row each, the icons of its windows, the empty-workspace label on an
  empty one, the current row in the soft accent with its border); a click
  on a row focuses that workspace and closes the list; the wheel moves the active
  workspace one step per notch, clamped at the first and last, with the list
  open or closed, and never closes the list
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-D-005](docs/evidence/2026-11-25-cel-d-005-workspaces-list.md)

## VAL-CEL-D-007 — The open piece keeps one pill and the others stay at full opacity

- **Status:** pending
- **Related implementation:** CEL-D-007 (with a capsule sheet open, the other
  pieces of its row are painted at their normal opacity, not dimmed; the open
  piece shows only the sliding indicator pill and never its own hover or
  press plate)
- **Requires:** the deployed shell after the landing of CEL-D-007 and a bar
  capsule with at least two pieces that open a sheet (the volume and
  brightness pieces)
- **Procedure:** press the volume piece. Read the other pieces of its row.
  Move the pointer over a neighbouring piece that opens a sheet and press it,
  then watch the switch. Leave the pointer on the open piece, then press it
  to close the sheet
- **Pass condition:** with a sheet open, the other pieces are at full opacity
  (the blurred edge where the sheet's window cuts the row is unchanged);
  switching to a hovered piece shows one pill travelling, never a fixed hover
  pill under it as well; hovering the open piece adds nothing; after the
  close the piece under the pointer shows its hover again
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-D-007](docs/evidence/2026-11-26-cel-d-007-focus-tweaks.md)

## VAL-CEL-D-008 — The capsule keeps its row and the open piece becomes the menu's subtitle

- **Status:** pending
- **Related implementation:** CEL-D-008 (a sheet is never narrower than its
  capsule, so every other piece of the row stays visible and clickable; the
  piece a sheet opens on leaves its capsule and the others close over its
  slot; its icon and text fly down into the menu's subtitle and fly back up
  to the slot when the sheet closes or switches)
- **Requires:** the deployed shell after the landing of CEL-D-008 and a bar
  capsule with at least three pieces, two of which open a sheet (the volume
  and the session pieces)
- **Note:** since CEL-D-014 the piece stays in its slot; the slot, subtitle-gesture
  and crossfade parts of this procedure no longer apply
- **Procedure:** press the volume piece and watch it. Read the row and the
  top of the sheet. Press the session piece in the same capsule, then press
  outside the sheet. Press the volume piece again, move the volume with the
  wheel while the sheet opens, and close it with Escape. With a sheet open,
  turn the wheel and right-click over its subtitle row (the workspaces sheet
  and the volume sheet), then click the subtitle. Open a sheet on the system
  load or the clock and watch the text. Open a sheet on the only piece of a
  capsule if the layout has one
- **Pass condition:** the volume icon and its percentage fly from the bar to
  the top of the sheet, growing and turning bolder, while the row closes over
  its slot with no gap; the sheet is never narrower than the row, and every
  other piece stays visible and answers a press; the subtitle shows the same
  icon and percentage and follows the wheel; the switch flies the old piece
  back up while the new one flies down, never painting a piece twice or
  leaving a gap; on close the slot reopens at once and the piece flies back
  and lands on it without a jump; a piece with no text shows the menu's name
  in the subtitle and flies only its icon; a sheet on the only piece of a
  capsule keeps a 40 px row; with reduced motion nothing flies and nothing
  slides; over the open sheet's subtitle the wheel steps the workspaces or the
  volume as it did over the piece, a right click mutes, and a click closes the
  sheet; the system load and the clock fly their text to the subtitle
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-D-008](docs/evidence/2026-11-26-cel-d-008-capsule-keeps-its-row.md)

## VAL-CEL-D-009 — Each capsule menu is its own window, lined up with its capsule

- **Status:** pending
- **Related implementation:** CEL-D-009 (a capsule's menu is a window of its
  own under the bar, sized to its content and lined up with the capsule's
  outer edge: the right edge for the end capsule, the left edge for the start
  capsule; it grows out of the open piece's pill and folds back into it; on a
  switch the old window folds while the new one grows)
- **Requires:** the deployed shell after the landing of CEL-D-009 and a layout
  whose end capsule holds the energy (power profile), Bluetooth, brightness
  and system load pieces, and whose start capsule holds the notifications piece
- **Procedure:** open the energy menu, then the Bluetooth, brightness and
  system load menus one by one, and read each window's edges and the space
  under its last row. Open a menu on the start capsule (notifications). Open
  a menu and watch it grow out of its piece. Switch to another piece of the
  same capsule while a menu is open. Close the menu by pressing outside it
- **Pass condition:** every window sits under the bar with its right edge on
  the end capsule's right edge (the left edge on the left edge for the start
  capsule), is as wide and as tall as its content needs (the energy window
  clearly narrower than the system load one, with no empty area under its
  rows), grows out of the open piece's pill on the spring and folds back into
  the piece's slot on close; during a switch exactly two windows show for
  about 150 ms (the old one folding, the new one growing) and one afterwards
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-D-009](docs/evidence/2026-11-26-cel-d-009-menu-windows.md)

## VAL-CEL-I-011 — Tray icons resolve and tray menus open

- **Status:** pending
- **Related implementation:** CEL-I-011 (the tray's icons and menus: the
  item's own icon directory, a fallback glyph, `ContextMenu` for an item
  with no dbusmenu or a failed layout, `ItemIsMenu` on the left click, a menu
  that fills late)
- **Requires:** the deployed shell after the landing of CEL-I-011, with Steam,
  Discord, nm-applet, blueman, solaar and shelly running, and none of them
  listed in `bar.tray_hidden`
- **Procedure:** (1) look at the six icons in the bar; none may be blank, and
  an app whose icon the shell cannot find shows the grey window glyph.
  (2) Right click each one. (3) Left click the ones that are only a menu
  (nm-applet and blueman usually are). (4) Open the menu of one, close it
  with Escape, open it again. (5) Install a new icon theme, restart nothing,
  and note whether an app that changes its icon picks it up
- **Pass condition:** (1) every item shows an icon or the grey glyph, never
  an empty slot. (2) each right click shows a menu under the pointer: the
  shell's own for apps with a dbusmenu, the app's for Steam and Discord. (3)
  the left click shows the menu and does not raise a window. (4) the menu
  opens and closes each time, with no second menu beside it. (5) a theme
  installed after the shell started is not seen until the shell restarts: that
  is the known limit (the `freedesktop-icons` cache), not a failure
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-I-011](docs/evidence/2026-11-26-cel-i-011-tray-fixes.md)

## VAL-CEL-I-017 — The launcher scrolls

- **Status:** pending
- **Related implementation:** CEL-I-017 (the launcher lists every answer;
  the wheel scrolls the list; a keyboard move keeps the selected row in view;
  a thin bar shows while it moves)
- **Requires:** the deployed shell after the landing of CEL-I-017
- **Procedure:** open the launcher with an empty field; turn the wheel over
  the list; hold Down for two seconds; type one letter
- **Pass condition:** the list scrolls smoothly past the first eight
  applications and a thin bar shows at its right edge, then fades; the
  selected row and its focus ring stay inside the card while Down repeats;
  typing brings the list back to its top
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-I-017](docs/evidence/2026-11-27-cel-i-017-scrolling.md)

## VAL-CEL-I-016 — The launcher's calculator

- **Status:** pending
- **Related implementation:** CEL-I-016 (a typed expression shows its result
  as the launcher's first row; Enter copies it as shown, marks it copied and
  closes the launcher 350 ms later)
- **Requires:** the deployed shell after the landing of CEL-I-016, with the
  clipboard duty on
- **Procedure:** open the launcher; type `(3+4)^2`; press Enter; paste into
  any text field; open the launcher again and type `sqrt(2)/2`, then `2+`
- **Pass condition:** the first row reads `= 49` and Enter shows the copied
  mark before the launcher closes; the paste is `49`; `sqrt(2)/2` shows
  `0,707106781187`; `2+` shows no calculator row
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-I-016](docs/evidence/2026-11-27-cel-i-016-calculator.md)

## VAL-CEL-I-015 — The launcher's search

- **Status:** pending
- **Related implementation:** CEL-I-015 (the launcher reads the Spanish
  names and keywords of the desktop files, matches words in any order, one
  typo and Spanish category words, and orders the list by how often and how
  recently each application was opened)
- **Requires:** the deployed shell after the landing of CEL-I-015, with the
  author's Flatpak applications installed
- **Procedure:** open the launcher; type the Spanish word for browser, then
  a word of an application's name with two letters swapped, then two words
  of an application's generic name in reverse order; open an application a
  few times and reopen the launcher with an empty field
- **Pass condition:** the browser word lists every installed browser (Zen,
  Floorp, LibreWolf); the typo still finds the application; the reversed
  words find it; the application opened most moves to the top of the empty
  list
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-I-015](docs/evidence/2026-11-27-cel-i-015-search.md)

## VAL-CEL-I-014 — The launcher's quick clear

- **Status:** pending
- **Related implementation:** CEL-I-014 (holding Backspace for 600 ms sweeps
  the launcher's query and empties it; a clear button sits in the field while
  it holds text; Ctrl+Backspace and Ctrl+Delete delete a word in every field)
- **Requires:** the deployed shell after the landing of CEL-I-014
- **Procedure:** open the launcher; type a word and press the clear button;
  type two words and press Ctrl+Backspace; type a word and hold Backspace for
  a second; type again, hold Backspace and let go before the field empties
- **Pass condition:** the button shows only with text and empties the field;
  Ctrl+Backspace removes the last word; the held Backspace deletes one letter,
  then a blue wash sweeps the text and the field empties; letting go early
  keeps the text
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-I-014](docs/evidence/2026-11-27-cel-i-014-quick-clear.md)

## VAL-CEL-I-013 — The chevron's plate and its window's place

- **Status:** pending
- **Related implementation:** CEL-I-013 (the chevron's glyph sits and turns
  in the middle of its 32 px plate; the hidden-apps window opens centred
  under the chevron, 8 px under the bar, 16 px from the screen's sides)
- **Requires:** the deployed shell after the landing of CEL-I-013, with at
  least one tray app hidden and pieces after the tray in its capsule
- **Procedure:** hover the chevron; press it and watch the glyph turn; look
  where the window opens; close it; repeat on the output's right end if a
  layout puts the tray there
- **Pass condition:** the glyph sits in the middle of the hover and open
  plate, which matches a tray icon's; it turns about its own centre; the
  window opens under the chevron, centred on it, and stays 16 px inside the
  screen
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-I-013](docs/evidence/2026-11-27-cel-i-013-chevron-fixes.md)

## VAL-CEL-I-012 — The collapsible tray

- **Status:** pending
- **Related implementation:** CEL-I-012 (a chevron at the tray's left end opens
  a 240 px window of the hidden apps, lined up with the right capsule's right
  edge; every app's menu ends with an entry that hides it from the bar or shows
  it again; the 7 px attention dot on the chevron)
- **Requires:** the deployed shell after the landing of CEL-I-012, with Steam,
  Discord, nm-applet, blueman and solaar running, a bar whose end zone keeps
  the `tray` piece, and two outputs for step (7)
- **Procedure:** (1) with the default layout, look at the bar: nm-applet and
  blueman are hidden, so the chevron shows. (2) Press the chevron: the window
  grows under it, the glyph turns and the chevron stays lit; press it again or
  press Escape. (3) Right click a bar icon (Steam): its menu ends with a line
  and "Ocultar de la barra"; choose it. (4) Open the window: Steam is there.
  Right click its row: the menu ends with "Mostrar en la barra"; choose it.
  (5) Make an app that is hidden ask for attention (a Discord message, or
  nm-applet with a pending connection): the 7 px dot shows on the chevron and
  the row carries the attention label. (6) Open the menu of a row and press
  Escape: the menu closes first, the window stays; press Escape again. (7) On
  two outputs press the chevron of one: the window opens on that output only;
  hide an app there and look at the other bar. (8) Show every hidden app: the
  window and the chevron go. (9) Repeat steps (2) and (3) with reduced motion
  on
- **Pass condition:** (1) the chevron shows while an app is hidden and goes
  when none is. (2) the window's right edge matches the capsule's
  right edge, its top sits 8 px under the bar, the glyph turns on a spring and
  the box stays lit while it is open. (3) the icon leaves its slot on the
  slot spring and the neighbours close up; the menu of an app with no dbusmenu
  is a card holding only that entry. (4) the row leaves the window and the
  icon enters the bar. (5) the dot shows only while a hidden app asks for
  attention, and the row's attention label shows whole (a long title ends
  in an ellipsis). (6) Escape closes the menu level
  first. (7) the window lives on the pressed output, and the other bar follows
  the new list. (8) the last show closes the window and folds the chevron out.
  (9) with reduced motion the glyph flips and nothing springs
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-I-012](docs/evidence/2026-11-26-cel-i-012-collapsible-tray.md)

## VAL-CEL-D-010 — A menu window blurs out towards its piece and the piece crossfades

- **Status:** pending
- **Related implementation:** CEL-D-010 (a closing menu window blurs, fades and
  shrinks towards its piece in 260 ms; the open piece no longer flies: it fades
  out of the bar, blurred and sinking, when its menu opens, and fades back in
  from the bar's bottom edge from the first frame of the close; the subtitle is
  never hidden)
- **Requires:** the deployed shell after the landing of CEL-D-010 and a layout
  whose end capsule holds the energy (power profile) and system load pieces
- **Note:** since CEL-D-014 the piece stays in its slot; the slot, subtitle-gesture
  and crossfade parts of this procedure no longer apply
- **Procedure:** open the energy menu and close it by pressing outside it;
  open the system load menu and close it with Escape; with one of them open,
  press another piece of the same capsule (a switch); open and close a menu
  again quickly; repeat once with reduced motion on
- **Pass condition:** on close the window blurs out and shrinks towards its
  piece while the piece fades in at its place from the bar's bottom edge, with
  no step out of order (the piece is not back after the window has gone, and
  nothing flies between the window and the bar); on open the piece fades out
  of the bar blurred and sinking while its slot closes; on a switch the old
  window blurs out while the new one grows and the old piece fades back in;
  Escape closes the same way; with reduced motion nothing blurs or moves and
  the opacities still fade; at rest nothing moves
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-D-010](docs/evidence/2026-11-26-cel-d-010-menu-close.md)

## VAL-CEL-D-011 — Notifications and the level display come out of the Island

- **Status:** pending
- **Related implementation:** CEL-D-011 (a notification is the Island growing
  into a notice; older live ones wait as sheets under it and fan out under the
  pointer; right click or wheel up closes a notice, middle click closes all;
  `osd.position: "island"` shows the level inside the Island; an open Island
  page shows the newest notice as a card at its top)
- **Requires:** the deployed shell after the landing of CEL-D-011 on the
  author's session with three monitors; a copy of `shell.json` to restore
- **Procedure:** (1) In `~/.config/celestina/shell.json` set the OSD block to
  `"osd": { "position": "island", "timeout_ms": 1500, ... }`, keeping its
  other keys (`kinds` and the rest) as they are. (2) With the Island at rest, send `notify-send -A reply=Reply -A
  archive=Archive "Title" "A body long enough to take two lines"` and look at
  the Island grow into the notice; leave it to expire and look at it shrink back
  into the clock. (3) Send three notifications in a row: the newest is the
  notice with two thin sheets under it; move the pointer over it and the
  sheets fan out as full notices; move away and wait: they fold back. (4)
  With the fan open, right click one notice, then wheel up over another, then
  middle click: each closes what it points at, the last closes every one.
  (5) Turn the volume up three steps with the keys: the Island shows the level
  (icon, track, percentage) and goes back to the clock after the timeout; send a
  notification during it and check that the notice waits until the level has
  gone. (6) Click the Island to open the Clock page and send a notification: it
  shows as a card at the top of the page, the page grows by its height, and its
  button and close button work; close the page with the notification still
  live: the Island grows into the notice. (7) Move the focus to another
  monitor and send a notification: it grows on the focused monitor's Island and
  the other monitors keep their clock. (8) Open a fullscreen window (a video),
  send a notification and check that it shows over it. (9) Repeat (2) and (3)
  with reduced motion on. (10) Set `"position": "bottom"` again and check that
  the level display is back at the bottom of the screen
- **Pass condition:** the Island grows from its own place and the notice
  settles at the top (y 8) with no second capsule behind it; the clock is back
  at its place after the shrink with no jump; sheets, fan, right click, wheel
  up and middle click behave as described; the level shows in the Island and a
  notice waits for it; the card sits at the top of the page without overflow;
  the notice follows the focused monitor only; it shows over a fullscreen
  window; with reduced motion nothing blurs or moves and the faces still fade;
  at rest nothing moves and no frame is drawn
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-D-011](docs/evidence/2026-11-26-cel-d-011-island-notices.md)

## VAL-CEL-I-018 — A held key repeats

- **Status:** pending
- **Related implementation:** CEL-I-018 (a held key repeats at the
  compositor's delay and rate in text fields, sliders, the launcher's rows
  and the menus' Up and Down; keys that act on their press act once; polkit's
  and the session lock's password fields do not repeat)
- **Requires:** the deployed shell after the landing of CEL-I-018
- **Procedure:** (1) open the launcher and hold a letter, then tap and hold
  Backspace. (2) Hold Down, then Up, over the launcher's rows. (3) Focus a
  slider in a menu window (volume or brightness) with Tab and hold an arrow.
  (4) In a bar menu, hold Down. (5) Hold Return on a launcher row; hold
  Escape with a menu open over a menu window; in the overview hold Tab
- **Pass condition:** (1) after the usual delay the letter keeps typing at
  the rate of the other applications; a tap deletes one letter and a hold
  sweeps the query empty as before. (2) the selection keeps walking and wraps
  at the ends. (3) the slider keeps stepping. (4) the focus keeps moving down
  the entries. (5) Return launches the application once; Escape closes one
  level only; the overview switches once
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-I-018](docs/evidence/2026-11-27-cel-i-018-key-repeat.md)

## VAL-CEL-D-012 — The launcher's tabs and grid

- **Status:** pending
- **Related implementation:** CEL-D-012 (the launcher opens on Recents,
  offers All and a tab per group of applications, and shows any tab or
  search as a list or a six-column grid whose choice it remembers)
- **Requires:** the deployed shell after the landing of CEL-D-012
- **Procedure:** (1) open the launcher; (2) click All, then press Ctrl+Tab
  twice and Ctrl+Shift+Tab once, then Alt+5; (3) type a word while a group is
  open, then click that group's tab; (4) press Ctrl+G, move with the four
  arrows with the field empty, then type a letter and press Left; (5) close
  and reopen the launcher; (6) click the view button
- **Pass condition:** (1) it opens on Recents with the applications used
  most first, and the open tab shows its label; (2) each switch shows its
  tab's applications from the top, the label moving to the open tab, and
  Alt+5 opens the fifth tab; (3) the tab row dims while the word is typed and
  the click empties the field and shows the group; (4) a six-column grid
  shows, the arrows move the selection by a cell and by a line, and with text
  Left moves the caret; (5) it reopens on Recents in the grid; (6) it is back
  to the list, and the card never changes size
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-D-012](docs/evidence/2026-11-27-cel-d-012-tabs-grid.md)

## VAL-CEL-D-014 — The open piece stays in its slot

- **Status:** pending
- **Related implementation:** CEL-D-014 (the open piece stays in its slot with
  the grey pill and its content at 80 %; the pill slides on a switch and fades
  on the close; no hover fill on the open piece; its gestures are its own; the
  window's header row is a title)
- **Requires:** the deployed shell after the landing of CEL-D-014 and a layout
  whose end capsule holds volume, brightness and the tray
- **Procedure:** (1) open the volume menu, hover and press the open piece, and
  turn the wheel on it; (2) switch to brightness; (3) right-click the volume
  piece; (4) open the chevron's window; (5) close with Escape and by pressing
  outside; (6) repeat once with reduced motion
- **Pass condition:** the bar never shows a gap; the pill sits behind the open
  piece and slides to the new one; no darker plate appears on hover; a click
  on the open piece closes the menu; the pill fades and the piece is at full
  opacity once the window has gone; with reduced motion everything changes at
  once; at rest nothing moves
- **Result:** not run
- **Evidence:** none; the unit's automated evidence is
  [CEL-D-014](docs/evidence/2026-11-27-cel-d-014-piece-in-slot.md)
