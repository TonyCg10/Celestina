# ADR 0003: Reuse the verified production artifact

- **Date:** 2026-08-03
- **Status:** accepted

## Context

Agent builds have commonly proved a separate binary while the installed program
remained unchanged. The author then had to rebuild to use the result, wasting
resources and weakening the connection between evidence and delivered code.

## Decision

Each registered project exposes separate build, verify and deploy entries.
Build creates the canonical release artifact and a fingerprinted manifest.
Verify exercises that exact artifact without installation or activation. Deploy
copies only a still-current, verified artifact and never compiles.
Deployable projects also expose a completion entry that executes those phases
in order and finishes by comparing the installed bytes.

The shell additionally separates activation because running it mutates the live
desktop surface. Incremental build caches are reused and no standard entry runs
`clean` by default.

## Consequences

- Agent evidence and the author's normal test binary refer to the same bytes.
- A completed bug fix or milestone already deploys without a second build.
- Stale or modified artifacts fail closed through their manifest.
- Existing convenience scripts that mix phases are not canonical verification
  entries and must be split or wrapped.

## Revisit when

Reproducible packaging provides an equally strong artifact identity across
machines, or a project has no deployable artifact and can document a narrower
equivalent without weakening the no-install verification boundary.
