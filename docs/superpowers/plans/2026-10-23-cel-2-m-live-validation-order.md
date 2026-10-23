# CEL-2-M — The live validations, in the order the author runs them

- **Date:** 2026-10-23
- **Design:** [the cutover design](../specs/2026-10-23-celestina-cel-2-m-cutover-design.md), §4 (the order) and §6 (recovery)
- **Source of every row:** `celestina/VALIDATION.md` on `main` at `a5427f7a`,
  plus the rows the branches `CEL-2-H-b`, `CEL-2-I` and `CEL-2-L` add. The
  row's own procedure is the authority; this list only orders them, names the
  one exact step that makes each row runnable, its risk, and how to undo it.
- **Who runs them:** the author, by hand, on the deployed shell
  (`~/.local/bin/celestina`), with Noctalia running unless the row says
  otherwise. No agent runs, closes or edits a row.
- **Before the first row:** `cp ~/.config/niri/config.kdl ~/.config/niri/config.kdl.pre-celestina-cutover`;
  start the shell by hand from a terminal (or `sh celestina/scripts/activate-production.sh`
  so the journal gets the log); confirm `journalctl --user -t celestina -f`
  shows it. **Blocks M** means the swap (design §4 step 12) must not happen
  before the row passed; **no** means the row can pass after the swap.

Legend of the undo column: **file** = set the duty back to `"off"` in
`~/.config/celestina/shell.json` and switch Noctalia's side on again; the shell
follows the file, no restart. **kdl** = restore the one line in `config.kdl`
(niri reloads live) or copy the backup back.

## Phase 0 — nothing switched: the Island as it is

| # | Row | Blocks M | Exact step | Risk | Undo |
|---|---|---|---|---|---|
| 1 | VAL-CEL-ISLAND | no | start with `celestina --metrics` on DP-1; click/Escape/`island toggle` ten times each; `celestina quit`, read the `DP-1` line | none | `celestina quit` |
| 2 | VAL-CEL-REST | no | `celestina --metrics`, idle five minutes, `celestina quit`; `rest_frames` ≤ 1/output/min, `rss_mib` < 100 | none | same |
| 3 | VAL-CEL-SCALE | no | compare the clock and the Activity page on DP-1 (1.5) and HDMI-A-1 (1.0); then `text_scale = "large"` in `appearance.toml` | none | revert `appearance.toml` |
| 4 | VAL-CEL-POPUP | no | the overflow menu and a tray item's nested DBusMenu, Escape, click outside, choose (Noctalia running for the dimmed do-not-disturb row) | none | — |
| 5 | VAL-CEL-KEYBOARD (first half) | **yes** (the launcher is what `Mod+Space` becomes) | keyboard-only on the pages and the launcher opened by `celestina launcher toggle` from a temporary bind; a multi-line and a > 64 KiB paste | none | remove the temporary bind |
| 6 | VAL-CEL-IME | **yes** (same key) | composed characters and a dead key in the launcher's field | none | — |
| 7 | VAL-CEL-NOCTALIA (first half, "beside") | no | Notifications and Tray pages name Noctalia as the owner, switch disabled | none | — |

## Phase 1 — the bar (design step 1)

