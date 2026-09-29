# Evidence: copy, move, Trash and restore keep their loss-free promise

- **Date:** 2026-09-26
- **Scope:** `SID-H1-B` (program unit P-8) of the
  [SID-H1 plan](../plans/active/2026-09-26-hardening.md): SID-2, SID-3,
  SID-4, SID-14, SID-15, SID-16 and SID-31 from the
  [Siderita audit](../../../docs/evidence/2026-09-26-monorepo-audit-siderita.md),
  with the matching share of SID-29 (the missing tests) and the two
  `siderita-ops` comments of SID-30; program row P-8 of the
  [monorepo audit](../../../docs/evidence/2026-09-26-monorepo-audit.md).
  SID-20 moved to `SID-H1-D` (ruling R-A17); review round 1 is recorded at
  the end
- **Environment:** session worktree `siderita-SID-H1-B` on branch
  `unit/siderita/SID-H1-B`, stacked on `unit/celestina-rs/RS-H1-A` at
  `14b0e02`; Linux container (kernel 6.18, ext4 `/tmp`), running as uid 0;
  rustc, cargo, clippy and rustfmt 1.97.1 from the pinned toolchain; Cargo
  `--offline --locked` with a target directory of this session's own (the
  shared worktree target served a `celestina-core` built from another
  checkout without `xdg::effective_uid`); `strace` 6.8; no Qt 6 SDK or
  CXX-Qt build, so the `siderita` application crate did not compile here
- **Artifact:** the landing builds it

## Procedure

```sh
cd celestina-rs
cargo test -p siderita-ops --offline --locked
cargo clippy -p siderita-ops --all-targets --offline --locked -- -D warnings
cargo clippy --workspace --all-targets --offline --locked -- -D warnings
cargo check --workspace --all-targets --offline
cargo test -p siderita-archive --offline --locked
cargo fmt --check
RUSTDOCFLAGS="-D warnings" cargo doc -p siderita-ops --no-deps --offline
strace -f -e trace=fsync,unlink,rmdir <siderita_ops test binary> \
  relocate::tests::relocate_by_copy_moves_a_directory_tree --exact
cd ../siderita
rustfmt --edition 2021 --check src/controller/paste.rs src/controller/actions.rs
cd ..
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
```

RED was taken twice. The seven regression tests ported from the auditor's
probes (`scratchpad/poc`, modes `race` and `restore`) and from the finding
text were written first and run against the unchanged crate. The tests
added with the fix (the volume-trust cases and the replacement) were then
shown RED by mutation: the fixed code was replaced, one piece at a time, by
the old behaviour, the tests were run, and the file was restored from a
copy (`cmp` confirmed the restore).

## Result

- **Exit:** every command above exited 0 on the fixed tree.
  `siderita-ops` went from 39 + 23 to 55 + 23 passing tests (16 new unit
  tests) in the first round, and to 62 + 23 after review round 1 (see the
  end); the race test runs 200 rounds in well under a second. Ten
  consecutive runs of the crate's tests all passed. `siderita-archive` (9 +
  13) and the whole workspace still check and lint clean against the
  changed API. `cargo doc` still reports two private-item links that
  predate this unit (`rename.rs` to `crate::reserve`, `trash.rs` to
  `trash_into`); the one this unit introduced was removed. Architecture
  contract OK; language contract OK (148 legacy files ratcheted, no new
  one); documentation contract OK.
- **Observed (RED against the unchanged crate, exit 101, 7 of 46 failed):**

| Test | Failure on the old code |
|---|---|
| `copy::racing_copies_never_report_success_for_a_file_that_is_gone` | "a reported success left no complete file in 41 of 200 rounds" (47 of 200 in a second run) |
| `copy::a_copy_that_loses_its_name_leaves_the_other_file_alone` | `Io { kind: AlreadyExists }`, and the other writer's file deleted |
| `copy::a_folder_copy_that_loses_its_name_leaves_the_other_folder_alone` | same, for a folder and its contents |
| `relocate::a_copy_move_keeps_what_arrived_or_changed_during_the_copy` | "b/new.txt was lost: source None, destination None" |
| `restore::a_relative_record_restores_under_its_volume_top` | `Io { path: "fotos/uno.jpg", kind: NotFound }`: restored against the working directory |
| `trash::an_orphan_body_in_files_is_never_replaced` | the orphan's `files/note.txt` was reused and replaced |
| `volume::a_symlinked_volume_trash_is_refused` | `Some(".../.Trash-0")`: the planted link was used |

- **Observed (RED by mutation, exit 101):**

