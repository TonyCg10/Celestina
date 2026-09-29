# Evidence: a bounded storage page and honest services and readers — HEM-H1-B

- **Date:** 2026-09-26
- **Scope:** `HEM-H1-B` (program unit P-18) of
  [the hardening plan](../plans/active/2026-09-26-hardening.md); closes
  HEM-1, HEM-2, HEM-5, HEM-6, HEM-7, HEM-11, HEM-12, HEM-13 and HEM-14 of
  [the Hematita and Grafita audit](../../../docs/evidence/2026-09-26-monorepo-audit-hematita-grafita.md)
  and the Hematita half of RS-4 of
  [the shared crates audit](../../../docs/evidence/2026-09-26-monorepo-audit-shared-crates.md).
  Stacked on `HEM-H1-A` with `RS-H1-A` merged in. Code:
  `celestina-rs/crates/hematita-core/src/` (`usage/view.rs`, `usage/marks.rs`
  new, `usage/verdicts.rs` new, `usage/mod.rs`, `rate.rs`) and `hematita/`
  (`src/analysis_session.rs`, `analysis_view.rs`, `analysis_workers.rs`,
  `analysis.rs`, `usage_worker.rs`, `services.rs`, `watchdog.rs` new,
  `sampler.rs`, `kernel_text.rs` new, `processes.rs`, `resources.rs`,
  `locations.rs`, `main.rs`, `build.rs`, `Cargo.toml`,
  `qml/components/ByteUnits.qml` new, `StoragePage.qml`, `ProcessTable.qml`,
  `PerformancePage.qml`, `qml/Main.qml`), `STATUS.md`, `README.md`,
  `VALIDATION.md`
- **Environment:** session worktree `unit/hematita/HEM-H1-B`, Linux 6.18
  container as uid 0 (the same test binaries were also run as uid 65534
  through `setpriv`); `celestina-rs` toolchain rustc, clippy and rustfmt
  1.97.1 (its `rust-toolchain.toml`) with `--offline` and the shared target
  directory, and the default toolchain 1.94.1 for the scratch crates and
  `rustfmt` on `hematita/` (Hematita declares `rust-version = "1.85"`,
  which the scratch crates declare too). No Qt 6 SDK (no `qmake`), so
  neither `cxx-qt-build` nor the `hematita` application crate can build,
  and `qmllint` and the Qt Quick smoke cannot run. The offline registry
  carries `zbus 5.18.0`, so the application's Qt-free modules were compiled
  and tested in scratch crates that include the worktree's own source files
  through `#[path]` (below)
- **Artifact:** the landing builds it (Hematita, and Siderita, which links
  `hematita-core`)

## What changed, per finding

1. **HEM-1 — capped rows, one projection.** `usage::view` gained
   `children_rows_where(tree, current, unreadable, limit, keep)`: the
   filter applies before the cap, so a small match is listed instead of
   being lost in the remainder, and only the matches past `limit` merge.
   `children_rows` delegates to it with a filter that keeps everything, so
   Siderita's call is unchanged. `Session::view` now builds the list from
   `children_rows_where(.., MAX_ROWS, ..)` under the two filters and the
   treemap from the unfiltered `children_rows(.., MAX_ROWS)`, both through
   `flat_rects`; the local loop over every child, `analysis_view::project`
   and its per-child marks lookups are gone. A row past the cap is the
   merged remainder, and a new `entryMerged` property carries how many
   entries it holds, which the page words as "otros (N)" in the neutral
   tone, without a path in the details card. Its id and its expansion are
   as fix round 1 below describes.
