# The own wire deployed: the headless peer pairs, reports, rings and is forgotten against the live daemon — MAG-P2 exit, first half

- **Date:** 2026-09-09
- **Scope:** the implementation exit of `MAG-P2` in
  [`../plans/active/2026-09-09-link.md`](../plans/active/2026-09-09-link.md),
  run on the author's request after `MAG-P2-A` to `-D` closed
- **Environment:** the author's host, `magnetitad` deployed by
  `magnetita/scripts/complete-production.sh` at 13:35:59 from the tree at
  `3419cff` (which also carries the author's announcer change), the real
  `wlan0` address `10.0.0.134`, Avahi, the KDE Connect phone (S25U) and a
  second KDE Connect desktop (`neko-void`) live on the other wire
- **Artifact:** `~/.local/bin/magnetitad`, verified and deployed by the
  canonical scripts (`artifact: magnetita current and verified`)

## Procedure

```sh
magnetita/scripts/complete-production.sh
journalctl --user -u magnetitad -n 40 --no-pager | grep -E "link|own wire"
avahi-browse -rpt _magnetita._udp
export MAGNETITA_PEER_DIR=/tmp/peer-test
URI=$(busctl --user call org.celestina.Magnetita /org/celestina/Devices1 org.celestina.Devices1 StartPairing | sed -E 's/^s "(.*)"$/\1/')
magnetita-peer pair "$URI"
magnetita-peer connect 10.0.0.134:1760 --battery 42 --hold 25 &
busctl --user call … ListDevices
busctl --user call … Ring s <peer id>
busctl --user call … Forget s <peer id>
```

## Result

- **Exit:** 0 for every command
- **Observed:**
  - The production build passed format, Clippy, the crate tests, QML lint
    and the isolated smoke; the daemon restarted once and came back with
    `[dbus] serving org.celestina.Devices1 and org.celestina.Mirror1`,
    `[link] own wire listening on 0.0.0.0:1760`, the S25U "already paired"
    on the KDE Connect wire and `neko-void` accepted on it — the old wire
    unchanged.
  - Avahi resolved `_magnetita._udp` for this host's device id on `wlan0`
    (IPv4 and IPv6) with `name=Celestina`.
  - `StartPairing` returned a `magnetita://pair?` text; the peer paired
    with it in one call and pinned the desktop's fingerprint; the daemon
    logged `[paired] magnetita-peer at 10.0.0.134:43128 on the own wire`.
  - The peer reconnected as a pinned device (`[accepted] … on the own
    wire`), `ListDevices` listed `magnetita-peer` with `battery 42` and
    `paired true`, `Ring` reached the peer as `find: ring`.
  - `Forget` cut the session within its tick (`forgotten; session
    closed`), the peer saw `connection lost`, and `ListDevices` no longer
    listed it: the barrier holds on the own wire.

## Limits

- The peer ran on the same host, dialling the LAN address through a real
  interface; the run from *another* host, and a socket migration keeping
  the session, are still owed. Loopback tests prove the migration
  mechanism; the checkpoint asks for it observed.
- Only `battery` and `find` cross the wire.

## Follow-up

The second half of the exit: `magnetita-peer` from another machine on the
LAN. `MAG-P3` does not depend on it.
