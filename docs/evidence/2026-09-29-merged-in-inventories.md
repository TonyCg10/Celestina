# Evidence: merging main into a stacked branch through the hooks

- **Date:** 2026-09-29
- **Scope:** `AUD-1-H` of the
  [monorepo hardening plan](../plans/archive/2026-09-26-monorepo-hardening.md):
  `merge_heads` in `scripts/repo_git.py`, `merged_in` in
  `scripts/check-staged-units.py`, `merged_inventory_endpoint` in
  `scripts/documentation_contract.py`, the `RealGuardLanding` test of
  `scripts/test-land-unit.py` and the stacked-branch paragraph of
  [the landing contract](../contracts/landing.md)
- **Environment:** session worktree `unit/suite/AUD-1-H` on the author's
  machine (Linux 7.2.8); Python 3.14.7, Git 2.55.0. The real-guard test clones
  the repository with its history into a temporary directory and pushes to a
  local bare origin; no production entry ran
- **Artifact:** not applicable; the unit changes no registered production or
  verification input

## Defect

The landing contract tells a stacked branch whose dependency's files changed
on `main` to merge `origin/main` in its session worktree. On 2026-09-29 that
merge, for `AUD-1-F`, could not be committed: `origin/main` brought the sealed
inventories of `AUD-1-B`, `AUD-1-C`, `AUD-1-D`, `HALT-SHELL-0` and `HALT-SHELL`,
which are new to the branch's HEAD, and three guards judged them as this
commit's own:

- the documentation contract, run over the index, required each one's base
  revision to be HEAD (`Base revision for AUD-1-B must be the HEAD before the
  commit`);
- `check-staged-units.py` and `commit-msg` counted them as a closure the merge
  makes (`a merge cannot close delivery units`, `a merge cannot include an
  inventoried delivery batch`).

`AUD-1-F` was then rebuilt linearly on `origin/main` instead.

## Change

An inventory that a merge in progress brings with exactly the bytes a merged
commit holds was sealed on that side. The staged-unit guard leaves it out of
the batch, and the documentation contract judges it like any committed
inventory, at the newest commit of the merged side's history that changed it,
with the same base, prefix and plan checks. `repo_git.merge_heads` names the
merged commits: from `MERGE_HEAD` when Git has written it (a stopped merge and
`commit-msg`), or from the `GITHEAD_<object id>` variables Git passes to
`pre-merge-commit` before it writes `MERGE_HEAD`, which a probe repository
showed on Git 2.55.0. Outside a merge nothing changes.

## Procedure

```sh
python3 scripts/test-land-unit.py RealGuardLanding
python3 scripts/test-land-unit.py
bash scripts/test-land-unit.sh
sh scripts/test-staged-units.sh
bash scripts/test-commit-scope.sh
sh scripts/test-documentation-contract.sh
bash scripts/test-architecture-scanners.sh
python3 scripts/test-language-contract.py
sh scripts/test-production-artifacts.sh
python3 scripts/test-production-artifacts.py
sh scripts/test-worktree.sh
python3 scripts/test-version-contract.py
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
python3 scripts/version_tool.py check
```

`RealGuardLanding` now lands its fixture unit, then, on a side branch that
has the old `main` and one commit of its own, merges `origin/main` through the
real hooks twice: as an automatic merge (`pre-merge-commit`, then
`commit-msg`) and as `merge --no-commit` followed by `git commit`
(`pre-commit`, then `commit-msg`).

## Result

- **RED:** with `origin/main`'s guards, the automatic merge failed with
  `Base revision for RG-1 must be the HEAD before the commit`; before the side
  commit was added, with HEAD equal to the base, it failed instead with
  `a merge cannot close delivery units` and `a merge cannot include an
  inventoried delivery batch`.
- **GREEN:** every command above exited 0 on the final tree.

## Limits

- `GITHEAD_` variables are how Git names merged commits to the merge
  machinery and its hooks; a future Git that stopped passing them would make
  an automatic merge fall back to the old refusal, which the test would catch.

## Landing

- **Base revision:** `63eb24e5471ed7ebc6d5d6a9bdb5b9287b6efcfe`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
