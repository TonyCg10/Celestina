# Mount identity, an entry ceiling and bounded duplicate readers — HEM-H1-A

- **Date:** 2026-09-26
- **Scope:** `HEM-H1-A` (program unit P-9) of
  [the hardening plan](../plans/active/2026-09-26-hardening.md); closes
  HEM-3, HEM-4, HEM-8, HEM-9, HEM-10 and HEM-15 of
  [the Hematita and Grafita audit](../../../docs/evidence/2026-09-26-monorepo-audit-hematita-grafita.md).
  Code: `celestina-rs/crates/hematita-core/src/usage/` (`identity.rs` new,
  `walk.rs`, `remove.rs`, `duplicates.rs`, `tree.rs`, `mod.rs`),
  `celestina-rs/crates/hematita-core/tests/usage_tree.rs`, and the
  consumers in `hematita/src/` (`usage_worker.rs`, `analysis_session.rs`,
  `analysis_view.rs`, `analysis.rs`, `actions.rs`)
- **Environment:** session worktree `unit/hematita/HEM-H1-A`, Linux 6.18
  container running as uid 0 with every capability; the same test binaries
  were also run as uid 65534 through `setpriv`. `celestina-rs` toolchain
  rustc and clippy 1.97.1, `cargo --offline` with the shared target
  directory. `unshare` and `mount` from util-linux 2.39.3. No Qt 6 SDK or
  CXX-Qt build and an offline registry without Hematita's zbus tree: the
  `hematita` and `siderita` application crates cannot build here
- **Artifact:** the landing builds it (Hematita, and Siderita, which links
  `hematita-core`)

## What changed

1. **Mounts told by identity (HEM-3).** A new crate-private
   `usage::identity` describes an entry with `statx`
   (`AT_SYMLINK_NOFOLLOW | AT_NO_AUTOMOUNT`, `STATX_BASIC_STATS |
   STATX_MNT_ID`): device, inode, type, allocated and apparent size, and the
   kernel's mount id. `Entry::same_mount` requires the same device and, when
   both are reported, the same mount id; a kernel without the id falls back
   to the device and the mount table, and one without `statx` to `fstatat`.
   - The walk compares every directory with the root's entry, so a bind
     mount or a subvolume on the root's own device is a leaf whatever path
     reaches it; the table stays as a second test.
   - The walk resolves the requested root first, on the thread that scans
     (`fs::canonicalize`, the last component never followed), stores the
     resolved path in `Tree::path`, and refuses with `ScanError::Root` and
     a typed `RootMoved` source when the resolved path is not the folder or
     the mount the requested one named. `ScanError` gained no variant,
     because Siderita matches it exhaustively.
   - `delete_tree` resolves `within` the same way (`Refusal::RootMoved` when
     it moved), opens every folder on the way from the resolved folder's
     descriptor and refuses a folder on the way that is on another mount
     or listed in the table (`MountRoot`, naming it). The target and every
     entry below it, files included (a bind-mounted file), must lie on the
     analysed folder's mount, in the read-only pass and again in the
     removal pass; every opened folder is re-checked by device, inode and
     mount on its descriptor.
   - `check_identity` (the trash and delete admission in `actions.rs`)
     looks the entry up in its holding folder and refuses it as
     `MountRoot` when its mount differs from that folder's.
   - The Hematita app keeps a handed path (command line, D-Bus `Open`,
     Siderita) as given on the Qt thread; the scan worker resolves it, and
     the deletion bound is `Tree::path`, now resolved.
