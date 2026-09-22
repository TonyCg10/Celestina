# The Processes and Applications pages — H3-C

- **Date:** 2026-09-22
- **Scope:** `H3-C` of
  [`../plans/active/2026-09-22-h3-processes.md`](../plans/active/2026-09-22-h3-processes.md):
  the sampler's process section, `HematitaProcesses`, the shared list helpers,
  the table, its two row shapes, the kill dialog, both pages and the style
  symlinks; plus the `H3-A` review carry-over on the Performance list
- **Environment:** Rust 1.97.1 (pinned), Qt 6 with CXX-Qt 0.9.1, the author's
  checkout, offscreen only
- **Artifact:** `hematita/target/release/hematita`, rebuilt and re-sealed by
  `hematita/scripts/verify-production.sh`

## What changed

1. **The sampler reads processes every second tick.** `PROCESS_TICKS = 2`
   lives once, in `sampler.rs`. `sample_processes` walks `/proc`, reads each
   PID's `stat` and `status` exactly once per tick, and takes `cmdline` and
   `cgroup` only the first time a `(pid, start_ticks)` pair is seen —
   `ProcessFacts` caches the name, the uid and the application id, and a PID
   the sweep did not see loses its entry. The resident size comes from the one
   `status` read; `io` is read only for the user's own processes, because the
   kernel refuses the others. The `elapsed` those rates are measured over is
   the time since the previous *process* read, not since the previous tick,
   and the CPU divisor is the last CPU reading's core count.
   `Snapshot.processes` is `None` on the ticks that read nothing.
   `ProcessSnapshot.users` is an `Arc<HashMap<u32, String>>` read once when the
   thread starts: a user created during a session is rare, and shows as a
   number.
2. **One thread, two hubs.** A hub object per sampler thread would have been
   two readers of one machine, so `sampler.rs` now owns a process-global hub:
   `subscribe` registers a `Fn(&Snapshot)` and spawns the thread the first
   time, `stop` sets the flag and joins. `HematitaResources` lost its
   `Sampler` field for a `started` flag and gained `shutdown()`, which
   `Main.qml` calls from `Component.onDestruction`. Both hubs subscribe; the
   binary runs exactly one `hematita-sampler` thread (below).
3. **`HematitaProcesses`.** Index-aligned lists plus a `revision`, like the
   resources: ten process columns, six group columns, the counts before and
   after the filter, the availability and its reason, and the outcome of the
   last action. The state QML sets — `sortField`, `sortAscending`,
   `filterText`, `grouped` — is read by `refresh()`, which re-projects the
   last snapshot through `hematita_core::process_view` and bumps `revision`
   last, so the page rebuilds once with every list in place. A section whose
   generation is not newer than the applied one is dropped.
4. **The only signal path.** `terminate` and `kill` both go through
   `send_signal`, which asks `owned_pid` first: a PID is a target only when it
   is a positive `u32`, greater than one, not `std::process::id()`, present in
   the latest snapshot, and carrying the process's own uid. Everything else is
   `refused` without a syscall. `rustix::process::kill_process` is the one
   call, so there is no `unsafe` in the binary.
5. **`lists.rs`.** `strings`, `doubles`, `nested` and `widen` moved out of
   `resources.rs`; both hubs import them. That is the real intersection of the
   two adapters — a list conversion, not a shared notion of a row.
6. **The table.** `ProcessTable.qml` is one component both pages use:
   `ProcessPage` gives it `grouped: false`, `ApplicationsPage` `grouped: true`.
   It weaves the published lists into rows and groups, lays them out — flat,
   or each application followed by its processes unless folded — and shows a
   search field, the live count, the terminate and kill capsule, one line of
   explanation, the sortable `ProcessHeader` and the rows.
   `ProcessRow` is the content family (the shared row highlight paints hover,
   press and selection); `ApplicationRow` carries the application's own icon
   through `IconImage`, falling back to the catalogue's window glyph.
   `KillDialog` asks before the irreversible one.
7. **The window owns the grouping.** Both pages share one
   `HematitaProcesses`, so `Main.qml` sets `grouped` from
   `onCurrentSectionChanged` and calls `refresh()`; `ProcessTable` never sets
   it. The Sensors placeholder is the one that remains, for H4.
8. **The `H3-A` review finding is closed.** `PerformancePage.qml` gained
   `onSelectedKeyChanged: list.currentIndex = page.indexOf(page.selectedKey)`.
   Arrow keys write `currentIndex`, which replaces the binding that kept the
   two the same; writing it back is what lets a later mouse click re-take the
   current item instead of leaving the view's current row behind.

## Three deliberate deviations from the brief

- **The delegate is static, not a `Loader`.** The brief's `Loader` reads
  `slot.entry` from inside `Component`s declared beside it, which
  `pragma ComponentBehavior: Bound` forbids — those components are not
  lexically inside the Loader and only the dynamic context would have
  supplied the id. Rather than suppress the warning or thread the entry
  through `onLoaded`, the delegate builds both row shapes and shows one. A
  `ListView` realises only the visible rows, so this is about twenty extra
  items, not two thousand.
