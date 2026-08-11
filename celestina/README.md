# Celestina Desktop

Celestina Desktop is the shell/session component of the suite for a personal
Niri/Wayland environment. It owns a truthful per-output panel, keyboard
overlays and a versioned session command service while Noctalia responsibilities
are replaced in reversible phases.

## Current capabilities

- One 40 px layer-shell panel per output with output-local Niri workspaces,
  active window and confirmed focus requests.
- Clock, phone, CPU/RAM, media, audio/microphone, network, Bluetooth, power
  profile, per-monitor DDC brightness and screenshot request surfaces.
- StatusNotifierItem host, DBusMenu rendering, a stable icon-grid inventory
  with visible/hidden and panel-pin controls, and a watcher service that takes
  over only when no other watcher owns the session name.
- Keyboard launcher over desktop entries and a shell-owned clipboard-history
  overlay.
- `org.celestina.Shell1` plus the transient `celestina msg` client.
- `--pick-output` chooser used by `xdg-desktop-portal-wlr`.

Current implementation state and the next phase are in
[STATUS.md](STATUS.md). Manual Niri/hardware checks are deliberately separate in
[VALIDATION.md](VALIDATION.md).

## Architecture

| Area | Responsibility |
|---|---|
| `../celestina-rs/crates/celestina-shell-core/` | Pure framing, provider/command vocabulary and policy |
| `src/niri_adapter.rs` | Pinned Niri IPC reduction and compositor actions |
| `src/provider_adapter/` | Aggregate bounded non-Qt provider IO |
| `src/*.cpp`, `src/*.h` | Qt process, D-Bus and layer-surface adaptation |
| `qml/` | Panel, menu, launcher, clipboard and chooser presentation |
| `../celestina-style/` | Canonical visual tokens and controls imported from source |
| `tests/` | Rust, QtTest and Qt Quick Test coverage |

The host is C++20 with Qt 6.9+, LayerShellQt 6.6+ and KF6WindowSystem 6.19+.
The helpers are Rust 2021 and the Niri adapter pins `niri-ipc` to the compatible
session protocol version declared in `Cargo.toml`.

## Build and verify

```sh
scripts/build-production.sh
scripts/verify-production.sh
scripts/complete-production.sh
scripts/status-production.sh
```

Build creates the production bundle without mapping a panel. Verify consumes
that same bundle and never activates the shell. Complete is the canonical exit
for an implemented shell bug or milestone: it builds once, verifies those exact
bytes, deploys the on-disk author-test bundle and checks its status without
replacing the live shell. Status reports whether the host, both Rust helpers and
source-imported style inputs are still current.

Activating a shell changes the live session and is intentionally separate:

```sh
scripts/activate-production.sh
```

Run it only when that live mutation is intended. `scripts/run.sh` remains a
human compatibility wrapper; it is not the verification entry point.

For rapid visual iterations against a live Niri session, use:

```sh
scripts/dev-restart.sh
```

It incrementally rebuilds CelestinaStyle and the shell, gracefully replaces
only the Celestina process that owns the session bus name, and runs the
build-tree bytes in the foreground. Press Ctrl-C to stop it. This development
loop does not verify or deploy an artifact and never replaces the required
`scripts/complete-production.sh` exit before a bug fix or milestone closes.

## Session command client

The already-built host also acts as a transient client:

```sh
celestina msg get-state
celestina msg launcher-toggle
celestina msg clipboard-toggle
celestina msg volume-step by=5
celestina msg volume-set level=40
celestina msg mute-toggle
celestina msg mic-mute-on
celestina msg brightness-step by=-5 output=DP-1
celestina msg brightness-set level=60 output=DP-1
celestina msg night-light-toggle
celestina msg caffeine-on
celestina msg displays-off
```

`lock` and `lock-and-suspend` are accepted vocabulary and refused in practice:
this shell has no locker provider yet, and it reports that rather than leaving
a session open that somebody believes is locked.

