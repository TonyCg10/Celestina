# Evidence: 2026-09-26 SID-H1-A archive extraction contained to its root

- **Date:** 2026-09-26
- **Scope:** unit `SID-H1-A` (program row P-4) of the
  [hardening plan](../plans/active/2026-09-26-hardening.md); closes SID-1 and
  SID-17 of the
  [Siderita audit](../../../docs/evidence/2026-09-26-monorepo-audit-siderita.md)
  and the SID-29 tests that belong to them. Code:
  `celestina-rs/crates/siderita-archive` (`src/contain.rs` and
  `src/listing.rs` new, `src/extract.rs`, `src/tool.rs`, `src/member.rs`,
  `src/error.rs`, `src/lib.rs`, `tests/archives.rs`, `tests/delegated.rs`
  new) and the one `measure` call in `siderita/src/controller/archive.rs`
- **Environment:** session worktree `siderita-SID-H1-A` in a Linux container
  (kernel 6.18, uid 0), `rustc 1.97.1`, `cargo 1.97.1`, Cargo `--offline` with
  the shared worktree target directory. Not available: `7z`, `7za`, `7zz`,
  `unrar` and `bsdtar` (only GNU `tar` and `unzip` are installed), the Qt 6 /
  CXX-Qt build of the `siderita` application crate, and any real session
- **Artifact:** the landing builds it

## What this fixes

**SID-1 (Critical).** The only guard on a link member was
`member::target_stays_inside`, which reads a target as text. Every member was
then written with `create_dir_all` and `File::create`, both of which follow
whatever links earlier members had already created. The auditor's four-member
tar (`d/`, `d/up -> ..`, `esc -> d/up/..`, `esc/pwned.txt`) passed the text
check at every hop and wrote `pwned.txt` beside the extraction folder while
`extract` answered `Ok`.

The fix gives containment one owner, `src/contain.rs` (`Root`). It holds the
extraction folder open by its canonical path and walks every path below it
on the real filesystem:

- Every write goes *through an open folder*, never through a name looked up
  again. `Root::make_dirs` and `Root::place` enter each component by opening
  it with `O_DIRECTORY | O_NOFOLLOW` (`OpenOptions::custom_flags`, the
  values per architecture) and comparing the handle's `(device, inode)` with
  the `lstat` that approved it. The member is then created through
  `/proc/self/fd/<fd>/<name>` while that folder stays open: `create_new` for
  a file, `mkdir`, `symlink` and `link` for the rest, none of which follows a
  link at the final name. The root's own name is re-checked on every walk.
  This is `openat` spelled with the standard library, since the crate takes
  no system-call dependency. An existing symlink component is
  `ArchiveError::UnsafeMember`, and a file in the way is the IO error "not a
  directory".
- `Root::link_stays_inside` resolves a link target the way the kernel does:
  it expands each link it meets and climbs the real parent on `..`. It refuses
  an absolute target, a climb above the root, and more than 40 hops. A
  component that does not exist yet is read as text, still bounded.
- `Root::verify_tree` walks a tree it did not write. Every symlink must
  resolve inside, and an inode must not have more names than the tree holds,
  because an extra name means a hard link to a file outside.

`extract.rs` now writes in this order:

1. Directories and regular files. A file is opened with `create_new`
   (`O_CREAT|O_EXCL`), so the open never follows a link. A duplicate regular
   member replaces the earlier file of the same extraction, as in tar. Mode and
   date are applied through the same open handle, so no path is looked up
   again.
2. Hard links (tar), created only to a regular file of this extraction reached
   through real folders, and checked after `link` to be that very file. Their
   target must be a safe relative name, or the member is `UnsafeMember`. A
   hard link to an archive symlink, a folder or a missing name is skipped as
   `UnsupportedKind`. A hard link whose name is already taken is refused,
   while a repeated regular file replaces the earlier one: the file is the
   newer bytes of an appended tar, and replacing a name with a link would
   silently drop a member already written in full.
