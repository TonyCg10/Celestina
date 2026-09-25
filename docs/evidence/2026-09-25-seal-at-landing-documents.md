# Evidence: the seal-at-landing decision and documents

- **Date:** 2026-09-25
- **Scope:** `LND-1-C` of the
  [seal-at-landing plan](../plans/active/2026-09-25-seal-at-landing.md):
  [ADR 0011](../decisions/0011-seal-at-landing.md) and its index row,
  [the landing contract](../contracts/landing.md) and its entry in
  `suite.shared_rules` of `docs/projects.toml`, and the amendments to
  `AGENTS.md`, `CONTRIBUTING.md`, `docs/governance/change-policy.md`,
  `docs/templates/plan.md` and `docs/README.md`
- **Environment:** repository checkout on Linux at base `2162080`; Git 2.43.0,
  Python 3.11
- **Artifact:** not applicable

## Procedure

```sh
sh scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
python3 scripts/agent-context.py scripts | grep -c landing.md
```

Also run before the commit:

```sh
python3 scripts/agent-context.py scripts | grep -c -x docs/contracts/landing.md
bash scripts/check-architecture-contract.sh
python3 scripts/version_tool.py check
python3 scripts/check-staged-units.py docs/inventories/2026-09-25-seal-at-landing/LND-1-C.numstat.tsv
```

Every sentence of the landing contract was checked against
`scripts/worktree.sh`, `scripts/landing.py`, `scripts/land-unit.py` and the
refusal in `scripts/production_artifact.py` as they are at the base.

## Result

- **Exit:** every command above exited 0. The documentation guard printed its
  known `SID-G7-D` errata lines and `Documentation contract: OK`; the language
  guard printed `Language contract: OK (148 legacy file(s) ratcheted)`; the
  architecture guard printed `Architecture contract: OK`; the version tool
  printed `version-contract: OK (8 owners)`.
- **Observed:** `agent-context.py scripts | grep -c landing.md` printed `2`,
  because the active plan's basename, `2026-09-25-seal-at-landing.md`, also
  ends in `landing.md`; the exact-line count printed `1`, so the registered
  contract is printed once.
- Where the design differs from the implementation, the contract describes the
  implementation: the lockfile rule stops only when a package `main` already
  locks changes version or checksum; the landing worktree's temporary commits
  are made with `git commit-tree` and only the sealed commit passes the hooks;
  `unbump` squashes the branch and restores only the version assignments and
  the history file; a `suite` unit without production inputs records
  `none; a suite unit has no production owner` as its build.

## Limits

- This unit was sealed on the branch with `landing.py`'s functions, not landed
  by `land-unit.py`; the author's first landing through the tool is the first
  real proof of the tool on this repository.
- `docs/projects.toml` is a verification input of every registered project, so
  existing manifests need re-verification before their next deployment.
  Nothing was built or deployed; this unit changes no deployable app.

## Follow-up

The contract describes three behaviours of the current tools that the author
may want to change in a later unit: `scripts/worktree.sh close` refuses for a
branch the tool has landed, because the landed commit is new; a project that is
not deployable and whose production inputs changed stops at `build_if_stale`
until its build entry has run, because its verify entry does not build; and a
`suite` unit whose changes touch only verification inputs runs no check.
Closing the `LND-1` checkpoint is a later administrative unit.
