# Handover: what Noctalia still does for the session

- **Written:** 2026-10-16, by CEL-1-H; the night light, caffeine and idle rows updated by CEL-2-B; the audio and brightness OSD rows by CEL-2-C; the polkit row by CEL-2-I; the session lock rows and the lock section by CEL-2-H-a
- **Purpose:** CEL-1 delivers the Island, the launcher, notifications and the
  tray. It does not replace Noctalia as the session's shell. This note lists
  what Noctalia keeps doing, so the author knows what the next deliveries
  must cover before Noctalia can be stopped. It records state and intent; it
  grants no authority and schedules nothing.
- **Method:** the list comes from the design's exclusions
  ([§1](../../docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md)),
  from `~/.config/niri/config.kdl` and from `noctalia msg --help`, `status`
  and `plugins list` on 2026-10-16. Nothing was changed in the session.

## What 1.0.0 means

The author accepted 1.0.0 on 2026-10-16 as the first stable version of the
shell, not the definitive one. The next steps are new units that widen its
capabilities and refine it; the duties below are their candidates. The nine
VAL-CEL rows stay `pending` in [VALIDATION.md](../VALIDATION.md) until the
author closes them.

## What the new shell does today

The shell claims `org.freedesktop.Notifications` and
`org.kde.StatusNotifierWatcher` only when the name is free, so beside Noctalia
it serves neither and its pages say who owns them. Stopping Noctalia hands
both names to the shell on its next start. The launcher answers
`celestina launcher toggle`; the niri bindings still call Noctalia's.

## What Noctalia still does

