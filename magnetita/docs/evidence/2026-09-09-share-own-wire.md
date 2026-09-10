# Resumable file share both ways on the own wire — MAG-P4-C

- **Date:** 2026-09-09
- **Scope:** `MAG-P4-C` of
  [`../plans/active/2026-09-09-daily-set.md`](../plans/active/2026-09-09-daily-set.md):
  `celestina-rs/crates/magnetita-link/src/{session,lib}.rs`,
  `celestina-rs/crates/magnetitad/src/link_wire/{mod,share}.rs`,
  `celestina-rs/crates/magnetitad/Cargo.toml`,
  `celestina-rs/crates/magnetita-mobile/`, `celestina-rs/crates/magnetita-peer/`,
  this record
- **Environment:** the workspace's loopback tests; the daemon deployed
  through `magnetita/scripts/complete-production.sh`; `magnetita-peer`
  and the S25U against it on the LAN
- **Artifact:** `magnetitad`, deployed

## Design

- **The link** exposes `Transfers`, a cloneable handle over the connection
  for bulk streams, so a transfer task owns its stream while the control
  stream keeps being read. The session accepts bulk streams on a task of
  its own: a `select!` drops the future of a losing branch, and a stream
  dropped half-accepted is a lost transfer.
- **Receiving:** an offer passes the share setting and takes a payload
  permit for the whole transfer; the bytes go to a hidden partial in the
  downloads directory, created by the same policy as the KDE Connect wire;
  a partial left by a broken link is found again by (device, name, size)
  and the acceptance names its length as the offset to resume from. A
  complete file is published without overwriting, only while the device
  is still paired (`if_pairing_allowed` plus the registry), and the UI log
  says where it landed; an interrupted one says it will resume.
- **Sending:** `Command::SendFile` (the `SendFileUri` method Siderita
  already calls) offers, the phone's acceptance names the offset, a task
  streams the file from there and ends with `ShareDone`.
- **Core:** `offer_file`, `write_transfer`, `finish_transfer`,
  `accept_file` (resumes a partial in the given directory, reports the
  end as an event with the path), `reject_file`, `send_text`; `next`
  intercepts the share control messages it must act on and still returns
  them. The peer sends `--send-file` and accepts into its `downloads`.

## Procedure

```sh
cd celestina-rs && cargo test -p magnetita-link -p magnetitad -p magnetita-mobile -p magnetita-peer
cd .. && magnetita/scripts/complete-production.sh
magnetita-peer pair "$(busctl … StartPairing)"
magnetita-peer connect 10.0.0.134:1760 --send-file big.bin --hold 15 &
busctl --user call … SendFileUri ss <peer id> file://…/big.bin
busctl --user call … SendFileUri ss 4eb6f9984054dd25 file://…/magnetita-icon.svg
adb shell ls -la /sdcard/Download/Magnetita/
```

## Result

- **Exit:** 0. Daemon: 92 tests. The loopback test sends half a 300 000
  byte file, drops the link, reconnects, sees the acceptance ask for the
  rest from the bytes held, completes, and reads the published file
  byte-equal; then `SendFile` offers a 100 000 byte file the phone endpoint
  accepts and drains. Link: 11 tests. Clippy clean.
- **On the LAN, 22:33:** the peer sent a 3 000 000 byte random file to the
  live daemon (published in the downloads directory, byte-equal) and
  received the same file from `SendFileUri` (byte-equal); the UI log shows
  the received and sent lines. At 22:34 `SendFileUri` to the S25U landed
  the desktop icon's SVG in the phone's `Download/Magnetita`, 6055 bytes.

## Limits

- A peer that reconnects while its previous session is still open is
  dropped until that one idles out (about 30 s); the first `SendFile` in
  that window goes to the stale session and is lost. The newer-session
  preference noted in `MAG-P3-C` would fix both.
- No size ceiling and no free-space check before accepting.
- `VAL-MAG-12` covers the phone's share sheet with real files.
