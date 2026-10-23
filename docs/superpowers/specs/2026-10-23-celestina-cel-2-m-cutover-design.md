# CEL-2-M — Cutover from Noctalia to Celestina — design

- **Date:** 2026-10-23
- **Status:** drafted for the author's review; nothing in it has run against
  the live session, and this document grants no authority to do so
- **Product:** the one unit of the parity program
  ([CEL-2 design §5](2026-10-16-celestina-parity-program-design.md)) after
  which Noctalia is no longer started: `celestina` in niri's autostart, the
  keys that call Noctalia rebound, the startup line deleted, the `off`
  defaults of the shipped duties retired, the handover note and the roadmap
  brought up to date. **It never replaces the live session on its own**
  (program §5, ledger row CEL-2-M).
- **Identifiers:** project `celestina`, plan `cel-2-parity`, unit `CEL-2-M`,
  commit prefix `celestina:`, kind `milestone`
- **Builds on:** the CEL-2 design, the handover note
  `celestina/docs/handover.md`, the session lock design (§8 recovery) and the
  polkit agent design (§7 recovery), and ADR 0011 (duties switched on one by
  one, off while Noctalia holds them)
- **Method:** every fact below was read on 2026-10-23 from `main` at
  `a5427f7a` (celestina 1.10.0), the unit branches `unit/celestina/CEL-2-H-b`,
  `CEL-2-I`, `CEL-2-L` and `SMOKE-FIX`, `~/.config/niri/config.kdl`,
  `~/.local/state/noctalia/settings.toml` (its third-party key is not copied),
  `noctalia msg --help`, `ls ~/.local/bin`, and the GitHub Actions runs of
  `main` and PR 4. Nothing was changed and nothing was run against the
  session.

## 1. Goal and scope

Noctalia stops being started and Celestina does everything Noctalia did for
the author's niri session (program §1). The unit is small in code and large
in consequence, so this design is mostly about **order, reversibility and
what the author must have verified first**.

In scope:

1. the `shell.json` the cutover runs with: which duty is on, which piece is
   placed where, and the order in which the duties take over from Noctalia;
2. the autostart line and the environment the shell starts in;
3. the niri binds that call Noctalia today and their replacements;
4. the Noctalia duties that are switched off before the stop, the stop itself,
   and what is deliberately left alone;
5. the recovery path: back to Noctalia in under a minute, without the shell's
   cooperation;
6. the automated checks in a nest before anything real is touched;
7. the code change: retiring the `off` defaults, and the documents.

Out of scope, by the program's decisions: Noctalia's plugins (they die with
it), screen corners, `swaylock`'s retirement (lock design §8: never proposed
by an agent), the greeter, and any uninstall of Noctalia (its binary and its
state stay; the unit edits one line of one file to stop it).

## 2. Facts the design stands on

### 2.1 What the session runs today (`~/.config/niri/config.kdl`)

| Line | Today | Calls Noctalia |
|---|---|---|
| 71 | `spawn-at-startup "noctalia"` | yes |
| 73 | `spawn-at-startup "sh" "-c" "sleep 1 && noctalia msg caffeine-enable && noctalia msg nightlight-force-toggle"` | yes |
| 592 | `Mod+K repeat=false { spawn "noctalia" "msg" "panel-toggle" "tony/bubbles:selector"; }` | yes |
| 594 | `Mod+Space { spawn "noctalia" "msg" "panel-toggle" "launcher"; }` | yes |
| 595 | `Mod+Shift+Escape { spawn "noctalia" "msg" "session" "lock-and-suspend"; }` | yes |
| 598–600 | three commented `celestina msg …` lines of the halted Qt shell | no (comments; the Rust binary rejects `msg` with exit 2) |
| 603 | `Mod+Shift+A repeat=false { spawn "swaylock" "-f" "-c" "000000" "-u"; }` | no — **the lock's recovery path, untouched** |
| 614–617 | volume and microphone keys run `wpctl` directly | no; Noctalia only draws the OSD |
| 620–623 | media keys run `playerctl` | no; unchanged by M (design §5, D) |
| 626–627 | brightness keys run `brightnessctl --class=backlight`, which has no device on this machine | no; they change nothing today |
| 72, 74–76 | `solaar`, `input-remapper-control`, `gnome-keyring-daemon`, `melibea.service` | no; untouched |

