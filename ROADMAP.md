# Celestina suite implementation roadmap

- **Status:** idle
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

## GOV-2 — Align the guards with the contract they enforce

**Hypothesis:** the mechanical gaps found by auditing GOV-1 and the author's
accepted typed-version convention can share one coherent guard boundary because
both are interpreted from the same committed registry and staged delivery.

**Tangible outcome:** a green run of every guard and fixture suite prints
English only; a project commit that shrinks a guarded file is accepted together
with its baseline row; `agent-context.py` prints the standards local contracts
require; typed product commits advance exact SemVer declarations and append
durable history; and no root document describes a workflow, exit command or CI
job the checkout lacks.

- [x] Translate every changed unratcheted guard and production entry point that
      emitted Spanish, and lower or remove each baseline row whose measured
      debt was eliminated by the translated sections.
- [x] Register `commit_policy.shared_ratchet_files` so a shrunk guarded file and
      its ratchet row land in the same commit instead of publishing a revision
      whose own architecture guard is red.
- [x] Register `suite.shared_rules` and print it from `scripts/agent-context.py`
      so the deterministic context is complete, not merely sufficient.
- [x] Correct ADR 0001, the CI contract map, every stale project exit command,
      the shell plan's deployment wording and the volatile claims in the root
      `STATUS.md`.
- [x] Adopt `bug`, `milestone`, `release` and `maintenance` commit kinds,
      register the six current product version sources, and enforce exact
      SemVer/history transitions without inventing retrospective releases.
- [x] Close the single ledger unit with its inventory and evidence record
      when the author requests the commit.

The remaining exclusions are deliberate and each needs an accepted decision first:
hardening the language detector (it would force a re-based ratchet), requiring
inventories for project-prefixed source commits, defining proportionality for
`complete-production.sh` fan-out across shared-crate consumers, and collapsing
the ledger rules that are currently written in five documents. They are recorded
in [the archived plan](docs/plans/archive/2026-08-03-guard-contract-alignment.md).

## ACT-1 — Source-first standalone library navigation

**Hypothesis:** the activation contract can name the configured media source as
the standalone library's top-level axis without weakening any invariant it
already enforces.

**Tangible outcome:** the contract describes Gallery and Music as catalogue
projections a selected source resolves to, an accepted ADR records why, and the
Fluorita implementation plan is authorized against a contract it does not
contradict.

- [x] Record the accepted decision and its index row.
- [x] Amend the single behavioural-invariant bullet naming the standalone
      surfaces, leaving the gesture mapping and embedded-surface boundary
      untouched.

The build order, exclusions and ledger are in the
[archived plan](docs/plans/archive/2026-08-04-source-first-library-navigation.md).
The product work it unblocked is owned by
[fluorita/ROADMAP.md](fluorita/ROADMAP.md).

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
