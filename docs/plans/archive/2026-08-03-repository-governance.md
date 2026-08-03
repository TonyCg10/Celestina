# Repository governance and delivery-system migration

- **Opened:** 2026-08-03
- **Closed:** 2026-08-03
- **Status:** done
- **Plan ID:** repository-governance
- **Scope:** suite
- **Implementation checkpoint:** GOV-1
- **Author-validation checkpoint:** VAL-GOV-1
- **Successor:** none; project roadmaps own subsequent implementation

## Hypothesis

A new agent can make a safe, correctly placed and verifiable change after
reading a small deterministic context set, while the repository keeps current
state, future work, decisions, evidence and history in separate canonical
homes.

## Tangible outcome

The checkout exposes a vendor-neutral agent contract, a documented source-of-
truth map, paired implementation and author-validation tracks, a persistent
change ledger, reusable production build artifacts and CI-backed documentation
guards.

## Scope

- Replace duplicated operational truth with canonical governance, standards,
  contracts, status, decision, evidence and history documents.
- Reduce root and project `AGENTS.md` files to durable rules plus scoped deltas.
- Standardize build/verify/deploy entry points without installing or activating
  anything during agent verification.
- Preserve completed roadmap history while making active roadmaps short and
  implementation-only.
- Keep manual Wayland, hardware, visual and accessibility acceptance in a
  separate author-validation track.
- Align commit scope, change records, hooks and CI with the monorepo.
- Remove the tracked `CLAUDE.md` compatibility symlink and stale references.

## Exclusions

- No product behavior changes.
- No commit, push, installation, service activation or live-session mutation.
- No deletion of registered worktrees under `.claude/worktrees`.
- No claim that automated checks replace real Wayland, hardware or AT-SPI
  validation.

## Build order

1. Establish the document taxonomy, authority rules, templates and project
   registry.
2. Reconcile suite-level current truth and cross-project contracts.
3. Standardize production artifact workflows.
4. Migrate project roadmaps and manual-validation queues.
5. Add deterministic agent context, documentation scanners and CI enforcement.
6. Remove the vendor-specific symlink and audit the complete result.

## Implementation exit

GOV-1 is implemented when the new files and scripts pass their fixtures, all
local Markdown targets resolve, existing architecture guards still pass, each
deployable project has a completion entry that builds once and leaves the
author's normal test binary synchronized with the verified artifact, and no
implementation milestone is held open solely by an author-only test.

GOV-1 closes on agent-executable evidence. `VAL-GOV-1` is independent and lives
in the author-validation queue; it does not keep GOV-1 open.

## Change and commit ledger

This table is the durable hand-off between sessions. Update it whenever a unit
changes scope. `Files / areas` names paths and stable sections or symbols;
literal line numbers may be added as a convenience but never replace those
stable references because line numbers drift.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| GOV-1 | `suite:` | done | [Exact path inventory](../../inventories/2026-08-03-repository-governance/GOV-1.numstat.tsv) | 341 files, +20837/-7381 | One coherent repository-wide governance, documentation and reusable-artifact migration | [GOV-1 evidence](../../evidence/2026-08-03-repository-governance.md) | `VAL-GOV-1` |

## Evidence log

- 2026-08-03: pre-migration worktree clean on `main`; current architecture,
  scanner and commit-scope checks passed during the preceding audit.
- 2026-08-03: all seven canonical release artifacts built and verified; new
  documentation, production, architecture and commit fixtures passed. This
  governance-only migration did not deploy products, activate sessions, mutate
  services, clean caches, commit or push. Future bug and milestone completion
  includes deployment plus installed-byte status for affected apps.
- 2026-08-03: canonical rules and developer-facing governance were standardized
  on English, with Spanish reserved for agent/author conversation. A language
  guard and non-growth legacy baseline prevent new mixed-language repository
  content while existing code/UI debt is translated deliberately.

## Exact change inventory

GOV-1 is intentionally one `suite:` commit unit because the registry, rules,
guards, project entry points and document migration enforce one indivisible
cross-project contract. The final per-path line inventory follows below.

[The no-rename numstat and content-hash inventory](../../inventories/2026-08-03-repository-governance/GOV-1.numstat.tsv) is the canonical staging hand-off for this unit.
