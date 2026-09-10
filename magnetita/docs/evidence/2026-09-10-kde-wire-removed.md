# The KDE Connect wire removed — MAG-P7-C

- **Date:** 2026-09-10
- **Scope:** `MAG-P7-C` of
  [`../plans/active/2026-09-10-storage-and-retirement.md`](../plans/active/2026-09-10-storage-and-retirement.md):
  `celestina-rs/crates/magnetita-core` (packets, identity, pairing,
  session, sftp, share, ping, battery, findmyphone, payload ports
  deleted; clipboard, notification and media trimmed to the shared
  shapes), `celestina-rs/crates/magnetita-net` (device, link, discovery,
  TLS session and deadline deleted; payload trimmed to the limiter; the
  certificate's pairing code gone), `celestina-rs/crates/magnetitad`
  (`main.rs` rewritten without the link threads; `admission`,
  `payload_handlers`, `remote_media` and `link_commands` deleted; `mount`,
  `runtime`, `revocation`, `artwork`, `notify`, `media`, `devices` and
  `subprocess` trimmed), `magnetitad.service`, the architecture and
  language baselines, this record
- **Environment:** the workspace's tests, the architecture and language
  contracts
- **Artifact:** `magnetitad`, deployed

## Design

The daemon keeps one wire. What survives in the two crates is what both
ends share: the device certificate, the trust store, the bulk-transfer
limiter, the clipboard rule, the notification shape, the media state and
actions, the mirror's state machine and options. `Devices1` keeps its
surface; `RequestPair` answers that pairing is by QR. The trust store
keeps its file, so a phone pinned by the old wire still lists as paired
until the person forgets it in the app.

## Procedure

```sh
cd celestina-rs && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
../scripts/check-architecture-contract.sh && python3 ../scripts/check-language-contract.py
```

## Result

- **Exit:** 0. The daemon's `main.rs` fell from 922 to 248 lines and the
  baseline follows it; four language-debt rows are gone with their files;
  78 daemon tests, 43 core, 13 net. The desktop app and the shell build
  against the trimmed `magnetita-core`.

## Limits

- The `adb`/`scrcpy` mirror path stays until `VAL-MAG-14` (`MAG-P7-D`).
- A phone that still runs the stock KDE Connect client cannot pair: the
  own application is the only client, as the discussion concluded.
