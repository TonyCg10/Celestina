# MAG-P7 — Storage over the own wire and retiring the second path

- **Opened:** 2026-09-10
- **Plan ID:** storage-and-retirement
- **Status:** active
- **Authorization:** the author said "sigue con MAG-P7" on 2026-09-10 with
  `MAG-P6` implemented
- **Scope:** magnetita, magnetitad, magnetita-core, magnetita-net,
  magnetita-proto, magnetita-mobile, magnetita-peer
- **Implementation checkpoint:** MAG-P7
- **Author-validation checkpoint:** `VAL-MAG-15` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md); `MAG-P7-D` waits on
  `VAL-MAG-14`

## Hypothesis

With every daily capability, control and the mirror on the own link, the
phone's files are one more capability: the desktop asks for listings,
entries and ranges, the phone answers from the document tree the person
shared, and the daemon presents the answers as a FUSE directory at the
path Siderita already browses. Then nothing needs the KDE Connect wire, its
pairing or its `sshfs` mount, and they go.

## Tangible outcome

Siderita browses the phone through the daemon with no `sshfs`; the daemon
binds one port, the own wire's; `magnetita-core` and `magnetita-net` hold
only what both ends share.

## Scope

- `MAG-P7-A` — the `storage` capability on the wire: list, stat, read
  ranges, write, mkdir, rename, delete, with the phone's state; the core's
  requests as events and its replies; the peer serving a directory; the
  discussion that chose the seam.
- `MAG-P7-B` — the daemon's storage client and the FUSE file system over
  it, mounted while the phone shares a root; `MAG-P6` archived and this
  plan opened.
- `MAG-P7-C` — the KDE Connect wire removed: `magnetita-net`'s link,
  discovery, TLS session and payload sockets, `magnetita-core`'s packets,
  pairing and session, the daemon's link threads, admission, payload
  handlers and the `sshfs` mount; the service unit renamed.
- `MAG-P7-E` — the mount's caches: listings and attributes answered
  once per browse, reads fetched by the wire's megabyte and served to the
  kernel's smaller reads from that window.
- `MAG-P7-F` — the range bound: a read or write carries the envelope's
  megabyte less room for its own fields, so a full window never trips the
  link's body bound.
- `MAG-P7-D` — the `adb`/`scrcpy` mirror path removed once `VAL-MAG-14`
  has observed the own mirror after a reboot; `Mirror1` keeps its methods.

## Exclusions

- Keeping any second path.
- Android's `MediaStore` beyond the document tree: the shared root is what
  the person picked in the system's picker.

## Build order

1. `MAG-P7-A`, `-B`, `-C`, `-E`, `-F`; `-D` after `VAL-MAG-14`.

## Implementation exit

The storage client browses the peer's tree in the loopback test and the
mounted directory reads and writes through `std::fs` in the daemon's own
mount test; the workspace builds without the KDE Connect crates' wire and
`scripts/complete-production.sh` passes; the architecture contract records
the shrunk daemon.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| MAG-P7-A | `magnetita:` | done | [inventory](../../inventories/2026-09-10-storage-and-retirement/MAG-P7-A.numstat.tsv) | 14 files, +1380/-3 | The `storage` capability, the core's events and replies, the peer's directory | [record](../../evidence/2026-09-10-storage-wire.md) | `VAL-MAG-15` |
| MAG-P7-B | `magnetita:` | done | [inventory](../../inventories/2026-09-10-storage-and-retirement/MAG-P7-B.numstat.tsv) | 19 files, +1508/-126 | The daemon's storage client and FUSE mount; `MAG-P6` archived and `MAG-P7` opened | [record](../../evidence/2026-09-10-phone-mount.md) | `VAL-MAG-15` |
| MAG-P7-C | `magnetita:` | done | [inventory](../../inventories/2026-09-10-storage-and-retirement/MAG-P7-C.numstat.tsv) | 45 files, +342/-8045 | The KDE Connect wire, pairing v8 and the `sshfs` mount removed | [record](../../evidence/2026-09-10-kde-wire-removed.md) | `VAL-MAG-15` |
| MAG-P7-E | `magnetita:` | done | [inventory](../../inventories/2026-09-10-storage-and-retirement/MAG-P7-E.numstat.tsv) | 5 files, +178/-26 | The mount's listing, attribute and read-window caches | [record](../../evidence/2026-09-10-mount-caches.md) | `VAL-MAG-15` |
| MAG-P7-F | `magnetita:` | done | [inventory](../../inventories/2026-09-10-storage-and-retirement/MAG-P7-F.numstat.tsv) | 7 files, +81/-8 | The range bound under the envelope's body limit | [record](../../evidence/2026-09-10-range-bound.md) | `VAL-MAG-15` |
| MAG-P7-D | `magnetita:` | planned | `celestina-rs/crates/magnetitad/src/mirror.rs`, `mirror_discovery.rs`, `src/devices.rs` | — | The `adb`/`scrcpy` mirror path removed after `VAL-MAG-14` | record | `VAL-MAG-14` |
