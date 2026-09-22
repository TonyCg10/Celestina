# The table's cursor, its outcome and its signal — H3-E

- **Date:** 2026-09-22
- **Scope:** `H3-E` of
  [`../plans/archive/2026-09-22-h3-table-fixes.md`](../plans/archive/2026-09-22-h3-table-fixes.md):
  the ten findings of the `H3` whole-branch review that the controller did
  not defer, and the 0.4.1 delivery
- **Environment:** Rust 1.97.1 (pinned), Qt 6 with CXX-Qt 0.9.1, the author's
  checkout, offscreen only
- **Artifact:** `hematita/target/release/hematita` at `0.4.1`, built, verified
  and deployed to `~/.local/bin/hematita` by
  `hematita/scripts/complete-production.sh`

## What changed, per finding

1. **(CRITICAL) The cursor drifted off the selection.** `list.currentIndex`
   was written only from `onSelectedPidChanged`, while `weave()` rebuilt the
   rows and the entries on every revision — so a second later the cursor
   pointed at whatever had moved into that index, and a filter, a sort or a
   fold moved it further. `ProcessTable` now has one `anchorCursor()`: it puts
   the cursor back on the selected pid's entry, and when that pid is no longer
   in the list — the process ended, or the search excluded it — it lets the
   selection go (`selectedPid = -1`) instead of pointing at a stranger. It is
   called at the tail of `weave()` and of `toggleGroup()`. An `anchoring` flag
   makes the three writers — `anchorCursor`, `onSelectedPidChanged` and the
   list's `onCurrentIndexChanged` — stand aside for each other rather than
   chase one another round.
2. **(IMPORTANT) The outcome was unreadable.** `apply` cleared it on every
   snapshot, which made `failed` and `refused` visible for under two seconds
   and contradicted `clear_action`'s own documentation. `apply` no longer
   clears anything. The outcome now ends in exactly two ways: the page calls
   `clearAction()` when the selection moves, and a new action overwrites it.
   The doc comment says so.
3. **(IMPORTANT) The signal could reach the wrong process.** The snapshot is
   up to two seconds old, so a PID that died in that window may already be
   somebody else's. `ProcessReading` now carries `start_ticks`;
   `expected_identity` reads the `(start_ticks, uid)` the table showed, and
   `send_signal` calls `read_identity_now`, which re-reads
   `/proc/<pid>/stat` and `/proc/<pid>/status` and checks that `stat` names
   the PID it was asked about. `still_the_same` — a pure function over the two
   values, and the one under test — decides; anything but an exact match, an
   unreadable `/proc` included, is `refused` and no syscall happens. This is
   blocking IO on the Qt thread: two small files, once per signal a person
   asked for. It is accepted for the same reason the `.desktop` read is — it
   happens on a human action, not on a tick — and it is the only way to close
   a gap that a longer cadence would only widen.
4. **(IMPORTANT) Every delegate was a Tab stop.** `ProcessRow` and
   `ApplicationRow` are `Qt.NoFocus`; the `ListView` is the table's one stop,
   which is what `activeFocusOnTab` and `keyNavigationEnabled` were for.
5. **(IMPORTANT) Groups could not be folded from the keyboard.** The list
   answers Space, Return and Enter by toggling the application under the
   cursor, Left by collapsing it and Right by expanding it. `foldCurrent`
   answers whether it acted, so those keys over a process row — or in the flat
   layout — stay unaccepted and reach whatever handles them next.
6. **(IMPORTANT) The "Usuario" title was an inert button.**
   `hematita_core::process_view::SortField` gained `User`, token `"user"`,
   ordering by uid with ties broken by pid (the uid, not the login name: a
   name is a label a uid carries), with a test case over a fixture whose uids
   vary. `ProcessTable` no longer swallows `sortRequested("user")` and starts
   it ascending like the other identifier columns. Separately, the active
   column's `Accessible.name` now appends its direction, composed in QML from
   `qsTr("ascendente")` / `qsTr("descendente")` — the arrow glyph said it only
   to a pair of eyes.
7. **(IMPORTANT) The columns overflowed a narrow window.** The six fixed
   widths summed to 580 beside a 160-wide name floor, against a 640 minimum
   window. The name column is now `Math.max(160, table.width - fixedTotal)`,
   and `read` and `write` are dropped — header cell and row cell both, by a
   `shown` flag in the column description — below 760 logical pixels, where
   `fixedTotal` follows.
