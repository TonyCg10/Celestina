# The range bound — MAG-P7-F

- **Date:** 2026-09-10
- **Scope:** `MAG-P7-F` of
  [`../plans/active/2026-09-10-storage-and-retirement.md`](../plans/active/2026-09-10-storage-and-retirement.md):
  `celestina-rs/crates/magnetita-proto/src/storage.rs`,
  `celestina-rs/crates/magnetitad/src/link_wire/storage.rs`,
  `docs/protocol.md`, this record
- **Environment:** the author's browse of the S25U; the workspace's tests
- **Artifact:** `magnetitad`, deployed

## Design

Reading a photo through the mount failed with an I/O error and the
session fell: the phone answered a full megabyte, and with the reply's
own fields the envelope's body was eleven bytes over the link's bound,
which drops the session as it must for any peer. A read or write now
carries at most `MAX_RANGE`, the megabyte less eight kilobytes, refused
at decode on both sides; the daemon's window is that size.

## Procedure

```sh
cd celestina-rs && cargo test -p magnetita-proto -p magnetitad
```

## Result

- **Exit:** 0; the protocol's test encodes a full range and finds it
  inside the envelope's bound.

## Limits

- None beyond the wire's: a browse moves at most a megabyte less eight
  kilobytes per round trip.
