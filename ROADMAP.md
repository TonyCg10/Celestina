# Celestina suite implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** AUD-1
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

## PRD-1 — The shell's desktop entry as a registered artifact

**Hypothesis:** a file the shell deploys is a file the manifest seals, and the
registry is the only place that decides which those are.

**Tangible outcome:** `celestina.desktop` is a production input and a sealed
artifact, so deploy copies only what verification sealed and the installed copy
can be reported on.

- [x] Register the entry and close the delivered LNG-1 checkpoint so exactly one
      suite checkpoint is active.

The build order, exclusions and results live in
[the archived PRD-1 plan](docs/plans/archive/2026-08-05-desktop-entry-registration.md).

## LNG-1 — Spanish product copy

**Hypothesis:** the language contract can hold development truth in English and
product copy in Spanish without weakening anything it enforces, because the
boundary between them is mechanical.

**Tangible outcome:** a Spanish desktop stops being contradicted by its own
rules. The guard accepts Spanish where a person reads it, keeps rejecting it
everywhere else, and its ratchet can record the reduction that acceptance
causes.

- [x] Record the decision and amend the standard, the root contract and the
      workflow documents that repeat it.
- [x] Teach the scanner the `qsTr()` and `product-copy` exemptions, with
      positive and negative fixtures for each.
- [x] Add the declared-migration escape to the commit guard, so a row whose
      reduction no source earned can still be recorded.
- [x] Record the reduction the migration earned.

The build order, exclusions and ledger are in the
[active plan](docs/plans/archive/2026-08-04-spanish-product-copy.md).

## LND-1 — Seal at landing

**Hypothesis:** the closure of a unit is a function of the commit that will be
its parent, so producing it at landing time removes every reason a second
session has to wait for the first.

**Tangible outcome:** sessions work in their own worktrees and never build
production there; `scripts/land-unit.py` lands one unit from the canonical
checkout with one build at most, and none when the product's inputs did not
move; no guard changes.

- [x] Add the session worktree entry with a shared Cargo cache and refuse
      production runs inside a session worktree.
- [x] Add the landing tool with its fixture tests and CI step.
- [x] Record the decision and move closure from hand-computed inventories to
      the landing.

The build order, exclusions and ledger are in
[the archived plan](docs/plans/archive/2026-09-25-seal-at-landing.md).

## AUD-1 — Monorepo hardening

**Hypothesis:** restoring the pipeline's enforcement first (green CI,
fingerprints that cover every crate an app links, and a landing that accepts
stacked branches) lets each of the 2026-09-26 audit's delivery units land as
one single-prefix commit with a trustworthy guard chain, and no project needs
a second active checkpoint to carry its share.

**Tangible outcome:** the audit's 184 findings are durable evidence, and every
scheduled finding has a ledger row in exactly one plan. When the suite rows
close, GitHub `contracts` is green on `main`, a fix to any linked crate stales
and rebuilds every app that links it, the Magnetita protocol has one owner on
both ends, and the hooks judge the index with committed rules.

- [ ] Record the audit as evidence and open the hardening plans (`AUD-1-A`).
- [ ] Restore green CI and cover Hematita with the style guard (`AUD-1-B`,
      P-1).
- [ ] Let the landing accept a stacked branch (`AUD-1-C`, P-0b).
- [ ] Derive production inputs from Cargo and declare Magnetita Android's
      inputs (`AUD-1-D`, P-2).
- [ ] Give the Magnetita protocol rules one owner and negotiate capabilities
      on both ends (`AUD-1-E`, P-13).
- [ ] Make the hooks and the landing trustworthy and fast, and correct the
      governance documents (`AUD-1-F`, P-20).

The project units of the same program are rows of each project's own plan;
the build order, exclusions and ledger are in
[the active plan](docs/plans/active/2026-09-26-monorepo-hardening.md), and the
findings in [the audit evidence](docs/evidence/2026-09-26-monorepo-audit.md).

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
