# Handover: from Noctalia to Celestina

- **Written:** 2026-10-16, by CEL-1-H; rewritten 2026-11-23 by CEL-2-M, the
  cutover, when every duty Noctalia performed had a delivery in this shell
- **Purpose:** what Noctalia did for the author's niri session, what does it
  now, how the session is switched over and how it is switched back. It
  records state and intent; it grants no authority and schedules nothing.
- **Sources:** [the cutover design](../../docs/superpowers/specs/2026-10-23-celestina-cel-2-m-cutover-design.md)
  (the order of its §4, the configuration of its §5, the recovery of its §6),
  [the live validation order](../../docs/superpowers/plans/2026-10-23-cel-2-m-live-validation-order.md)
  and [ADR 0024](decisions/0024-cutover-defaults.md).
- **What agents did and did not do:** every delivery was verified in a nested
  niri on a private bus. No agent started, stopped or replaced the live
  shell, edited `~/.config/niri/config.kdl` or `shell.json`, or ran a live
  row: the switch-over below is the author's, by hand. The author did it on
  2026-11-24; the cutover's evidence records the report.

## Who does what

| Duty | Noctalia did it through | Celestina does it through | Switch | Live row(s) |
|---|---|---|---|---|
| Notifications | owning `org.freedesktop.Notifications` | the notification server, claimed when the name is free (CEL-1-F) | none: taken at the first start without Noctalia | VAL-CEL-BANNERS, VAL-CEL-NOCTALIA |
| Tray | owning `org.kde.StatusNotifierWatcher` | the watcher and host, claimed when free (CEL-1-G) | none | VAL-CEL-POPUP, VAL-CEL-NOCTALIA |
| Launcher | `Mod+Space` → `noctalia msg panel-toggle launcher` | `celestina launcher toggle` (CEL-1-E) | the key | VAL-CEL-KEYBOARD, VAL-CEL-IME |
| Bar | its bar on every output | one wide surface per output, always drawn; the zones are the `bar` table of `shell.json` (CEL-2-A, CEL-2-M) | none (ADR 0024) | VAL-CEL2-BAR-LIVE |
| Audio and brightness OSD | drawing over the `wpctl` and `brightnessctl` keys | `duties.osd`: the `celestina-osd` surface, the `volume`, `microphone` and `brightness` verbs (CEL-2-C) | `osd` | VAL-CEL2-OSD-AUDIO, -BRIGHTNESS, -PIECES |
| Media | its player widget | `duties.media`: the MPRIS client, the piece and the page; the keys keep `playerctl` (CEL-2-D) | `media` | VAL-CEL2-MEDIA, -COVER |
| Wallpaper | `noctalia-wallpaper` on the Background layer | `duties.wallpaper`: one `celestina-wallpaper` surface per output (CEL-2-E) | `wallpaper` | VAL-CEL2-WALLPAPER |
| Night light | the startup line's `nightlight-force-toggle` | `duties.nightlight` with `energy.night_light` (CEL-2-B) | `nightlight` | VAL-CEL2-NIGHTLIGHT |
| Caffeine | the startup line's `caffeine-enable` | `duties.caffeine` with `energy.caffeine`, a logind `idle` inhibitor: it stops idling only, and a suspend asked for by hand goes through (CEL-2-B, CEL-2-B-CAFFEINE, ADR 0025) | `caffeine` | VAL-CEL2-CAFFEINE |
| Idle | screen off at 20 s, lock-and-suspend at 240 s | `duties.idle`: the same chain on `ext-idle-notify`; the second step needs the lock (CEL-2-B, CEL-2-H-a) | `idle` | VAL-CEL2-IDLE, VAL-CEL2-LOCK-SUSPEND |
| Session lock | `Mod+Shift+Escape` → `noctalia msg session lock-and-suspend` | `duties.lock`: `celestina lock` and `celestina lock suspend`, three processes, PAM in a child (CEL-2-H-a, CEL-2-H-b) | `lock`, **on by default** ([ADR 0026](decisions/0026-lock-and-polkit-on-by-default.md)); install the PAM service first | VAL-CEL2-LOCK-PAM, -RECOVERY, -LIVE, -SUSPEND, -HOTPLUG, -NOTICES, -COEXIST, -BACKDROP, -CLOCK |
| Polkit agent | `polkit_agent = true` | `duties.polkit`: the agent, the prompt card, the helper conversation (CEL-2-I); the helper is contacted only once the password is submitted, so a cancelled prompt never reaches PAM and cannot count as a failed login (CEL-2-I-CANCEL, the amendment of ADR 0021, which lists the cases that can still count after a submit) | `polkit`, **on by default** ([ADR 0026](decisions/0026-lock-and-polkit-on-by-default.md)) | VAL-CEL2-POLKIT-LIVE |
| Clipboard history | its history and `clipboard-*` verbs | `duties.clipboard`: bounded, secret-aware, text persisted 0600 (CEL-2-G); an unknown value of the key reads as off | `clipboard` | VAL-CEL2-CLIPBOARD-LIVE, -SECRET, -IMAGE, -LOOK |
| Phone state | the `tony/magnetita` plugin | `duties.phone`: the Magnetita piece and page (CEL-2-J) | `phone` | VAL-CEL2-PHONE |
| Minimised windows | the `tony/bubbles` plugin, `Mod+K` | `duties.bubbles`: the bubble group and `celestina bubbles select` (CEL-2-J) | `bubbles`, the key | VAL-CEL2-BUBBLES, -BUBBLES-KEYS |
| Network, Bluetooth, battery, power profile | bar widgets | pieces placed in the bar's zones; `nm-applet` and `blueman` stay the editors (CEL-2-F) | none: a piece shows when placed | VAL-CEL2-NETWORK, -BLUETOOTH, -POWER |
| System monitor, weather, calendar | bar widgets | the `sysmon`, `weather` and `clock` pieces (CEL-2-K); the weather only with a manual location | none | VAL-CEL2-SYSMON, -WEATHER, -CALENDAR, -GLANCE-TABS |
| Overview and shortcuts | the workspace-preview plugin | `celestina overview toggle`, `celestina shortcuts toggle` (CEL-2-L) | keys of the author's choice | VAL-CEL2-OVERVIEW, -SHORTCUTS |
| Autostart | `spawn-at-startup "noctalia"` | `spawn-at-startup "sh" "-c" "exec systemd-cat -t celestina celestina"` | the line | the swap, below |

