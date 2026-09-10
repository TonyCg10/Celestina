# Media control both ways on the own wire — MAG-P4-D

- **Date:** 2026-09-09
- **Scope:** `MAG-P4-D` of
  [`../plans/archive/2026-09-09-daily-set.md`](../plans/archive/2026-09-09-daily-set.md):
  `celestina-rs/crates/magnetitad/src/link_wire/{mod,media}.rs`,
  `celestina-rs/crates/magnetita-mobile/src/{phone,mobile}.rs`,
  `celestina-rs/crates/magnetita-peer/`, this record
- **Environment:** the workspace's loopback tests; the daemon deployed
  through `magnetita/scripts/complete-production.sh`; the S25U on the own
  wire with the application of `AND-2-D`
- **Artifact:** `magnetitad`, deployed

## Design

- **Phone to desktop:** `MediaState` from the phone maps to the
  registry's `PlayerState` (`Band - Song` as the now-playing line, `-1`
  for an unknown length) through the existing `set_media`, so the desktop
  app's media card and the shell's projection see the own-wire phone
  exactly as they saw the KDE Connect one; an empty player name clears
  it. `Command::Media` (the `MediaAction` method) becomes a
  `MediaCommand` for the player the phone last reported.
- **Desktop to phone:** a per-session `SessionMedia` owns a `playerctl`
  worker while the media setting is on. A `MediaRequest` from the phone
  makes the desktop's states flow for ninety seconds: the player list
  every ten seconds, each player's state every two, players that left
  reported once with an empty state. The phone's `MediaCommand` becomes an
  `MprisRequest` (button, volume) on the same worker; seeking is not in
  the worker's vocabulary yet.
- **Core:** `send_media_state`, `send_media_command`, `request_media`;
  events carry the decoded state or command. The peer sends
  `--media PLAYER|TITLE|ARTIST` and prints media events.

## Procedure

```sh
cd celestina-rs && cargo test -p magnetitad -p magnetita-mobile -p magnetita-peer
cd .. && magnetita/scripts/complete-production.sh
busctl --user --json=short call … ListDevices   # the phone's media fields
```

## Result

- **Exit:** 0. Daemon: 95 tests. The module's tests pin the state
  mapping and the addressing of the desktop's buttons; the loopback test
  sees the media request the session opens with, sends a phone state and
  finds it on the registry entry with its now-playing line and length,
  then feeds `Command::Media(Next)` and receives the `MediaCommand` for
  that player. Clippy clean.
- **On the S25U, 22:44:** as the session opened, the phone reported its
  music player's session: `ListDevices` shows the title, artist, length,
  position and the seek and pause flags of the paused station on the
  registry entry.

## Limits

- Not driven live: the desktop had no MPRIS player at the time, and
  toggling the phone's player from here would have started music on the
  author's phone. `VAL-MAG-12` covers both.
- The phone's cover art is not carried on the own wire yet; the card
  shows text only.