| Mutation | Tests that failed |
|---|---|
| a volume's per-user Trash accepted as before (`create_dir_all`, no checks) | `a_symlinked_volume_trash_is_refused`, `a_missing_volume_trash_is_created_private`, `a_volume_trash_others_can_write_is_refused`, `a_volume_trash_another_user_owns_is_refused`, `a_symlinked_uid_in_the_shared_trash_is_refused` |
| replacement trashes first and places second (the old paste order) | `a_placement_that_fails_leaves_the_old_entry_in_place` |
| replacement without the ancestor refusal | `replacing_a_folder_that_holds_the_source_is_refused` |

- **Observed (fsync before removal):** the syscall trace of a forced
  copy-move of `src/nested/leaf.txt` shows `fsync` of the copied file, of
  `dst/nested`, of `dst` and of the folder holding `dst`, and only then
  `unlink` of the source file and `rmdir` of `src/nested` and `src`; no
  `remove_dir_all` of the source remains.

### Per finding

- **SID-2 (closed).** `copy_to` creates every entry exclusively and records
  it, with the device and inode it was given, in a journal. A failure or a
  cancellation removes the journal's entries newest first, each only while
  it is still the entry this call created, with `remove_file` and
  `remove_dir` (never `remove_dir_all`). A taken destination name is now
  `OpError::AlreadyExists` and removes nothing.
- **SID-3 (closed).** The copy half of a move (and of a Trash or restore
  across filesystems) syncs every copied file, every copied folder and the
  folder holding the new name before anything is removed. The source is then
  removed from the journal, not by `remove_dir_all`. Each file is stamped
  (device, inode, length, modification and change time) from the open file
  before its first byte is read. At removal it is kept when any of those
  differ, so a write during the copy, even into a part already read, keeps
  it. The change time is not compared for a file with more than one name,
  because unlinking a sibling name changes it. A folder is removed only when
  empty. Whatever stayed is returned in the new `left_behind` field of
  `Moved`, `Trashed` and `Restored`; a restore that left something keeps its
  `.trashinfo`, so the remainder stays listed. The controller reports it as a
  batch line and keeps a cut source on the clipboard.
- **SID-4 (closed).** One rule, `volume::resolve_original(recorded,
  trash_root)`, now serves the listing and restore: a relative `Path=` is read
  from `$topdir` for `.Trash-$uid` and `.Trash/$uid` and from
  `$XDG_DATA_HOME` for the home Trash. Restore goes through
  `volume::restore_target`, which also refuses a relative record with `..`, a
  path still relative, and, in a volume's Trash, any place not under
  `$topdir` once the receiving folder is resolved. The record is read with
  `read_bounded` (64 KiB, regular file only). The listing applies the same
  rule and flags a refused record in `TrashEntry::unrestorable`, which the
  row shows instead of the path (round 2). Records this crate writes into a
  volume's Trash are relative to `$topdir`, as GIO's are, so they restore
  wherever the volume is mounted next; older absolute records from this
  crate that point outside the volume's current `$topdir` (the stick mounted
  elsewhere since) are refused and listed as not restorable.
- **SID-14 (closed).** Trash and restore move with
  `reserve::rename_without_replacing`. Trash also skips a candidate name whose
  `files/<name>` exists and takes the next name if one appears after the
  choice.
- **SID-15 (closed).** A new domain verb pair, `replace_with_copy` and
  `replace_with_move`, places the new entry first (synced) under a hidden
  `.siderita-replace-<pid>-<n>` sibling, then trashes the old one, then renames
  the new one into the freed name without replacing. Each failure gives back
  what earlier steps took (the copy is rolled back, a moved source moved
  home), and a target that holds the source is refused with the new
  `OpError::SourceInsideTarget` before anything moves. `paste.rs` calls it for
  "Reemplazar"; a cut source counts as unmoved only when it is really still
  there. The residual window: if another writer takes the freed name between
  the Trash and the final rename, the new entry is given back and the old one
  stays in the Trash, which the error does not say. A Trash that left part of
  the old entry in place, and a moved entry that cannot go back, are
  reported as that split state (round 1).