Not carried over, by the program's decisions: Noctalia's other plugins (video
wallpapers, wallhaven, notes, translator, the colour picker, the companions),
screen corners, a dock, desktop and lock-screen widgets, and the greeter.

## The defaults since the cutover

With no `duties` key, or with a key missing, every shipped duty is **on**,
`lock` and `polkit` included; the bar is always drawn. A `shell.json` that
cannot be read starts the shell with **every duty off** until its first clean
read ([ADR 0024](decisions/0024-cutover-defaults.md)). The clipboard, the lock
and the polkit agent fail closed: a value of their key that is neither `"on"`
nor `"off"` reads as off, so a mistyped `"off"` never turns a privileged duty
on ([ADR 0026](decisions/0026-lock-and-polkit-on-by-default.md)). An explicit
`"off"` already in the file, such as one the author wrote while Noctalia held
a duty, is honoured.

With `idle` and `lock` both on, a session whose file chose nothing turns its
screens off after 20 s of idle and locks and suspends after 240 s; before ADR
0026 the second step was refused for want of a locker. `"idle": "off"` or
`"lock": "off"` keeps the machine from suspending on idle.

The lock needs its PAM service installed before the first start (the README's
"Installing the lock's PAM service"). Without it the shell warns at every
start, the lock still covers and denies, and anything that starts the lock
(a `lock` verb, the idle chain's lock-and-suspend step, or any sleep logind
announces: a lid, the power key, `systemctl suspend`) locks a session that
cannot be unlocked from the lock itself; the way back is "The session lock:
rehearsal and recovery" below, and `"lock": "off"` is the alternative. The
polkit agent asks to be the session's agent at start and is refused, never
forced, while another agent holds it; polkitd keeps whichever agent
registered first, so `"polkit": "off"` is how the session is left to another
agent that starts later.

Two consequences of these defaults. The default layout places the
`brightness` piece and `osd` is on, so since 1.13.1 (CEL-2-C-DDC) a start
that does not say `"osd": "off"`, take `brightness` out of the layout or set
`CELESTINA_DDC=0` runs one `ddcutil` detection and one read per monitor when
the duty's workers start: Noctalia's brightness widget stays closed then, as
VAL-CEL2-OSD-BRIGHTNESS requires. And `nightlight` on only starts its
worker: no output's gamma is held until `energy.night_light` is switched on.

## Switching over (the author, by hand)

The order is the design's §4: one duty at a time while Noctalia still runs,
its counterpart switched off through Noctalia's own verb or setting, the
duty's live row run, and the autostart line swapped last.