2. **Entry ceiling (HEM-4).** `walk::MAX_ENTRIES` is 10 000 000 entries
   (about 1.5 GB of nodes at the audit's 150 bytes each); `scan` applies it
   and `scan_bounded` takes another. Past it the walk stops with
   `ScanError::TooManyEntries { path, limit }` instead of growing; the
   `NodeId` addressing limit is folded into the same ceiling.
3. **Candidates by size (HEM-8).** `duplicates::candidates` keys by
   `st_size` (the tree's `apparent`), leaving out empty files and the
   sizeless second names of hard links; groups are ordered by the bytes
   they free (every copy's allocation but the largest), then by size.
   `Group::size` and `Verified::size` are now that shared `st_size`.
4. **Bounded content check (HEM-9).** `duplicates::members` copies a
   group's paths and recorded identities out of the tree; `confirm` takes
   that list, so the Hematita worker drops its strong `Arc<Tree>` before
   reading a byte. Each member is looked up first (a device or a FIFO is
   refused without being opened), opened with `O_RDONLY | O_NOFOLLOW |
   O_NONBLOCK | O_CLOEXEC`, and read only when the descriptor is a regular
   file with the recorded device and inode; otherwise the new
   `ConfirmError::Changed` names it. The worker reports that group as
   unreadable, as for a read error.
5. **Batch prune (HEM-10).** `Tree::prune_many` filters each parent's
   children once against a set and skips the root, unknown, repeated and
   no-longer-live ids and ids below another pruned one, so nothing is
   subtracted twice (the old per-id `prune` subtracted a node below an
   already pruned folder from its live ancestors again). `Tree::prune`
   delegates to it; the Hematita session prunes an action's whole result
   with one call, and "select all but one" checks the selection against a
   set instead of scanning it per copy.
6. **Root-safe tests (HEM-15).** The six tests that counted what the walk
   sees add `locked_files()`/`locked_bytes()` (the fixture's mode-000
   `secret.txt` is counted only for root); the candidate tests needed no
   branch once keyed by size. The fixture root is canonical, since the walk
   now reports resolved paths.

## TDD evidence

RED, before any production change:

- HEM-15: `cargo test -p hematita-core --offline` as uid 0 — `usage_tree`
  22 passed, 6 failed (`the_walk_counts_files_once_and_follows_no_link`,
  `progress_is_reported_on_a_large_folder` 606 ≠ 605,
  `candidates_group_files_of_equal_allocation`,
  `confirm_keeps_only_identical_content_together`,
  `a_mount_boundary_is_a_leaf_even_on_the_same_device`,
  `only_a_second_name_of_a_linked_file_is_counted_as_a_hard_link`).
- The new regressions were run against the old API from a temporary probe
  test (deleted afterwards), all four failing:
  - symlinked parent with the mount listed by its real path: the walk
    descended (`other_device=false`) and `delete_tree` removed 2 entries
    behind the listed mount;
  - `/proc/sys/kernel/hostname` under `/`: the deletion reached `unlinkat`
    (`Io`, permission denied) instead of refusing, and `check_identity`
    accepted `/proc`;
  - sizes: `[["short.txt", "longer.txt"]]` grouped (100 and 3000 bytes, one
    block each) and the sparse/dense pair missed;
  - symlink swap: `confirm` followed the link and verified the outside twin
    as a copy; FIFO: `confirm` blocked (`Timeout` after 5 s).
- A real bind mount, from the same probe inside `unshare --mount
  --propagation private` with no mount table handed in: the walk crossed it
  (`other_device=false`, its file counted) and deleting its holder removed
  the file inside the bind mount (`source_kept=false`) before `rmdir` failed
  with `EBUSY`: the data loss HEM-3 describes, reproduced.
- `prune_many`: the unit test failed to compile (no such method).

GREEN, after the change: every test below passes as uid 0 and as uid 65534.
The new tests are `the_walk_stops_at_its_entry_ceiling`,
`candidates_are_keyed_by_size_not_by_allocation`,
`a_file_swapped_for_a_link_after_the_scan_is_neither_read_nor_deleted`,
`a_fifo_in_place_of_a_candidate_does_not_block_the_check`,
`a_mount_listed_by_its_real_path_bounds_a_root_reached_through_a_link`,
`a_mount_on_the_way_or_at_the_entry_is_refused_by_identity`,
`a_bind_mount_on_the_same_device_is_a_boundary_by_its_mount_id` and the
unit test `prune_many_unlinks_each_parent_once_and_counts_nothing_twice`.

The bind-mount test runs its own binary again under `unshare --mount
--propagation private` (adding `--user --map-root-user` when not root),
makes a real `mount --bind` inside, and checks the walk, both deletions
and `check_identity` with an empty mount table; it unmounts before the
fixture is removed and skips with a printed reason where no namespace or
no bind mount is allowed.

## Procedure

```sh
cd celestina-rs
cargo test -p hematita-core --offline
cargo clippy -p hematita-core --all-targets --offline -- -D warnings
cargo fmt -p hematita-core --check
# the same binaries as an unprivileged user
cd /tmp
setpriv --reuid=65534 --regid=65534 --clear-groups env HOME=/tmp \
  <target>/debug/deps/usage_tree-<hash> --nocapture
setpriv --reuid=65534 --regid=65534 --clear-groups env HOME=/tmp \
  <target>/debug/deps/hematita_core-<hash>
setpriv --reuid=65534 --regid=65534 --clear-groups env HOME=/tmp \
  <target>/debug/deps/captures-<hash>
cd <worktree>
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
```

## Result

- **Exit:** every command above exited 0.
- **Observed:** as uid 0, 95 unit, 11 capture and 35 `usage_tree` tests
  passed (the bind-mount test's namespace copy: 1 passed, 34 filtered out);
  three tests printed their existing root skips. As uid 65534, the same
  95, 11 and 35 passed with no skip, the bind mount made through a user
  namespace. Clippy with `-D warnings` and `fmt --check` were clean. The
  architecture, language and documentation guards printed OK.
- The swap test first failed after the fix because the removed file's
  inode number was given to the new link at once, and a device and inode
  pair cannot tell the two apart; the test now moves the file aside, as a
  real swap does, and the limit is recorded below.

The canonical owner of an entry's identity is `hematita-core::usage::identity`;
the walk, the deletion and the content check each used their own `lstat`,
`statat`/`fstat` or `File::open` before and now all delegate to it, and no
second path remains. Dependency direction is unchanged (no new dependency;
`rustix` `fs` already provided `statx`, `open` and `mkfifoat`).

## Limits

- The `hematita` and `siderita` application crates were not compiled or
  tested here; their edits (`usage_worker.rs`, `analysis_session.rs`,
  `analysis_view.rs`, `analysis.rs`, `actions.rs` doc) were checked by
  reading against the new signatures. Siderita needs no edit: `scan`,
  `ScanError` and the mount-table functions keep their shape
  (`TooManyEntries` only gained a field, which its `{ .. }` pattern
  accepts).
- Behaviour visible in the apps that the author should see on a real
  session: a folder reached through a link is analysed and shown under its
  resolved path (Hematita's tree and deletion bound, Siderita's current
  path and first crumb); duplicate rows show the files' size (`st_size`)
  and are ordered by the space they free; a scan past ten million entries
  fails as Hematita's generic failure and Siderita's `too-many`.
- The deletion and trash identity (`Scanned`) compares the tree's kind
  (folder, file, other), not the full file type: two "other" entries (a link
  and a FIFO) reusing one inode number are not told apart, and both are
  removed as a name, never descended into. Time stamps and sizes are not
  compared, so a file written to after the scan is still accepted.
- On an overlay filesystem without `xino` whose layers live on different
  filesystems, a file reports its layer's device rather than the overlay's,
  so the deletion's per-entry mount check refuses it as `MountRoot`: it
  fails closed there instead of deleting.
- `scan_subtree` (the graft after a stopped deletion) applies the entry
  ceiling to the subtree it scans, not to the whole tree it is grafted into.
- The walk still lists folders by path, so a folder swapped for a link
  between its lookup and its listing is counted where the link leads (its
  subfolders are then leaves by mount identity); it only counts, never
  deletes.
- Kernels older than 5.8 report no mount id: there the device number and
  the mount table (now compared under the resolved root) decide, as
  before.

## Review corrections

The review of this unit found one important and four minor points; one
further commit on the branch addresses them.

1. **Important — the actions tests on a tmpfs `/tmp`.** `run_one` in
   `hematita/src/actions.rs` used `std::env::temp_dir()` itself as the
   admissible item; `check_identity` now refuses a mount root, so where
   `/tmp` is a tmpfs both tests calling it would fail at the landing's
   `cargo test`. It now makes a folder of its own under the temporary
   directory (`TempFolder`, removed on drop) and every assertion is kept.
2. **The kind in the identity.** `remove::Scanned { dev, ino, kind }`
   (`Scanned::of(node)`) replaces the `(dev, ino)` pair in `delete_tree`'s
   `expected` and in `check_identity`; `actions::Item` carries `kind` from
   the node and hands `Item::scanned()` to both. An inode number reused by
   another kind of entry is `Changed`. Test:
   `an_inode_number_reused_by_another_kind_of_entry_is_refused` (the same
   device and inode recorded as another kind, then a file removed and a
   link made in its place). Its RED is the first round's observation: the
   swap test's link received the removed file's inode number and the
   deletion accepted it (`Removed { entries: 1 }`).
3. **`EPERM` falls back.** `identity` treats `EPERM` from `statx` like
   `ENOSYS` and answers through `fstatat`/`fstat`, so a seccomp profile that
   refuses `statx` cannot turn every entry into a vanished one.
4. **Author validation.** `VAL-HEM-H1` in `hematita/VALIDATION.md` covers the
   resolved path Siderita now shows, the bind-mount refusals and the
   duplicate sizes; the ledger row names it.
5. **Limits.** The overlay filesystem and `scan_subtree` lines above.

Commands after the corrections, all exit 0: `cargo test -p hematita-core
--offline` as uid 0 and the same binaries as uid 65534 (95 unit, 11
capture, 36 `usage_tree`), clippy `-D warnings`, `fmt --check`, and the
three guards. The `actions.rs` and `analysis_session.rs` edits were checked
by reading: `Kind` is imported where `Item` names it, `Scanned` is `Copy`,
and `TempFolder` uses only `std`.

## Follow-up

- `HEM-H1-B` (P-18) adopts the rest of the storage publication findings.
- Out of this unit: a compact node layout (names in one arena) would lower
  the ceiling's memory; single `toggle`s still scan the selection, which is
  O(k) per keystroke and was left as is.
