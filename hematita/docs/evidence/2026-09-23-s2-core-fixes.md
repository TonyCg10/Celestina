# The descriptor-based deletion's review corrections — S2-A2

- **Date:** 2026-09-23
- **Scope:** `S2-A2` of
  [`../plans/archive/2026-09-23-s2-hardening.md`](../plans/archive/2026-09-23-s2-hardening.md):
  the review findings on `S2-A`'s `hematita-core::usage::remove`
- **Environment:** the author's checkout; fixture under the system
  temporary directory (tmpfs)
- **Artifact:** not applicable — pure crate, no production binary touched

## What changed

1. **Inner mounts refused before anything goes (important).** `S2-A`
   removed a folder's files before opening its subfolders, so a mount two
   levels down was found after its siblings were gone, breaking S1's
   guarantee. `delete_tree` now runs the same descent twice: a read-only
   pass (`openat` with `O_DIRECTORY | O_NOFOLLOW`, `Dir::read_from`,
   `statat` without following, cancellation honoured, no `unlinkat`) that
   refuses `MountRoot` on any inner folder on another device or in the
   boundary set, then the removal pass, which keeps every check as the
   defence against a change between the two. The module doc states the
   guarantee again. Consequence: a folder that cannot be listed, or a tree
   deeper than the cap, also fails the read-only pass, so nothing is
   removed (what cannot be listed cannot be checked for a mount).
2. **Opened folders re-checked (important).** `open_checked` follows every
   `openat` of a folder with `fstat` of the new descriptor and refuses
   `Changed` unless its device and inode are the ones `statat` listed; the
   device check therefore holds on the opened descriptor, not only on the
   earlier `statat`.
3. **Depth cap (minor).** `MAX_DEPTH` is 256: a deeper tree fails closed
   with `Io` in the read-only pass, before the default limit of 1024 open
   descriptors could be reached.
4. **Missing on the way (minor).** `ENOENT` while opening a folder on the
   way to the target is `Refusal::Missing`; `ELOOP` and `ENOTDIR` stay
   `Refusal::Symlink`.
5. **Docs (minor).** `Tree::graft`'s doc says only the old root is zeroed
   and the nodes below stay unreachable; the long line in `walk.rs`'s module
   doc is rewrapped.

## TDD evidence

Tests first. RED: the unit test
`an_opened_folder_is_checked_against_the_identity_it_was_listed_with`
failed to compile (no `open_checked`), and three integration tests failed:
`an_inner_mount_two_levels_down_is_refused_before_anything_is_removed`
(the files of `docs` were gone), `a_missing_folder_on_the_way_is_reported_as_missing`
(`Symlink`), `a_tree_deeper_than_the_cap_fails_closed_before_anything_is_removed`
(cap 4096). After the fix, S2-A's mode-`000` partial-result test failed as
expected, since the read-only pass now refuses first; it became
`an_unreadable_inner_folder_fails_closed_before_anything_is_removed`
(nothing removed), and the partial result is covered by
`a_deletion_stopped_by_an_unwritable_folder_reports_what_it_removed`
(inner folder at mode `555`: listed by the read-only pass, its file refused
by `unlinkat`; three entries and the two files' bytes reported; the mode
restored by `ModeGuard` on drop). The symlink-swap test still refuses.

## Procedure

```sh
cd celestina-rs
cargo test -p hematita-core
cargo fmt --all --check
cargo clippy --locked -p hematita-core --all-targets -- -D warnings
```

## Result

- **Exit:** 0 for all three commands.
- **Observed:** unit tests `88 passed` (87 plus 1); captures `11 passed`;
  `usage_tree` `28 passed` (24 plus 4); doc-tests `0`. No
  `hematita-usage-` directory left behind.

## Limits

- The read-only pass and the removal pass are separate walks: a mount
  created between them is still caught by the removal pass's checks, but
  after whatever it had removed before meeting it.
- The `fstat` re-check is tested where it passes and with a wrong identity;
  a real swap between `statat` and `openat` is not reproducible
  deterministically.

## Follow-up

`S2-B`. `VAL-S2` stays pending.