1. **Before anything:** `cp ~/.config/niri/config.kdl ~/.config/niri/config.kdl.pre-celestina-cutover`.
2. The bar (`noctalia msg bar-hide`), then OSD (`noctalia msg osd-disable`,
   its brightness widget closed), media, the wallpaper, caffeine and the night
   light (their verbs, and the startup line with `caffeine-enable` deleted),
   idle (both behaviours off in Noctalia's settings), the clipboard, the phone
   and the bubbles (`Mod+K` rebound).
3. The lock: the PAM service installed by hand (the README says how), the
   recovery rehearsed from a VT **before** the first real lock, then
   `"lock": "on"` (written explicitly then; since ADR 0026 it is the
   default) and `Mod+Shift+Escape` rebound to `celestina lock suspend`.
   `Mod+Shift+A` keeps `swaylock`.
4. Polkit: `polkit_agent = false` in Noctalia's settings and
   `noctalia msg config-reload`, then `"polkit": "on"` (written explicitly
   then; since ADR 0026 it is the default).
5. `Mod+Space` rebound to `celestina launcher toggle`; the overview and the
   shortcut sheet bound to keys of the author's choice.
6. **The swap:** the `spawn-at-startup "noctalia"` line replaced by the
   autostart line of the table, then a new login. The shell takes the
   notification and tray names at that start; a running shell does not take
   them when Noctalia stops.

`celestina/scripts/cutover-config-probe.sh FILE` checks a **copy** of the niri
configuration for the result (no `noctalia` left in a spawn, `swaylock` still
on `Mod+Shift+A`, the startup line gone, `celestina` started); it refuses a
path under the configuration home.

## Switching back, in under a minute, without the shell

- **One duty misbehaves while Noctalia still runs:** set it to `"off"` in
  `~/.config/celestina/shell.json` (the shell follows the file, no restart)
  and switch Noctalia's side back on. If the shell itself is wedged, stop it
  by its pid (`pidof celestina`, then `kill -TERM` that pid; never by name):
  its workers are joined, the inhibitor and the gamma released.
- **After the swap, the shell does not come up or cannot be used:** from any
  terminal, or from a VT (`Ctrl+Alt+F3`),
  `cp ~/.config/niri/config.kdl.pre-celestina-cutover ~/.config/niri/config.kdl`
  restores every bind at once (niri reloads the file live); stop the shell
  with `celestina quit` or by pid, then `noctalia &` for this login. The next
  login starts Noctalia from the restored line.
- **Locked out:** the section below. For polkit there is nothing to recover:
  without an agent every action fails closed, and `sudo` needs no agent.

The backup copy, the `swaylock` bind and `sudo` are the three things no unit
touches.

## The session lock: rehearsal and recovery

The lock has never run in the author's session. Everything agents ran was in a
nested niri on a private session bus, against a fake logind on a private
system bus and a PAM service that does not exist, and recorded in
[the evidence](evidence/2026-10-22-cel-2-h-a.md).

**Run the rehearsal** (R-1 to R-9: cover and prompt, a second lock, a killed lock,
a kill between request and cover, a restarted shell, unreadable `/proc`, wrong
passwords on the rehearsal service, the unlock probe, the idle chain), from
`celestina/` after building the lock pair and the shell in release into a
private directory:

```sh
sh scripts/lock-rehearsal.sh --bin-dir DIR --fake-logind PATH \
   --unlock-probe PATH --pointer-tool PATH --out docs/evidence/NAME
```

`--check` runs the refusals and the preparation, prints the plan and starts
nothing; steps are named on the command line (`R-1` to `R-9`, `R-9a` to `R-9c`).

It runs only inside `dev-session.sh --own-bus`, refuses to start if the lock of
a live session is running or `/etc/pam.d/celestina-lock-rehearsal` exists, and
stops every process by the pid it recorded, never by name. The R-9 steps wait
four minutes each because the idle delays have no setting. Never run it, the
lock, the verifier or `pamtester` against the real session, and never type the
author's password into a rehearsal. Since round 5 the lock refuses to connect
unless the shell started it (`CELESTINA_LOCK_LAUNCHER=shell`) or the
compositor is a nest started by `dev-session.sh` (ADR 0022 A4; `--rehearsal`
and the probes need the nest always): a bare run exits 15 and sends nothing.
That guard exists because a bare `--rehearsal` with the desktop's display
locked the real session on 2026-10-22; it is a barrier against accidents, so
keep using `env -i` with the nest's own display.

**Recover from a stuck lock** (the full text is the README's "If the lock does
not let you in"): `Ctrl+Alt+F3`, read the pid in
`$XDG_RUNTIME_DIR/celestina/lock-<display>.state`, `kill -KILL` that pid only;
the shell starts the lock again, or `swaylock -c 000000` takes over the dead
lock from the console. When `faillock` or `pam_nologin` closes the console and
`swaylock` too, the last resort is root (`faillock --user <name> --reset`).
Switching `duties.lock` off never touches a running lock.

## What stays the author's

The live rows of [VALIDATION.md](../VALIDATION.md), in
[the order of the cutover](../../docs/superpowers/plans/2026-10-23-cel-2-m-live-validation-order.md);
the retirement of `swaylock`, which no agent proposes. The author reported
four lock rows (`VAL-CEL2-LOCK-PAM`, `-LIVE`, `-RECOVERY`, `-SUSPEND`) and
`VAL-CEL2-POLKIT-LIVE` as passed on 2026-11-24, and the defaults of `lock` and
`polkit` were flipped in one unit
([ADR 0026](decisions/0026-lock-and-polkit-on-by-default.md)); the other five
lock rows stay pending and do not gate them.
