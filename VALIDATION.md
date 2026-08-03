# Author validation — suite

This queue contains only checks that require the author's session, perception,
hardware, or decision. It contains no implementation work and never blocks a
`ROADMAP.md` milestone from closing.

## States

- `pending`: the author has not run the check.
- `passed`: the result satisfies the contract, with date and evidence.
- `failed`: the result violates the contract and links a new remediation unit.
- `deferred`: the check is outside the active queue because a deliberate
  precondition is absent; it returns to pending when available.
- `obsolete`: a linked decision replaced the contract.

A failure does not become a code task list here. The original implementation
remains historical and remediation is planned as a new unit.

## Active queue

### VAL-GOV-1 — Navigation and terminology

- **Status:** pending
- **Related implementation:** GOV-1
- **Requires:** implemented governance migration with green automated checks
- **Procedure:** ask an agent unfamiliar with the migration to locate the rules,
  active implementation work, author-only checks, commit unit, and production
  artifact command for one project; then assess whether the English repository
  terminology is coherent while the agent communicates with the author in
  Spanish.
- **Pass condition:** the agent finds that context without a provider-specific
  file or earlier conversation and does not confuse implementation with author
  validation.
- **Result:** not run
- **Evidence:** none

## Closed results

No suite-level author validation has been closed under this system yet.
