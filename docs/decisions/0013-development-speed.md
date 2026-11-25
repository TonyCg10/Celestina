# ADR 0013: Development speed — bound the session cache, tier the preflight, lighten small units

- **Date:** 2026-11-24
- **Status:** accepted

## Context

Measured on 2026-11-24, units cost wall-clock time and agent tokens for
reasons that are not the unit's own work. The shared session Cargo target held
407 GiB (970 copies of the shell's test binary, 181 GiB of incremental state)
and three private targets 74 GiB more, because closing a worktree removed the
shared target only with the last one and 53 worktrees were never closed.
`scripts/agent-context.py celestina/src` orders 22 documents, 373 KB, read
completely by every session; the root `AGENTS.md` is 438 lines. The shell's
smoke is one 2 165-line file that runs 22 nested niri runs in series. Review
rounds reach 10 on single units, and 87 of the last 190 commits are
`maintenance`. The time of one full landing and the cold and incremental build
times were not measured; the first unit records them as the baseline.

The design is
[the development speed spec](../superpowers/specs/2026-11-24-suite-dev-speed-design.md);
the delivery is [the plan](../plans/archive/2026-11-24-dev-speed.md).

## Decision

- **Four independent units, cheapest first:** the build cache and disk (a
  lighter session dev profile written by `worktree.sh open`, a cap on the
  session target and the closing of a landed unit's worktree), the preflight in
  tiers with a budget, the smoke in phases (the shell's own plan), and the
  ceremony.
- **Medium cut for the ceremony.** A small `maintenance` or `bug` unit that
  touches no sensitive path uses a light lane: no design or task plan
  document, a short evidence record from a template, and one review round. Any
  unit takes at most two review rounds, three on sensitive paths. The record
  stays because the landing writes its `## Landing` section into it, and the
  full smoke stays at the landing because it runs inside the verify script
  whose result seals the artifact.
- **Standing tracks.** This amends the sentence of the standing-tracks design
  of the same day, "each entry is one ordinary unit with the repository's
  rules unchanged": a track entry that qualifies uses the light lane.
- **Plan housekeeping.** The suite roadmap allows one active plan, so the RSH-1
  plan is archived and `dev-speed` is opened by an administrative commit.

## Consequences

- A landing verifies exactly what it verifies today: production artifacts,
  their verification, version bumps, inventories, commit scope and hook
  integrity rules do not change.
- Historical plans, specs, evidence and inventories stay as written.
- The lighter lane is never open to a sensitive change: the lock, polkit, PAM,
  the guards and the landing code.
- A shorter preflight never hides a rule: every moved rule stays reachable
  from a one-line pointer with its trigger.
- The shared scripts the first unit changes are not verification inputs, so
  its landing rebuilds and re-verifies no project.

## Revisit when

The measured baseline shows the cap or the dev profile do not pay back, a
light-lane unit causes a defect its review round would have caught, or the
`now` budget forces a rule out of reach of the sessions that need it.
