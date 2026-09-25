# Evidence: the landing tool

- **Date:** 2026-09-25
- **Scope:** `LND-1-B` of the
  [seal-at-landing plan](../plans/active/2026-09-25-seal-at-landing.md):
  `scripts/landing.py`, `scripts/land-unit.py`, `scripts/test-land-unit.py`,
  `scripts/test-land-unit.sh`, the `Commit scope` step of
  `.github/workflows/contracts.yml`, and
  `production_artifact.production_input_patterns`, extracted from
  `production_fingerprint` so that the landing reads the same input list
- **Environment:** repository checkout on Linux; Git 2.43.0, Python 3.11
  standard library, `/bin/sh` is dash; fixture repositories with a bare
  `origin`, fake guards, fixture `pre-commit` and `commit-msg` hooks through
  `core.hooksPath`, a fake `complete-production.sh` over the real artifact
  runner, a fake `cargo`, a `git` wrapper that interrupts the seal, takes
  `origin` away at the push or reports a push that landed as failed, and a
  `pre-receive` hook
- **Artifact:** not applicable

## Procedure

```sh
bash scripts/test-land-unit.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
python3 scripts/version_tool.py check
bash scripts/test-staged-units.sh
bash scripts/test-production-artifacts.sh
bash scripts/test-worktree.sh
python3 scripts/check-staged-units.py docs/inventories/2026-09-25-seal-at-landing/LND-1-B.numstat.tsv
```

Test-first order: `python3 scripts/test-land-unit.py LandingFunctions -v` ran
before `scripts/landing.py` existed, and
`LandUnitFixture.test_fast_forward_landing` ran before `scripts/land-unit.py`
existed.

## Result

- **Exit:** every command above exited 0; the architecture guard printed
  `Architecture contract: OK`, the language guard `Language contract: OK`, the
  documentation guard `Documentation contract: OK` after its known `SID-G7-D`
  errata lines, and `version-contract: OK (8 owners)`.
- **Before the change:** the unit tests stopped at
  `ModuleNotFoundError: No module named 'landing'`, and the smoke test failed
  with `FileNotFoundError` for `scripts/land-unit.py` while the fixture copied
  it.
- **After the change:** `scripts/test-land-unit.sh` ran 27 tests, all passing.
  Ten `LandingFunctions` tests cover unit discovery, the ledger merge by
  `Unit`, the ratchet merge by key in both ratchet layouts, the lockfile
  decision, numstat rows and the rendered inventory, the diffstat, the ledger
  closure, the `Landing` section, the state file and the version unbump, each
  on literal inputs. Seventeen `LandUnitFixture` cases run `land-unit.py`
  against a fixture repository; the fake `complete-production.sh` ran:
  - once in the fast-forward landing (inputs changed), which landed one
    `app-bug` commit whose parent is the inventory's `Base revision`, with the
    ledger row closed, the diffstat equal to `git show --numstat`, the
    `Landing` section, `0.1.1`, the history row, the five fake guards in
    order, and exactly one `pre-commit` and one `commit-msg` run: the
    temporary commits of the landing worktree and the rebase run no hook;
  - never when `main` advanced outside the product's inputs and the artifact
    was already current (`maintenance`, `check` exit 0, `artifact current; no
    build`);
  - once when `main` landed the same product first: the unit landed as
    `0.1.2`, both ledger rows survived and the ratchet row took `8` of `9`
    and `8`;
  - once when the session had bumped: the bump was stripped with the warning
    `the session bumped`, and the landed version was the one computed at
    landing, with one history row;
  - never when the plan was archived on `main`: the landing stopped at the
    rebase naming the plan;
  - once for a `STATUS.md` conflict: the landing stopped naming the file with
    `main` unchanged, and `--continue` after a manual resolution in the
    landing worktree landed on the unchanged base;
  - never when merging `app/Cargo.lock` needed the network: the fake `cargo
    metadata --offline` failed and the landing stopped naming the lockfile;
  - once for two pushes when the first push was rejected because `main`
    moved: the retry reused the artifact and landed on the moved `main`;
    once for four pushes when every push lost the race, after which the
    landing stopped with `main` back on `origin/main`;
  - never in the six preflight stops: a row already sealed, a branch editing
    a tracked inventory, a missing evidence link, evidence outside the
    owner's `docs/evidence/`, a dirty canonical checkout, and a canonical
    checkout on another branch; none created a landing worktree;
  - once before an interruption: Ctrl-C during the fake build left the
    canonical checkout detached, the state file present and `origin/main`
    unchanged, and `--abort` restored `main` and removed the landing
    worktree;
  - once before a guard failure: the landing stopped naming
    `check-language-contract.py`, with no commit and the sealed change still
    staged;
  - never for a `suite` unit that changes no registered production input,
    whose `suite-bug` request is also refused at preflight;
  - once for a `suite` unit that edits `app/src/main.rs`: the product whose
    `production_input_patterns` hold the path got the check, which failed,
    and the build;
  - once before a Ctrl-C between writing the inventory and staging it:
    `--abort` removed the untracked inventory that the state file names and
    left the canonical checkout clean on `main`;
  - once for a push while `origin` is unreachable: the landing stopped at
    `push` with local `main` restored to its previous commit and the state
    at `commit_and_push`; `--abort` left `main` there and the checkout
    clean, and after `origin` returned, `--continue` pushed the commit sealed
    before the outage, one landed commit through the hooks once;
  - once for a push that Git reported as failed although `origin/main`
    already was the sealed commit: the landing reported it landed and made
    no second commit;
  - once before a failing `pre-commit` hook: the landing stopped at `commit`
    with no commit in the canonical checkout, `main` and the branch
    unchanged, and `origin/main` unchanged.
- `scripts/test-staged-units.sh`, `scripts/test-production-artifacts.sh` and
  `scripts/test-worktree.sh` still pass; the extraction in
  `scripts/production_artifact.py` leaves every fingerprint byte-identical.
- **Design points settled here:** the landing worktree holds temporary
  commits only (one squashed commit of the branch, then the bump), recorded
  with `git commit-tree` because the repository hooks reject `fixup!`
  subjects and version deltas that are never published; the one published
  commit is made in the canonical checkout through the hooks. The unbump
  returns only the registered version assignments and the history file to
  `main`'s, so dependencies a session added to the same manifest stay. A
  merged lockfile stops the landing when `cargo metadata --offline` fails or
  when a package `main` already locks changed version or checksum; added
  packages are allowed. The fixture is built in Python rather than shell, as
  the
  [implementation plan](../superpowers/plans/2026-09-25-parallel-unit-landing.md)
  records.

## Limits

- No real `cargo` ran: the fake records `metadata --offline` and either fails
  or leaves `main`'s lockfile, so a real lockfile regeneration is unproven.
- The fixture's hooks and guards are doubles. They prove the sealed commit
  goes through `core.hooksPath` exactly once and that a hook failure stops the
  landing; that the real hooks accept it rests on running their commands
  first, and the first real landing by the tool is the author's.
- `scripts/production_artifact.py` is a verification input of every
  registered project, so existing manifests need re-verification before their
  next deployment. Nothing was built or deployed; this unit changes no
  deployable app.

## Follow-up

`LND-1-C` records the decision and moves closure to the landing.
