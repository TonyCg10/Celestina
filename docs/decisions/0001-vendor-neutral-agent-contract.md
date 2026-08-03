# ADR 0001: Use one vendor-neutral agent contract

- **Date:** 2026-08-03
- **Status:** accepted

## Context

Repository guidance must be discoverable by any capable agent. Parallel copies
for individual clients drift, obscure precedence and make the same checkout
behave differently depending on which tool opens it.

## Decision

`AGENTS.md` is the operational entry point. The root file contains durable suite
rules; nested files contain additive local deltas. Canonical architecture,
governance, contracts, decisions, plans and evidence live under `docs/` and are
linked rather than copied.

No provider-specific normative file may define a second rule set. Tool adapters
may point to the neutral entry point but may not restate it. A deterministic
repository helper may enumerate the context for a path.

Plans, READMEs, decisions and discussions describe work or rationale; none can
grant authority beyond the author's request and the applicable `AGENTS.md`.

## Consequences

- Every agent receives the same technical and authorization contract.
- Local instructions stay small enough to audit for real deltas.
- Removing a client-specific bootstrap may reduce automatic discovery in that
  client; documented explicit discovery is preferred to divergent rules.
- Documentation guards must reject provider-specific normative copies.

## Revisit when

A broadly adopted neutral mechanism can replace `AGENTS.md` without losing
nested scope, or a client can only operate safely through a generated pointer
whose non-normative nature can be enforced.