3. Symlinks. Each is created only after its target resolves inside on the real
   tree. A name that is already taken, typically by a folder a later member
   made by writing "through" the link, is `UnsafeMember`.
4. Every symlink is re-resolved on the finished tree, because a later link can
   change where an earlier one points.

The text check stays as an early first filter. A zip symlink's target, which
a zip stores as member data, is now read with a 4096-byte bound.

Lifting the archive's own single top folder out of its wrapper moves the tree
one level up. A link that climbed to the wrapper would then point at the
person's folder, so the lift now happens only when the inner folder passes
`verify_tree`; otherwise the wrapper stays.

For RAR and 7z, the tool writes the tree itself, so it is checked twice.
Before the tool may write, its own listing (`7z l -slt`, `unrar lt`) is read
line by line by `src/listing.rs` and the archive is refused, with nothing
written, for an absolute or `..` name, a symlink target that leaves as text,
a hard link target that is not a safe name, a member under or named like a
symlink member, or a listing line too long to check whole. A 7-Zip
`Copy Link` is read like a hard link. Output that does not look like the
tool's listing is refused too: 7z's must reach its `----------` rule, and
unrar's must carry its `Details:` header or at least one `Name:` record, so an
older layout, a localised build or another program named `unrar` fails
closed. unrar lists and extracts with `-c-`, so an archive comment is never
printed where it could be read as output. Memory stays bounded: one 64-bit
hash per folder, name and link, at most two million, and a collision can only
refuse an honest archive. Link names are also kept, to name a refused member,
up to 1 MiB in all; past that a link is remembered by its hash and reported
as "(a symbolic link)". After the tool ran, its result
goes through `Root::verify_tree`. The old post-check
`tool::no_symlink_escapes` used the same text test and is deleted.

**SID-17 (Minor).** A password was passed as `-p<password>`, which any local
user could read in `/proc/<pid>/cmdline`. `tool.rs` now works as follows:

- With a password there is no `-p` switch. The password is written to the
  tool's stdin as one line in a single write into the empty pipe (at most 4095
  bytes, which a pipe always takes), and the pipe is then closed. A password
  that no line can carry (a line break, a NUL, or too long) answers
  `WrongPassword` without running the tool. With no password, stdin stays
  `/dev/null` and `-p-` stays.
- Output is bounded. A line is cut at 8 KiB, and at most 1024 lines wait
  unread, so the pipe gives backpressure. The runner still reads a busy pipe
  at full speed: it waits for output for at most one 50 ms poll instead of
  sleeping, takes at most 1024 lines per turn, and checks the tool's exit and
  the cancellation on every turn. The complaint and the listing keep
  only their last 64 KiB (`Tail`). `total_bytes` no longer holds a listing's
  whole stdout.
- The executable is found only in absolute `PATH` folders. It runs by its
  canonical path, with `argv[0]` kept as the name it was found under. Before,
  an empty or relative `PATH` entry could run a `7z` from the current folder.
- When `setsid` is installed, the tool runs through it, in a session of its
  own with no controlling terminal, so a password prompt cannot open
  `/dev/tty` and wait there even when Siderita was started from a terminal.
  `setsid` is invoked with `argv[0]` "setsid" and without `--wait`, so
  BusyBox's applet works too. A child spawned here leads no process group,
  so `setsid` never forks: it execs the tool. The tool's own `argv[0]` is
  then its canonical path.
- One supervised runner serves the extraction, the pre-flight listing and the
  measurement. It polls cancellation and stops the tool, and the measurement
  also stops at a 60-second deadline. Under `setsid` the tool leads its own
  process group, and `kill -KILL -- -<pid>` stops that group whenever the
  tool ends: on cancel or deadline, and also when it exits on its own. On
  exit, the group is killed while the tool is still an unreaped zombie
  (seen in `/proc/<pid>/stat`), so the id names only what the tool left
  behind. So a tool that forks, a wrapper that does not `exec`, or a
  background job the tool left running can neither keep writing into a
  folder being removed nor write into the extraction after it has been
  checked and returned. After the tool exits, the rest of its output is
  read with cancellation and the deadline still polled, so a descendant that
  escaped the group and holds a pipe cannot hold the extraction hostage. The
  standard library cannot signal a group without `unsafe`. Without `setsid`
  the group would be Siderita's own, so only the tool is killed. `measure` now takes the job's
  cancellation token, and the application's one call passes it.

