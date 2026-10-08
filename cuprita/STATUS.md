# Cuprita status

- **Updated:** 2026-10-08
- **Implementation:** only CUP-1-F is open; its landing closes it and
  publishes 1.0.0. Red
  runs on NetworkManager, Bluetooth on BlueZ with Cuprita as the pairing
  agent, and Audio on PipeWire through WirePlumber's `wpctl`;
  `CUPRITA_FAKE=1` runs all three over the scripted backends
- **Author validation:** `VAL-C`, `VAL-D` and `VAL-E` pending: the live
  network, Bluetooth and audio checks in [VALIDATION.md](VALIDATION.md)

## Current checkout truth

- The landing of CUP-1-F publishes 1.0.0 (the history row is the landing's). The
  project is registered and builds a release binary. The window shows the
  pill strip with Red, Bluetooth and Audio; Ctrl+1, Ctrl+2 and Ctrl+3 jump
  between them.
- Keyboard: the strip is one Tab stop (Left and Right walk it), Tab then
  reaches the page's switches, buttons and list; the arrows walk a list,
  Enter runs the current row's primary action (join or leave a network,
  switch a VPN, pair, connect or disconnect a device, mute an application),
  Space switches a VPN row and any focused switch, Menu or Shift+F10
  opens a device row's menu, Left/Right move a focused volume slider by
  1 % (Shift: 5 %), Esc closes the dialogs and menus. For a screen reader every row is a list item named by kind,
  name and state (and the bars of four, the battery or the volume), the
  switches are check boxes, and the dialogs announce their title.
- `cuprita-core` holds the models, the `Network`, `Bluetooth` and `Audio`
  traits with typed errors, the scripted fakes, the network ordering and
  signal bars, the device ordering, the pairing-agent state machine and its
  dialog tokens, volume clamping, the NetworkManager client `nm` with its
  change watcher and join follower, and the BlueZ client `bluez` with its
  change watcher and the exported `org.bluez.Agent1` pairing agent (the two
  share the error classification and the debounced watcher in `bus`), and
  the audio client `wpctl` over `wpctl` and `pw-cli` with a watcher that
  polls every 2 s and wakes on `pw-mon` events (64 tests).
- `src/controller/` holds the three section controllers (worker thread per
  section, each re-reading when its client's watcher reports a change) and four
  `QAbstractListModel`s that reconcile snapshots by key; the window shows
  their notices in a pill. `src/agent.rs` carries BlueZ's pairing requests
  to `BluetoothController` and the answers back. Airplane mode turns Wi-Fi
  off and powers the Bluetooth adapter off; while it lasts the adapter
  switch is disabled and switching it on is refused, and it stays off when
  airplane mode ends.
- The pages: Wi-Fi and airplane switches with the network list; the adapter
  switch, search and the device list; output and input cards with a default
  selector, volume (0–150 % in 1 % steps, a tick at 100 %) and mute, the
  application streams, and a profile card per sound card with a choice.
  Joining a protected Wi-Fi network without a saved profile asks for its
  passphrase in a dialog; pairing asks in `PairingDialog` (a PIN to type, a
  passkey to confirm, or one to type on the device). «Buscar» spins while the
  adapter searches; a paired device's menu forgets it. Joining and forgetting
  against the live NetworkManager (`VAL-C`) and pairing and forgetting
  against the live BlueZ (`VAL-D`), and switching output, muting an
  application and changing a profile against the live PipeWire (`VAL-E`),
  await the author's check. Until they pass, nm-applet, Blueman and
  pavucontrol stay installed; their removal is the author's action (see the
  [exit evidence](docs/evidence/2026-10-08-exit.md)).

## Blockers

None recorded.
