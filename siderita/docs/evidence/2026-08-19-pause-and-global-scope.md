# Evidence: 2026-08-19 pausing a job, and one register for the process

- **Date:** 2026-08-19
- **Scope:** `SID-A3-A`; plan
  [pause-and-global-scope](../plans/archive/2026-08-19-pause-and-global-scope.md)
- **Environment:** Arch-derived Linux, Qt 6.11.1, `cargo` stable
- **Artifact:** `siderita/target/release/siderita`, built, verified and deployed
  by `scripts/complete-production.sh`

## What was wrong

**The dock was scoped to a tab.** The job register lived inside each
controller, and each tab owns its own controller, so a copy started in one tab
disappeared from the surface the moment a person switched to another — as if it
had finished, when it was still writing.

**Nothing could be paused.** Every write verb answered only Cancel.

## What changed

`CancellationToken` gained `pause()`, `resume()` and `is_paused()`; its existing
`is_cancelled()` now blocks while paused before answering, at exactly the safe
points every copy, move and archive verb already checks it. No signature in
`siderita-ops` or `siderita-archive` changed — the same token a worker already
holds is now also the one that can hold it.

The job register moved out of `SideritaControllerRust` into a process-wide
`Mutex<Registry>`. Every controller subscribes on start and is queued a
republish whenever the register changes, so the dock, the callout and the
pause/resume button read the same list of jobs from any tab.

## Procedure

| Check | Result |
|---|---|
| `cargo test -p celestina-core` | 34 tests pass, including two that spawn a worker thread and assert it blocks on a paused token and unblocks on cancel |
| `cargo test -p siderita-ops` | 39 tests pass, including one that pauses a copy before it starts, asserts zero bytes were written after 250 ms, then resumes it and asserts it finished |
| `cargo test` (application) | 115 tests pass |
| `scripts/qml-tests.sh` | 72 tests pass, including a new one asserting a held ring shows the pause mark and its callout offers to resume |
| `cargo clippy --workspace --all-targets` | no warnings |
| `scripts/complete-production.sh` | built, verified and deployed |

## Result

Everything above passes and the deployed binary is the one those bytes were
verified as. A copy started in one tab and paused there was watched from a
second tab in the same run, confirming the register is shared rather than
scoped per controller.

## Limits

- A delegated RAR/7z extraction runs as another process; this checkpoint
  pauses only the writes `siderita-ops` and `siderita-archive` perform
  themselves.
- `VAL-SID-09` — the author's own pass: a copy paused and resumed on the live
  session, and a job started in one tab watched and paused from another.
