# Author validation — PROJECT

This queue contains no implementation work and never blocks `ROADMAP.md`.

Statuses: `pending`, `passed`, `failed`, `deferred`, `obsolete`. Use `deferred`
only when a missing real-world precondition moves a check out of the active
queue; it does not block implementation.

## VAL-ID — Perceptible result

- **Status:** pending
- **Related implementation:** ID
- **Requires:** verified artifact and required real environment
- **Procedure:** minimum manual steps
- **Pass condition:** observable, falsifiable outcome
- **Result:** not run
- **Evidence:** none

After pass or failure, `Result` records the observation and `Evidence` links a
dated `.md` record under the registered owner's `docs/evidence/`. On failure,
add for example `Remediation: ID-FIX in [plan](...)`; `ID-FIX` must exist in a
registered active or archived plan ledger. Do not put the solution here. For
`obsolete`, preserve a result and link the superseding decision or evidence.
The same required fields apply to table form and cells never remain empty.
