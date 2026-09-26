# Evidence: dormant shared owners in celestina-core

- **Date:** 2026-09-26
- **Scope:** `RS-H1-A` (program unit P-6) of the
  [RS-H1 plan](../plans/active/2026-09-26-hardening.md): the owner halves of
  RS-1, RS-2/FLU-2, RS-3, RS-4, RS-6, RS-9 and RS-10, and RS-7 and RS-19 from
  the [shared crates audit](../../../docs/evidence/2026-09-26-monorepo-audit-shared-crates.md)
  (FLU-2 in the [Fluorita audit](../../../docs/evidence/2026-09-26-monorepo-audit-fluorita.md));
  contradiction 4 and ruling R-A3 of the
  [monorepo audit](../../../docs/evidence/2026-09-26-monorepo-audit.md)
- **Environment:** session worktree `celestina-rs-RS-H1-A` on branch
  `unit/celestina-rs/RS-H1-A` from `2a3c74f`; Linux container (kernel 6.18),
  running as uid 0; rustc and cargo 1.97.1 from the pinned toolchain; Cargo
  `--offline` with the shared session target directory; no Qt 6 SDK, CXX-Qt
  build, libmpv link, Wayland session or AT-SPI bus
- **Artifact:** the landing builds it; this workspace is not versioned and the
  unit bumps no product

## Procedure

```sh
cd celestina-rs
cargo test -p celestina-core --offline --locked
cargo clippy -p celestina-core --all-targets --offline --locked -- -D warnings
cargo clippy --workspace --all-targets --offline --locked -- -D warnings
cargo check --workspace --all-targets --offline --locked
cargo fmt --check
RUSTDOCFLAGS="-D warnings" cargo doc -p celestina-core --no-deps --offline
cd ..
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
```

RED was demonstrated after the owners were written, by mutation: each new
owner in turn was replaced by the behaviour of the copy or the gap it closes,
`cargo test -p celestina-core --offline --locked` was run, and the file was
restored with `git checkout`. The script is not part of the unit.

## Result

- **Exit:** every command above exited 0. `celestina-core` went from 34 to 82
  passing tests (49 new, 4 of them from the review corrections below; the
  environment-dependent
  `the_user_directory_comes_before_the_system_ones` became the hermetic
  `the_legacy_directory_list_keeps_its_order_and_its_gaps`). Workspace clippy
  reported nothing. `cargo doc` for `celestina-shell-core` fails with two
  unresolved links (`SearchOutcome` in `launcher.rs`, `Settings::apply` in
  `settings.rs`) both before and after this unit; the crate doc this unit
  rewrote adds none.
- **Observed (RED, one mutation at a time, every one exit 101):**

