# Handover: what Noctalia still does for the session

- **Written:** 2026-10-16, by CEL-1-H
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
| Audio OSD | the volume and mute keys run `wpctl` directly; Noctalia draws the feedback | no | an OSD delivery that observes PipeWire/WirePlumber, since the keys do not tell the shell anything |
| Brightness OSD | the brightness keys run `brightnessctl`; Noctalia draws the feedback | no | an OSD delivery that observes the backlight (sysfs or logind) |
| Media (MPRIS) | the play, stop, previous and next keys run `playerctl`; Noctalia shows the player | no | a media delivery with an MPRIS client and its Island page or widget |
| Clipboard history | Noctalia's history (`clipboard-*` verbs) | no | a clipboard delivery with a bounded, hostile-input-safe history and its own privacy design |
| Session lock | `Mod+Shift+Escape` runs `noctalia msg session lock-and-suspend` | no | a lock screen (`ext-session-lock-v1`) plus the suspend chain; `Mod+Shift+A` already uses `swaylock` and is unaffected |
| Polkit agent | Noctalia's authentication dialogs; the earlier roadmap named it the last blocker | no | a polkit agent with a password prompt; without it, privileged prompts have no UI |
| Wallpaper | Noctalia draws it (the niri config pins the `wallpaper` namespace to the backdrop); today a still image, though the enabled `mpvpaper` plugin and its cache show video wallpapers have been used | no | a wallpaper delivery, including video if the author still wants it |
| Night light | niri startup runs `noctalia msg nightlight-force-toggle` | no | a night-light delivery (gamma control) and a replacement for that startup line |
| Caffeine (idle inhibitor) | niri startup runs `noctalia msg caffeine-enable` | no | an idle-inhibit delivery and a replacement for that startup line |
| Bar | Noctalia's bar is visible (`noctalia msg status`) with workspaces, clock, network, Bluetooth, volume, power and the session menu | no, only the Island | the design's full-width bar delivery, or the author's decision to live on the Island alone |
| Idle management | Noctalia sets the screen-off idle behaviour and reacts to logind lock hints; stopping it ends idle screen-off, which matters on the OLED | no | an idle delivery (`ext-idle-notify`, DPMS) before stopping Noctalia |
| Magnetita plugin | `tony/magnetita` shows the phone's battery, connection and media in a bar widget and panel, fed by `org.celestina.Magnetita` | no | decide where phone state shows once the bar is gone; the plugin dies with Noctalia |
| Bubbles | `Mod+K` runs `noctalia msg panel-toggle tony/bubbles:selector`; `tony/bubbles` also has a bar widget | no | decide whether it survives; plugins are an exclusion of the design for good |
| Autostart | `spawn-at-startup "noctalia"` | no | start `celestina` in its place, as the last step |

Noctalia has more enabled plugins (`noctalia msg plugins list`, for example a colour picker and a Claude companion); the list above does not judge them. It also exposes dock, desktop and lock-screen widgets, Bluetooth,
keyboard backlight, DPMS, colour schemes and EasyEffects verbs. The session's
niri configuration does not call them today, so none blocks stopping it; they
are listed so a later feature does not appear to regress by surprise.

## Order that follows from this

1. Land CEL-1 and let the author run the VAL-CEL rows.
2. Cover the duties the session depends on every day: the two OSDs, media,
   lock and the polkit agent. These are the ones a user notices missing.
3. Replace the two autostart lines (night light, caffeine) and the wallpaper.
4. Rebind the niri keys, then replace `spawn-at-startup "noctalia"`.

Each item is a later delivery with its own design, as the program's
exclusions say. This note orders nothing; the author does.