The public API changes in one place: `measure` gains its `cancellation`
argument. The application's exhaustive `SkipReason` match
(`siderita/src/controller/archive.rs`) still compiles by construction. The
doc of `SkipReason::UnsupportedKind` now covers a hard link whose target is
not a file this archive wrote.

## Procedure

```sh
cd celestina-rs
cargo test -p siderita-archive --offline          # RED before each fix, GREEN after
cargo clippy -p siderita-archive --all-targets --offline -- -D warnings
cargo fmt --check
cd ..
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
```

## Result

- **Baseline before any change:** 9 unit and 13 integration tests passed.
- **RED, first pass** (new tests against the old code): 7 failed.
  - `a_chain_of_links_that_each_look_inside_cannot_carry_a_write_out` failed
    with "the write escaped the extraction root": the auditor's PoC reproduced
    and `destino/pwned.txt` was written (the fixture names were Spanish then;
    round 2 renamed them in English).
  - `a_chain_of_links_that_resolves_outside_is_refused_in_either_order`,
    `a_file_is_never_written_through_a_link_the_archive_made` and
    `a_hard_link_to_a_file_outside_is_refused` failed because `extract`
    answered `Ok`. The hard link was silently skipped.
  - `links_that_stay_inside_are_extracted` failed because the hard link was
    skipped.
  - `a_password_reaches_the_tool_on_stdin_and_never_on_its_command_line`
    failed with the password found in the fake tool's recorded argv.
  - `lifting_the_archives_own_folder_never_lets_a_link_reach_past_it` failed
    because `top/arriba -> ..` (now `top/upward`) resolved to the person's
    folder after the lift.
  - `an_absolute_member_is_refused` and `a_parent_member_in_a_tar_is_refused`
    already passed; they stay as regression guards.
- **RED, review round.**
  - `a_folder_swapped_for_a_link_during_extraction_never_carries_a_write_out`
    (2000 members, a thread swapping `sub` for a link to an outside folder,
    up to 150 trials) against the first-pass code, which re-resolved each path
    as text: a member landed outside in trial 17 of one run and trial 61 of
    another.
  - `the_chain_is_refused_from_the_listing_before_the_tool_writes`, with the
    pre-flight listing call disabled: "the tool was allowed to write". The
    finished-tree check still refused, after the tool had run.
- **RED, review round 2.**
  - `stopping_a_tool_stops_every_process_it_started` (a fake tool whose shell
    runs `sleep 30 &` without `exec`, cancelled) against round 1: "the tool's
    child … is still running".
  - `an_unrar_listing_it_does_not_recognise_is_not_trusted`, with unrar's
    old "started from the first line" rule restored: the foreign output
    passed.
- **RED, review round 3**, the reviewer's probes as tests in
  `tests/delegated.rs`, against round 2:
  - `a_child_left_holding_the_output_neither_blocks_nor_survives`: the tool
    exits normally, leaving `sleep 8 &` holding its output, and is cancelled
    at 1 s. "extract took 8.09 s".
  - `a_cancel_is_honoured_while_an_escaped_child_holds_the_output`: the same
    with `setsid sleep 8 &`, outside the group. "extract took 8.06 s".
  - `a_child_left_behind_writes_nothing_after_extract_returns`: a detached
    job writes a link 2 s after the tool exits. "a child of the tool wrote
    after extract returned".