| # | Row | Blocks M | Exact step | Risk | Undo |
|---|---|---|---|---|---|
| 8 | VAL-CEL2-BAR-LIVE | **yes** | `noctalia msg bar-hide`; `"duties": {"bar": "on"}` in `shell.json`; checks (1)–(5) of the row on two scales | two bars stacked if `bar-hide` is forgotten; clicks swallowed by a wrong input region (the row's check 4) | `noctalia msg bar-show`; `"bar": "off"` (file) |
| 9 | VAL-CEL2-GLANCE-TABS | no | open the Island, read the five (now more) tabs at both scales | cosmetic | — |
| 10 | VAL-CEL2-NETWORK | no | place `network` in `bar.end`; compare with `nm-applet` and `nmcli`; join a saved network from the page; `Editar…` opens `nm-connection-editor` | a join the author did not want; no secret passes (ADR 0019) | remove the piece from the file |
| 11 | VAL-CEL2-BLUETOOTH | no | place `bluetooth`; compare with `bluetoothctl show`; connect and disconnect one paired device from the page | a device connected or disconnected; never a scan, pairing or removal | remove the piece |
| 12 | VAL-CEL2-POWER | no | place `battery` and `power-profile`; compare with `upower -i` and `powerprofilesctl`; change the profile from the page and by middle click | a profile change (reversible from the same page) | remove the pieces; `powerprofilesctl set <previous>` |
| 13 | VAL-CEL2-SYSMON | no | place `sysmon`; `celestina island open sysmon`; load the CPU past 80 % and 90 %; remove the piece and check `top -H -p $(pidof celestina)` shows no sampler | none | remove the piece |
| 14 | VAL-CEL2-CALENDAR | no | place `clock`; `celestina island open clock`; the grid, Previous/Next/Today; the day change | none | remove the piece |
| 15 | VAL-CEL2-WEATHER | no | place `weather` with **no** `weather` key: `ss -tnp` shows no connection; then add a manual location; block the network and wait 15 min | one `curl` to Open-Meteo per refresh, only with a location (ADR 0016) | remove the key and the piece |

## Phase 2 — OSD and media (steps 2–3)

| # | Row | Blocks M | Exact step | Risk | Undo |
|---|---|---|---|---|---|
| 16 | VAL-CEL2-OSD-AUDIO | **yes** (daily feedback the swap removes from Noctalia) | `noctalia msg osd-disable`; `"osd": "on"` before starting the shell; the `wpctl` keys, mute, a `pavucontrol` change, the microphone key, the five verbs; `niri msg layers` shows `celestina-osd` only while shown | two OSDs if Noctalia's is not off | `noctalia msg osd-enable`; `"osd": "off"` (file) |
| 17 | VAL-CEL2-OSD-BRIGHTNESS | **yes** | Noctalia's OSD off **and its brightness widget closed**, no other `ddcutil`; `brightness` placed; watch `pgrep -a ddcutil` and the journal for one `detect` at start; `celestina brightness up` once, then a burst, `set 40 DP-1` | **GPU loss, flicker or freeze** if two `ddcutil` share the bus (happened twice in the halted shell): stop at the first black flash | `"osd": "off"`; remove the piece; if the GPU is lost, the monitor's own buttons and a VT |
| 18 | VAL-CEL2-OSD-PIECES | no | `volume` and `brightness` placed; scroll, click, middle click, the slider by pointer and keyboard; no OSD for a popup-driven change | a volume jump from a scroll | remove the pieces |
| 19 | VAL-CEL2-MEDIA | no | Noctalia's media widget off; `"media": "on"`; a browser and mpv playing; the piece, the Medios page, the three buttons, the pin; the keys still run `playerctl` | none | `"media": "off"` (file) |
| 20 | VAL-CEL2-MEDIA-COVER | no | a player with a file cover, then an `https` cover (none shown, nothing fetched) | none | — |

## Phase 3 — wallpaper and energy (steps 4–6)

| # | Row | Blocks M | Exact step | Risk | Undo |
|---|---|---|---|---|---|
| 21 | VAL-CEL2-WALLPAPER | **yes** | Noctalia's wallpaper off (its settings); `"wallpaper": "on"` and `"wallpaper": {"folder": …}`; `celestina wallpaper next DP-1`, `mode fit|stretch|center`, hotplug, restart; then Noctalia's wallpaper on again and `celestina wallpaper retry` shows the refusal | a black or doubled background for a moment; nothing destructive | Noctalia's wallpaper on; `"wallpaper": "off"` (file) |
| 22 | VAL-CEL2-CAFFEINE | **yes** (replaces startup line 73) | `noctalia msg caffeine-disable`; `"caffeine": "on"`, `"energy": {"caffeine": true}`; `celestina caffeine on`, `systemd-inhibit --list` shows a `Celestina` `idle:sleep` block; toggle; `celestina quit` leaves no lock. **Backup `config.kdl` first**, then delete line 73 | the session may idle-suspend (through Noctalia's idle, still on) while caffeine is off between the two: run with Noctalia's idle disabled or keep the mouse moving | `noctalia msg caffeine-enable`; `"caffeine": "off"`; restore line 73 (kdl) |
| 23 | VAL-CEL2-NIGHTLIGHT | **yes** (same line) | `noctalia msg nightlight-disable` and `nightlight-force-toggle` (undo the forced mode); `"nightlight": "on"`, `"energy": {"night_light": true}`; three outputs turn warm; `celestina nightlight toggle`; hotplug; `night_light_kelvin`; `celestina quit` returns neutral | a neutral or doubly warm screen for a moment; `wlr-gamma-control` is per client, the compositor arbitrates | `noctalia msg nightlight-force-toggle`; `"nightlight": "off"` |
| 24 | VAL-CEL2-SESSION | no | `celestina session suspend` is refused with `no locker yet` (while `lock` is off); `busctl … CanReboot`; **never** run `reboot` or `power-off` as a check | `log-out` ends the session (niri confirms first): skip it unless ready | — |
| 25 | VAL-CEL2-IDLE | **yes** (OLED) | Noctalia's `screen-off` **and** `lock-and-suspend` disabled in its idle settings; caffeine off; `"idle": "on"`; hands off: outputs off at 20 s, wake on input; past 240 s nothing suspends and the journal says `refused` once; `"idle": "off"` stops it | a suspend if Noctalia's idle is still on (both chains armed); the OLED stays lit if neither is on and the author walks away | Noctalia's idle on; `"idle": "off"` (file) |

## Phase 4 — clipboard, phone, bubbles (steps 7–8)

| # | Row | Blocks M | Exact step | Risk | Undo |
|---|---|---|---|---|---|
| 26 | VAL-CEL2-CLIPBOARD-LIVE | no (the author decides whether the duty is part of the cutover) | Noctalia's clipboard widget off; `"clipboard": "on"`; copy in a browser and a terminal; copy from the password manager (never listed); select, Delete, pause/resume, clear | a text the author copies is written to `~/.local/state/celestina/clipboard.json` (0600) until the duty goes off | `"clipboard": "off"` deletes the file |
| 27 | VAL-CEL2-CLIPBOARD-SECRET | **yes if `clipboard` is in the cutover** | `stat -c '%a'` on the file (600) and the directory (700); off → file gone; `journalctl --user -b | grep -F <string>` finds nothing | none beyond the above | same |
| 28 | VAL-CEL2-CLIPBOARD-IMAGE | no | a screenshot copy round trip; no image bytes under `~/.local/state/celestina/` | none | same |
| 29 | VAL-CEL2-CLIPBOARD-LOOK | no | the six tabs at four text scales, the "N more" line, a transparent thumbnail | cosmetic | — |
| 30 | VAL-CEL2-PHONE | no | Noctalia's `tony/magnetita` plugin may stay on for this row; `"phone": "on"`, `phone` placed; the Phone page vs the phone; plug/unplug; `phone media` verbs; stop Magnetita; duty off | none | `"phone": "off"` |
| 31 | VAL-CEL2-BUBBLES | no | `"bubbles": "on"`, `bubbles` placed; a temporary bind to `celestina bubbles minimize`; minimise past the group's size (`+N`); restore; reduced motion; stop Melibea | a window minimised the author must restore; Melibea stopped by hand | `"bubbles": "off"`; `systemctl --user start melibea.service` |
| 32 | VAL-CEL2-BUBBLES-KEYS | **yes if `Mod+K` is rebound in M** | `Mod+K` → `celestina bubbles select` (kdl edit 2); filter, arrows, Enter, Delete (asks the window to close), Escape, click outside, focus change | Delete closes a real window (asks, not kills) | restore the `Mod+K` line (kdl) |

## Phase 5 — the session lock (step 9; privileged, root needed once)

Run in this order; each needs the one before it. The recovery row comes
**before** suspend, hotplug and notices on purpose: the author must have
killed a lock by pid and taken over with `swaylock` once before trusting the
lock across a suspend.

| # | Row | Blocks M | Exact step | Risk | Undo |
|---|---|---|---|---|---|
| 33 | VAL-CEL2-LOCK-PAM | **yes** (the lock's VAL rows gate M-3 and the `Mod+Shift+Escape` rebind) | read `celestina/contrib/pam.d/celestina-lock` (four lines); `sudo install -m 0644 -o root -g root … /etc/pam.d/celestina-lock`; `pamtester celestina-lock <user> authenticate` with the right password **once** | `pam_faillock`: three wrong passwords in 15 min lock the account everywhere for 10 min; a wrong file could refuse every unlock (F2 fails locked) | `sudo rm /etc/pam.d/celestina-lock`; `faillock --user <name> --reset` as root |
| 34 | VAL-CEL2-LOCK-RECOVERY | **yes** | lock with `celestina lock`; `Ctrl+Alt+F3`; `cat $XDG_RUNTIME_DIR/celestina/lock-wayland-1.state`; `kill -KILL <pid>` (that pid only); the shell relaunches; stop the shell, kill again, `WAYLAND_DISPLAY=… swaylock -c 000000` from the VT and unlock; **write down the root path** | a dead lock with no shell: `swaylock` from the VT is the way in; without a VT login or root, locked out | the row is the undo rehearsal itself |
| 35 | VAL-CEL2-LOCK-LIVE | **yes** | `"lock": "on"`; Noctalia's idle lock already off (phase 3); `swaylock` still on `Mod+Shift+A`; `celestina lock`; the right password; one wrong password (2 s, then 4 s; **never a third**); Caps Lock; accents and dead keys; contrast on both scales; the 30 s fade and its recovery | a lockout by `faillock`; TM-15 (other clients see keys while locked) is an accepted residual, not a failure | `"lock": "off"` never touches a running lock: unlock with the password, or row 34 |
| 36 | VAL-CEL2-LOCK-COEXIST | no | lock with Noctalia's session menu, then `celestina lock` is refused; the reverse order | none (the protocol allows one lock) | unlock with the password |
| 37 | VAL-CEL2-LOCK-NOTICES | no | with `swaylock` holding the session, `celestina lock suspend` notifies a refusal and nothing suspends; `mv` the PAM file away as root, duty off and on, the missing-service notice; put it back | a minute with no PAM service: every attempt denied (fails locked) | `mv` the file back |
| 38 | VAL-CEL2-LOCK-SUSPEND | **yes** (the `Mod+Shift+Escape` bind becomes `lock suspend`) | `"idle": "on"` too; hands off 240 s: lock, then suspend, the lock up on resume; then lid, power button, `systemctl suspend` one at a time; time any glimpse of the session (F24, logind's inhibitor cap: a finding, recorded) | a resume showing the session for a moment; an unlock that fails after resume → row 34 | `"idle": "off"`, `"lock": "off"` |
| 39 | VAL-CEL2-LOCK-HOTPLUG | no | plug and unplug outputs while locked, the prompt follows | none beyond a locked session | unlock |
| 40 | VAL-CEL2-LOCK-BACKDROP (H-b) | no (gates M-3's `lock` default only if H-b landed) | `wallpaper` on with a bright picture; `celestina lock`: dimmed, blurred picture per output, the field readable; hotplug; duty off → black | none | unlock |
| 41 | VAL-CEL2-LOCK-CLOCK (H-b) | no | the clock and date vs the system's; across a minute; after a suspend; the no-`date +%z` case **in a nest only** | none | unlock |
| 42 | `Mod+Shift+Escape` → `celestina lock suspend` (kdl edit 3; not a row, the bind itself) | **yes** | press it once with no unsaved work: lock, then suspend, the lock on resume | the first real use of the whole chain from a key | restore the line (kdl); `Mod+Shift+A` all along |

## Phase 6 — polkit (step 10; privileged)

| # | Row | Blocks M | Exact step | Risk | Undo |
|---|---|---|---|---|---|
| 43 | VAL-CEL2-POLKIT-LIVE (I branch) | **yes** (program §5: I before M) | `polkit_agent = false` in Noctalia's settings, `noctalia msg config-reload`; `"polkit": "on"`; `pkexec /usr/bin/true`: the card, the right password, exit 0; one wrong password; Escape dismisses; a second exclusive-keyboard client cancels within 2 s; `systemctl restart polkit` tears the request down and a new one prompts; `celestina quit` → `pkexec` fails | a prompt takes the keyboard exclusively: not during work you cannot lose focus in; `faillock` again (`polkit-1` shares it) | `"polkit": "off"` (cancels, unregisters); `polkit_agent = true` and `config-reload`; `sudo` works throughout |

## Phase 7 — the launcher key and CEL-2-L (step 11)

| # | Row | Blocks M | Exact step | Risk | Undo |
|---|---|---|---|---|---|
| 44 | `Mod+Space` → `celestina launcher toggle` (kdl edit 4; rows 5–6 already passed) | **yes** | rebind; open, type, launch, Escape, click outside | the most-used key: if the launcher fails, no launcher until the line is restored | restore the line (kdl) |
| 45 | VAL-CEL2-OVERVIEW (L branch) | no | **in a nest first** (`dev-session.sh --own-bus`); then a bind to `celestina overview toggle`; blocked windows black in the preview; one capture per opening | a screen capture of the output: on demand only, never kept | remove the bind |
| 46 | VAL-CEL2-SHORTCUTS (L branch) | no | **in a nest first**; a bind to `celestina shortcuts toggle`; the sheet vs `config.kdl` bind for bind; add a bind, reopen | the config is read in a sandboxed child; none | remove the bind |

## Phase 8 — the swap (step 12) and the rows it unblocks

| # | Row | Blocks M | Exact step | Risk | Undo |
|---|---|---|---|---|---|
| 47 | the swap (not a row) | — | line 71 → `spawn-at-startup "sh" "-c" "exec systemd-cat -t celestina celestina"` (or the bare word); log out (`celestina session log-out`, niri confirms) and in; `busctl --user list | grep -e Notifications -e StatusNotifier` shows the shell's unique name; `niri msg layers` shows no `noctalia-*` | no shell at login if the line is wrong: a bare desktop with the binds still live | design §6 (2): copy the backup back, `noctalia &`, under a minute |
| 48 | VAL-CEL-NOCTALIA (second half, "without it") | after | a notification, the tray items of `nm-applet`, Blueman, Solaar, a pixmap-only item; Steam or Dropbox if at hand | none | — |
| 49 | VAL-CEL-BANNERS | after | `notify-send -u low|normal|critical`, an action, four at once, hover pauses, dismiss, do-not-disturb | none | — |
| 50 | VAL-CEL-GLASS | after | the Haze material on the capsule, a page, a banner, a popup over a busy window | none | — |
| 51 | VAL-CEL-POPUP (the do-not-disturb row now enabled) | after | the overflow menu's do-not-disturb row flips and survives a restart | none | — |

## Summary

**Blocks the swap** (must pass first): rows 5, 6, 8, 16, 17, 21, 22, 23, 25,
33, 34, 35, 38, 42, 43, 44, and 27 and 32 if `clipboard` and `Mod+K` are in
the cutover. Everything else can pass after, and rows 48–51 can only pass
after. Rows 40–41 and 45–46 exist on branches that have not landed; they
become runnable when they do.

**Needs root once:** row 33 (and the undo of 37). **Needs a VT login that
works:** row 34, which is the rehearsal of the way back in. **Can lock the
account for 10 minutes:** rows 33, 35, 43 (`pam_faillock`, shared with `sudo`
and VT login): never a third wrong password in 15 minutes.

**The one irreversible-looking step** is 47, and its undo is one `cp` and one
`noctalia &`.