niri reloads `binds` and layer rules live on a config change; a
`spawn-at-startup` line runs only at compositor start. So the key rebinds
take effect at once and the autostart change only at the next login.

### 2.2 What Noctalia has on (`settings.toml`, read-only)

Idle: `screen-off` at 20 s and `lock-and-suspend` at 240 s, both enabled.
`polkit_agent = true`. OSD on for volume, microphone and brightness. The bar on
all three outputs (`bar-hide` hides it). Wallpaper per output from a folder.
Night light and caffeine are forced from the niri startup line, not from the
file. Noctalia owns `org.freedesktop.Notifications` and
`org.kde.StatusNotifierWatcher`, and is the session's only polkit agent.

Noctalia's verbs that switch a duty off at run time, from `noctalia msg
--help`: `bar-hide`, `osd-disable`, `caffeine-disable`, `nightlight-disable`
(and `nightlight-force-toggle` to undo the forced mode). There is **no verb
for idle, lock or the polkit agent**: those are switched off in Noctalia's
settings (its settings window or `settings.toml`, then `config-reload`), and
the wallpaper has only `wallpaper-set`, no disable verb. During coexistence
the author turns them off through Noctalia's settings, as the VAL rows say.

### 2.3 What the shell does at start (`celestina/src/main.rs`, `app.rs`)

- The bus name `org.celestina.Shell` is claimed first; a second shell exits 0
  with "the shell is running". One shell per session, always.
- Every duty is **off by default** (`Duties::default()`); with the defaults
  **no worker starts**. `duties.bar` is `Option<Duty>`: `None` until chosen
  and then never written, precisely so that CEL-2-M's new default reaches
  every file that did not choose (`settings.rs`, the field's comment).
- Each duty's handle reads `shell.json` once at start (`handle.apply`) and the
  shell follows the file live: **a switch is never a restart** (ADR 0011).
- Notifications and the tray are claimed only when the name is free; beside
  Noctalia the pages name the owner. **Stopping Noctalia does not hand the
  names over to a running shell**: the handover note says the shell takes them
  "on its next start". So after Noctalia stops, the shell must be restarted
  once (or started after Noctalia is gone) to serve notifications and the
  tray. This is the one place the cutover needs a restart, and §4 orders it.
- The wallpaper, lock and polkit duties detect an owner (a layer namespace, a
  lock in force, the agent slot) and **refuse** while another program holds
  it; idle, gamma, caffeine have no owner signal and are only the author's
  switch (ADR 0011).
- The deployed shell is `~/.local/bin/celestina` with `celestina-lock` and
  `celestina-lock-verify` beside it (`deploy-production.sh`); the artifact is
  verified by the landing. `activate-production.sh` starts it detached through
  `systemd-cat -t celestina` so the journal gets its log, and refuses while
  `org.celestina.Shell` has an owner.

### 2.4 Verbs the binds can call (`celestina --help`, `main` plus CEL-2-L)

`launcher toggle`, `island toggle|open <page>|close`, `lock`, `lock suspend`,
`bubbles select`, `bubbles minimize`, `volume up|down|mute`, `volume set N`,
`microphone mute|set N`, `brightness up|down [OUTPUT]`, `brightness set N
[OUTPUT]`, `caffeine on|off|toggle`, `nightlight on|off|toggle`, `session
reboot|power-off|log-out`, `quit`; and from CEL-2-L (not on `main` yet)
`overview open|close|toggle`, `shortcuts open|close|toggle`. There is no
`msg` and no `lock-and-suspend` word: the commented lines 598–600 are wrong
for this binary.

### 2.5 The duties and their rows

| Duty / piece | Landed | Default on `main` | Live row(s), all pending |
|---|---|---|---|
| `bar` | A | `None` (= off) | VAL-CEL2-BAR-LIVE |
| `idle` | B | off | VAL-CEL2-IDLE |
| `caffeine` | B | off | VAL-CEL2-CAFFEINE |
| `nightlight` | B | off | VAL-CEL2-NIGHTLIGHT |
| `session` verbs | B | — | VAL-CEL2-SESSION |
| `osd` | C | off | VAL-CEL2-OSD-AUDIO, -BRIGHTNESS, -PIECES |
| `media` | D | off | VAL-CEL2-MEDIA, -COVER |
| `wallpaper` | E | off | VAL-CEL2-WALLPAPER |
| connectivity pieces | F | no duty | VAL-CEL2-NETWORK, -BLUETOOTH, -POWER |
| `clipboard` | G | off | VAL-CEL2-CLIPBOARD-LIVE, -SECRET, -IMAGE, -LOOK |
| `lock` | H-a | off | VAL-CEL2-LOCK-PAM, -LIVE, -SUSPEND, -HOTPLUG, -RECOVERY, -NOTICES, -COEXIST; H-b adds -BACKDROP, -CLOCK |
| `polkit` | I (branch) | off | VAL-CEL2-POLKIT-LIVE (on the branch) |
| `phone`, `bubbles` | J | off | VAL-CEL2-PHONE, -BUBBLES, -BUBBLES-KEYS |
| glance pieces | K | no duty | VAL-CEL2-SYSMON, -WEATHER, -CALENDAR, -GLANCE-TABS |
| overview, shortcuts | L (branch) | no duty | VAL-CEL2-OVERVIEW, -SHORTCUTS (on the branch) |

None has been run (`VALIDATION.md`, "None has been run and none is closed by
an agent"). The CEL-1 rows VAL-CEL-GLASS, -ISLAND, -REST, -POPUP, -KEYBOARD,
-IME, -SCALE, -NOCTALIA, -BANNERS are pending too.

## 3. Decisions this design proposes (the author decides; §10 lists them)

| # | Decision | Proposal | Why |
|---|---|---|---|
| M-1 | How Celestina is started | `spawn-at-startup "celestina"` in `config.kdl`, replacing line 71 verbatim; stderr goes to the journal as `activate-production.sh` does it (a two-word wrapper, §5.2) | the shell is one process with one loop (ADR 0009); niri already starts the other session services this way; no systemd unit is added because the shell's own bus guard already makes a second start exit 0 |
| M-2 | Which duties the cutover file turns on | everything Noctalia did that the author uses: `bar`, `idle`, `caffeine`, `nightlight`, `wallpaper`, `osd`, `media`, `clipboard`, `lock`, `polkit`; `phone` and `bubbles` on if the author keeps them (they replace two Noctalia plugins) | program §1: parity with Noctalia's bar and background duties |
| M-3 | Which defaults the code retires | `bar`, `idle`, `caffeine`, `nightlight`, `wallpaper`, `osd`, `media`, `clipboard`, `phone`, `bubbles` flip to `on`; **`lock` and `polkit` flip only if their live rows passed**, as the lock and polkit designs say (lock §8, polkit §7: "the duty's default flips to `on` only in CEL-2-M, and only if that row has") | ADR 0011; a privileged duty that nobody validated must not switch itself on for a file that chose nothing |
| M-4 | Keys | `Mod+Space` → `celestina launcher toggle`; `Mod+Shift+Escape` → `celestina lock suspend`; `Mod+K` → `celestina bubbles select`; the volume, mute and microphone keys keep running `wpctl` (the shell observes PipeWire and draws the OSD regardless); brightness keys → `celestina brightness up|down` (today they do nothing); media keys unchanged (`playerctl`); `Mod+Shift+A` untouched (`swaylock`, the recovery path) | each is the verb the unit's VAL row prescribes; the design says the media keys keep `playerctl` |
| M-5 | New keys for CEL-2-L | `Mod+Shift+Tab` (or the author's choice) → `celestina overview toggle`; `Mod+Shift+Slash` is niri's own `show-hotkey-overlay` today, so the shortcut sheet takes another chord or replaces it, the author's call | the two verbs exist on the L branch only; VAL-CEL2-OVERVIEW and -SHORTCUTS are "test in a nest first" |
| M-6 | The order of the takeover | one duty a day, in the order of §4, each with its VAL row, Noctalia's counterpart off through its own switch, and Noctalia still started by niri; the autostart swap is the **last** step and happens only after the day with every duty on and Noctalia idle has passed | ADR 0011; a half-migrated session is supported by the design ("Stopping point") |
| M-7 | Recovery | a copy of `config.kdl` taken before the first edit, kept at a fixed path, and a one-line restore (§6); plus `swaylock` and `sudo` as the lock's and polkit's independent paths | lock §8 and polkit §7: the recovery never depends on the shell |
| M-8 | What is **not** rebound | `Mod+Shift+A`; the media keys; `Mod+N`/`Mod+M` (Melibea's own scripts) | out of M's scope by the lock design and the program |

## 4. The order of activation (coexistence, then the swap)

Noctalia keeps running and started by niri through steps 1–9. Each step is
one duty: turn Noctalia's side off, turn the shell's side on in `shell.json`
(the shell follows the file), run the VAL row, live with it for a day. If the
row fails, turn the duty off and Noctalia's side on again: that is the step's
whole rollback, and it needs nothing from this unit. The order goes from the
cheapest to undo to the one that needs root to undo, and puts what the OLED
needs early.

| Step | Shell on | Noctalia off, how | Row(s) | Why here |
|---|---|---|---|---|
| 1 | `bar` on, the layout of §5.1 | `noctalia msg bar-hide` | VAL-CEL2-BAR-LIVE, then -NETWORK, -BLUETOOTH, -POWER, -SYSMON, -CALENDAR, -GLANCE-TABS as pieces are placed | everything else is seen through the bar |
| 2 | `osd` | `noctalia msg osd-disable`; Noctalia's brightness widget closed (two `ddcutil` on one bus lost the GPU twice) | VAL-CEL2-OSD-AUDIO, -BRIGHTNESS, -PIECES | daily feedback; reversible with one verb |
| 3 | `media` | Noctalia's media widget off | VAL-CEL2-MEDIA, -COVER | small, independent |
| 4 | `wallpaper` + `wallpaper.folder` | Noctalia's wallpaper off (its settings; there is no verb) | VAL-CEL2-WALLPAPER | the shell refuses an output Noctalia holds, so a mistake shows nothing wrong, it just draws nothing |
| 5 | `caffeine` + `energy.caffeine: true`, `nightlight` + `energy.night_light: true` | `noctalia msg caffeine-disable`, `noctalia msg nightlight-disable` and `nightlight-force-toggle` to undo the forced mode; **line 73 of `config.kdl` deleted** (it would re-enable both at the next login) | VAL-CEL2-CAFFEINE, -NIGHTLIGHT | replaces the startup line; the first `config.kdl` edit, under the backup of §6 |
| 6 | `idle` | Noctalia's `screen-off` and `lock-and-suspend` behaviours disabled in its idle settings | VAL-CEL2-IDLE | OLED screen-off; without `lock` the second step is refused (logged once), the design's intended state |
| 7 | `clipboard` | Noctalia's clipboard widget off; its history stays in `~/.local/state/noctalia/clipboard` untouched | VAL-CEL2-CLIPBOARD-LIVE, -SECRET, -IMAGE, -LOOK | privacy: the author checks the state file and the journal |
| 8 | `phone`, `bubbles`; `Mod+K` rebound | Noctalia's `tony/magnetita` and `tony/bubbles` plugins disabled | VAL-CEL2-PHONE, -BUBBLES, -BUBBLES-KEYS | the first key rebind; `Mod+K` is also the second `config.kdl` edit |
| 9 | `lock` (PAM service installed by hand first); then `Mod+Shift+Escape` rebound | Noctalia's `lock-and-suspend` already off (step 6); its lock control not used | VAL-CEL2-LOCK-PAM, -LIVE, -RECOVERY **before** -SUSPEND, -HOTPLUG, -NOTICES, -COEXIST; H-b's -BACKDROP, -CLOCK | privileged: needs root for the PAM file, `swaylock` stays on `Mod+Shift+A`, a root path is known before the first lock |
| 10 | `polkit` | `polkit_agent = false` in Noctalia's settings, `noctalia msg config-reload` | VAL-CEL2-POLKIT-LIVE | privileged; fails closed without an agent; `sudo` is the terminal path; the shell refuses while Noctalia holds the slot, so the order is safe either way |
| 11 | `Mod+Space` rebound; CEL-2-L keys added | — | VAL-CEL-KEYBOARD, -IME (launcher), VAL-CEL2-OVERVIEW, -SHORTCUTS (nest first) | the launcher is the key the author presses most: last among the coexistence steps so every earlier one is already proven |
| 12 | **The swap**: line 71 becomes `spawn-at-startup "celestina"`; log out and in | Noctalia is not started | VAL-CEL-NOCTALIA (the "without it" half), -BANNERS, -GLASS, -POPUP: the shell now owns notifications and the tray | the only step that needs a new login; §6 is the way back |

After step 12 the shell owns `org.freedesktop.Notifications` and
`org.kde.StatusNotifierWatcher` from its first start. The CEL-1 rows that
need Noctalia stopped (VAL-CEL-BANNERS, VAL-CEL-GLASS's banner, the second
half of VAL-CEL-NOCTALIA) are run then. The code change of §7 lands **after**
step 12 has held for the time the author chooses, so that the file's explicit
`on` values and the new defaults agree and a reinstall changes nothing.

## 5. The cutover configuration

### 5.1 `~/.config/celestina/shell.json` at step 12

The file is the author's; the shell writes it atomically and compatibly. What
the cutover file holds, as the units' VAL rows prescribe (`lock` and
`polkit` only if steps 9 and 10 passed; `weather` only if the author wants a
location in the file, ADR 0016):

```json
{
  "schema_version": 1,
  "do_not_disturb": false,
  "duties": {
    "bar": "on", "idle": "on", "caffeine": "on", "nightlight": "on",
    "wallpaper": "on", "osd": "on", "media": "on", "clipboard": "on",
    "lock": "on", "polkit": "on", "phone": "on", "bubbles": "on"
  },
  "energy": { "caffeine": true, "night_light": true },
  "wallpaper": { "folder": "<the author's folder>", "mode": "fill" },
  "bar": {
    "start": ["workspaces", "phone", "notifications"],
    "centre": ["weather", "clock"],
    "end": ["bubbles", "sysmon", "tray", "network", "bluetooth",
            "volume", "brightness", "clipboard", "media", "battery",
            "power-profile"],
    "tray_hidden": ["nm-applet", "blueman", "blueman-tray"]
  }
}
```

The zones mirror Noctalia's bar (program §3, `settings.toml`: start =
workspaces, magnetita, notifications; centre = weather, clock; end = bubbles,
sysmon, tray, network, bluetooth, volume, brightness, clipboard), with the
pieces Celestina has (`media`, `battery`, `power-profile` are additions the
author may drop; `battery` is absent on a machine without one). The `weather`
piece draws nothing without a `weather` key. The exact ids are the piece
table's (`celestina --help` lists the pages; the ids are in
`celestina/src/pieces/`).

### 5.2 Autostart

Line 71 of `config.kdl` becomes:

```kdl
spawn-at-startup "sh" "-c" "exec systemd-cat -t celestina celestina"
```

so the shell's stderr reaches `journalctl --user -t celestina`, which every
VAL row reads; without `systemd-cat` the plain `spawn-at-startup "celestina"`
is the fallback. Order among the other `spawn-at-startup` lines does not
matter: the shell claims names only when free and detects owners itself, and
`melibea.service` is a systemd unit the bubbles worker finds absent or present
as a normal state (ADR 0020). The environment is niri's: `WAYLAND_DISPLAY`,
`XDG_RUNTIME_DIR`, `NIRI_SOCKET`, the session bus. `~/.local/bin` must be on
niri's `PATH` (it is: `noctalia` is found the same way); if the author prefers
the absolute path, `/home/toni/.local/bin/celestina`.

### 5.3 Binds (the final `binds { }` diff)

```kdl
-    Mod+K repeat=false { spawn "noctalia" "msg" "panel-toggle" "tony/bubbles:selector"; }
+    Mod+K repeat=false { spawn "celestina" "bubbles" "select"; }
-    Mod+Space { spawn "noctalia" "msg" "panel-toggle" "launcher"; }
+    Mod+Space { spawn "celestina" "launcher" "toggle"; }
-    Mod+Shift+Escape { spawn "noctalia" "msg" "session" "lock-and-suspend"; }
+    Mod+Shift+Escape { spawn "celestina" "lock" "suspend"; }
-    // Mod+Space { spawn "celestina" "msg" "launcher-toggle"; }
-    // Mod+Shift+Escape { spawn "celestina" "msg" "lock-and-suspend"; }
-    // Mod+Ctrl+Escape { spawn "celestina" "msg" "lock"; }
+    Mod+Ctrl+Escape { spawn "celestina" "lock"; }
-    XF86MonBrightnessUp allow-when-locked=true { spawn "brightnessctl" "--class=backlight" "set" "+10%"; }
-    XF86MonBrightnessDown allow-when-locked=true { spawn "brightnessctl" "--class=backlight" "set" "10%-"; }
+    XF86MonBrightnessUp allow-when-locked=true { spawn "celestina" "brightness" "up"; }
+    XF86MonBrightnessDown allow-when-locked=true { spawn "celestina" "brightness" "down"; }
+    // CEL-2-L (once landed): the author's chords
+    Mod+Shift+Tab repeat=false { spawn "celestina" "overview" "toggle"; }
+    Mod+Shift+Slash { spawn "celestina" "shortcuts" "toggle"; }   // replaces show-hotkey-overlay, the author's call
```

Lines kept verbatim: `Mod+Shift+A` (`swaylock`), the four `wpctl` keys, the
four `playerctl` keys, `Mod+N`, `Mod+M`. `allow-when-locked=true` on the
brightness keys is kept: `celestina brightness` is a bus verb, and with the
lock up the shell still serves it; whether an OSD should show over the lock is
a finding for VAL-CEL2-LOCK-LIVE, not a reason to drop the flag.

### 5.4 What is switched off in Noctalia and what is left alone

Switched off, each at its step (§4): the bar (`bar-hide`), OSD
(`osd-disable`), caffeine and night light (verbs plus the deleted startup
line), the wallpaper, the media, clipboard, magnetita and bubbles widgets and
plugins, both idle behaviours and `polkit_agent` (settings). Left alone: the
Noctalia binary, `~/.local/state/noctalia/` with its settings, clipboard
history and wallpaper cache (the recovery of §6 needs them intact), the
greeter (a system service, out of scope), and `swaylock`. Noctalia's settings
window stays usable to switch its duties back on.

## 6. Recovery: back to Noctalia in under a minute, without the shell

Taken **before the first edit of `config.kdl`** (step 5):

```sh
cp ~/.config/niri/config.kdl ~/.config/niri/config.kdl.pre-celestina-cutover
```

and kept until the author deletes it. The three ways back, none of which asks
the shell for anything:

1. **A duty misbehaves while Noctalia still runs (steps 1–11):** set that
   duty to `"off"` in `shell.json` (any editor; the shell follows the file) and
   turn Noctalia's side back on with its verb or setting. Under a minute, no
   restart. If the shell itself is wedged, `kill -TERM <pid of celestina>` by
   pid (`pidof celestina` is read, never `pkill`): every duty's worker is
   joined on shutdown, the caffeine inhibitor and the gamma are released, the
   wallpaper and bar surfaces go away. Noctalia is unaffected.
2. **After the swap (step 12), the shell does not come up or is unusable:**
   from any terminal in the session (or a VT with `Ctrl+Alt+F3` if there is
   none),
   `cp ~/.config/niri/config.kdl.pre-celestina-cutover ~/.config/niri/config.kdl`
   restores every bind at once (niri reloads live), then `noctalia &` starts
   Noctalia by hand for this login; the next login starts it from the restored
   line 71. The restored line 73 re-enables Noctalia's caffeine and night
   light at that login. The shell, if running, keeps the names it holds until
   it is stopped with `celestina quit` or by pid; Noctalia then takes them on
   its next start (`noctalia` exits or logs the conflict otherwise; stop the
   shell first).
3. **Locked out:** the lock design's §8 and the handover note, unchanged:
   `Ctrl+Alt+F3`, the pid in `$XDG_RUNTIME_DIR/celestina/lock-<display>.state`,
   `kill -KILL` that pid, `swaylock -c 000000` from the VT against the
   session's display, or `Mod+Shift+A` beforehand; `faillock --user <name>
   --reset` as root. For polkit, nothing: without an agent actions fail closed
   and `sudo` works (polkit §7).

The recovery copy, the `swaylock` bind and `sudo` are the three things the
unit never touches. The unit's evidence records the copy's path and that
`diff` against the live file shows exactly the lines of §5.

## 7. The code change (what lands as CEL-2-M)

Small, and last (§4, after step 12):

1. `celestina-rs/crates/celestina-shell/src/settings.rs`: `Duty`'s
   `#[default]` moves from `Off` to `On` for the duties of M-3, through a
   per-field default (`#[serde(default = "...")]`) rather than the enum's
   derive, so `lock` and `polkit` keep `Off` unless their rows passed; the
   `bar: Option<Duty>` default returns `On`; the tests that assert `Off`
   defaults flip, the lenient-read tests stay (an unknown value still
   degrades, now to the field's default). The round-trip test for a 1.0.0 file
   with no `duties` key asserts the new defaults.
2. `celestina/src/app.rs` §6.1 of the program: the capsule-only fallback
   layout used while `bar` was off is removed ("that fallback is a stage of the
   migration, not a second layout to maintain: CEL-2-M removes it"); the
   `bar_layout` live flip stays, the shape change does not. Characterisation
   tests of the bar before, asserted unchanged after.
3. `celestina/scripts/smoke.sh`: the nested runs that start the shell "with
   the defaults" now start it with every duty on, so a new throwaway
   `shell.json` that says `"off"` is what the rest-frame runs need; the run
   that checks "with the duty off the verbs are rejected" keeps its file.
4. `celestina/docs/handover.md`: rewritten as "what Noctalia did, and what
   does it now", with the recovery of §6 and the dates the rows passed;
   `celestina/ROADMAP.md`, `STATUS.md`, `README.md` (the autostart and the
   binds), the CEL-2 ledger row to `done`, the program's plan archived.
5. `VALIDATION.md`: no new row of its own; the rows the swap unblocks are
   listed in the evidence.

Nothing in this change touches `~/.config/niri/config.kdl` or `shell.json`:
those are the author's edits of §4 and §5, recorded in the evidence as the
author reports them. Kind `milestone` (the program closes), so the landing
bumps the minor version.

## 8. Automated checks in a nest before anything real is touched

All in `scripts/dev-session.sh --own-bus` with `env -i`, a throwaway
`XDG_CONFIG_HOME` and the dead system bus `smoke.sh` already uses; none
touches the session, its buses or `~/.config`.

| Check | What it proves | Exists |
|---|---|---|
| `celestina --check` on the deployed binary | theme, fonts, appearance file load; from L, the keybinds child reads the author's `config.kdl` copy | yes |
| `smoke.sh --binary ~/.local/bin/celestina` | every nested run of every unit, including the lock pair beside the shell | yes; SMOKE-FIX (uncommitted) makes a host-throttled nest inconclusive-and-retried instead of red |
| **New: the cutover run** (`smoke.sh`, one more nested run) | the shell started with the §5.1 file (every duty on, `lock` and `polkit` included, the dead system bus): exit 0, one `celestina-panel` per output, a `celestina-wallpaper` surface over the fixture picture, the energy workers armed or unavailable, the lock duty reporting the PAM service missing (F2: warned, locked on request), the polkit registration refused (no polkitd on the dead bus), no frame at rest beyond the documented budget; then `shell.json` rewritten with every duty off and the same surfaces gone | **M adds it**; it is the automated form of "the defaults flip and nothing starts twice" |
| **New: the config probe** | a copy of the author's `config.kdl` with the §5.3 diff applied is parsed by the L keybinds child (`--keybinds-probe`) and by `niri validate -c FILE`: every bind present once, no `noctalia` word left, `swaylock` still on `Mod+Shift+A`, line 73 gone | **M adds it** as a script under `celestina/scripts/` that takes the file path and never reads `~/.config` itself |
| `lock-rehearsal.sh --check`, then R-1 to R-9 | the lock in the nest with a fake logind | yes (H-a) |
| The polkit nested runs | registration on a fake bus, the helper socket faked | yes (I branch) |
| Guards | `check-architecture-contract.sh`, language, documentation, `verify-production.sh` on the landed tree | yes |

The cutover run and the config probe are the only new automation of M; both
run inside `smoke.sh`'s existing nest and bus discipline.

## 9. Dependencies before M, and what runs in parallel

Facts as of 2026-10-23:

| Item | State | Blocks M? | Parallel with |
|---|---|---|---|
| **CEL-2-H-b** (backdrop, clock) | `unit/celestina/CEL-2-H-b`, 6 commits on `main`, clean tree, rows -BACKDROP and -CLOCK written | **no** for the swap; **yes** for M-3's `lock` default (its rows are part of "every LOCK row passed") | I, L, the CI fix |
| **CEL-2-I** (polkit) | `unit/celestina/CEL-2-I`, merged with `main` at `f409de7f`, re-review done (no blocking finding), waits for the author's live validation | **yes** for the swap (program §5: "H and I are required before M"; without an agent `pkexec` prompts have no UI) | H-b, L |
| **CEL-2-L** (overview, shortcuts) | `unit/celestina/CEL-2-L`, one commit, waits for the smoke | **no**: the program lists L before M but M's binds for it are two optional lines; the swap does not need them | everything |
| **SMOKE-FIX** (throttled nest → retry) | uncommitted diff in `unit/celestina/SMOKE-FIX` (`smoke.sh` +88/−23) | **indirectly**: L's landing waits for the smoke, and M's two new nested runs inherit the same throttling | its own session |
| **CI green** | `main` is red: `celestina` fails Clippy 1.99 (`chunks_exact` with a constant size in `celestina-paint`), `celestina-rs` fails two tests on the runner; PR 4 (`unit/suite/CI-GREEN`) fixes them, its last run has `celestina` and `celestina-rust` still red at 18:51 | **yes** for landing anything (the landing contract runs the guards; a red `main` hides a regression M could introduce) | nothing conflicts with it, but every landing waits on it |
| **Removing `celestina-qt`** | not started; the author wants no Qt and no backup | **no** for the swap, **yes** for "the program closes": `docs/projects.toml` entry, `celestina-rs/Cargo.toml` member `celestina-shell-core`, `check-sealed-colours.py`'s generator path, `check-architecture-contract.sh:327`, `celestina-qt.yml`, `AGENTS.md` "Halted projects" (the salvage list stays as history or is rewritten), `radius-baseline.tsv`, the roadmap's fronts table; `celestina/src/clipboard/worker.rs` names it in a head comment only | H-b, I, L; **not** with any landing that touches the root guards |

So: **I is the one unit M waits for**; H-b and L are desirable before M but
not required for the swap; the smoke fix and CI green gate every landing; the
Qt removal is independent of the swap and can run beside the live
validations. In parallel today: (a) the author's live rows of §4 steps 1–8,
which need nothing unlanded; (b) H-b, I and L in their worktrees, each with
its own files; (c) the CI fix; (d) the Qt removal as its own suite unit after
CI is green (it edits the root guards, so it goes alone).

## 10. Decisions only the author can take

1. **Which duties the cutover turns on** (M-2): all twelve, or without
   `phone`/`bubbles`, or without `clipboard`.
2. **Whether `lock` and `polkit` flip their defaults in M** (M-3): only if
   their live rows passed; otherwise they stay `off` in the code and `on` in
   the author's file.
3. **The bar layout** (§5.1): Noctalia's order with `media`, `battery`,
   `power-profile` added, or a different one; whether `weather` gets a
   location.
4. **The autostart form** (M-1): `systemd-cat` wrapper or the bare word.
5. **`Mod+Ctrl+Escape` → `celestina lock`** (a lock without suspend, the
   halted shell's commented line): add it or not.
6. **The brightness keys** (M-4): rebind to `celestina brightness`, or leave
   `brightnessctl` (which does nothing on this machine).
7. **CEL-2-L's chords** (M-5), and whether `Mod+Shift+Slash` leaves niri's
   hotkey overlay.
8. **The order of §4**: as proposed, or lock before clipboard, or polkit
   before lock.
9. **Whether M waits for H-b and L** or lands with only I.
10. **Whether `swaylock` and `Mod+Shift+A` stay after M** (the lock design
    says yes until the author retires it in a separate change).
11. **When the Qt removal happens** relative to M, and whether the
    `AGENTS.md` salvage list is kept as history or deleted with the tree.
12. **The faillock root path** the author keeps (VT root or `ssh` key) before
    step 9, as VAL-CEL2-LOCK-RECOVERY asks.

## 11. What the unit never does

- Never starts, restarts or stops the live shell, Noctalia or the compositor;
  never edits `~/.config/niri/config.kdl`, `shell.json` or anything in
  `/etc`: those are the author's hands, recorded afterwards.
- Never removes `swaylock`, its bind or its PAM service.
- Never runs the lock, the verifier, `pamtester` or a polkit prompt against
  the real session; nests only, with `env -i` and the nest's display.
- Never lands or pushes without the author's direct authorization in chat.

## 12. Verification of this unit

The design and the plan pass the language and documentation guards; the
ledger of the suite plan gains the row `CEL-2-M-DOCS`. The implementation
unit CEL-2-M is verified by §8 and by the author's rows of
[the live validation order](../plans/2026-10-23-cel-2-m-live-validation-order.md).
