# ADR 0002: Separate implementation from author validation

- **Date:** 2026-08-03
- **Status:** accepted

## Context

Wayland, appearance, real keyboard flow, hardware and assistive technology often
require the author and a live session. Keeping implementation milestones open
for those tests makes roadmaps noisy and hides whether code work is actually
finished.

## Decision

Every project has two independent delivery lanes:

1. `ROADMAP.md` and active plans contain implementation plus agent-executable
   checks. Their units close when code and automated evidence satisfy the exit.
2. `VALIDATION.md` contains author-only procedures with independent `VAL-*` ids
   and lifecycle.

Pending author validation never blocks an implementation unit. A failed manual
test records its result and opens a new linked remediation unit; it does not turn
the validation queue into a patch checklist or erase the prior delivery record.

## Consequences

- Roadmaps show actionable implementation state without manual-test noise.
- Manual acceptance remains visible and cannot be misreported as automated.
- Status documents can report both checkpoints without conflating them.
- Evidence and templates need explicit lane ownership.

## Revisit when

A supposedly author-only test becomes reliable and non-invasive automation. It
then moves to the implementation exit through a new decision or plan update;
historical validation results remain preserved.

