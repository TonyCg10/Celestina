# Celestina suite implementation roadmap

- **Status:** done
- **Active implementation checkpoint:** none
- **Author validation:** `VAL-GOV-1` in [VALIDATION.md](VALIDATION.md), independent

This file contains only cross-project implementation. Each project's
`ROADMAP.md` is canonical for its own work. Manual Wayland, hardware, visual and
assistive-technology checks live in the corresponding `VALIDATION.md`; they do
not keep an implementation checkpoint open.

The detailed suite roadmap that preceded this structure is preserved in
[history](docs/history/suite-roadmap-through-2026-08-03.md).

## GOV-1 — Neutral repository governance and delivery

**Hypothesis:** an unfamiliar agent can discover the correct rules, current
state, implementation unit, author-only checks, commit scope and production
artifact command without relying on a provider-specific file or prior chat.

**Tangible outcome:** one documented source-of-truth system, short local agent
deltas, reusable verified artifacts and registry-backed guards across every
project.

- [x] Establish governance, standards, contracts, decisions, templates and the
      machine-readable project registry.
- [x] Migrate root and project README/STATUS/ROADMAP/VALIDATION/AGENTS documents,
      preserving detailed completed work under history.
- [x] Standardize build, verify, status, deploy and shell activation commands so
      verification and deployment consume the same production artifact.
- [x] Make the persistent change ledger and strict registered commit prefixes
      enforceable without session context.
- [x] Add deterministic agent-context and documentation-contract checks with
      positive and negative fixtures, then run them in CI.
- [x] Standardize canonical repository content on English, keep author dialogue
      in Spanish, and ratchet remaining legacy code/UI language debt downward.
- [x] Remove the tracked vendor-specific instruction symlink and stale
      references without touching registered worktrees.

The exact build order, exclusions, commit inventory and results live in the
[archived plan](docs/plans/archive/2026-08-03-repository-governance.md) and
[evidence record](docs/evidence/2026-08-03-repository-governance.md).

## Project implementation fronts

| Project | Canonical implementation queue |
|---|---|
| Celestina shell | [celestina/ROADMAP.md](celestina/ROADMAP.md) |
| Shared Rust crates | [celestina-rs/ROADMAP.md](celestina-rs/ROADMAP.md) |
| Shared visual language | [celestina-style/ROADMAP.md](celestina-style/ROADMAP.md) |
| Siderita | [siderita/ROADMAP.md](siderita/ROADMAP.md) |
| Magnetita | [magnetita/ROADMAP.md](magnetita/ROADMAP.md) |
| Grafita | [grafita/ROADMAP.md](grafita/ROADMAP.md) |
| Fluorita | [fluorita/ROADMAP.md](fluorita/ROADMAP.md) |

## Later suite-level implementation

New cross-project work starts only after a concrete consumer exposes it and a
plan defines ownership and an automated exit. Likely fronts include stable
release/version contracts for shared crates, common activation semantics and
packaging beyond the author's machine. They are not active tasks until promoted
from an accepted decision.

## Implementation exit rule

A suite checkpoint closes when its code or documents, same-change tests,
registry/consumer updates and agent-executable evidence pass. A pending author
validation does not keep it open. A failed author validation creates a new
linked remediation unit instead of reopening or rewriting the completed work.
