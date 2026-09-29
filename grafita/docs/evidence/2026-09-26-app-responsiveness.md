# Evidence: the editor stops doing quadratic or disk work on its GUI thread

- **Date:** 2026-09-28
- **Scope:** unit `GRA-H1-B` (program unit P-19) of the
  [hardening plan](../plans/active/2026-09-26-hardening.md); it closes GRA-4,
  GRA-5, GRA-8 and GRA-9 of
  [the Hematita and Grafita audit record](../../../docs/evidence/2026-09-26-monorepo-audit-hematita-grafita.md)
  and Grafita's half of RS-1 from
  [the shared-crates audit record](../../../docs/evidence/2026-09-26-monorepo-audit-shared-crates.md).
  Code: `celestina-rs/crates/grafita-core` (`src/highlight.rs`,
  `src/recent.rs`, `src/worker.rs`, `src/session.rs`, `src/preferences.rs`,
  `src/lib.rs`, `tests/sessions.rs`) and the application (`grafita/src/`
  `syntax.rs`, `session.rs`, `preferences.rs`, `url.rs`, `main.rs`,
  `activation.rs`; `grafita/cpp/highlighter.{h,cpp}`;
  `grafita/qml/components/DocumentView.qml`; `grafita/Cargo.toml` and
  `Cargo.lock`); `grafita/STATUS.md`
- **Environment:** session worktree `grafita-GRA-H1-B` on branch
  `unit/grafita/GRA-H1-B`, based on `003c843` (`GRA-H1-A` merged with
  `RS-H1-A`, which provides `celestina_core::file_uri`); Linux container
  running as uid 0; `rustc 1.97.1`, `cargo 1.97.1`; Cargo `--offline` with a
  private target directory. No Qt 6 SDK, CXX-Qt build, Wayland session or
  AT-SPI bus, and the offline registry lacks `cxxbridge-cmd`, so neither
  Grafita nor Siderita could be compiled for real here
- **Artifact:** the landing builds it (`grafita/scripts/complete-production.sh`
  and `siderita/scripts/complete-production.sh`, because Siderita links
  `grafita-core`)

## What changed

| Finding | Change |
|---|---|
| GRA-4 (Important) | `grafita_core::highlight::line_utf16` returns `Utf16Run { start, len, token }` in UTF-16 code units, converted from the lexer's byte spans in one forward pass (the spans are ordered and disjoint, so each boundary counts only the characters since the previous one; a span off a boundary is dropped, never painted elsewhere). `grafita/src/syntax.rs` hands those runs across the cxx bridge as `Run { start, length, token }`, and `highlighter.cpp` paints them as given: its `utf16Offset` prefix re-decode, the second owner of the offset rule, is gone. The four palette setters now schedule one queued `rehighlight` per event-loop turn instead of re-colouring the document each; `setLanguage` still re-colours at once, since it arrives once per document. |
| GRA-5 (Important) | The session decides and the worker does the IO, in two jobs. `recent::change(store, change)` (`Job::RecentChange`, answered by `Completion::RecentChanged`) reads, changes and writes the list under a process-wide lock that covers only that, because each tab has its own worker; it never asks whether anything exists. `recent::list(store, token)` (`Job::RecentList`, answered by `Completion::RecentListed { paths }`) sends the existence checks to one detached, process-wide prober thread that holds nothing and is never joined, and waits at most `PROBE_WAIT` (1 s) while polling its cancellation token; an entry not answered in time is left out, and while the prober has been stuck on one check for 500 ms a list is answered at once with the stored entries unverified (opening a missing one fails and forgets it). A batch the list gives up on is flagged, and the prober skips what is left of it. `DocumentSession::receive` for an open returns the change job beside the `PushText` event (record on success, forget on a refused read) instead of loading and storing inline, so an open, and every Siderita preview, never waits on a `stat`; only `refresh_recent()` lists, and `recent_documents()` reads its last answer. Neither job is superseded, so a newer question cannot drop a record. Preferences: `PreferenceWriter` owns a thread that writes the newest submitted set once `STORE_QUIET` (400 ms) passes without a change; dropping it writes what is still waiting and waits for that at most `CLOSE_WAIT` (2 s), then detaches a thread still inside the atomic replace. The application adapter submits to it instead of calling `store()`. The QML list binds the new `recentDocuments` property and asks with `refreshRecent()`. |
| GRA-8 (Minor) | `m_target` is `QPointer<QQuickTextDocument>`; the target's `destroyed` signal emits `targetChanged`, and by then the pointer already reads null, so a binding re-reading `target` never sees a freed object. The connection is replaced when the target changes. |
| GRA-9 (Minor) | STATUS is dated 2026-09-28, the stale "Grafita is 1.2.0 and installed" line is replaced by a pointer to `Cargo.toml` and the version history (so the file stops restating a number it cannot keep current), and `GRA-H1-B` is recorded. |
| RS-1 (Grafita) | `url::local_path` delegates to `celestina_core::file_uri::to_path` and keeps only Grafita's choice to accept a plain path (`NotFileScheme`, and `file:` without the `//` marker, so `grafita file:notas.txt` opens a relative name as before); its private percent decoder that ended in `String::from_utf8` is deleted, so `file:///…/%FF` names that byte instead of being refused, and a decoded NUL is refused. `main.rs` reads `args_os` (`std::env::args` panics on a non-UTF-8 argument), and a path that is not UTF-8 crosses QString — the window's `initialPath`, the `OpenDocument` activation call, the recent list, the session's published `path` that the window compares tabs by, and the file held in `encodingRetry` — as its canonical `file://` URI (`url::qml_argument`), which `openPath` and the encoding retry read back through the same `local_path`. A UTF-8 path still travels as itself, so an older running Grafita still understands the activation call. |