- **`status` is read before the facts branch.** The brief's listing read it
  twice for a known PID; the restructure the brief itself asked for reads it
  once and uses it for both.
- **The scroll bar is a sibling, not an attachment.** `CelestinaScrollBar` is
  built from primitives and is not a `QtQuick.Controls.ScrollBar`, so
  `ScrollBar.vertical:` refused it (two qmllint warnings). It now sits beside
  the `ListView` inside the panel's content item, the way Grafita's document
  view places it.

## Procedure

One build cycle, after every edit above. The two new dependencies and the lock
entry they need landed before it; the lock was brought up to date with
`cargo metadata`, which added `celestina-core` and named `rustix` among
Hematita's dependencies and moved no other crate.

```sh
cd hematita && cargo fmt --all
hematita/scripts/build-production.sh
(cd hematita && cargo test --release --locked --all-targets \
    && cargo clippy --release --all-targets --locked -- -D warnings \
    && cargo fmt --all --check)
hematita/scripts/verify-production.sh
```

The one rebuild inside that cycle was the scroll-bar correction above, found
by qmllint; nothing was built "to check" between tasks.

## Result

- **Exit:** `0` for the build, the tests, clippy, `fmt --check` and the
  verification; `manifest: hematita/target/production-artifact.toml (verified)`.
- **Build:** `>> Hematita release build steps completed (not installed)`.
- **Tests:** `test result: ok. 8 passed; 0 failed` in `hematita` (the four new
  ones cover `row_of`'s actionable rule, a reading with no rates yet, and
  `owned_pid` refusing 0, 1, this process, a negative PID, another user's
  process and a PID no snapshot lists); `52 passed; 0 failed` plus
  `8 passed; 0 failed` for `hematita-core` and its captures.
- **Clippy** `-D warnings` and **`cargo fmt --all --check`:** clean.
- **qmllint:**
  `qmllint-production: OK — org.celestina.hematita (0 non-fatal baseline warning(s))`
  — the row stayed at `0`, with nothing suppressed, across the eight new
  symlinks and the seven new components.
- **Smoke:**
  `smoke: OK — binary alive for 10 s, the first row published the CPU contract, no QML errors, no auto-bindings`.
- **The offscreen run:** eight seconds with `HEMATITA_SMOKE_SHAPE=1` on the
  `offscreen` platform, `rc=124` (still alive). The shape line still prints —
  `qml: hematita-shape cpu 3 60` — and a grep for
  `TypeError|ReferenceError|Unable to assign|Cannot read property` finds
  nothing. Both pages are built at startup inside the `StackLayout` even
  though the Performance page is the one showing, so the table, both row
  shapes and the dialog all construct.
- **One thread, and it reads `/proc`:** the running binary's
  `/proc/PID/task/*/comm` lists a single `hematita-sample[r]` beside the Qt
  threads, with both hubs subscribed. Its read count over two seconds went
  from `syscr: 17588` to `syscr: 24820` — about seven thousand reads, which
  is the two-to-three files per PID that one process tick over this machine's
  469 processes costs.

## Limits

- **No visual check.** Nothing was opened on the live or the nested session.
  The columns' widths, the indentation of a grouped row, the dialog's card and
  the row highlight's three states are argued from the shared components'
  contracts and shown only by a clean offscreen start. Seeing them is `VAL-H3`.
- **No signal was sent.** `terminate` and `kill` were never invoked against a
  live process; `owned_pid`'s refusals are unit-tested, the `done` path is
  not. Sending one belongs to `VAL-H3`.
- **The icon-theme lookup is unverified.** `IconImage`'s `name` resolution
  through Qt's icon theme, and the `file://` path branch for a `.desktop`
  file that gives an absolute icon, were not observed rendering; the offscreen
  platform loads no icon theme. `VAL-H3`.
- **The `.desktop` read is on the Qt thread.** `entry_for` opens a handful of
  small files, once per application id, cached for the life of the window
  (including the absence of one). It is the same trade Siderita's icon
  resolution makes and is accepted for H3; it is the only blocking IO outside
  the sampler thread.
- **The keyboard walk was not performed**, so the `H3-A` carry-over is argued
  from the `ListView` contract rather than observed: no screen reader was
  attached and no key was sent. Injecting keys into the author's live session
  is forbidden.
- **The grouped layout was not measured against a busy machine.** `project`
  and `group` run over every row on each `refresh`, which is every two
  seconds and on every keystroke in the search field. Nothing here claims that
  is fast enough; it claims only that it is correct.

## Follow-up

`VAL-H3` in [`../../VALIDATION.md`](../../VALIDATION.md); `H3-Z` closes the
checkpoint and ships 0.4.0.
