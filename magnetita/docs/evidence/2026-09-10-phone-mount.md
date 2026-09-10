# The phone as a directory — MAG-P7-B

- **Date:** 2026-09-10
- **Scope:** `MAG-P7-B` of
  [`../plans/archive/2026-09-10-storage-and-retirement.md`](../plans/archive/2026-09-10-storage-and-retirement.md):
  `celestina-rs/crates/magnetitad/src/link_wire/storage.rs`, the session's
  storage branches in `link_wire/mod.rs`, the `fuser` dependency, the
  plan and roadmap records that archive `MAG-P6` and open `MAG-P7`,
  `VAL-MAG-15`, the README's contract, this record
- **Environment:** the workspace's tests; this host, where `/dev/fuse` and
  `fusermount3` serve the daemon's user
- **Artifact:** `magnetitad`, deployed

## Design

- `StorageClient` sends requests on the session's outbox and matches the
  replies by id with a thirty-second bound; listings page until `more` is
  false.
- `PhoneFs` is the FUSE file system over the client: inodes handed out per
  path and moved on rename, attributes cached two seconds, reads in
  512 KiB chunks, writes chunked the same way, `create` a truncating empty
  write, `setattr(size)` a truncating write at that offset. Every
  operation is one round trip on the link and blocks only FUSE's thread.
- The session mounts at `mount::mountpoint_for(device_id)` when the phone
  says a root is shared, publishes `mounted` and `mountPath` in `Devices1`
  as the `sshfs` mount did, and unmounts when the phone withdraws the root
  or the session ends. `fuser` mounts through `fusermount3` from safe Rust.

## Procedure

```sh
cd celestina-rs && cargo test -p magnetitad
cargo test -p magnetitad the_phone_is_a_directory -- --ignored
```

## Result

- **Exit:** 0. The loopback test lists, stats, reads a range, writes,
  makes a directory, renames, deletes and sees a missing entry refused,
  all against the peer's directory server. The ignored test mounts for
  real: `read_dir`, `read`, `metadata`, `write`, `create_dir`, `rename`,
  `remove_file` and `remove_dir` through `std::fs` on the mount reach the
  served tree, and the mount releases when the session closes.

## Limits

- One mount per phone, read and write, no permissions beyond the daemon's
  user; Android's document tree has none to show.
- The mount test is ignored in the default run because it needs the FUSE
  device; it passed on this host.
