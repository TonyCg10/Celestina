# Pausing a job, and the register that belongs to the process

- **Opened:** 2026-08-19
- **Plan ID:** pause-and-global-scope
- **Status:** active
- **Scope:** siderita
- **Implementation checkpoint:** SID-A3
- **Author-validation checkpoint:** `VAL-SID-09` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

The author found the operations dock scoped to the tab that started the job — a
copy launched in one tab vanished from view when they switched to another,
though it was still running. Moving the job register from each controller to
one shared by the process, and letting a long write be held rather than only
cancelled, is a design correction to the surface `SID-A2` introduced, not a new
subsystem.

## Tangible outcome

A job started in any tab is visible, and pausable, from every tab. Pausing rides
on the existing cancellation token rather than a second one, so it reaches every
safe point a long copy or move already checks without a single signature
changing in `siderita-ops` or `siderita-archive`.

## Scope

- `celestina_core::CancellationToken` gains `pause`/`resume`/`is_paused`; its
  `is_cancelled` blocks while paused, at the same points every operation was
  already asking it.
- The job register moves to a process-wide singleton; every controller
  subscribes to it on start and republishes when it changes.
- The dock and its callout show and control the held state.

## Exclusions

- Pausing a delegated RAR/7z extraction: the writer there is another process,
  and holding it needs a signal this checkpoint does not send.
- The author's own pass on the live session, tracked as `VAL-SID-09`.

## Build order

1. The token: pause/resume/is_paused, and the blocking `is_cancelled`.
2. The registry: one per process, controllers as listeners.
3. The surface: the pause button, the held ring and callout state.

## Implementation exit

Close `SID-A3` when a paused copy writes nothing until resumed (proven with a
test, not only observed), the dock shows a job started in another tab, and
`scripts/complete-production.sh` builds, verifies and deploys those exact bytes.
The author's own pass belongs to `VAL-SID-09`.

## Change and commit ledger

`celestina_core::CancellationToken` is shared infrastructure with its own
registered component prefix. Its half of this checkpoint — `pause`/`resume`/
`is_paused`, and a blocking `is_cancelled` — is a separate atomic
`celestina-core:` commit that carries code and its own tests but, per the
component-prefix rule, no ledger row or inventory here.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| SID-A3-A | `siderita:` | done | [inventory](../../inventories/2026-08-19-pause-and-global-scope/SID-A3-A.numstat.tsv) | 16 files, +528/-105 | A process-wide job register, the pause proof in `siderita-ops`, and the dock's pause control, built on `celestina-core`'s new pause | [evidence](../../evidence/2026-08-19-pause-and-global-scope.md) | `VAL-SID-09` |

Like every plan in this repository, this one records intent and grants no
authority.