- **GREEN:** `cargo test -p siderita-archive --offline` exits 0 with 31 unit
  tests (9 before), 27 tests in `tests/archives.rs` (13 before) and 7 in
  `tests/delegated.rs` (new). The delegated tests ran clean four times in a
  row: the cancelled runs return within 3 s (round 4 added a child flooding
  stdout, RED at 8.09 s on round 3; round 5 added
  `a_long_listing_is_read_at_full_speed`, a 400 000-line listing that took
  20.7 s at 1024 lines per poll and now finishes in about a second; round 6
  made it count the runner's pauses instead of timing it, so a starved
  scheduler cannot fail it: the round-4 loop shape pauses 382 times, the
  current one fewer than 20; it passed under `nice -n 19` against eight busy
  loops on four CPUs), the child in the group is gone
  within 3 s, and the late write never happens. The race test now runs up to 500 trials (20 s
  bound); it ran clean in six runs of 150 and three of 500. Most trials end
  in `UnsafeMember` at the first swap the walk sees, a few in "not found" when
  the swap removed the folder mid-walk; none wrote outside. Every hostile case fails with `ArchiveError::UnsafeMember`
  and leaves the destination folder empty, with nothing written beside it. A
  secret hard-linked from outside keeps its bytes and a link count of 1.
  - The fake `7z` on `PATH` (`tests/delegated.rs`): the chain is refused from
    the listing with the tool never asked to extract; a tool whose listing
    looked honest but which lays the chain anyway is refused afterwards and
    its result removed; an honest run is kept.
  - The runner (`tool.rs` tests): a listing that sleeps is stopped within
    5 seconds by cancellation (measurement and pre-flight) and by a 300 ms
    deadline; a tool run through `setsid` reports a session other than the
    test's and terminal `0`; a cancelled tool's forked child is gone within
    3 seconds.
  - The reviewer's native cases: an absolute link then a member under it; a
    folder replaced by a link; hard links to a symlink, a folder and nothing
    (skipped); a loop; no lift of a lone `top -> .`.
- **Clippy:** exit 0 with `-D warnings`. **fmt:** `cargo fmt --check` exit 0;
  `rustfmt --check` on the edited application file exit 0.
- **Guards:** architecture contract OK; language contract OK (148 legacy files
  ratcheted, none raised); documentation contract OK. It prints the
  pre-existing `SID-G7-D` inventory erratum, which this unit does not touch.
- **Owner and reuse:** `rg` found no other containment walker in the monorepo.
  `siderita-ops` has no symlink-safe join, and the old tool post-check was the
  second copy of the text-only rule. That rule now has one owner
  (`contain::Root`), used by the native writer, the tool path and the folder
  lift. `member::target_stays_inside` remains the early text filter, and
  `listing.rs` applies the same member rules to a listing, before anything
  exists on disk to walk.

## Limits

- **No real tool ran.** `7z`/`7zz`/`unrar` are not installed here. The
  runner tests use a `/bin/sh` stand-in that records argv, stdin and its
  session, which proves what Siderita sends and not how the real tools
  answer. The `7z l -slt` and `unrar lt` field names the listing check reads
  (`Path`, `Symbolic Link`, `Link`, `Hard Link`, `Copy Link`,
  `Attributes`; `Details`, `Name`, `Type`, `Target`) are from recollection of
  the tools' output, not checked here. The two recognition markers fail
  closed: a 7z listing without its `----------` rule, or an unrar listing
  without `Details:` and without any `Name:` record, refuses the archive.
  The record keys do not. If the real tool spells a link differently from
  `Symbolic Link`, `Link`, `Hard Link`, `Copy Link`, a `Type:` value holding
  "symbolic link", "junction", "hard link" or "file reference", or `Target:`,
  the pre-flight is blind to that link and lets the archive through. The
  after-the-run `verify_tree` is then the only guard: it refuses and removes
  an escaping result, but cannot undo a write the tool made outside during
  its run. The raw listings `VAL-SID-16` collects are what confirm the keys. From recollection of their sources too, they read a prompted password
  from stdin when they have no controlling terminal: 7-Zip reads stdin
  directly, while p7zip and unrar use `getpass`, which reads stdin only when
  `/dev/tty` cannot be opened. `setsid` makes that true; without `setsid`, a
  Siderita started from a terminal would have the tool ask on that terminal,
  where the run is still cancellable. The author must check both tools on the
  real session: `VAL-SID-16`.