2. **HEM-2 — coalesced verdicts, incremental marks.** The content check
   gathers its verdicts in `usage::verdicts::VerdictBatch`: the first is
   published at once, then at most one batch per `PROGRESS_INTERVAL`
   (250 ms), also from inside a long read (the check's progress callback),
   and the rest at the end, before `confirm_finished`. The hub's
   `apply_verdicts` applies a batch and publishes once. The marks moved to
   the new `usage::marks::Marks`, which counts per node how many members lie
   at or below it, so taking one member out lowers exactly the ancestors it
   raised: a verdict now touches its own group's members times the tree's
   depth (`Session::set_verdict`), never allocates node-sized vectors, and
   `mark_duplicates` (a full rebuild) runs only for a new, pruned or
   grafted tree. `analysis_view::marks_below`/`marks_exact` were deleted.
3. **HEM-5 — a real action timeout.** `src/watchdog.rs` owns one rule:
   run a call on its own thread and wait for its answer at most a given
   time (`Answered`, `Expired`, `Lost`). A unit action now opens its
   connection and makes its call under `ACTION_TIMEOUT` (five minutes); on
   expiry the action reads `failed` and the watchdog closes the call's
   connection, whose socket shutdown fails the pending call and ends its
   thread (zbus `socket_reader.rs` fails every pending call when a read
   fails). A shared `CallLine` records whether the connection is opening,
   open or given up, so a connection that opens after the watchdog gave up
   is never used to raise a prompt the page already called failed. The
   inert `method_timeout` on the action's builder and the comment claiming
   the wait was bounded are gone; `ROADMAP.md` already names the item under
   `HEM-H1`.
4. **HEM-6 — zbus error typing.** `sampler::failure_of` maps a listing
   failure: `Variant` and `InvalidReply` (a reply of the wrong shape) are
   `malformed` and keep the connection; `MethodError` (any D-Bus error
   reply: `ServiceUnknown`, `AccessDenied`) is an unavailable bus section
   that keeps the connection, since the bus carried the answer; anything
   else (IO, a timeout) drops the connection as before.
5. **HEM-7 — live facts.** A reading's owner is the current tick's
   `status` uid, so the signal gate and the privileged path follow a
   `setuid`. The cached name and application are read again when the
   kernel's `comm` changes (an `exec`) or on the process's own refresh
   turn, once every `FACTS_REFRESH_READS` (15) process reads — half a
   minute — staggered by pid so the re-reads spread over the reads.
6. **HEM-11 — bounded command line, no per-tick key strings.** The command
   line is read through `kernel_text::read_prefix` (first `ARGV0_LIMIT` =
   4096 bytes, `O_NONBLOCK`, regular files only); only its first argument
   is ever shown. `rate::NamedCounters` takes its key type as a parameter
   (default `String`, so disks and interfaces are unchanged) and the IO
   counters are keyed by `(pid, start_ticks)`; `io_key` is gone.
7. **Bounded kernel reads (the dispatch's `read_bounded` item).**
   `src/kernel_text.rs` is the one reader of `/proc`, `/sys` and
   `/etc/passwd` text: `read_text` goes through
   `celestina_core::atomic_file::read_bounded` with a 4 MiB limit (a larger
   file is `malformed`, anything else `unreadable`); the sampler's `read`,
   the signal path's `/proc/<pid>` re-read, the mount tables of the
   locations and the storage workers all use it.
8. **HEM-12 — a shutdown that cannot hang.** Each sampling run owns its
   stop flag and a channel whose sender the thread drops when it ends.
   `stop()` raises the flag, drains the subscribers and waits at most
   `STOP_WAIT` (500 ms); a thread still inside a slow read is detached, and
   because it checks its own flag under the subscribers lock before
   publishing, it never reaches the next run's subscribers. The tick checks
   the flag after the process read and after the services, and the session
   bus is not asked once the flag is up. The comments in `sampler.rs`,
   `resources.rs` and `Main.qml` now say this.
9. **HEM-13 — documents.** `STATUS.md`'s "current checkout truth" head
   says `1.2.2`, six sections and the real validation state; the installed
   line no longer says `0.4.0`; the `ACTION_TIMEOUT` item records its
   closure in this checkout. `README.md` no longer says the core does no IO
   or that polkit "arrives in H5". `Cargo.toml` justifies zbus by the
   activation, the systemd listing and the unit actions, and
   `celestina-core` and `rustix` by what they are now used for.
10. **HEM-14 — one byte formatter.** `qml/components/ByteUnits.qml`, a
    `pragma Singleton` registered in `build.rs` like the theme, words every
    amount (`size`, up to TiB, `qsTr("%1 B")` for bytes) and rate (`rate`);
    the three copies in `StoragePage.qml`, `ProcessTable.qml` and
    `PerformancePage.qml` are gone (the process table keeps only its "—"
    for an unreadable rate). Siderita's copy is another application's and
    is not touched.
11. **RS-4 (Hematita) — the desktop-entry lookup.** The sampler thread
    looks up each listed application's entry once while any of its
    processes is listed, through `celestina_core::desktop_entry::find` over
    `application_search_dirs()` (regular files of at most 64 KiB, the one
    shadowing rule, the specification's escapes), and the snapshot carries
    `applications: Arc<HashMap<id, Application>>`. `HematitaProcesses`
    reads names and icons from the snapshot; its Qt-thread
    `read_to_string` loop over `application_dirs()` and its cache are
    deleted.

Canonical owners and dependency direction: the row projection, the
filters' marks and the verdict batching belong to `hematita-core::usage`
(pure, tested on hand-built trees, no Qt, no file IO); the bounded kernel
reader and the watchdog are app `src/` modules, because the local contract
keeps `/proc` and `/sys` file reading out of `hematita-core`. Searched for
equivalents: `rg` over the monorepo found no other counted-marks,
verdict-batching or call-watchdog recipe outside the shell (in standby,
not reused by order); the only other throttle is the scan's progress
`due()`, which drops intermediate values rather than gathering them. No
dependency was added.

## TDD evidence

RED, before the production change:

- `cargo test -p hematita-core --offline` with the new core tests and no
  implementation failed to compile: `Marks`, `Verdict` and `VerdictBatch`
  not found, `NamedCounters` "takes 1 generic argument but 2 were
  supplied", `children_rows_where` missing.
- Row cap: a probe test against the `HEAD` versions of
  `analysis_session.rs` and `analysis_view.rs` (scratch copy, not
  committed) on a folder of 1000 files panicked with `published 1000 rows`.
- Verdict storm: on a hand-built 1 001 001-node tree with 500 candidate
  groups, the `HEAD` session (one `set_verdict`, which rebuilds every mark,
  and one `view` per verdict, as each queued verdict did) took 276 ms in
  release; the new session applied the same 500 verdicts as one batch and
  one `view` in 1.1 ms, and its marks equal a `mark_duplicates` rebuild.
  The old code also published 500 times; the new one once per batch.
- HEM-5: read in zbus 5.18: `Proxy::call_with_flags`
  (`proxy/mod.rs:863-893`) awaits `call_method_raw` directly, and only
  `Connection::call_method` (`connection/mod.rs:269`) applies
  `method_timeout`.
- HEM-6, HEM-7, HEM-11, HEM-12: the new sampler and watchdog tests failed
  to compile against the old sampler (`failure_of`, `facts_current`,
  `finish`, `Run` and `kernel_text` did not exist); the old behaviour is in
  the audit record's citations (`MethodError` read as `Malformed`, `uid`
  cached per PID, `std::fs::read` of the whole `cmdline`, an unconditional
  `join`).

GREEN, after the change (every command below exits 0):

- `hematita-core`: 107 unit tests (95 before; new:
  `a_folder_of_ten_thousand_entries_projects_at_most_the_row_cap`,
  `a_filter_is_applied_before_the_cap_so_small_matches_are_listed`,
  `a_filter_that_keeps_nothing_projects_nothing`,
  `a_filter_keeping_everything_is_the_plain_projection`, four `marks`
  tests including `incremental_changes_match_a_rebuild_of_the_same_set`,
  three `verdicts` tests including
  `a_storm_of_verdicts_becomes_one_publication_per_interval`, and
  `counters_keyed_by_an_identity_need_no_string`), 11 capture and 36
  `usage_tree` tests; the unit tests also as uid 65534.
- The application's Qt-free modules, compiled from the worktree files in a
  scratch harness with `hematita-core`, `celestina-core`, `rustix 1.1.4`
  and `zbus 5.18.0`: 33 tests as uid 0 and as uid 65534 — `kernel_text`
  (5: an oversized file refused, a FIFO neither read nor waited on, a 1 MiB
  command line read to 4096 bytes and its first argument kept, a missing
  file, non-UTF-8), `watchdog` (3: answered, expired within the bound,
  lost), `sampler` (9: the three zbus error classes, facts after an `exec`
  or a recycled pid, one staggered refresh per period, application lookups
  kept only while listed, the kernel-file reasons, a stop that joins and
  one that detaches a stuck thread within the bound), `analysis_session`
  (12, four new: a verdict batch marks only its group and equals a
  rebuild, a decided or unknown group changes nothing, an unreadable group
  keeps its marks, a folder of 1000 entries lists 40 rows and a 40-tile
  map with and without the filter) and `analysis_view` (4).
- `services.rs`'s `CallLine` and `outcome_of_wait` with their two new
  tests, extracted verbatim into a scratch crate with `watchdog.rs`: 5
  tests pass.
- `usage_worker.rs` type-checked, clippy-clean and its tests passing
  against a stand-in `cxx_qt::CxxQtThread` with the real `queue` bound
  (`FnOnce(Pin<&mut T>) + Send + 'static`), which is what the batching
  closure must satisfy.

## Procedure

```sh
cd celestina-rs
cargo test -p hematita-core --offline
cargo clippy -p hematita-core --all-targets --offline -- -D warnings
cargo fmt -p hematita-core --check
setpriv --reuid=65534 --regid=65534 --clear-groups env HOME=/tmp \
  <target>/debug/deps/hematita_core-<hash>
cd ../hematita
rustfmt --edition 2021 --check src/*.rs build.rs
# scratch harness: src/lib.rs includes hematita/src/{kernel_text,sampler,
# watchdog,analysis_session,analysis_view}.rs through #[path], with the two
# plain types analysis_session takes from Qt-bound modules (actions::Item,
# usage_worker::WorkerHandle) copied field for field; rust-version 1.85
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
setpriv --reuid=65534 --regid=65534 --clear-groups env HOME=/tmp TMPDIR=/tmp \
  <harness test binary>
# the stand-in check of usage_worker.rs and the extracted services check
cargo clippy --offline --all-targets -- -D warnings && cargo test --offline
cd <worktree>
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
```

## Result

- **Exit:** every command above exited 0.
- **Observed:** the counts in the GREEN list; clippy with `-D warnings`
  clean on `hematita-core` and on both scratch crates; `rustfmt --check`
  clean on every Hematita source; the architecture guard (QML registration
  of `ByteUnits.qml` included), the language guard and the documentation
  guard printed OK.

## Limits

- The `hematita` crate was not built and its own `cargo test` did not run:
  there is no Qt SDK here. The CXX-Qt bridge files (`analysis.rs`,
  `analysis_workers.rs`, `services.rs`, `processes.rs`, `resources.rs`)
  were checked by reading against the tested signatures; the tests inside
  `processes.rs` and the bridge half of `services.rs` ran only as noted
  above. `qmllint`, the Qt Quick smoke and every QML change
  (`ByteUnits`, the remainder row, the three pages' units) are unverified
  here; the landing's `complete-production.sh` builds, lints and smokes
  them.
- The expiry of a unit action closes its connection (`close()` on a
  clone, from the worker, while the call thread waits on the other) so
  the waiting call fails; that zbus fails pending calls on a socket
  shutdown is read from its source, not exercised against a real bus. If
  it does not, the `hematita-systemd-call` thread of that action leaks,
  parked until the bus answers or the process exits (one thread per
  expired action; the page still reads `failed` at 300 s, and
  `VAL-HEM-H1-B` checks `ps -T` for it). A bus whose handshake never
  completes is still unbounded where nothing waits on it: an action
  expiring during its handshake has no connection to close, so its call
  thread stays the same way, and the sampler's own bus opening has no
  deadline (a hung bus stalls the sampler's ticks, not the window's
  shutdown).
- Verdicts gathered but not yet published when a check is cancelled are
  dropped (at most one interval's worth); the page keeps what was
  published, as "keeping what it verified" describes.
- With the duplicate filter on, each publication still rebuilds the group
  and member lists (`group_rows`, `path_of` per member); coalescing bounds
  how often, not their size.
- Behaviour the author will see (`VAL-HEM-H1-B`): forty rows per folder,
  the last the merged row (its count and an Enter hint), forty more per
  activation,
  and at most forty tiles; the map no longer shows every sliver; the
  memory row reads in adaptive units (MiB below a gibibyte); a unit action
  nobody answers ends `failed` after five minutes; an application whose user entry is unreadable now shows its id
  rather than the system entry's name (the shared shadowing rule), and
  escaped names read unescaped.
- `celestina-core`'s `desktop_entry` module documentation still says no
  consumer calls `find`; it is `RS-H1-A`'s file, outside this prefix, and
  was left as it is.
- The merged row's accessible name comes from the shared
  `CelestinaUsageList`, which words any non-folder row's kind as
  a file; the row's own label (its count and an Enter hint) carries
  what it is and how to use it. The shared control is `celestina-style`'s
  and was not changed.
- A person who pages a folder of a hundred thousand entries all the way
  gets a hundred thousand rows published, by their own request; the
  default and each step stay bounded.

## Follow-up

- [VAL-HEM-H1-B](../../VALIDATION.md).
- `celestina-core::desktop_entry`'s adoption note, and a deadline for
  opening a bus connection, are recorded as unscheduled follow-ups in
  [STATUS.md](../../STATUS.md).
- Siderita's `FolderUsage.qml` byte formatter (the fourth copy of HEM-14)
  may move to `celestina-style` once its semantics match Hematita's
  `ByteUnits`.

## Fix round 1

The review of `54887ea` found two important and four minor points; one
further commit on the branch addresses them.

1. **Important — the remainder read as "nothing chosen".** The list's
   merged row was published with id `-1`, the value the storage page holds
   for "no entry chosen", so an unchosen list showed the merged row in the
   details card. The hub now publishes it as `MORE_ID` (`-2`,
   `analysis_session.rs`), distinct from `NO_ID` (`-1`); the map's
   remainder tile keeps the core's `REMAINDER_ID` (`-1`), which the shared
   treemap draws but never lets anyone choose. The hub's guards are
   unchanged (`node_id` refuses every negative id, Space needs an id of 0
   or more). The page finds rows through one `rowOf(id)`. Test:
   `the_remainder_never_carries_the_id_of_no_entry` (the constants differ,
   the published remainder is `MORE_ID`, no published id is `NO_ID`).
2. **Important — entries past the cap were unreachable (ruling R-A20).**
   `Session::show_more_rows` raises the analysed folder's row limit by
   another `MAX_ROWS`, bounded by the folder's child count, and keeps it per
   folder (`row_limits`, cleared by a new scan or a reset); the list is
   projected with that limit, filtered or not, while the map keeps
   `MAX_ROWS`. The bridge's new `showMoreRows` invokable republishes. On
   the page, Enter or a double click on the merged row (a `ListItem` in
   the shared list, labelled with its count and an Enter hint) calls it;
   Enter on any other row still enters a folder. Test:
   `every_entry_past_the_cap_is_reached_a_page_at_a_time` (1 000 files: 39
   rows and the merged one, 80 rows after one page, the map still at most
   40 tiles, every entry and no merged row at the end, the paging ends,
   another folder starts at the cap, a reset forgets).
3. **Minor — `gib(kib)` in `PerformancePage.qml`.** Replaced by
   `kibText(kib)`, which hands `kib * 1024` to `ByteUnits.size`; no
   hand-formatted unit is left in Hematita's QML.
4. **Minor — the account table read once.** The sampler reads
   `/etc/passwd` again, bounded like every kernel file, when a listed uid
   has no name and the last read is at least `USERS_REFRESH_READS` (30
   process reads, a minute) old; a failed read keeps the known names.
   Test: `the_account_table_is_read_again_only_for_an_unknown_uid_and_not_often`.
5. **Minor — the connection close is unproven.** Kept; the possible
   leaked call thread is stated in the Limits above, and `VAL-HEM-H1-B`
   now checks that an unanswered action reads `failed` at 300 s and that
   `ps -T` shows no action thread afterwards.
6. **Minor — follow-ups only in the evidence.** The bus-handshake
   deadline and the stale `desktop_entry` note are now in `STATUS.md` as
   unscheduled follow-ups.

RED: the new session tests failed to compile against the first round
(`MORE_ID`, `NO_ID`, `show_more_rows` and `row_limits` did not exist); in
the first round the remainder's published id was `-1`, equal to the page's
"none". GREEN: the scratch harness now runs 36 tests (three new) as uid 0
and as uid 65534, clippy with `-D warnings` is clean on it; the core's 107
unit, 11 capture and 36 `usage_tree` tests pass in a private target
directory; `rustfmt --check` is clean on every Hematita source; the three
guards print OK.

## Landing

- **Base revision:** `6f89ee951d37e43d56ee7251762357a3917c5592`
- **Check:** `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact digest or set does not match the recorded build; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:d730ecdfa0fce54f8558d681920862b5699d4e560ffaff695dcc137ef1edc5b0, verification_fingerprint sha256:bf933973a88a62a9110f42d5cab15ba9c469487d46317d7c23231d021035f024; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:0d851e2a6147aa5b12a04bee8cd92406a876bc4ffc116074fe41faf4ecacdefe, verification_fingerprint sha256:5186dd5531f76f4416649288a783cc08e04043f19a7103cf6a0b5b086c9f9bb1; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:11e0353522b047e3e6ac178c50885abf4cf9cb30ba3316a5f2f54f334baafa7f, verification_fingerprint sha256:7d31ed8dfa9c5c7a2b29aea59826d8462f7f9ad91e5309901b6f6673d52257d9
- **Deploy:** after the push: hematita: deploy-production.sh, status-production.sh; siderita: deploy-production.sh, status-production.sh
