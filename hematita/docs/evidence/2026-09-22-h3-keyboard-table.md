# The process table is operable from the keyboard — H3-D

- **Date:** 2026-09-22
- **Scope:** `H3-D` of
  [`../plans/archive/2026-09-22-h3-processes.md`](../plans/archive/2026-09-22-h3-processes.md):
  the five findings the `H3-C` review left open — the sort header and the row
  cursor for the keyboard, the action outcome that never cleared, the IO
  counters keyed by PID alone, the subscriber lock failure that reported
  success, and the group row's broken `checked` binding
- **Environment:** Rust 1.97.1 (pinned), Qt 6 with CXX-Qt 0.9.1, the author's
  checkout, offscreen only
- **Artifact:** `hematita/target/release/hematita`, rebuilt and re-sealed by
  `hematita/scripts/verify-production.sh`

## What changed

1. **The sort header is a row of buttons.** Each cell in `ProcessHeader.qml`
   was a plain `Item` with a `MouseArea`: a column only a pointer could sort.
   It is now an `AbstractButton` with `focusPolicy: Qt.TabFocus`,
   `Accessible.role: Accessible.Button` with the column's title as its name,
   and `onClicked: header.sortRequested(field)` — so Tab reaches it and Space
   or Return sorts by it, from the control's own contract rather than from a
   key handler written here. The lift stays the controls family (hover wash,
   pressed wash, never the accent ramp) and a `CelestinaFocusRing` follows the
   lift rectangle's corner radius, shown on `visualFocus`.
2. **The list's cursor and the selection are one thing.** `ProcessTable.qml`
   had `keyNavigationEnabled` and `activeFocusOnTab` on the `ListView` but
   nothing tying `currentIndex` to `selectedPid`, so an arrow key moved an
   invisible cursor that selected nothing. Two functions name each in the
   other's terms — `entryOfPid` and `pidOfEntry`, both aware that an entry may
   be an application row — and the two writes close the loop the way
   `PerformancePage` does: `onCurrentIndexChanged` selects the entry's process
   (and selects nothing when the cursor lands on an application row, which is
   a place in the list, not a row to act on), and `onSelectedPidChanged`
   scrolls the cursor to the selected pid. `Accessible.role: Accessible.List`
   and its name are unchanged.
3. **The outcome is forgotten.** `actionOutcome`, `actionKind` and
   `actionPid` were set once and never cleared, so the line reporting a
   signal sent to one process outlived the moment it described. `HematitaProcesses` gained
   `#[qinvokable] fn clear_action()`, which `apply` calls whenever a new
   process snapshot lands and which the table calls from
   `onSelectedPidChanged` — a new selection is a new question.
4. **The IO counters are keyed by identity, not by number.** `NamedCounters`
   was keyed by the PID as text, so a recycled PID inherited the rates of
   whatever held that number before it. The key is now
   `io_key(pid, start_ticks)` — `"{pid}:{start_ticks}"` — the same
   `(pid, start_ticks)` identity the CPU sampler already keys on, so a
   recycled PID starts its rates over.
5. **A poisoned subscriber lock is an error.** `subscribe` used
   `if let Ok(...)` on the `subscribers` mutex and returned `Ok(())` after
   silently dropping the callback — a hub that answered "started" while
   listening to nothing. The lock failure now maps to
   `io::Error::other(...)`, the way the `handle` lock already did, so the
   caller sets `startFailed` instead.
6. **The group row no longer fights its own binding.** `ApplicationRow` was
   `checkable: true` with `checked: row.expanded`; the first click made the
   button write its own `checked`, which replaced the binding and left the
   chevron telling a different story from the rows below it. Both properties
   are gone. The fold lives in the table's `collapsed` map, `Accessible.checked`
   still reports `expanded`, and the chevron reads it.

## Procedure

One build cycle, after all five edits.

```sh
cd hematita && cargo fmt --all
hematita/scripts/build-production.sh
(cd hematita && cargo test --release --locked --all-targets \
    && cargo clippy --release --all-targets --locked -- -D warnings \
    && cargo fmt --all --check)
hematita/scripts/verify-production.sh
```

## Result

- **Exit:** `0` for the build, the tests, clippy, `fmt --check` and the
  verification; `manifest: hematita/target/production-artifact.toml (verified)`.
- **Build:** `>> Hematita release build steps completed (not installed)`.
- **Tests:** `test result: ok. 8 passed; 0 failed` in `hematita`;
  `52 passed; 0 failed` plus `8 passed; 0 failed` for `hematita-core` and its
  captures, run again by the verification.
- **Clippy** `-D warnings` and **`cargo fmt --all --check`:** clean.
- **qmllint:**
  `qmllint-production: OK — org.celestina.hematita (0 non-fatal baseline warning(s))`
  — the row stayed at `0`. The header's rebuild into a `Control` moved the
  title row inside a `contentItem`, where it is wrapped in an `Item` because
  a `Row` positions its children's `x` and forbids them anchors.
- **Smoke:**
  `smoke: OK — binary alive for 10 s, the first row published the CPU contract, no QML errors, no auto-bindings`.
- **The offscreen run:** eight seconds with `HEMATITA_SMOKE_SHAPE=1`,
  `rc=124` (still alive); `qml: hematita-shape cpu 3 60` still prints and a
  grep for `TypeError|ReferenceError|Unable to assign|Cannot read property`
  finds nothing. The whole output is the shape line and the two expected
  no-session-bus lines, so the rebuilt header and the two new handlers
  construct and run without an anchor or binding warning.

## Limits

- **Nothing was operated.** No Tab was pressed, no arrow key was sent, no
  screen reader was attached: injecting keys into the author's live session is
  forbidden, and the offscreen platform takes no input. That Tab reaches a
  header cell, that Space sorts by it, that the focus ring appears, and that
  an arrow key now moves the selection are argued from `AbstractButton`'s and
  `ListView`'s contracts and shown only by a clean start. Seeing them is
  `VAL-H3`.
- **The outcome line is now short-lived.** Clearing on every snapshot means a
  terminate or kill message is visible for at most the two seconds until the
  next process reading. That is what the review asked for and it is honest —
  the message describes a moment — but nobody has watched one to judge whether
  two seconds is long enough to read it. `VAL-H3`.
- **The recycled-PID case was not reproduced.** The IO key is argued from the
  same identity the CPU sampler uses and is not covered by a new test:
  producing a PID reuse on a live machine is not something this run can
  arrange.
- **The poisoned lock was not reproduced.** Nothing in this binary panics
  while holding the subscribers mutex, which is why the branch was unreachable
  and wrong in the first place.

## Follow-up

`VAL-H3` in [`../../VALIDATION.md`](../../VALIDATION.md). Deferred by the
controller: the modal scrim covering the page rather than the window (the
suite's convention, to be judged at `VAL-H3`), and the grouped layout's
complexity (`H4`).