| Mutation | Tests that failed |
|---|---|
| `file_uri::to_path` with lenient decode and no query, fragment or NUL check | `escapes_are_strict`, `a_nul_byte_is_refused`, `a_query_or_a_fragment_is_refused_rather_than_read_as_a_name` |
| `file_uri::to_path` taking any host as local (Siderita's copy) | `localhost_is_this_host_and_any_other_host_is_not` |
| `xdg` runtime directory checked only for absoluteness | `the_runtime_dir_must_be_a_private_directory_of_this_user` |
| `xdg::ensure_private_dir` as `create_dir_all` | `a_private_dir_and_its_missing_parents_are_created_0700`, `an_own_wider_directory_is_tightened_and_a_foreign_one_refused`, `a_symlink_or_a_file_is_not_a_private_dir` |
| `replace_private` delegating to the old `replace` | `private_state_is_0600_in_0700_parents_even_over_a_wider_file`, `a_failed_directory_sync_after_the_rename_is_reported_as_published`, `a_write_that_cannot_start_leaves_everything_alone` |
| media publish by plain `rename` | `landing_media_refuses_to_replace_an_existing_name`, `staged_media_over_a_name_still_taken_is_refused` |
| media sibling left at the umask mode | `landed_media_keeps_the_source_mode_and_group`, `a_private_source_lands_private_and_set_id_bits_are_not_copied` |
| `read_bounded` as `fs::read` | `a_directory_or_a_fifo_is_not_a_state_file`, `a_file_over_the_limit_is_refused`, `reading_a_file_bounds_it_and_names_what_is_wrong`, `a_fifo_named_like_an_entry_is_refused_without_blocking` |
| open without `O_NONBLOCK` | `a_fifo_swapped_in_after_the_type_check_does_not_block_the_reader` (5 s timeout) |
| `desktop_entry` values not unescaped | `string_values_are_unescaped_before_exec_is_split`, `lists_split_only_on_an_unescaped_semicolon` |
| scan claiming an id only after a successful read (Siderita's rule) | `a_scan_gives_each_id_to_its_most_specific_directory` |
| search directories as the legacy list | `the_search_dirs_put_the_user_first_and_follow_the_specification`, `the_search_dirs_keep_non_utf8_directory_names` |
| `is_cancel_requested` as the blocking `is_cancelled` | `asking_for_a_cancel_request_never_waits_on_a_pause` |

The `fs::read` mutation run did not finish on its own: the unbounded read of a
FIFO blocked the test until a writer was attached by hand after about ten
minutes, which is the hazard RS-4 and RS-6 describe.

- **GREEN:** with every owner restored, 78 passed, 0 failed; after the review
  corrections, 82 passed, 0 failed.

## Observed facts

Canonical owners, equivalent recipes searched, and what each consumer needs:

- **RS-1 — `celestina_core::file_uri`.** `to_path` lifts magnetitad's strict
  `path_for_file_uri` (empty or `localhost` authority, `decode_strict`, NUL
  refused) and adds what the other copies disagreed on: ASCII-case-insensitive
  scheme and `localhost`, byte-exact non-UTF-8 names (ADR 0008, Grafita's copy
  refused them), a raw `?` or `#` refused, and a typed `FileUriError` with six
  variants. `from_path` is the inverse (`percent::encode`, absolute paths only)
  so the shell can publish the canonical re-encoding RS-1 asks for. Copies
  read: `celestina/src/provider_adapter/media.rs`, `siderita/src/dbus.rs`,
  `grafita/src/url.rs`, `fluorita/src/folders.rs`,
  `fluorita/src/activation.rs`, `celestina-rs/crates/magnetitad/src/devices.rs`,
  `celestina-rs/crates/celestina-shell-core/src/notifications.rs`. Callers that
  also accept a plain path (Grafita, Fluorita activation) treat
  `NotFileScheme` as "not a URI".
- **RS-2/FLU-2 — `atomic_file`, one module, two new explicit entry points**
  (contradiction 4). `replace_private` creates its sibling `0600`, sets exactly
  `0600`, and creates missing parents `0700`; it is the owner that
  `magnetita-net`'s `write_private` and the clipboard history move to.
  `land_media` and its two-step form `stage_media`/`StagedMedia::publish` take
  the source's permission bits (set-id and sticky bits cleared) and, best
  effort, its group (dropping the group bits when the group cannot be
  copied), sync the sibling and its directory before returning, and publish
  through the public `publish_without_replacing`: `hard_link` plus unlink,
  which the kernel refuses atomically on an existing name; on any other link
  error (FAT, exFAT, some FUSE) an exclusive create reserves the name first,
  which a third party can still replace and which a crash leaves as an empty
  file. The two-step form serves
  FLU-1's "stage, trash the original, then publish" order in P-7. `replace`
  keeps its behaviour and its doc now states its two gaps.
- **RS-3 — `xdg::runtime_dir` and `xdg::ensure_private_dir`.** No `/tmp`
  fallback: a typed `PrivateDirError` when the variable is unset or empty,
  relative, missing, a symlink or not a directory, owned by another uid,
  granting group or other access, or lacking owner `rwx` (permission bits
  must be exactly `0700`; sticky and set-id bits are not examined). `ensure_private_dir` creates missing components `0700`,
  refuses a symlink or another user's directory, and tightens this user's own
  wider directory to `0700`. The effective uid comes from `/proc/self/status`,
  read as bytes, through the public `xdg::effective_uid`.
  Copies read: `magnetitad/src/{mount.rs, link_wire/mirror.rs, artwork.rs}`,
  `magnetita/src/mirror_view.rs`,
  `celestina/src/provider_adapter/{brightness.rs, melibea.rs}`, and the style
  import root of SH-2 (`celestina/src/main.cpp`, `celestina/src/lock/main.cpp`).
- **RS-4 and RS-10 — `desktop_entry::{read, scan, find}`.** `read` bounds the
  file to `MAX_ENTRY_BYTES` (64 KiB) through `read_bounded`, requires UTF-8,
  and undoes the specification's string escapes and `\;` in lists. `scan`
  claims each id for the first directory that holds a file of that name
  (the shell launcher's rule, which is the specification's; Siderita's
  claim-after-parse differs), caps the ids it reads, reports truncation, and
  stops on cancellation. `find` is the per-id lookup Hematita needs, under the
  same rule, refusing ids with a `/`. `application_search_dirs` drops relative
  and empty `$XDG_DATA_DIRS` entries and falls back to the default. `parse` and
  `application_dirs` keep their behaviour (R-A3); `application_dirs` now
  delegates to a pure function whose hermetic test records its two gaps.
  Copies read: `siderita/src/apps.rs`, `siderita/src/ownicon.rs`,
  `celestina/src/provider_adapter/launcher.rs`, `hematita/src/processes.rs`.
- **RS-6 — `atomic_file::read_bounded`.** Regular files only (checked before
  the open, opened with `O_NONBLOCK`, checked again on the descriptor), at most
  `limit + 1` bytes read, `Ok(None)` for a missing file, typed `ReadError`.
  Readers read: `celestina/src/provider_adapter/settings.rs`,
  `fluorita-engine/src/{catalogue_store.rs, edit_store.rs, source_store.rs}`,
  `grafita-core/src/{preferences.rs, recent.rs}`, `magnetitad/src/settings.rs`,
  `magnetita-net/src/{trust.rs, cert.rs}`,
  `siderita/src/{bookmarks.rs, settings.rs}`; the bounded pattern of
  `celestina/src/provider_adapter/clipboard.rs` is the one generalized.
- **RS-9 — `atomic_file::Published`.** The new entry points answer
  `Ok(Published::DirectoryNotSynced(error))` when only the directory sync after
  the rename failed, and `Err` only when the destination is unchanged; tests
  inject the sync failure and cover the write that cannot start.
- **RS-7 — `CancellationToken::is_cancel_requested`.** A plain load that never
  waits on a pause; a pause after cancel does not hold a worker.
- **RS-19 — documentation.** The workspace README table now lists every crate
  and the current `celestina-core` and `celestina-shell-core` scope, the local
  AGENTS boundary names `magnetita-link`, `magnetita-mobile`, `magnetita-peer`,
  `magnetita-proto` and the `journal` exception, STATUS records the dormant
  owners, the `celestina-shell-core` crate doc lists `media`, `melibea`,
  `diagnostics` and `journal` and states the journal's IO exception, and
  `image.rs` names its one real caller. `celestina-core` gained a crate doc,
  and each module states which product unit adopts it.

Dependency direction is unchanged: `celestina-core` still has no dependency.
`rustix` would have given `geteuid`, `renameat2(RENAME_NOREPLACE)` and a named
`O_NONBLOCK`, but adding it to `celestina-core` changes the six application
lockfiles that pin `celestina-core` (`celestina`, `fluorita`, `grafita`,
`hematita`, `magnetita`, `siderita`), which lie outside this unit's prefix, so
std-only equivalents are used and `Cargo.lock` is untouched.

No existing function changed behaviour (R-A3): `replace`, `parse`,
`application_dirs`, `is_cancelled` and `pause` are byte-for-byte the same
logic; `replace`'s temporary creation gained an optional mode that it passes as
`None`. No consumer calls a new owner, so no product version changes.

## Review corrections

The controller's review of `b11ad15` found three Important and six Minor
points; all were corrected in one further commit, every earlier assertion kept.

| Point | Correction | RED (mutation back to the reviewed code) |
|---|---|---|
| I-1 `effective_uid` failed on a non-UTF-8 `Name:` line | reads bytes and parses the `Uid:` line alone | `a_name_that_is_not_utf8_does_not_hide_the_uid` failed |
| I-2 a symlink was accepted as the runtime directory | `symlink_metadata`; a symlink is `NotADirectory` | `the_runtime_dir_must_be_a_private_directory_of_this_user` failed |
| I-3 existing copies of the new recipes | recorded below and in the `xdg` and `atomic_file` Adoption sections; `effective_uid` and `publish_without_replacing` made public so the copies can delegate | not applicable |
| M-1 `0500` accepted | permission bits must be exactly `0700`: new `NotOwnerAccessible`; `01700` still accepted | same runtime-dir test failed |
| M-2 group bits landed on the caller's group when the group copy failed | `media_mode` drops `0o070` then | `group_bits_survive_only_with_the_group` failed |
| M-3 fallback over-promised | `publish_without_replacing` documents the third-party replace, the empty file after a crash, and that every non-`AlreadyExists` link error takes it | not applicable |
| M-4 "durable" staged media | `stage_media` syncs the sibling's directory; docs say what is synced | not applicable (no failure injection for this sync) |
| M-5 `file_uri` edge cases | documents `%2F` as a separator (Qt; GLib refuses) and the `file:/x` refusal; tests `..`, control bytes, `\`, `file:///C:/x`, `file:////host/share` | not applicable (characterization) |
| M-6 ledger wording | the row names `desktop_entry::{read, scan, find}` and `application_search_dirs` | not applicable |

Existing copies of the new recipes, and who adopts them:

- `siderita-ops/src/volume.rs` `uid()` reads the owner of `/proc/self`
  (root for a non-dumpable process) with `unwrap_or(0)`: adopts
  `xdg::effective_uid` in `SID-H1-B` (P-8), which already carries "get the uid
  safely".
- `siderita-ops/src/reserve.rs` `rename_without_replacing` moves an existing
  file or directory and must return `EXDEV` for the copy fallback; a directory
  cannot be hard-linked, so it is a different operation. `SID-H1-B` (P-8)
  decides whether its file case delegates to `publish_without_replacing`.
- `magnetitad/src/incoming_file.rs` `publish` is the same hard-link-then-unlink
  primitive inside a free-name loop, without a no-hard-link fallback: its link
  step adopts `publish_without_replacing` in `MAG-D1-D` (P-11); the name loop
  stays Magnetita's.

## Limits

- No application crate was compiled: `celestina`, `siderita`, `grafita`,
  `fluorita`, `hematita` and `magnetita` need the Qt/CXX-Qt toolchain. The
  change only adds items; no consumer uses a glob import from
  `celestina_core`, so no name can collide. The landing rebuilds every
  application that links `celestina-core`.
- `O_NONBLOCK` is the generic Linux value, named per architecture; on an
  architecture not listed only the pre-open type check guards against a FIFO.
- The effective uid is read from `/proc/self/status`; without procfs,
  `runtime_dir` and `ensure_private_dir` answer `UnknownUser`.
- `land_media` does not copy extended attributes or ACLs, and copies the group
  only when the caller may set it. The FAT/exFAT reservation path was not
  exercised here (the scratch filesystem supports hard links).
- The tests ran as uid 0, which bypasses permission checks; ownership refusal
  is tested by passing a different expected uid, not by a second account.
- The FIFO tests need `mkfifo(1)`; it was present, and a missing one is logged
  and the case skipped.
- Consumer behaviour is unchanged until the adopting units land, so the
  audit's live defects (the shell cover URI, the world-readable clipboard
  history, the `/tmp` fallbacks, the Qt-thread `.desktop` reads) remain.

## Follow-up

- Adoption in P-7 (`FLU-H1-A`), P-10 (`SURF-1-E`), P-11 (`MAG-D1-D`), P-14
  (`FLU-H1-B`), P-15 (`SID-H1-C`), P-17 (`SURF-1-F`), P-18 (`HEM-H1-B`) and
  P-19 (`GRA-H1-B`), each named in the adopted module's documentation.
- Not closed here by ruling R-A3: RS-10's unescaping inside `parse` and the
  `application_dirs` fixes (callers get them by moving to `read`, `scan` and
  `application_search_dirs`); RS-7's `Condvar` parking and a `pause` that is a
  no-op once cancelled; RS-9 inside `replace` itself.
- Outside this unit: the two unresolved rustdoc links in
  `celestina-shell-core`, and the README's MSRV sentence (RS-14, backlog).
