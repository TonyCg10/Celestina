# Cuprita status

- **Updated:** 2026-10-08
- **Implementation:** CUP-1-A, CUP-1-C and CUP-1-D are active; Red runs on
  NetworkManager and Bluetooth on BlueZ with Cuprita as the pairing agent;
  Audio reports itself unavailable until CUP-1-E brings its client;
  `CUPRITA_FAKE=1` runs all three over the scripted backends
- **Author validation:** `VAL-C`, `VAL-D` and `VAL-E` not yet requested

## Current checkout truth

- Version 0.1.0. The project is registered and builds a release binary. The
  window shows the pill strip with Red, Bluetooth and Audio; Ctrl+1, Ctrl+2 and
  Ctrl+3 jump between them.
- `cuprita-core` holds the models, the `Network`, `Bluetooth` and `Audio`
  traits with typed errors, the scripted fakes, the network ordering and
  signal bars, the device ordering, the pairing-agent state machine and its
  dialog tokens, volume clamping, the NetworkManager client `nm` with its
  change watcher and join follower, and the BlueZ client `bluez` with its
  change watcher and the exported `org.bluez.Agent1` pairing agent; the two
  clients share the error classification and the debounced watcher in `bus`
  (46 tests).
- `src/controller/` holds the three section controllers (worker thread per
  section; the network and Bluetooth workers re-read on their service's
  coalesced change signals, audio still polls every 5 s) and four
  `QAbstractListModel`s that reconcile snapshots by key; the window shows
  their notices in a pill. `src/agent.rs` carries BlueZ's pairing requests
  to `BluetoothController` and the answers back. Airplane mode turns Wi-Fi
  off and powers the Bluetooth adapter off; while it lasts the adapter
  switch is disabled and switching it on is refused, and it stays off when
  airplane mode ends.
- The pages: Wi-Fi and airplane switches with the network list; the adapter
  switch, search and the device list; output and input cards with a default
  selector, volume and mute, the application streams and the card profile.
  Joining a protected Wi-Fi network without a saved profile asks for its
  passphrase in a dialog; pairing asks in `PairingDialog` (a PIN to type, a
  passkey to confirm, or one to type on the device). «Buscar» spins while the
  adapter searches; a paired device's menu forgets it. Joining and forgetting
  against the live NetworkManager (`VAL-C`) and pairing and forgetting
  against the live BlueZ (`VAL-D`) await the author's check.

## Blockers

None recorded.