Canonical owners: the UTF-16 rule for highlight runs is
`grafita-core/src/highlight.rs` (`display.rs` stays the owner for buffer
positions; the two convert different inputs, a buffer position and a line's
byte spans, and neither host counts units any more). Recent-list IO:
`grafita-core/src/recent.rs` (`change`, `list`). Debounced preference writes:
`grafita-core/src/preferences.rs::PreferenceWriter`. `file://` parsing:
`celestina-core/src/file_uri.rs`. Searched with
`rg "utf16|fromUtf8|encode_utf16"` over `grafita/` and `siderita/`,
`rg "Recent::|\.store\(\)"` over the monorepo and
`rg "percent_decode|file://"` over `grafita/`: the C++ mapping, the inline
recent IO and Grafita's decoder were the only copies in this unit's scope and
each is removed. Siderita consumes only unchanged APIs (`DocumentSession::new`,
`receive`, `Preferences::{load, store}`) and matches `Event` exhaustively, so
no `Event` variant was added; its adapter submits whatever job an outcome
carries, which is how its embedded editor's `Job::Recent` reaches its worker.
Dependency direction is unchanged; Grafita gains a direct path dependency on
`celestina-core`, already linked through `grafita-core`.

## Procedure

```sh
T=$SCRATCH/gra-h1-b-target
cd celestina-rs
CARGO_TARGET_DIR=$T cargo test -p grafita-core --offline --no-fail-fast      # baseline
CARGO_TARGET_DIR=$T cargo test -p grafita-core --offline --lib highlight    # RED (compile)
# mutation RED: `advance_to` re-counting the prefix, as the C++ side did
CARGO_TARGET_DIR=$T timeout 90 cargo test -p grafita-core --offline --lib highlight
# mutation RED: `receive_open` also writing the list inline
CARGO_TARGET_DIR=$T cargo test -p grafita-core --offline --test sessions
# mutation RED: the writer ignoring its quiet period
CARGO_TARGET_DIR=$T cargo test -p grafita-core --offline --lib -- preferences
CARGO_TARGET_DIR=$T cargo test -p grafita-core --offline --no-fail-fast      # GREEN
cargo fmt --all --check
CARGO_TARGET_DIR=$T cargo clippy -p grafita-core --all-targets --offline -- -D warnings
CARGO_TARGET_DIR=$T cargo doc -p grafita-core --no-deps --offline
# application crate: a scratch mirror of grafita/src in which every CXX-Qt and
# cxx bridge is a stub of the same shape and `fn main` (Qt setup) is dropped
python3 $SCRATCH/gra-h1-b-stubgen.py
cd $SCRATCH/gra-h1-b-appcheck
CARGO_TARGET_DIR=$T cargo build --tests --offline
CARGO_TARGET_DIR=$T cargo clippy --tests --offline -- -D warnings
CARGO_TARGET_DIR=$T cargo test --offline
# mutation RED: the mirror's `local_path` replaced by the old decoder
CARGO_TARGET_DIR=$T cargo test --offline url
cd grafita && rustfmt --check --edition 2021 src/main.rs
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
```

## Result

