# The storage capability on the wire — MAG-P7-A

- **Date:** 2026-09-10
- **Scope:** `MAG-P7-A` of
  [`../plans/active/2026-09-10-storage-and-retirement.md`](../plans/active/2026-09-10-storage-and-retirement.md):
  `celestina-rs/crates/magnetita-proto/src/storage.rs`,
  `celestina-rs/crates/magnetita-mobile/src/storage.rs` and the core's
  events and senders, the peer's `--serve DIR`, `docs/protocol.md`, the
  [discussion](../discussions/2026-09-10-storage-mount-or-dbus.md), this
  record
- **Environment:** the workspace's tests
- **Artifact:** `magnetita-peer`; the `magnetita-mobile` bindings

## Design

- Capability 13: every request carries an id its reply echoes; paths are
  relative to the shared root, `/`-separated, with no empty, `.` or `..`
  component, refused at decode. Listings page at 256 entries with `more`;
  reads and writes carry at most 1 MiB inside the message, so a browse
  needs no extra stream; `state` says whether a root is shared at all.
- The core decodes each request into one `StorageRequest` the application
  answers, and builds the replies; `serve(root, request)` answers from a
  directory on the local file system, which the peer and the daemon's
  tests use in place of Android's document tree.

## Procedure

```sh
cd celestina-rs && cargo test -p magnetita-proto -p magnetita-mobile -p magnetita-peer
```

## Result

- **Exit:** 0. 65 protocol tests (a golden vector for the list, the
  round trips of every message, the refused paths and the oversized read)
  and the directory server's list, read, write, stat, rename and delete.

## Limits

- Bytes inside messages cost one round trip per megabyte: a phone copy runs
  at the link's latency, not its bandwidth; the fixed-id streams of the
  mirror are the path if that ever matters.
