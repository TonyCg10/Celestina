# Evidence: 2026-08-07 LVR-3 live validation closure

- **Date:** 2026-08-07
- **Scope:** the deferred live evidence for `LVR-3-A` through `LVR-3-G`
- **Environment:** a live Niri session on celestina 0.6.8, followed by a
  controlled return to Noctalia on the author's normal monitors and network
- **Artifact:** commit `497d60f`, already built, verified and deployed by the
  canonical production exit before activation

## Procedure

Noctalia first owned StatusNotifierWatcher and Notifications. It was stopped as
one transient unit, Celestina was started as `celestina-transition.service`, and
the host and both helpers were checked inside that cgroup. The author then
exercised the panel, four registered tray items, tray actions and dismissal,
first-generation MPRIS media, Bluetooth retention through a power cycle, and
output hotplug with prompt DDC brightness discovery. The complete Celestina
unit was stopped before Noctalia was recreated, and the kernel journal was read
after the retained crash interval.

## Result

- Slack, Solaar, NetworkManager and Blueman all reached the drawer and retained
  left-click, right-click and outside-click behavior.
- An already-present media player appeared without replacing the helper; its
  title, transport, progress, disappearance and return followed MPRIS.
- Bluetooth remained visible with no connection, while powered off and after
  returning to the powered state.
- A newly enabled output mapped its panel and gained a working brightness
  control within seconds. No concurrent or surviving `ddcutil` was observed.
- Wi-Fi remained the default route and its text stayed present throughout the
  exercised session.
- Celestina stopped without leaving its host, helpers, DDC child, `wlsunset`,
  inhibitor, held sleep, transient unit or D-Bus names. Noctalia reclaimed both
  session names and restarted its own DDC and wallpaper paths.
- Four minutes after the return, the kernel had recorded no matching fence,
  VCN, flip, PCIe or device-loss error. This exceeded the retained crash's
  approximately 82-second return-to-first-fence interval.

## Limits

The Ethernet link in this session carries laptop image mirroring rather than
Internet. Deliberately disconnecting Wi-Fi from the test terminal was therefore
not a safe offline experiment, and that half of `VAL-R1-NET` remains deferred.
No recurrence is strong negative reproduction evidence, not proof that a
lower-probability kernel, firmware, PCIe or hardware fault is impossible.
Screen-reader, weather, connected-Bluetooth-device and full Noctalia-removal
validation remain in their own deferred cases and do not keep LVR-3 open.