- **Baseline:** exit 0; `grafita-core` 227 tests (104 lib, 4, 28, 44, 16, 31).
- **RED:**
  - The new highlight tests do not compile before `line_utf16` exists.
  - With the prefix re-count, the 5 MB test was still running when
    `timeout 90` killed it (exit 124).
  - With the inline write, `an_open_is_remembered_by_a_worker_job_and_receiving_it_touches_no_disk`
    fails on "receiving the answer on the host's thread must not write the list".
  - With no quiet period,
    `nothing_is_written_while_changes_keep_coming_and_closing_writes_the_last`
    fails.
  - With the old decoder, 3 of the 5 url tests fail: the NUL, the `%FF`
    name and the round trip.
  - Each mutation was reverted.
- **GREEN:** exit 0; `grafita-core` 245 tests after the review rounds (118
  lib, 4, 28, 44, 16, 35).
  - New in lib: 3 highlight, 5 recent, 4 preferences and 2 worker; the
    `existing()` test went with `Recent::existing`.
  - New in `sessions`: 4.
  - The 5 MB single line (about 777 000 runs) colours in about 2 s in the
    unoptimised test build, against a 10 s bound.
- **Checks:** `fmt --all --check`, clippy with `-D warnings` and the guards
  (architecture, language, documentation) all exit 0. `cargo doc` has 2
  warnings, both from before this unit.
- **Application mirror:** `build --tests` and `clippy --tests -D warnings`
  exit 0, and `cargo test` passes 13 tests. They include
  `a_five_megabyte_single_line_colours_within_a_time_bound` (6 runs per piece,
  about 874 000 runs), `runs_cross_the_bridge_in_utf16_units` and the two
  review-round session tests. The mirror was deleted afterwards.

## Limits

- The real Grafita and Siderita builds did not run here, and neither did the
  C++ compile, moc, `qmllint` or a Qt Quick smoke. The C++ changes, the stubbed
  CXX-Qt shape and the QML binding to `recentDocuments` were checked by reading
  only. The landing's `complete-production.sh` for both apps is the first real
  compile of `highlighter.{h,cpp}`: `QPointer`, the member-pointer
  `QMetaObject::invokeMethod` and the new `Run { start, length }` fields.
- The mirror proves the Rust adapters type-check against the stubbed bridge
  and that their pure tests pass. It does not prove CXX-Qt's generated code,
  signal delivery, the QStringList property reaching QML, or runtime
  behaviour.
- Coalescing was reasoned against Qt's documented queued invocation; nothing
  here counts rehighlight passes. The same goes for `QPointer` clearing
  before `destroyed` is emitted.
- A `stat` hung on a dead mount strands the process's prober thread, never
  a worker. The first list waits at most `PROBE_WAIT` and leaves the
  unanswered entries out. Later lists, while the prober is stuck, skip it and
  offer every stored entry unverified, so a missing file can be offered until
  opening it fails and forgets it. Batches given up on are not checked after
  the prober recovers. The prober is never joined; the process exit ends it.
- Still on the GUI thread:
  - The one read of the preferences file when a window is built, so the first
    frame shows the user's text size.
  - Waiting for a `PreferenceWriter`'s last write when the window closes, at
    most `CLOSE_WAIT` (2 s); a write still syncing after that finishes on the
    detached thread.
  - `siderita/src/preferences.rs` storing on every change, which is Siderita's
    unit (GRA-5 names it; this unit cannot touch `siderita/`).
- For the author, in a real session:
  - Open a 5 MB minified `.js` and confirm the window stays responsive.
  - Change the text size with the Ctrl wheel and confirm it is remembered
    after closing.
  - Open a file whose name is not UTF-8 from the chooser and from the
    command line.

## Follow-up

- Siderita's preference adapter should submit to `PreferenceWriter` in a
  Siderita unit.
- RS-2 lists the recent-documents file among suite state that
  `atomic_file::replace_private` should write. That was not in this unit's
  findings, and `recent::change` still uses `replace`.
- GRA-6 (a whole-document round trip per keystroke) stays the later
  milestone the plan excludes.

## Fix round 1

The review of `06a9f91` asked for one Important and two Minor changes and
three smaller ones. Each item, as it now stands:

- **F1 (Important), recent-list stats blocked workers, other tabs and a
  window close.** The lock now covers only read, change and write.
  - `Job::Recent` became `Job::RecentChange`, which never `stat`s: opens,
    forgets and Siderita previews never wait on another file.
  - `Job::RecentList`, the empty-state refresh, uses the detached prober with
    the 1 s bound, the 500 ms stuck rule and the cancellation token described
    above. `Recent::existing` is removed.
  - Tests, with an injected check that sleeps 30 s like a dead mount:
    - `a_hung_check_leaves_its_entries_out_after_the_wait_and_then_is_skipped`:
      about 1 s, then about 0 s.
    - `a_list_waiting_on_a_hung_check_holds_no_lock_and_stops_when_cancelled`:
      another tab's record returns in under 500 ms, and a cancel stops the
      list in under 300 ms.
    - `a_worker_listing_behind_a_hung_check_is_dropped_promptly`: dropping
      the worker takes under 300 ms and publishes nothing.
  - RED: with the checks made synchronous in `list`, the last two fail after
    30 s.
