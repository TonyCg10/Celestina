# Cuprita status

- **Updated:** 2026-10-08
- **Implementation:** CUP-1-A and CUP-1-B are active; the three pages run
  over the scripted backends (`CUPRITA_FAKE=1`); without it every section
  reports itself unavailable until CUP-1-C..E bring the real clients
- **Author validation:** `VAL-C`, `VAL-D` and `VAL-E` not yet requested

## Current checkout truth

- Version 0.1.0. The project is registered and builds a release binary. The
  window shows the pill strip with Red, Bluetooth and Audio; Ctrl+1, Ctrl+2 and
  Ctrl+3 jump between them.
- `cuprita-core` holds the models, the `Network`, `Bluetooth` and `Audio`
  traits with typed errors, the scripted fakes, the network ordering and
  signal bars, the pairing-agent state machine and volume clamping (14 tests).
- `src/controller/` holds the three section controllers (worker thread per
  section, 5 s poll) and four `QAbstractListModel`s that reconcile snapshots
  by key; the window shows their notices in a pill.
- The pages: Wi-Fi and airplane switches with the network list; the adapter
  switch, search and the device list; output and input cards with a default
  selector, volume and mute, the application streams and the card profile.
  The Wi-Fi password and pairing dialogs are not built yet.

## Blockers

None recorded.
