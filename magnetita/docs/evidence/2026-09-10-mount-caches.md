# The mount's caches — MAG-P7-E

- **Date:** 2026-09-10
- **Scope:** `MAG-P7-E` of
  [`../plans/archive/2026-09-10-storage-and-retirement.md`](../plans/archive/2026-09-10-storage-and-retirement.md):
  `celestina-rs/crates/magnetitad/src/link_wire/storage.rs`, this record
- **Environment:** the workspace's tests; the author's browse of the S25U
- **Artifact:** `magnetitad`, deployed

## Design

The author found the browse slower than the `sshfs` mount: every kernel
operation was one round trip, and a directory of a hundred entries cost a
listing plus a hundred lookups, a file a round trip per 128 KiB. Now a
listing is asked once per directory and kept fifteen seconds; the lookups
the kernel makes for each entry are answered from it, as are attributes;
a read fetches the wire's megabyte from the offset asked and the kernel's
smaller reads are cut from that window for five seconds. Directories
changed through the mount forget their listing; a write or a truncation
forgets the file's window; a rename forgets everything.

## Procedure

```sh
cd celestina-rs && cargo test -p magnetitad
cargo test -p magnetitad the_phone_is_a_directory -- --ignored
```

## Result

- **Exit:** 0; the loopback and the real mount pass unchanged.

## Limits

- A file changed on the phone within fifteen seconds of a listing shows
  its old size until the cache lapses; content read through a live
  window is at most five seconds old.
