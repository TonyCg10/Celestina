# Clipboard both ways on the own wire — MAG-P4-A

- **Date:** 2026-09-09
- **Scope:** `MAG-P4-A` of
  [`../plans/active/2026-09-09-daily-set.md`](../plans/active/2026-09-09-daily-set.md):
  `celestina-rs/crates/magnetitad/src/link_wire/mod.rs`,
  `celestina-rs/crates/magnetitad/src/link_commands.rs`,
  `celestina-rs/crates/magnetita-mobile/src/{phone,mobile}.rs`,
  `celestina-rs/crates/magnetita-peer/src/{lib,main}.rs`, the plan and
  roadmap records that archive `MAG-P3` and open `MAG-P4`, this record
- **Environment:** the workspace's loopback tests; the daemon deployed
  through `magnetita/scripts/complete-production.sh`
- **Artifact:** `magnetitad`, deployed

## Design

- **Desktop to phone:** the daemon's clipboard watcher already fills one
  pending slot per connected device id (`PendingClipboards`), newest value
  wins; the own-wire session now drains its own entry on its one-second
  tick and sends `ClipboardText`, and clears it when the session ends. No
  change in `main.rs`.
- **Phone to desktop:** `ClipboardText` from the phone passes the daemon's
  clipboard setting and `is_syncable`, is recorded as last synced (so the
  watcher does not echo it back) and written through a `ClipboardSink`:
  the Wayland adapter in the daemon, a recorder in tests, so no test ever
  writes the author's clipboard. The UI log gets the same line the KDE
  Connect wire writes.
- **On connect:** the daemon sends `ClipboardRequest`; the phone answers
  only while its application is in front, which is Android's rule.
- **Core:** `PhoneSession::send_clipboard`; `Event.text` carries the
  decoded clipboard text so Kotlin never reads the wire; the peer sends
  `--clipboard TEXT` and prints received clipboards. Both ends now
  announce the clipboard capability in their hello.

## Procedure

```sh
cd celestina-rs
cargo test -p magnetitad -p magnetita-mobile -p magnetita-peer
cargo clippy -p magnetitad -p magnetita-mobile -p magnetita-peer --all-targets -- -D warnings
cd .. && magnetita/scripts/complete-production.sh
```

## Result

- **Exit:** 0. Daemon: 88 tests; the new loopback test
  `the_clipboard_flows_both_ways_on_the_own_wire` pairs a phone endpoint,
  sees the desktop's request first, sends a text that lands in the sink
  and in `last_clipboard`, then fills the pending slot and receives the
  desktop's text on the session. The two older loopback tests learned to
  skip the request the session now opens with. Clippy clean; deployed.

## Limits

- Not exercised against the phone or the author's clipboard in this
  record: writing the desktop's clipboard from a test peer would touch
  the live session, and the phone was unplugged. `VAL-MAG-12` is the
  author's run: copy on the desktop, paste on the phone, and back through
  the tile.
- The desktop's current clipboard is not pushed at connect; only changes
  after it are. The KDE Connect wire does push it; the own wire will when
  a session-open read can be kept out of tests.