| Duty | How the session uses it today | Covered by CEL-1 | Needed before stopping Noctalia |
|---|---|---|---|
| Notifications | owns the name unless stopped | yes (CEL-1-F) | VAL-CEL-BANNERS and VAL-CEL-NOCTALIA passed |
| Tray | owns the watcher unless stopped | yes (CEL-1-G) | VAL-CEL-POPUP and VAL-CEL-NOCTALIA passed |
| Launcher | `Mod+Space` runs `noctalia msg panel-toggle launcher` | yes (CEL-1-E) | rebind the key to `Mod+Space { spawn "celestina" "launcher" "toggle"; }` (the commented `celestina msg ...` lines in the niri config belong to the halted shell and the new binary rejects them with exit 2); VAL-CEL-KEYBOARD and VAL-CEL-IME passed |
| Audio OSD | the volume and mute keys run `wpctl` directly; Noctalia draws the feedback | delivered in CEL-2-C, off by default (`duties.osd`), observing `pactl subscribe` | VAL-CEL2-OSD-AUDIO passed, with Noctalia's OSD off; the keys keep running `wpctl` until CEL-2-M rebinds them |
| Brightness OSD | the brightness keys run `brightnessctl`; Noctalia draws the feedback | delivered in CEL-2-C, off by default (`duties.osd`), sysfs where present and DDC otherwise; the monitor's own buttons are not observed | VAL-CEL2-OSD-BRIGHTNESS passed, with Noctalia's OSD and brightness widget off so no two `ddcutil` run at once; a lock-keys OSD is not delivered |
| Media (MPRIS) | the play, stop, previous and next keys run `playerctl`; Noctalia shows the player | no | a media delivery with an MPRIS client and its Island page or widget |
| Clipboard history | Noctalia's history (`clipboard-*` verbs) | no | a clipboard delivery with a bounded, hostile-input-safe history and its own privacy design |
| Session lock | `Mod+Shift+Escape` runs `noctalia msg session lock-and-suspend` | security core delivered in CEL-2-H-a, off by default (`duties.lock`); the PAM service is not installed; the dimmed wallpaper and the clock wait for CEL-2-H-b | VAL-CEL2-LOCK-PAM, -LIVE, -SUSPEND, -HOTPLUG, -RECOVERY, -NOTICES and -COEXIST passed, with Noctalia's idle lock off; `Mod+Shift+A` keeps using `swaylock` (the recovery path) until the author retires it; rebinding `Mod+Shift+Escape` to `celestina lock suspend` is CEL-2-M |
| Polkit agent | Noctalia's authentication dialogs; the earlier roadmap named it the last blocker | delivered in CEL-2-I, off by default (`duties.polkit`) | VAL-CEL2-POLKIT-LIVE passed; set `duties.polkit` to `"on"` in `shell.json` with Noctalia's agent off (the shell refuses while another agent holds the session); recovery is the duty `"off"` or stopping the shell, then Noctalia's agent again; without any agent, privileged prompts have no UI |
| Wallpaper | Noctalia draws it (the niri config pins the `wallpaper` namespace to the backdrop); today a still image, though the enabled `mpvpaper` plugin and its cache show video wallpapers have been used | no | a wallpaper delivery, including video if the author still wants it |
| Night light | niri startup runs `noctalia msg nightlight-force-toggle` | delivered in CEL-2-B, off by default (`duties.nightlight`, `energy.night_light`) | VAL-CEL2-NIGHTLIGHT passed; replace the startup line by setting `duties.nightlight` to `"on"` and `energy.night_light` to `true` in `shell.json`, with Noctalia's night light off |
| Caffeine (idle inhibitor) | niri startup runs `noctalia msg caffeine-enable` | delivered in CEL-2-B, off by default (`duties.caffeine`, `energy.caffeine`) | VAL-CEL2-CAFFEINE passed; replace the startup line by setting `duties.caffeine` to `"on"` and `energy.caffeine` to `true` in `shell.json`, with Noctalia's caffeine off |
| Bar | Noctalia's bar is visible (`noctalia msg status`) with workspaces, clock, network, Bluetooth, volume, power and the session menu | no, only the Island | the design's full-width bar delivery, or the author's decision to live on the Island alone |
| Idle management | Noctalia sets the screen-off idle behaviour and reacts to logind lock hints; stopping it ends idle screen-off, which matters on the OLED | screen-off delivered in CEL-2-B, off by default (`duties.idle`); the lock-and-suspend step is refused until a locker exists, and with `duties.lock` on it locks, then suspends once the compositor confirms the lock (CEL-2-H-a) | VAL-CEL2-IDLE passed, with Noctalia's idle behaviour off; suspend after idle waits for the lock screen |
| Magnetita plugin | `tony/magnetita` shows the phone's battery, connection and media in a bar widget and panel, fed by `org.celestina.Magnetita` | no | decide where phone state shows once the bar is gone; the plugin dies with Noctalia |
| Bubbles | `Mod+K` runs `noctalia msg panel-toggle tony/bubbles:selector`; `tony/bubbles` also has a bar widget | no | decide whether it survives; plugins are an exclusion of the design for good |
| Autostart | `spawn-at-startup "noctalia"` | no | start `celestina` in its place, as the last step |

Noctalia has more enabled plugins (`noctalia msg plugins list`, for example a colour picker and a Claude companion); the list above does not judge them. It also exposes dock, desktop and lock-screen widgets, Bluetooth,
keyboard backlight, DPMS, colour schemes and EasyEffects verbs. The session's
niri configuration does not call them today, so none blocks stopping it; they
are listed so a later feature does not appear to regress by surprise.

## The session lock: rehearsal and recovery (CEL-2-H-a)

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

## Order that follows from this

1. Land CEL-1 and let the author run the VAL-CEL rows.
2. Cover the duties the session depends on every day: the two OSDs, media,
   lock and the polkit agent. These are the ones a user notices missing.
3. Replace the two autostart lines (night light, caffeine) and the wallpaper.
4. Rebind the niri keys, then replace `spawn-at-startup "noctalia"`.

Each item is a later delivery with its own design, as the program's
exclusions say. This note orders nothing; the author does.