- **SID-16 (closed).** The volume is chosen from the entry's own device
  (`symlink_metadata`), and the walk to the mount point starts from the
  entry's canonical folder, so a symlink on `/home` pointing at a drive goes
  to the home Trash. `.Trash-$uid` (and `.Trash/$uid` inside a sticky shared
  `.Trash`) is created `0700`, one level, and used only when it is a real
  directory owned by the user (GIO's rule, ruling R-A19); the listing
  applies the same test. The uid comes from `celestina_core::xdg::effective_uid`;
  when it cannot be read no volume Trash is used, instead of `.Trash-0`. A
  directory that is itself a mount point is refused with
  `OpError::MountPoint` when `/proc/self/mountinfo` lists it, a bind mount
  from the same filesystem included; only when the table cannot be read
  does a device different from its folder's decide (rounds 2 and 3).
- **SID-20 (moved to `SID-H1-D`, R-A17).** The finding's fix moves `siderita-archive`'s
  `Zone` trait into `celestina-core` and gives `trash` a zone. This unit may
  change neither: `celestina-core` is `RS-H1-A`'s owner and outside the
  `siderita:` scope, and `siderita-archive` belongs to the sibling unit
  `SID-H1-A`. Defining a second zone trait in `siderita-ops` would give one
  rule two owners, and changing `trash`'s signature breaks Fluorita, Hematita
  and `fluorita-engine`, which other prefixes own. `DeletionDate` is still
  written in UTC.
- **SID-31 (closed).** The dead `unwritable` and its `#[allow(dead_code)]`
  are gone.
- **SID-29 (this unit's share).** The race, relative-restore, newcomer,
  orphan-body, volume-trust and replace tests above, and one app-crate test
  (`replacing_a_folder_that_holds_the_source_is_refused_and_touches_nothing`
  in `paste.rs`), which could not run here.
- **SID-30 (the `siderita-ops` share).** `purge.rs` linked a nonexistent
  `crate::list_home_trash`; it links `crate::list_trash`. The misleading
  `getuid` comment in `volume.rs` went with the uid change.

### Owners and reuse

- `celestina_core::xdg::effective_uid` replaces `volume.rs`'s `/proc/self`
  owner read with its root fallback.
- `celestina_core::atomic_file::publish_without_replacing` was compared with
  `reserve::rename_without_replacing`, as `RS-H1-A` asked. Its file case does
  not delegate there: a move must fail as a whole when the source cannot be
  unlinked, while the publish treats a failed unlink after the hard link as
  success (both names then hold the file); the hard link also fails on the
  FAT and exFAT volumes most cross-drive moves touch, and a move must hand
  `EXDEV` back to fall back to copying. Directories cannot be hard-linked at
  all. The two stay separate operations.
- The replacement lives in `siderita-ops` because it is a loss-free ordering
  rule the crate already owns for every other verb; the controller only
  chooses copy or move.

## Limits

- The `siderita` application crate did not build: it needs Qt 6 and CXX-Qt.
  The `paste.rs` and `actions.rs` edits were checked by `rustfmt` and by
  reading only, and the new app test did not run.
- No real second filesystem was available, so the cross-device paths ran
  through `relocate_by_copy` directly. The device choice for a symlinked
  entry (SID-16) is checked by reading only.
- `fsync` is proven by the syscall trace, not by a power cut.
- The owner checks ran as uid 0; the foreign-owner case needs privilege and
  prints "skipped" without it. The mount-point case uses `/proc` and prints
  "skipped" where `/proc` is not its own filesystem.
- A write during a copy is seen through the file's timestamps. On a kernel
  without multigrain timestamps, a write in the same clock tick as the stamp
  that leaves the length unchanged can go unseen (kernel 6.18 here has them).
- The overlayfs case of round 2 (files report their layer's device) is
  covered by the decision function's tests with injected devices; no
  overlay was mounted here.

## Follow-up

- SID-20 is the planned row `SID-H1-D`: it waits for a `celestina-core`
  local-time owner that every `trash` consumer can pass.
- `celestina-rs/STATUS.md` says no application calls `xdg::effective_uid` yet;
  after this unit lands `siderita-ops` does, and the next `celestina-rs`
  unit should correct it (it is `RS-H1-A`'s text, outside this prefix).
- `Trashed::left_behind` is discarded by two callers outside this prefix,
  for their owners' ledgers: Hematita's `spawn_trash` step
  (`hematita/src/actions.rs`, the `siderita_ops::trash` call in the trash
  action) and Fluorita's `run_trash` (`fluorita/src/library/work.rs`). Each
  should report a partial Trash instead of counting it as done.
- The author's checks in a real session are `VAL-SID-17` in
  [VALIDATION.md](../../VALIDATION.md).

## Review round 1

Review of `ae46380` asked for one Important and six Minor changes. The same
commands as above ran again on the fixed tree and exited 0: `siderita-ops`
62 + 23 passing (7 new), clippy `-D warnings` for the crate and the
workspace, `cargo fmt --check`, `rustfmt --check` of the four app files
touched, `siderita-archive` 9 + 13, and the three guards. `cargo doc` shows
only the two older private links. Five consecutive test runs passed.

- **I-1 (fixed).** The source file's stamp was taken after its last byte was
  read, so a write into the part already read landed before the stamp and
  the source was unlinked. The stamp is now taken from the open file before
  the first read and kept. New test
  `a_file_rewritten_during_its_own_copy_is_kept` (256 KiB file; `NEWDATA`
  written at offset 0 once 64 KiB are copied; the source's modification time
  is set far back first so the test does not depend on clock granularity).
  RED on `ae46380`: "the rewrite was lost: source None, destination starts
  Some("ooooooo")". The SID-3 sentence above is corrected.
- **M-1 (fixed in Siderita; two follow-ups).** The Trash job
  (`controller/fileops.rs`), undo's restore (`fileops.rs`) and both restore
  verbs (`controller/trash.rs`) now report `left_behind` with one Spanish
  line owned by `controller/display.rs` (`left_behind_line`). A cut paste with
  leftovers pushes the source to `unmoved`, so the clipboard is kept, and
  records no undo for that entry. The Hematita and Fluorita callers are in
  Follow-up. Checked by `rustfmt` and reading; the app crate needs Qt.
- **M-2 (fixed).** A file with more than one name is compared without its
  change time. New test `a_hard_linked_pair_is_moved_whole`. RED on
  `ae46380`: `left_behind` was `[".../src/a-link.txt"]`.
- **M-3 (fixed).** `volume::restore_target` refuses a relative record with
  `..`, and in a volume's Trash an absolute record outside `$topdir` or a
  record through a folder on the volume that is a symlink elsewhere; the
  record is kept. `.trashinfo` reads in restore and the listing go through
  `read_bounded`. New tests `a_volume_record_that_leaves_its_volume_is_refused`
  (three records) and `an_absolute_record_inside_its_volume_restores`. RED
  on `ae46380`: `Path=../escaped-by-probe.txt` restored to
  `volume/../escaped-by-probe.txt`.
- **M-4 (changed, R-A19).** The group/other-write test is gone; GIO's rule
  stands (a directory, not a symlink, owned by the uid), and the sticky-bit
  rule for the shared `.Trash` stays. The test
  `a_volume_trash_others_can_write_is_refused` became
  `an_owned_volume_trash_is_used_whatever_its_mode`, which fails when the old
  mode check is put back. The Limits bullet about FAT, exFAT and NTFS masks
  no longer applies and was removed.
- **M-5 (fixed).** `volume::refuse_mount_point` returns
  `OpError::MountPoint` when an entry's own device differs from its folder's;
  `trash_home_for` calls it first, before anything is created. New test
  `a_mount_point_is_not_sent_to_the_trash` on `/proc`, which fails when the
  refusal is disabled (the test stops at its first assertion, before
  `trash_home_for` could create a Trash on `/`).
- **M-6 (fixed).** A Trash that returns a non-empty `left_behind` is reported
  as a split ("only part of it went to the Trash, to …") and the new entry is
  given back; a moved entry that cannot go back because its source folder
  still holds leftovers names the hidden path and the reason. New tests
  `a_partial_trash_is_reported_and_undone` and
  `a_move_that_cannot_go_back_says_where_it_is`, each RED when its branch is
  disabled.
- **M-7 (done).** The plan row no longer promises a local `DeletionDate`;
  `SID-H1-D` is a new planned row for SID-20 (R-A17), in the plan's scope
  and build order and in the roadmap; the three real-session checks are
  `VAL-SID-17` in `VALIDATION.md` and the row's Author validation; the
  `celestina-rs/STATUS.md` correction and the Hematita and Fluorita callers
  are in Follow-up.

RED by mutation in this round (exit 101, each file restored and compared
with `cmp`): mount-point refusal disabled; mode check put back; the
`restore_target` checks disabled; the split-Trash branch disabled; the
cannot-go-back branch disabled. Each failed exactly the test named above.

## Review round 2

The re-review of `5db0e4f` confirmed round 1 and found one Important
regression and two Minor points. The same commands ran again and exited 0:
`siderita-ops` 68 + 23 passing (6 new), three consecutive runs; clippy
`-D warnings` for the crate and the workspace; `cargo fmt --check`;
`rustfmt --check` of the app files touched; `siderita-archive` 9 + 13; the
three guards. RED was shown by mutation, each file restored and compared with
`cmp`.

- **N-1 (fixed).** Round 1's mount-point refusal compared devices alone, so
  on overlayfs, whose files report their layer's device, every regular file
  was refused, and so was a btrfs subvolume folder. Now only a directory can
  be refused, and a device mismatch is confirmed against
  `/proc/self/mountinfo` (read with `read_bounded`, 8 MiB; the mount-point
  field matched against the entry's canonical path with its octal escapes
  undone); when the table cannot be read the device comparison decides, for
  directories only. The decision is `volume::is_mount_point` with injected
  devices and table answer. New tests
  `a_file_on_another_device_is_not_a_mount_point`,
  `a_directory_is_a_mount_point_only_when_the_table_says_so` and
  `mountinfo_names_its_mount_points_with_escapes_undone`; `/proc` is still
  refused. RED: with round 1's rule put back, the first two fail. The escape
  decoder is now byte-based and shared with the `/proc/self/mounts` reader.
- **Minor A (fixed).** The listing runs `volume::restore_target` and flags a
  refused record in the new `TrashEntry::unrestorable:
  Option<Unrestorable>` (`ClimbsOut`, `Relative`, `OutsideVolume`), with
  `original` then the `Path=` as recorded. Siderita's Trash row and detail
  show the reason in Spanish (`controller/trash.rs::origin_line`, used by
  `selection.rs`) instead of the path. New test
  `a_hostile_volume_record_is_listed_as_unrestorable`, RED when the listing
  goes back to the bare resolution.
- **Minor B (fixed).** `trash_into` writes `Path=` relative to `$topdir`
  (percent-encoded) in a volume's Trash, from the entry's canonical folder,
  and absolute in the home Trash (`volume::recorded_path`). New tests
  `a_volume_record_is_relative_and_survives_a_remount` (trashed under one
  directory, the directory renamed, restored under the new one) and
  `a_home_record_stays_absolute`; the first is RED when absolute records are
  written everywhere. Older absolute records outside the current `$topdir`
  are refused, as SID-4 above says.

## Review round 3

Round 2 was approved with one Minor left: a directory's device was compared
first and the mount table asked only on a mismatch, so a bind mount of a
folder from the same filesystem (one ext4 for the bind and for
`XDG_DATA_HOME`) was not refused, and trashing it emptied the bound folder
into the Trash with `left_behind` naming the mount point.

- **Fixed.** `volume::is_mount_point` now asks `/proc/self/mountinfo` for
  every directory (read once per `trash` call, only for a directory) and
  follows its answer; the device comparison decides only when the table
  cannot be read. New test `a_same_device_bind_mount_is_a_mount_point`
  (an injected table listing `/srv/x` with equal devices), RED before the
  change; `a_directory_is_a_mount_point_only_when_the_table_says_so` now also
  covers the fallback with equal devices. The Limits bullet about same-device
  bind mounts was removed.
- **Cosmetic, fixed.** `volume::lies_under` resolves the deepest existing
  ancestor of the target and keeps the rest as written (refusing `..` in
  it), so the listing marks `fotos/sub/x`, with `fotos` a symlink out of the
  volume and `sub` missing, as not restorable. New test
  `a_missing_folder_behind_a_symlink_out_is_not_under_the_volume`, RED
  before the change.

The same commands ran again and exited 0: `siderita-ops` 70 + 23 passing
(2 new), three consecutive runs; clippy `-D warnings` for the crate and the
workspace; `cargo fmt --check`; `siderita-archive` 9 + 13; the three guards.

## Landing

- **Base revision:** `b14d089c9b963feac17f4b6323e58caac9ee5764`
- **Check:** `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact digest or set does not match the recorded build; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact digest or set does not match the recorded build; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:233de17cd7cb449a401cb38a46acf226dd0d4c320db208faeb9e643b61396306, verification_fingerprint sha256:7d31ed8dfa9c5c7a2b29aea59826d8462f7f9ad91e5309901b6f6673d52257d9; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:daffd51803183d431024314a48141952e4164413585f1ea7398f5a6fd00e772e, verification_fingerprint sha256:5186dd5531f76f4416649288a783cc08e04043f19a7103cf6a0b5b086c9f9bb1; magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:476a67ae25040698dee6b024cab8663084dc1f628009f2749387253ff0bddd1b, verification_fingerprint sha256:7384804718f24132de5b942dd5bc55a361b6d6c582a0b4a5872e6be7dcaea211; fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:52a4b74818f60aba50c252685fea9dc3e661820d85a35d54b4a92548374e0422, verification_fingerprint sha256:33c8ff2eda0afca2a6864b65af6d918717b00555a2e3be4c9491d8c91c61e00c; hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:0af400134e7b6ca60e21b900c19167acf04a66d66d5606d02cac1ba7615f046f, verification_fingerprint sha256:bf933973a88a62a9110f42d5cab15ba9c469487d46317d7c23231d021035f024
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh
