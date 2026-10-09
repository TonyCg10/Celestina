# Cuprita status

- **Updated:** 2026-10-08
- **Version:** 1.0.1
- **Implementation:** CUP-1-A to CUP-1-G are done; CUP-1-H, the
  grouped-card layout the author approved, is open as maintenance (no
  version change).
  Red runs on NetworkManager, Bluetooth on BlueZ with Cuprita as the pairing
  agent, and Audio on PipeWire through WirePlumber's `wpctl`;
  `CUPRITA_FAKE=1` runs all three over the scripted backends
- **Author validation:** `VAL-C`, `VAL-D` and `VAL-E` pending: the live
  network, Bluetooth and audio checks in [VALIDATION.md](VALIDATION.md)

## Current checkout truth

- 1.0.0 is published (the landing deploys it to the author's test prefix). The window
  shows the
  pill strip with Red, Bluetooth and Audio; Ctrl+1, Ctrl+2 and Ctrl+3 jump
  between them.
- Keyboard: the strip is one Tab stop (Left and Right walk it), Tab then
  reaches the page's switches, buttons and list; the arrows walk a list,
  Enter runs the current row's primary action (join or leave a network,
  pair, connect or disconnect a device, mute an application), Space
  switches any focused switch (a VPN has its own switch row), Menu or Shift+F10
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
  wakes on `pw-mon` events and polls every 2 s only while `pw-mon` is
  missing or PipeWire does not answer (68 tests).
- `src/controller/` holds the three section controllers (worker thread per
  section, each re-reading when its client's watcher reports a change) and four
  `QAbstractListModel`s that reconcile snapshots by key; the window shows
  their notices in a pill. `src/agent.rs` carries BlueZ's pairing requests
  to `BluetoothController` and the answers back. Airplane mode turns Wi-Fi
  off and powers the Bluetooth adapter off; while it lasts the adapter
  switch is disabled and switching it on is refused, and it stays off when
  airplane mode ends.
- The pages are grouped cards, each under its section label, and each page
  scrolls as one column. Red: the connection in use (kind, name, its kind
  and IPv4 address and a disconnect button, or a muted line saying there
  is none), the Wi-Fi and
  airplane switches, the other networks with four signal bars and their
  security, and a VPN card with a switch per VPN when one exists.
  Bluetooth: the adapter switch and the search, then the devices. Audio:
  output and input cards with a default selector, volume (0–150 % in 1 %
  steps, a tick at 100 %) and mute, the application streams, and a profile
  card per sound card with a choice.
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

## What CUP-1-H changes

The author's verdict on 1.0.1 was that the content sat too close to the
edges and read as things put one after another. CUP-1-H regroups the three
pages in the suite's grouped-card grammar (see the
[grouped-cards evidence](docs/evidence/2026-10-08-grouped-cards.md)): the
words and controls sit 16 px from each card's edge, rows light on hover and
focus, setting rows are split by hairlines, and the connection in use moves
out of the network list into a card of its own that reads its IPv4 address
(`Network::address`, from NetworkManager's `Ip4Config`). Until a section's
first reading arrives its list cards show a spinner with a muted line, and a
command in flight turns a small spinner beside the card's label.

## What CUP-1-G fixes

From the final review of 1.0 (see the
[final-review evidence](docs/evidence/2026-10-08-final-review.md)):

- The NetworkManager connection gives up on any call after 120 s (BlueZ's
  stays 90 s), so a hung NetworkManager or an unanswered polkit prompt no
  longer holds the network worker forever; the proxy builder and the error
  mapping are one copy in `bus.rs`.
- Notices are Spanish throughout: known D-Bus error names map to the
  not-found, unavailable, denied and busy sentences of `message_es`; any
  other service detail is written to the log (`eprintln!`) and the notice
  says only that the service answered with an error. The same holds for
  `wpctl` failures.
- Audio no longer polls every 2 s while `pw-mon` runs.
- The Wi-Fi passphrase dialog accepts 8–63 characters (or a 64-digit
  hexadecimal key) and says so otherwise; the passphrase and PIN fields are
  marked sensitive for input methods.
- A WEP-only network is labelled as WEP and unsupported instead of
  protected.

## Author validation

`VAL-C` (Red), `VAL-D` (Bluetooth) and `VAL-E` (Audio) in
[VALIDATION.md](VALIDATION.md) are the author's live checks; nm-applet,
Blueman and pavucontrol stay installed until they pass.

## Blockers

None recorded.
