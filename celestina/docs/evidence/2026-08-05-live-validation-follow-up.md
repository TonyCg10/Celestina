# Evidence: 2026-08-05 live validation follow-up

- **Date:** 2026-08-05
- **Scope:** author-run follow-up after `LVR-1-A`; corrective checkpoint
  [LVR-2](../plans/archive/2026-08-05-live-validation-follow-up.md)
- **Environment:** live Niri/Wayland session on Arch Linux; active outputs
  `DP-1` at 2560x1440 logical/1.5 scale and `DP-2` at 1920x1080/1.0 scale;
  Firefox MPRIS player; WirePlumber, DDC/CI, `wlsunset`, systemd-logind and
  xdg-desktop-portal
- **Artifact:** deployed Celestina 0.6.1 production bundle built from the
  `LVR-1-A` checkout; the same source was rebuilt, verified and deployed after
  the live run without reactivating Celestina. Corrective version 0.6.2 was
  subsequently built, verified and deployed without activation

## Procedure

The author exercised each live surface while the agent ran the accompanying
read-only state, D-Bus, process and portal commands. Physical hotplug and visual
outcomes were reported by the author; command outputs were inspected directly.

### Automated baseline and production hand-off

The production exit completed after the live run:

```sh
celestina/scripts/complete-production.sh
```

The release host, both Rust helpers, compiled CelestinaStyle module and desktop
entry were sealed and installed. Rust checks, QML lint, 13/13 CTest cases and
the eight-second release smoke passed. Completion did not activate Celestina;
the session had already returned to Noctalia.

## Result

### Panel, providers, launcher and clipboard

| Path | Result | Observation |
|---|---|---|
| Per-output panel/workspaces | passed | `GetState` reported confirmed output-local workspaces and focus on both active outputs; focus requests and panel geometry behaved normally |
| Audio, microphone, DDC, CPU/RAM and OSD | passed for exercised paths | readings remained visible, controls confirmed their results and the panel had enough space |
| Launcher | passed for normal launch/search | the unfiltered view intentionally showed a short bounded list; searching by application name exposed the remaining indexed applications |
| Clipboard discoverability | passed | visible row delete, keyboard Delete, context-menu deletion and `Vaciar` worked |
| Clipboard empty dismissal | passed | Escape closed the empty overlay and the overlay reopened and closed normally |
| Media after helper restart | passed | title appeared, pause/resume worked, stopping the player removed the region and starting playback restored it |
| Media on full shell startup | **failed** | `playerctl status` saw the playing Firefox source, but no media region appeared after starting Celestina; terminating only `celestina-provider-adapter` caused its replacement generation to publish media immediately |

The media result disproves the earlier claim that clipping was the complete
cause. `LVR-1-A` made the flank capable of showing media, but the initial helper
generation can still miss an already-playing source.

### Tray and notification ownership

Celestina became the sole owner of both `org.kde.StatusNotifierWatcher` and
`org.freedesktop.Notifications` after Noctalia stopped. The watcher retained
Solaar, NetworkManager and Blueman registrations. The tray drawer, left-click
activation and right-click menu worked. Stopping `nm-applet` removed only its
item; restarting it registered the item again without affecting the other
providers.

The notification server passed these live paths:

- ordinary notification and toast;
- replacement with the same notification ID;
- explicit `CloseNotification`;
- action activation returning `show`;
- do-not-disturb withholding an ordinary toast while preserving history;
- critical notification bypassing do-not-disturb;
- unread count, history, keyboard deletion and history clearing;
- notification publication without Wi-Fi, Bluetooth, audio, CPU or RAM
  disappearing.

The notification centre nevertheless **failed** one keyboard requirement:
Escape did not dismiss it after focus moved away from the inner list. Clicking
the panel's notification count closed it. The current handler belongs to an
inner child, so surface dismissal depends on focus ownership.

### Session verbs, control centre and lifecycle

The exercised normal paths passed:

- volume, microphone, brightness and OSD readings;
- night light on/off and truthful `wlsunset` presence;
- killing `wlsunset` restored the normal screen state and cleared the reading;
- caffeine on/off and truthful inhibitor presence;
- killing the active caffeine child cleared the reading;
- killing the aggregate provider helper caused a new helper generation and
  provider recovery;
- DPMS turned displays off and the panels/providers recovered on wake;
- `lock` and `lock-and-suspend` refused with exit status 1 rather than
  suspending without a locker;
- control-centre normal controls and the two-step session-menu confirmations;
- do-not-disturb persistence across a full Celestina restart.

After Celestina was stopped and the session returned to Noctalia, explicit
suspend did not proceed. `systemd-inhibit --list` exposed four processes with
the exact command:

```text
systemd-inhibit --what=idle:sleep --who=Celestina --why=The session was asked to stay awake --mode=block sleep infinity
```

The four holders had PIDs 8957, 8978, 8993 and 9071, shared the same start
second, were reparented to the user manager and survived after both Celestina
processes had exited. They were validated by `/proc/<pid>/cmdline`, terminated
individually, and `systemd-inhibit --list` then reported no Celestina holder.
This is a confirmed held-child lifecycle failure, not merely stale UI state.

