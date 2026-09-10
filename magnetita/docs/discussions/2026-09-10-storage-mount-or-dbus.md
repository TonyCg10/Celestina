# Siderita's phone browsing: a FUSE mount or a D-Bus surface

- **Opened:** 2026-09-10
- **Status:** open
- **Question:** when `storage` over the own wire replaces the `sshfs` mount,
  does the daemon expose the phone as a FUSE mount at the same runtime path,
  or as a Siderita-facing D-Bus surface (list, stat, read, write)?

## Context

Siderita browses the phone through `Devices1`'s `mounted` and `mountPath`
fields: a phone is a directory, and every one of Siderita's operations
(open, thumbnails, copy, move, archive) works on paths. The mount today is
`sshfs` against the KDE Connect sftp plugin, which `MAG-P7-C` removes. The
roadmap left the seam to a discussion opened at `MAG-P7`'s start.

## Strongest case

A FUSE mount, served by the daemon itself over the link's `storage`
capability. Siderita changes nothing: the same fields, the same path,
the same operations; `siderita-ops` copies with `std::fs` as it does on any
volume. The daemon keeps one owner for the phone's storage and the kernel
does the caching, the permissions and the partial reads. The `fuser` crate
speaks the protocol from safe Rust and mounts through `fusermount3`, which
`sshfs` already needs on this host, so no new host requirement appears.

## Counter-case

A D-Bus surface avoids a kernel round trip per operation and the
`/dev/fuse` dependency, and could expose Android's document metadata
(MIME types, MediaStore ids) that a directory cannot. But it would make
Siderita grow a second file model next to paths — every operation that
today is `std::fs` would need a phone-specific branch — and every other
consumer (the shell's file dialogs, a terminal) would lose the phone.

## Alternatives

- FUSE in the daemon, at `$XDG_RUNTIME_DIR/magnetita/<device-id>/`. Leading
  option; `MAG-P7-B` implements it.
- A D-Bus surface consumed only by Siderita. Rejected for the second file
  model unless the author overrides.
- A GVfs/KIO backend. Rejected: neither is in the suite's dependency set.

## Falsifiers and evidence needed

Siderita opening, copying to and from the phone through the FUSE mount with
its existing tests and the peer serving a directory; the daemon's own tests
mounting and reading through `std::fs`. If FUSE cannot be mounted from the
daemon's unit on this host, the D-Bus surface returns.

## Conclusion

Open for the author's word. The leading option, **a FUSE mount at the same
path**, is implemented provisionally under "sigue con MAG-P7" as
`MAG-P7-B`; reversing it is that one unit and touches no other.
