# Evidence: 2026-08-04 live validation failures

- **Date:** 2026-08-04
- **Scope:** author-run validation of the deployed Celestina 0.6.0 bundle on a
  live Niri session
- **Environment:** America/Detroit live session with two physical outputs
  (`DP-1`, 2560x1440 at scale 2, and
  `HDMI-A-1`, 1920x1080 at scale 1); Noctalia initially owned the notification
  and StatusNotifierWatcher names and was stopped temporarily for handover
- **Artifact:** `build/production-artifact.toml`, current and verified; the six
  registered host, helper, style and launcher artifacts were reported installed
  under `~/.local`
- **Corrective plan:** [LVR-1 live validation remediation](../plans/archive/2026-08-04-live-validation-remediation.md)

## Procedure

The author built, verified, deployed and then explicitly activated the
production bundle. The recorded probes included:

```sh
./scripts/verify-production.sh
./scripts/status-production.sh
celestina msg get-state
./scripts/activate-production.sh 2>&1 | tee /tmp/celestina-live.log
celestina msg get-state | tee /tmp/celestina-state-r1.json
playerctl --list-all
playerctl status
playerctl metadata --format '{{playerName}} | {{title}} | {{mpris:length}} | {{mpris:artUrl}}'
celestina msg night-light-toggle
pgrep -a wlsunset
celestina msg caffeine-toggle
systemd-inhibit --list
celestina msg lock
celestina msg lock-and-suspend
busctl --user --no-pager status org.kde.StatusNotifierWatcher
busctl --user --no-pager status org.freedesktop.Notifications
busctl --user get-property org.kde.StatusNotifierWatcher \
  /StatusNotifierWatcher org.kde.StatusNotifierWatcher \
  RegisteredStatusNotifierItems
notify-send -p -a 'Celestina Test' 'Prueba R4' 'Mensaje uno'
```

The launcher, clipboard, audio and microphone controls, DDC brightness,
workspaces, panel focus behaviour, control centre and session menu were also
exercised directly. Testing stopped immediately after the first live
notification exposed the aggregate-provider failure below. No source fix was
made as part of this record.

## Result

| Area | Result | Observed fact |
|---|---|---|
| Production verification | passed | CTest reported 13/13, the release host and compiled `CelestinaStyle` survived the eight-second smoke, and the manifest was verified |
| Panel and workspaces | passed for the normal path | one panel mapped on each output, blur was armed on both, `niriAvailable` was true, each output had one active workspace and only the current workspace was focused |
| Audio, microphone and DDC | passed for exercised paths | gestures and per-output brightness changes reached confirmed state, showed truthful OSD feedback and did not take focus |
| Launcher | passed with a bounded-results observation | launching and name search worked; the initial unfiltered view was shorter than Noctalia's because the provider deliberately publishes at most 24 ranked rows |
| Clipboard | failed in the empty-state path | entries could be selected and removed by keyboard or context menu, but there was no discoverable visible per-row delete action; after `Vaciar`, the empty overlay could not be dismissed normally because its key-owning list was no longer visible |
| Media | failed | Firefox exposed a playing MPRIS player and a non-empty title, but neither panel displayed `MediaMini`, for that stream or for a film |
| Session holds | passed for exercised paths | night light created and released `wlsunset`; caffeine created and released Celestina's `sleep:idle` inhibitor; `lock` and `lock-and-suspend` refused instead of suspending without a locker |
| StatusNotifierWatcher handover | partial | Celestina became the sole owner and retained three registrations (`solaar`, `nm_applet`, `blueman`); item activation, context-menu behaviour and rollback were not exercised before the stop |
| Notification handover | failed | Celestina's provider helper became the sole notification owner, but the first `Notify` made the host reject the aggregate frame and withdraw unrelated provider readings |
| Control centre and session menu | partial | exercised normal paths behaved as described; visible product copy remained in English, the notification row initially reported the foreign owner, and weather was absent because no location was configured |
| Wallpaper/startup integration | failed diagnostics | the wallpaper appeared, but Qt reported an invalid `Accessible` attachment twice and portal app registration failed because app information for `celestina` was not found |

## Critical notification failure

Before the live notification, the session names were owned by the deployed
Celestina processes:

```text
org.kde.StatusNotifierWatcher
PID=77196
Comm=celestina
Exe=/home/toni/.local/libexec/celestina/celestina

org.freedesktop.Notifications
PID=77210
Comm=celestina-provi
Exe=/home/toni/.local/libexec/celestina/celestina-provider-adapter
```

Immediately after the first `notify-send`, the bar lost Wi-Fi, Bluetooth,
audio, CPU and RAM readings and the live log repeated:

```text
Celestina rejected a provider helper frame: a provider published an unusable value
qrc:/qt/qml/CelestinaDesktop/qml/AudioLevel.qml:77: TypeError: Cannot read property 'volume' of undefined
```

Read-only source inspection identified the complete causal chain:

1. `src/provider_adapter/notifications.rs::entry_json` publishes an `actions`
   array inside every notification row, including an empty array.
2. `src/providerstates.cpp::readRow` rejects every array or object nested in a
   provider list row, so the first non-empty notification list invalidates the
   complete provider frame.
3. `src/shellprovidersclient.cpp::applyLine` handles any invalid frame with
   `setUnavailable()`, which clears all previously published aggregate state.
4. QML then receives `undefined` for every provider. `AudioLevel.qml` guards
   its visible text but not the `Accessible.name` binding that reads
   `reading.volume`, producing the secondary TypeError.

The existing private-bus notification test observes the helper's JSON directly
and therefore did not exercise the stricter C++ host decoder. The failure needs
an end-to-end producer-helper-host regression test; accepting arbitrary nested
JSON is not implied by this finding.

## Media finding

The live MPRIS probes returned one player:

```text
firefox.instance_1_2594
Playing
firefox | lofi hip hop radio 📚 beats to relax/study to |  |
```

A live stream is allowed to omit duration and artwork; `MediaMini.qml` only
requires a non-empty `nowPlaying`, so those omissions do not explain the hidden
widget. The provider runs both `playerctl --list-all` and the metadata call
through a 750 ms deadline. The interactive probe took approximately two
seconds for its sequence, making a provider timeout the leading code-level
hypothesis, but no raw helper frame or per-call timing was captured before the
notification failure. The corrective unit must reproduce and measure this path
rather than treating the hypothesis as confirmed.

## Other live diagnostics

Startup emitted these independent diagnostics:

```text
QML Wallpaper: Accessible attached property must be attached to an object deriving from Item or Action
qt.qpa.services: Failed to register with host portal: App info not found for 'celestina'
```

The first corresponds to `Accessible` being attached to the root `Window` in
`qml/Wallpaper.qml`. The portal diagnostic is consistent with the host naming
`celestina` as its desktop file while the deployed bundle has no discoverable
application information for that id. The live wallpaper itself was visible;
these diagnostics do not prove the remaining hotplug, portal-value or Niri
colour checks in `VAL-R7`.

## Limits

- No tray item was activated and no tray context-menu action was selected.
- Notification replacement, close, actions, do-not-disturb, history, unread
  state, phone mirroring and screen-reader behaviour were not run after the
  first notification failed.
- DPMS, helper crash/restart recovery, physical monitor hotplug, forced setting
  write failure and persistence across a shell restart remain untested.
- The malformed bracketed-paste media-format command is not treated as
  evidence; the separate successful `playerctl` probes above are.
- The session-owner rollback to Noctalia was requested after the stop but was
  not confirmed in the captured transcript. It must be checked before assuming
  that Noctalia again owns either session name. A later read-only `busctl`
  attempt from the repository sandbox was denied access to the user bus, so no
  owner was inferred from that failure.

## Follow-up mapping

- `LVR-1-A`: restore truthful media publication and guard bar QML against
  absent provider readings (`VAL-R1-01`).
- `LVR-1-B`: make clipboard deletion discoverable and keep empty-state
  dismissal keyboard-accessible (`VAL-R2-02`).
- `LVR-1-C`: make notification payload and host framing compatible without a
  bad provider withdrawing unrelated state (`VAL-R4`).
- `LVR-1-D`: remove the live wallpaper accessibility and portal application-id
  diagnostics (`VAL-SHELL-03`).
- `LVR-1-E`: translate the exposed shell surfaces completely into Spanish
  (`VAL-COPY-01`).