8. **(IMPORTANT) `stop()` was one-way and leaked.** It set the flag and
   joined, leaving the flag raised (so a later `subscribe` spawned a thread
   that exited at once) and every closure's captured `CxxQtThread` alive for
   the life of the process. Under the `handle` lock it now drains
   `subscribers` after the join and lowers the flag, so the hub is stopped,
   empty and able to start again.
9. **(MINOR) `Accessible.checkable`.** `ApplicationRow` declares it beside
   `Accessible.checked`, so a screen reader is told the state it is reading
   is one that can change.
10. **(MINOR) Note precedence.** A table that could not be read says so before
    it says anything about a row in it; the rows are the stale ones. The
    unavailability branch moved above the not-actionable branch.

Deferred by the controller and deliberately not attempted: the subscribers
lock held across the publish, the double weave on the page that is not
showing, a debounce on the search field, telling a refused `io` read apart
from a zero rate, the per-tick snapshot clones, and the `revision` wrap.

## Procedure

`cargo fmt --all` first, then one production run — `complete-production.sh`
builds, verifies (fmt-check, clippy, the crate and application tests, the
qmllint ratchet, the smoke) and deploys in one pass, so the author validates
the bytes that were checked.

```sh
python3 scripts/version_tool.py bump hematita bug --unit H3-E \
    --summary "Fix the process table cursor, the action outcome and the signal re-validation"
python3 scripts/version_tool.py check
cd hematita && cargo fmt --all
hematita/scripts/complete-production.sh
hematita/scripts/status-production.sh
sha256sum hematita/target/release/hematita ~/.local/bin/hematita
```

## Result

- **Exit:** `0`. See the runs quoted in the report accompanying this unit.
- **Version:** `hematita: 0.4.0 -> 0.4.1 (bug)`;
  `version-contract: OK (8 owners)`.
- **Tests:** `hematita` `9 passed; 0 failed` (the new one is
  `a_pid_is_the_same_process_only_with_the_same_start_time_and_owner`);
  `hematita-core` `53 passed; 0 failed` (the new one is
  `sorting_by_user_orders_by_uid_and_breaks_ties_by_pid`) plus its
  `8 passed; 0 failed` captures.
- **Clippy** `-D warnings` and **`cargo fmt --all --check`:** clean.
- **qmllint:** the `hematita` row stayed at `0`, nothing suppressed.
- **Smoke and the offscreen run:** the binary stays alive, the shape line
  still prints, and no `TypeError`, `ReferenceError`, `Unable to assign` or
  `Cannot read property` appears.
- **Deployment:** the installed `~/.local/bin/hematita` matches the checkout's
  `target/release/hematita` byte for byte by `sha256sum`.

## Limits

- **Nothing was operated.** No key was pressed, no signal was sent, no window
  was opened: the offscreen platform takes no input and injecting keys into
  the author's live session is forbidden. That the cursor now stays on the
  selection for ten seconds, that Left folds an application, that Tab lands on
  the list once, and that the columns fit at 640 are argued from the code and
  shown only by a clean start. `VAL-H3` now asks for the ten-second cursor
  check and the keyboard fold explicitly.
- **The TOCTOU window is narrowed, not closed.** Between
  `read_identity_now` and `kill_process` there is still a gap; only the kernel
  could close it (a pidfd), and that is a larger change than this correction.
  What is fixed is the two-second gap, which was the one wide enough to
  matter.
- **PID reuse was not reproduced.** `still_the_same` is unit-tested over the
  four cases that matter; arranging a real reuse on a live machine is not
  something this run can do.
- **The 760-pixel threshold was not looked at.** It is arithmetic over the
  column widths, not a judgement anybody has seen; whether dropping the rates
  is the right thing to drop is a `VAL-H3` question.
- **`stop()`'s restart path is unexercised.** Hematita stops the sampler once,
  when its only window goes away. The drain and the lowered flag are correct
  by inspection and by nothing else.

## Follow-up

`VAL-H3` in [`../../VALIDATION.md`](../../VALIDATION.md), whose procedure and
required version this unit updated. The six deferred findings are recorded in
the plan's Exclusions for whoever opens `H4`.
