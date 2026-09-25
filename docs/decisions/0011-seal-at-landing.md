# ADR 0011: Seal a unit at landing, on the commit that will be its parent

- **Date:** 2026-09-25
- **Status:** accepted

## Context

The author runs several agent sessions at once on one clone. Every value that
closes a unit is a function of the commit that will be its parent: the
inventory's `Base revision` must be that commit and its numstat is measured
against it, the next SemVer number follows the version `main` declares, the
ledger closure copies the inventory's diffstat, and the canonical production
artifact is current only for the exact inputs of the tree that lands. A session
that seals its unit before another session publishes has computed all of them
against a parent that is no longer the tip of `main`, and a sealed unit cannot
be rebased because the documentation guard requires the base to stay the
direct parent. Landing by merge would keep the parent but `commit_scope.py`
rejects a merge that carries an inventory, the published-history replays
audit only non-merge commits, and two units of one product would still claim
the same version.

The closure therefore belongs to the moment the parent is known, and no human
or session can know it earlier than the landing does.

## Decision

- Inventories, ledger closure fields, version bumps and the canonical
  production run are produced by `scripts/land-unit.py`, from the canonical
  checkout, on top of the current `origin/main` at the moment of landing.
  Sessions work in worktrees opened with `scripts/worktree.sh` and never
  produce them; a session keeps its ledger row open and writes its evidence
  record.
- The landing squashes the branch into one temporary commit without version
  changes, rebases it onto `origin/main`, merges the plan ledger, the debt
  ratchets and Cargo lockfiles semantically when they conflict, bumps the
  version for `bug`, `milestone` and `release`, asks
  `production_artifact.py check` whether each affected artifact is current and
  builds only when it is not, seals, runs the hooks' own guards, and
  publishes exactly one ordinary commit through the repository hooks by
  fast-forward. When `main` moved during the push, the sealed commit is
  discarded and recomputed on the new `main`, at most three times.
- The exact procedure is [the landing contract](../contracts/landing.md).
  This decision extends [ADR 0004](0004-monorepo-change-ledger.md), whose
  ledger and exact inventory it now produces mechanically, and
  [ADR 0003](0003-reusable-production-artifacts.md), whose artifact identity
  decides whether a landing builds. It changes no guard, hook, inventory
  format, version rule or the one-active-checkpoint rule: history stays
  linear, the base stays the direct parent, inventories stay immutable and a
  unit stays one commit.

## Consequences

- Parallel sessions land in arrival order without resealing by hand; a unit
  that arrives second is recomputed on the first one's commit.
- A landed unit builds at most once, in the canonical checkout, and not at all
  when `production_artifact.py check --require-verified` accepts the existing
  artifact for its inputs.
- Prose conflicts remain the author's: a conflict outside the plan ledger, the
  ratchets and the lockfiles stops the landing, and so does a plan archived on
  `main` while the unit is open.
- Temporary commits on the landing worktree are made with `git commit-tree`
  and never published; only the sealed commit passes the hooks.
- `CONTRIBUTING.md`, `AGENTS.md`, the change policy and the plan template stop
  describing hand-computed inventories and point at the landing; the format
  they describe is unchanged.

## Revisit when

Landing needs a merge commit, a second active checkpoint per project, or the
artifact contract stops deciding currency from registered inputs.