- **F2 (Minor), the writer's drop joined over two fsyncs.** It now waits on
  a `done` flag for at most `CLOSE_WAIT`, joins only a finished thread and
  detaches a stuck one.
  - Test: `closing_waits_a_bounded_time_for_a_write_stuck_on_the_disk`
    injects a 30 s write; the drop takes 2 s.
  - RED: always joining makes it take 30 s.
- **F3 (Minor), the encoding retry was lossy.** `encodingRetry` is written
  with `url::qml_argument` and read back with `url::local_path`, through
  `retry_argument` and `retry_path`.
  - Test: `a_name_that_is_not_utf8_is_retried_as_itself`.
  - RED: publishing with `to_string_lossy` fails it.
  - The refusal's `Forget` already carried the core's exact `PathBuf`; what
    was lossy was the retried open.
- **F4 (nit), tab matching.** The session publishes `path` in the same
  `qml_argument` form an open request arrives in, so `Main.qml`'s comparison
  finds an open non-UTF-8 document.
  - Test: `the_published_path_is_the_form_an_open_request_arrives_in`.
- **F5 (nit), `file:` names.** `file:` without `//` is a plain relative name
  again.
  - Test: the `file:notas.txt` and `FILE:dir/nota` cases.
  - RED: without the arm they are refused.
- **F6 (nit), language changes.** `setLanguage` re-colours synchronously;
  only the palette setters are coalesced.

## Final touch

Two changes from the approval of fix round 1:

- **N1, a list while the prober is stuck.** It returned an empty list, and on
  a mount that never returns the recent list stayed empty for the rest of
  the process. It now returns the stored list unverified; the first list
  keeps its 1 s bound.
  - Test:
    `a_hung_check_bounds_the_first_list_and_then_the_stored_list_is_offered_unverified`.
    The first list is `[vivo]` in about 1 s; the second is `[vivo, muerto]`
    in under 200 ms.
  - RED: returning an empty list fails it.
- **N2, abandoned batches.** Each `Probe` batch carries an `abandoned` flag
  that the list sets when it stops waiting, and the prober checks it before
  every check. Batches queued behind a slow check therefore do not re-hang
  the prober one entry at a time after it recovers.
  - Test: `a_batch_given_up_on_is_not_checked_after_the_prober_recovers`
    uses an injected 800 ms check and a second list queued at 100 ms. The
    prober starts only the first batch's two slow checks.
  - RED: ignoring the flag makes the prober start a third.
- Checks: `grafita-core` passes 245 tests, twice. Formatting, clippy with
  `-D warnings`, `cargo doc` (the 2 older warnings), the stub mirror (build,
  clippy and 13 tests; deleted afterwards) and the three guards all pass.


## Pre-landing lint fix

The coordinator's faithful stub mirror (`deadcode-stubgen.py`) keeps unused
`mut` visible, where this unit's own mirror had allowed it. It found an
unneeded `mut self` on `refresh_recent` in `grafita/src/session.rs`, which the
production `cargo clippy --all-targets --locked -- -D warnings` would reject.

- The `mut` is removed.
- The mirror's `cargo clippy --all-targets` now reports 0 warnings, the same
  as at `2a3c74f`. With the `mut` put back it reports the warning.
- `grafita-core` passes 245 tests, and the three guards pass.

## Landing

- **Base revision:** `862b2bc0dbc1c3f21477b0759421058bf8d153f3`
- **Check:** `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact digest or set does not match the recorded build; tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:42f4117b49f7560b59a04f30eae36568946d09e99d865c9618fc7257f754dda6, verification_fingerprint sha256:bba7b031d56aff5f9a02d69293fadbcbb1634161371788c3a990f144705f2015; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:23fb312fd5b8535f44aa4cc0f78553d37a952c958031d5bea1830031e494b4ce, verification_fingerprint sha256:700af072fbc47870c17c24de90146068ce58764c1663e32e459898f66929334a; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:73cc6ce724022ec9dc660b6f6265e02d473a68cd4fac9797ee71d29e93112e89, verification_fingerprint sha256:7d31ed8dfa9c5c7a2b29aea59826d8462f7f9ad91e5309901b6f6673d52257d9
- **Deploy:** after the push: grafita: deploy-production.sh, status-production.sh; siderita: deploy-production.sh, status-production.sh
