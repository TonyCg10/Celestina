# The storage usage domain's review corrections — S1-A2

- **Date:** 2026-09-23
- **Scope:** `S1-A2` of
  [`../plans/archive/2026-09-23-s1-storage.md`](../plans/archive/2026-09-23-s1-storage.md):
  the review findings on `S1-A`'s `hematita-core::usage`
- **Environment:** the author's checkout; fixture under the system
  temporary directory (tmpfs)
- **Artifact:** not applicable — pure crate, no production binary touched

## What changed

1. **Symlinked components (important).** `delete_tree`'s `Outside` check is
   lexical, but the kernel resolves every intermediate component: with
   `ROOT/docs` swapped for a link to another folder after the scan,
   `ROOT/docs/sub` passed as inside and the removal went to the link's
   target. `delete_tree` now `lstat`s every component from the analysed
   folder down to the target's parent and refuses with
   `Refusal::Symlink { at }` when one is not a real directory. The target
   itself may still be a link; it is removed as a link. Residual: a swap
   during the removal itself is not guarded; the window is the removal's
   own duration.
2. **Empty folders (important).** `files_below` counts only regular files,
   so a folder holding only a link, socket or FIFO was offered as empty.
   `Node` gained `others_below`, aggregated by the walk and subtracted by
   `prune` like `files_below`; `empty_folders` requires both to be zero.
3. **Mount boundaries (important).** A bind mount or a btrfs subvolume
   mounted from the same device shares `st_dev` with its parent, so the
   device number alone missed it. `usage::mounts` gained `MountPoint`,
   `parse_mountinfo` (the `/proc/self/mountinfo` format, octal escapes
   decoded, every filesystem type kept) and `mount_targets`. `walk::scan`
   and `remove::delete_tree` take `boundaries: &HashSet<PathBuf>`: the walk
   makes a directory at a boundary (other than the scanned root) a leaf
   flagged `other_device`; the deletion refuses `MountRoot` when the target
   or any directory below it is a boundary. The `st_dev` checks stay. The
   callers in `S1-B` to `S1-D` read `/proc/self/mountinfo` on their threads.
4. **`Removed` (minor).** Its doc comment says `bytes` counts a hard-linked
   inode once even when a name outside the subtree keeps it alive, so the
   space actually freed can be smaller.
5. **Duplicates by allocation (minor, recorded).** `candidates` groups files
   by allocated size, as the design decides. Identical files whose
   allocation differs — a reflinked copy on btrfs, a sparse copy, a
   compressed extent against an uncompressed one — are therefore never
   offered as candidates. This is the spec's decision, not a defect.

## TDD evidence

Tests first, against the new signatures with the old behaviour and
`todo!()` for the mountinfo functions. RED: `cargo test -p hematita-core
--no-fail-fast` — 1 unit test (`mountinfo_names_every_mount_point`) and 4
integration tests (`a_folder_holding_only_a_link_is_not_empty`,
`a_mount_boundary_is_a_leaf_even_on_the_same_device`,
`delete_tree_refuses_a_listed_mount_at_or_below_the_target`,
`delete_tree_refuses_a_folder_swapped_for_a_link_after_the_scan`) failed.
Before the fix, the swapped-folder test ran the removal through the link
into a second temporary directory, reproducing the finding. GREEN after the
implementation, first run.

## Procedure

```sh
cd celestina-rs
cargo test -p hematita-core
cargo fmt --all --check
cargo clippy --locked -p hematita-core --all-targets -- -D warnings
```

## Result

- **Exit:** 0 for all three commands.
- **Observed:** unit tests `87 passed` (86 plus the mountinfo test);
  captures `11 passed`; `usage_tree` `18 passed` (14 plus 4); doc-tests
  `0`. Clippy first reported one needless `mut`, removed. No
  `hematita-usage-` directory left behind.

## Limits

- No test mounts a real filesystem; boundaries are exercised by handing
  the walk and the deletion a set naming fixture folders, and the parser by
  lines in the shape of the author's `mountinfo`.
- The component check closes the scan-to-delete window, not a swap during
  the removal itself.

## Follow-up

`S1-B`. `VAL-S1` stays pending.