- **The tool path is checked before and after the run, not during it.** The
  listing refuses what the archive declares. A tool that writes something its
  listing did not declare — a bug or a divergence between the two commands —
  is caught only afterwards: `verify_tree` refuses and removes the result, but
  it cannot undo a write the tool made outside the folder during its run.
  Older tools do not refuse such links themselves: p7zip 16.02, which answers
  to `7z`, the first name searched for a 7z, predates 7-Zip's `-snld` guard,
  and unrar refuses them only since 6.12 (CVE-2022-30333).
- **A folder moved away while it is being filled.** Writes go through the
  folder that was checked, whatever its name becomes, so a folder swapped for
  a link is never followed. But a process with write access to the
  destination can *move* a folder the extraction already entered out of the
  root, and the members written after that land where it went. A process
  that can write there is not necessarily one that can write everywhere the
  extraction can: a sandboxed application granted only that folder is one.
  What it achieves is moving the extraction's own new files, not steering
  them onto an existing path.
- **Case-folding and normalising filesystems.** The rules compare names byte
  for byte. On a filesystem that folds case or normalises Unicode (vfat,
  exfat, ntfs3, ext4 or bcachefs with casefolding), `Esc` and `esc` are one
  entry. A folder cannot become a link through that, because every write goes
  through an opened folder, and a link placed over a taken name is refused.
  But the listing check's "member under a link" rule, which works on the
  stored text, can miss a pair that differs only in case, and there the
  after-the-run check is the guard.
- A symlink loop inside the archive is now refused as `UnsafeMember` rather
  than extracted.
- `/proc/self/fd` is Linux's. The `O_DIRECTORY | O_NOFOLLOW` values are
  compiled for the Linux and Android architectures listed in `contain.rs`.
  Anywhere else `open_folder` answers "unsupported", so an extraction fails
  before it writes anything.
- A group kill needs `setsid` and a `kill` program on `PATH` (util-linux,
  procps or BusyBox). Without them, only the tool itself is stopped. A
  background job it left behind then keeps running after a normal exit, and
  can write into the extraction after `extract` has returned. The zombie check
  reads `/proc`; where that fails, the group is killed just after the tool is
  reaped. While the group still has members, that reaches only them, because
  Linux keeps the id reserved while a group of that id has members. Once the
  group is empty the id is free, and a later process could take it. A
  descendant that left the group (its own `setsid`) is out of reach either
  way. It cannot delay a cancel beyond one poll plus one read, even while it
  floods the output: cancellation is checked on every turn of the drain
  after the tool exits, and before the exit at least once per queue's worth
  of lines. Until then it can keep writing.
- The `siderita` application crate (Qt/CXX-Qt) did not build here. Its one
  changed line (`measure(archive, &options, token)`) and its use of the rest
  of the API were read, not compiled. The landing's
  `complete-production.sh` is the first build.

## Follow-up

- `VAL-SID-16` in [VALIDATION.md](../../VALIDATION.md): encrypted 7z and RAR
  extraction with the password on stdin, a hostile listing refused by the
  real tools, and a launch from a terminal, on the real session.
- None of the unit's findings stays open.

## Landing

- **Base revision:** `32e669bf0b229f08189a5972525908808781e6fc`
- **Check:** `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:66c9aa1168e80f3fd35a319e1e247dc1fed8c5789f92761fe2bf44563098a9ed, verification_fingerprint sha256:7d31ed8dfa9c5c7a2b29aea59826d8462f7f9ad91e5309901b6f6673d52257d9; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:f20b89535f6a0936c15a2bb06a957d7a6d1d6c6f957fff4ccb85d482bc8f1ed2, verification_fingerprint sha256:70a8844d1dbf3f05e8ea8899cf195d167e687f8587c8ab9b32c794589ee6f628
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh
