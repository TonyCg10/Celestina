# ADR 0004: Keep a persistent monorepo change ledger

- **Date:** 2026-08-03
- **Status:** accepted

## Context

All suite projects share one Git history, while work can cross sessions and
project boundaries. Relying on conversational context to reconstruct changed
files produces accidental mixed commits and misleading prefixes.

## Decision

Every active plan carries a change and commit ledger. Each unit records its
status, registered commit prefix, stable files or areas, final diffstat,
intended result, automated evidence and related author validation.

Paths and section or symbol names are canonical. Literal line numbers may be
added for convenience but never stand alone because edits make them drift. The
ledger is updated whenever scope changes and when the unit closes. Its exact
path inventory uses no-rename numstat so later heuristic rename detection cannot
change the recorded hand-off.

Commits remain an explicit author-requested operation. Their subjects use the
registered project prefix and English imperative; `suite:` is reserved for a
genuinely cross-project unit.

## Consequences

- A new session can stage and commit without guessing prior intent.
- Cross-project changes are visible before they become one oversized commit.
- Hook and CI policy can be derived from `docs/projects.toml` instead of a
  second hard-coded project map.
- Plan maintenance is part of delivery evidence, not retrospective prose.

## Revisit when

The repository adopts another durable, versioned change ledger that preserves
the same path, evidence and commit-scope guarantees.
