# Cuprita status

- **Updated:** 2026-10-08
- **Implementation:** CUP-1-A, CUP-1-B and CUP-1-C are active; Red runs on
  NetworkManager, Bluetooth and Audio report themselves unavailable until
  CUP-1-D and E bring their clients; `CUPRITA_FAKE=1` runs all three over the
  scripted backends
- **Author validation:** `VAL-C`, `VAL-D` and `VAL-E` not yet requested

## Current checkout truth

- Version 0.1.0. The project is registered and builds a release binary. The
  window shows the pill strip with Red, Bluetooth and Audio; Ctrl+1, Ctrl+2 and
  Ctrl+3 jump between them.
- `cuprita-core` holds the models, the `Network`, `Bluetooth` and `Audio`
  traits with typed errors, the scripted fakes, the network ordering and
  signal bars, the pairing-agent state machine and volume clamping, and the
  NetworkManager client `nm` with its change watcher and join follower
  (28 tests).
- `src/controller/` holds the three section controllers (worker thread per
  section; the network worker re-reads on NetworkManager's coalesced change
  signals, Bluetooth and audio still poll every 5 s) and four `QAbstractListModel`s that reconcile snapshots
  by key; the window shows their notices in a pill.
- The pages: Wi-Fi and airplane switches with the network list; the adapter
  switch, search and the device list; output and input cards with a default
  selector, volume and mute, the application streams and the card profile.
  Joining a protected Wi-Fi network without a saved profile asks for its
  passphrase in a dialog; the pairing dialog is not built yet. Joining and
  forgetting against the live NetworkManager await the author's check.

## Blockers

None recorded.