Noctalia's own Caffeine command was disabled separately. Its residual `idle`
record did not claim `sleep`; it is not the Celestina defect recorded here.

### Startup diagnostics and Spanish product copy

A clean Celestina restart emitted none of the prior signatures:

```text
Accessible attached
App info not found
TypeError
rejected a provider
unusable value
Failed to register
```

The shell and provider helper reacquired their expected names, and a
post-restart notification worked. Launcher, clipboard, notification centre,
control centre, session menu and panel copy were inspected and reported fully
Spanish. These reruns pass `VAL-SHELL-03` and `VAL-COPY-01` for the exercised
surfaces.

### Appearance portal

The direct backend owned
`org.freedesktop.impl.portal.desktop.celestina-shell` and returned:

```text
color-scheme: uint32 1
accent-color: (0.24313725490196078, 0.56862745098039214, 1.0)
```

Before registration, the public portal returned GTK's different accent
`(0.20784313976764679, 0.51764708757400513, 0.89411765336990356)`. Copying the
generated descriptor alone was insufficient in this Niri session because the
user preference file selected GTK by default. The live integration therefore:

1. installed the generated descriptor byte-for-byte at
   `~/.local/share/xdg-desktop-portal/portals/celestina-shell.portal`;
2. added
   `org.freedesktop.impl.portal.Settings=celestina-shell` to
   `~/.config/xdg-desktop-portal/niri-portals.conf`;
3. restarted `xdg-desktop-portal.service`;
4. read the exact Celestina values through `org.freedesktop.portal.Desktop`;
5. removed both selections and restarted the portal, observing GTK's original
   accent again;
6. restored both selections and observed Celestina's exact values again.

Siderita retained `org.freedesktop.impl.portal.desktop.celestina` throughout,
so its FileChooser backend was not displaced. The restored Celestina Settings
selection remains the final session state.

### Wallpaper and output hotplug

Both active outputs showed their own shell wallpaper and no black fallback.
`DP-2` was physically removed: Niri reduced its active set to `DP-1`, Celestina
removed `DP-2` workspaces, and shell, notification and tray ownership remained
alive. Reconnection restored `DP-2`, its 1920x1080 logical geometry, workspaces
11-15, panel and correct wallpaper immediately without restarting Celestina or
changing `DP-1`.

The author explicitly declined the generated Niri-colour include test because
their current Niri configuration is intentional. That portion is omitted, not
passed or failed.

### Rollback to Noctalia

Stopping Celestina and starting Noctalia restored both watcher and notification
ownership to Noctalia and its panel/tray behaved normally. Celestina was later
started again for additional tests and finally stopped; the session ended on
Noctalia. The four orphan Celestina inhibitors described above were the only
failure discovered after that final rollback.

## Corrective work

| Area | Implemented correction | Automated exit |
|---|---|---|
| Media bootstrap | the first helper generation retries MPRIS discovery every 500 ms for a bounded ten-second startup window, then returns to the five-second idle cadence | `media::tests::the_first_generation_retries_a_late_player_without_a_restart` passed |
| Notification dismissal | `StandardKey.Cancel` belongs to the notification window rather than its inner list | `tst_notifications.qml::test_escape_dismisses_the_centre_at_the_window_boundary` passed in CTest |
| Held-child lifecycle | SIGTERM, SIGINT and SIGHUP release both helper-owned holds before exit; stdin shutdown also receives enough time to drain a bounded command | `held_shutdown::sigterm_releases_a_held_child_before_the_helper_exits` passed with a fake inhibitor process |
| Appearance portal | README now requires descriptor installation, Niri Settings selection, broker restart and the inverse rollback while keeping FileChooser separate | documentation and architecture guards passed |

All four corrections were consolidated into product bug unit `LVR-2-A`; no new
shell capability was added.

### Canonical 0.6.2 exit

```sh
bash scripts/check-architecture-contract.sh
celestina/scripts/complete-production.sh
python3 scripts/version_tool.py check
```

The architecture guard passed. Production verification passed 27 common
fixtures, 27 provider-helper tests including the new process case, 13/13 CTest
cases including the QML shortcut regression, QML lint and the eight-second
release smoke. The verified 0.6.2 bundle was deployed to the normal test prefix
without activating Celestina.

## Limits

- Screen lock and Polkit remain unimplemented and still require Noctalia or
  another approved provider.
- Screen-reader/AT-SPI coverage, a paired-phone notification, forced provider
  write failure and numeric resource ceilings were not available.
- Weather remained absent because no location was configured; no stale weather
  was shown.
- Niri colour adoption was omitted by author choice.
- The launcher result cap was observed but not classified as a defect because
  name search reached the remaining application index.

## Conclusion

Clipboard remediation, provider-frame isolation, tray takeover/rollback,
notification protocol flows, Spanish product copy, startup diagnostics,
appearance portal integration and output hotplug all passed their exercised
paths. Version 0.6.2 implements and automatically verifies the bounded media,
Escape and held-child corrections. Their live validation records remain failed
until the author reruns them. Screen lock and Polkit remain unimplemented, and
several assistive-technology and configuration-dependent cases remain deferred,
so Celestina still cannot replace Noctalia.