A corner on-screen display appears whenever a provider publishes a new volume,
microphone or monitor level — from these verbs, from the panel's wheel, from
another application or from a monitor's own buttons. It is driven by readings,
so a verb that changed nothing shows nothing, and it never takes focus or the
keyboard.

A session verb is answered twice: `pending` when the shell forwards it, and
then `confirmed` or `failed` once the provider reports what actually happened —
or does not report it in time. The helper accepting a request is not the device
having changed, so `accepted` is never published as an outcome.

Client mode does not start a shell or claim the session service name. Panel mode
requires a live Wayland compositor with layer-shell support; `offscreen` is a
test mode and never evidence about Niri geometry, focus or blur.

The client exits 0 on `confirmed` and non-zero on `failed` or on no answer, and
prints the final state on stdout — so a verb can be tried from a terminal
before it is ever bound to a key.

## Optional Niri integration

Celestina never edits a Niri configuration. Nothing below is applied by
installing, building or running the shell: it is a block to copy into
`~/.config/niri/config.kdl` by hand, and deleting it is the whole rollback.

### Blur the composed scene

Niri 26.04 [automatically enables `xray`](https://niri-wm.github.io/niri/Window-Effects.html)
whenever a background effect such as blur is active. Xray is the efficient
wallpaper-only path: it deliberately ignores application windows below the
layer surface. Celestina can request the exact finite blur shape through
`ext-background-effect`, but that protocol does not let the client override
Niri's xray policy.

Add this rule when Celestina's panel and interactive glass should blur the real
composed application content below them. The rule deliberately does not match
the wallpaper, toast or OSD surfaces:

```kdl
layer-rule {
    match namespace="^celestina-(panel|panel-menu|panel-child-menu|overlay)$"

    background-effect {
        xray false
    }
}
```

Non-xray effects require Niri to recompute the scene below the glass and are
therefore more expensive. Niri 26.04 also documents them as experimental: the
effect can disappear during window open/close animations and while a tiled
window is being dragged. Removing this rule restores the wallpaper-only xray
path.

### Session key bindings

Each verb needs the tool that carries it, and refuses in one sentence when it
is missing rather than reporting a change that did not happen:

| Verbs | Tool |
|---|---|
| `volume-*`, `mute-*`, `mic-mute-*` | `wpctl` (WirePlumber) |
| `brightness-*` | `ddcutil`, and a monitor that answers DDC/CI |
| `night-light-*` | `wlsunset` |
| `caffeine-*` | `systemd-inhibit` (systemd) |
| `displays-off` | Niri itself |
| `lock`, `lock-and-suspend` | nothing yet — these are refused on purpose |

```kdl
binds {
    XF86AudioRaiseVolume  allow-when-locked=true { spawn "celestina" "msg" "volume-step" "by=5"; }
    XF86AudioLowerVolume  allow-when-locked=true { spawn "celestina" "msg" "volume-step" "by=-5"; }
    XF86AudioMute         allow-when-locked=true { spawn "celestina" "msg" "mute-toggle"; }
    XF86AudioMicMute      allow-when-locked=true { spawn "celestina" "msg" "mic-mute-toggle"; }

    // Brightness names the monitor it means; there is no "the" screen.
    XF86MonBrightnessUp   { spawn "celestina" "msg" "brightness-step" "by=5" "output=DP-1"; }
    XF86MonBrightnessDown { spawn "celestina" "msg" "brightness-step" "by=-5" "output=DP-1"; }

    Mod+Shift+N { spawn "celestina" "msg" "night-light-toggle"; }
    Mod+Shift+C { spawn "celestina" "msg" "caffeine-toggle"; }
    Mod+Shift+D { spawn "celestina" "msg" "displays-off"; }
}
```

Adapt the keys and the output name: `celestina msg get-state` lists the outputs
this session actually has, and the panel's brightness widget already knows which
monitor each panel speaks for.

Two states are held by a child process this shell owns — night light and
caffeine — and both are released when the shell stops, including when it
crashes. Nothing survives a session to be undone by hand.

The old Noctalia bindings remain the rollback and keep working: they are
separate lines calling a different program, so both can coexist while the
handover is checked, and removing either one changes nothing about the other.

## Optional session look

Three things the shell offers and never applies. Each is generated into
`$XDG_DATA_HOME/celestina/generated/` and referenced by you, or not.

**Wallpaper.** The panel's wallpaper menu lets you choose a local folder at
runtime. Its supported images are shown as a thumbnail gallery, in deterministic
pages when the folder exceeds one bounded provider payload; every accepted
image remains selectable for the output whose panel opened the menu. The shell
atomically imports that choice into `$XDG_DATA_HOME/celestina/wallpapers` under
the output's name. Files placed there manually still work — `DP-1.png`,
`HDMI-A-1.jpg`, or `default.png` for every screen without its own image. An
output with no image paints a plain fallback rather than another screen's
picture, and changed files are picked up within a few seconds.

**Niri colours.** `niri-colours.kdl` holds the focus ring and backdrop
generated from the sealed theme, so the compositor's borders match the panel's.
Reference it from `config.kdl`:

```kdl
include "~/.local/share/celestina/generated/niri-colours.kdl"
```

Deleting that line is the whole rollback. The shell never edits your Niri
configuration.

**Appearance portal.** `celestina-shell.portal` registers the shell as the
backend answering `color-scheme` and `accent-color`, so applications stop
guessing and dark dialogs stay dark:

```bash
cp ~/.local/share/celestina/generated/celestina-shell.portal ~/.local/share/xdg-desktop-portal/portals/
```

It sits *beside* Siderita's `celestina.portal`, which serves the file chooser —
they are different backend names and different files. Do not merge them. The
descriptor makes the backend available; Niri must also select it for Settings.
Add this key to the existing `[preferred]` section in
`~/.config/xdg-desktop-portal/niri-portals.conf`:

```ini
org.freedesktop.impl.portal.Settings=celestina-shell
```

Then restart the broker so it rereads both files:

```bash
systemctl --user restart xdg-desktop-portal
```

The shell answers only those two appearance keys and refuses every other
Settings key, so Siderita remains the FileChooser backend. Rollback is exact:
remove the Settings line and `celestina-shell.portal`, then restart
`xdg-desktop-portal` again.

## Leaving Noctalia behind

Two scripts, and the first one decides.

```bash
celestina/scripts/handover-status.sh
```

Read-only: it starts nothing, writes nothing, and can run while both shells
are up. It lists what this shell has taken over and what it has not, reading
`VALIDATION.md` for what you have actually recorded as working — code alone
never counts, because everything here compiles and much of it has never been
seen on a screen.

```bash
celestina/scripts/handover-remove.sh --confirm
```

Refuses while that report is incomplete, which today it is. It uninstalls
nothing: it moves Noctalia's autostart entry aside, so the way back is moving
it back. The rollback file is written **before** anything changes, and if it
cannot be written nothing changes. Without `--confirm` it only says what it
would do.

Screen lock and the polkit agent are on the list and deliberately unbuilt —
they wait on [SHELL-D2](docs/discussions/2026-08-03-first-party-session-lock.md)
and [SHELL-D3](docs/discussions/2026-08-03-polkit-agent.md). Until those are
decided, this session still needs Noctalia for them, and the report says so.

## Project documents

- [The diagnostic journal](docs/diagnostics.md)
- [Current status](STATUS.md)
- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Local agent rules](AGENTS.md)
- [Open discussions](docs/discussions/README.md)
- [Accepted decisions](docs/decisions/README.md)
- [Historical replacement work orders](NOCTALIA-REPLACEMENT.md)
