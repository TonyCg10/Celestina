# Evidence: the landing after the whole-branch review

- **Date:** 2026-09-26
- **Scope:** `LND-1-D` of the
  [seal-at-landing plan](../plans/archive/2026-09-25-seal-at-landing.md):
  `scripts/landing.py`, `scripts/land-unit.py`, `scripts/test-land-unit.py`,
  `scripts/worktree.sh`, `scripts/test-worktree.sh`, one comment in
  `scripts/production_artifact.py`, the
  [landing contract](../contracts/landing.md), `AGENTS.md`,
  `CONTRIBUTING.md`, the verification standard, the version contract, and
  section 10 of the
  [design](../superpowers/specs/2026-09-25-parallel-unit-landing-design.md)
- **Environment:** repository checkout on Linux; Git 2.43.0, Python 3.11
  standard library, `/bin/sh` is dash; the fixture repositories of
  `scripts/test-land-unit.py` gain a second, non-deployable project `lib`
  whose `lib/src` is a production input of `app`, with build and verify
  entries in the real entries' shape over the real artifact runner, and fake
  guards that can fail only once the seal staged the unit
- **Artifact:** not applicable

## Procedure

```sh
bash scripts/test-land-unit.sh
sh scripts/test-worktree.sh
bash scripts/test-production-artifacts.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
python3 scripts/version_tool.py check
python3 scripts/check-staged-units.py docs/inventories/2026-09-25-seal-at-landing/LND-1-D.numstat.tsv
git diff --cached --name-only | .githooks/commit-msg --check 'suite-maintenance: Fix the landing before its first use'
```

Test-first order: every new or changed test ran and failed before its fix.

## Result

- **Exit:** every command above exited 0; the architecture guard printed
  `Architecture contract: OK`, the language guard `Language contract: OK`, the
  documentation guard `Documentation contract: OK` after its known `SID-G7-D`
  errata lines, and `version-contract: OK (8 owners)`.
- **Before the change:** `scripts/test-land-unit.sh` ran 34 tests with 6
  failures and 6 errors. The fixture reproduced each finding: a library unit
  stopped at `build_if_stale` because `verify-production.sh` refused the
  missing build; a branch that rewrote the plan's prose landed with that
  prose silently replaced by `main`'s; a second landing of a landed unit
  reached the rebase and stopped on an evidence conflict; a failing language
  guard ran only after the fake build. The state file rejected an emoji
  (`Escaped character is not a Unicode scalar value`). `test-worktree.sh`
  failed with `unit/app/APP-1 has commits that are not on origin/main` after
  a fixture landing.
- **After the change:** `scripts/test-land-unit.sh` ran 34 tests, all
  passing: 13 `LandingFunctions` tests and 21 `LandUnitFixture` cases.
  `scripts/test-worktree.sh` passed its seven cases, and
  `scripts/test-production-artifacts.sh` its 31 tests and the
  `production-common` fixtures.
  - The plan merge compares the branch's plan with the fork point's, both
    without the unit's row. The literal tests accept the unit's own row
    (changed or added) and stop at `rebase`, naming the plan, for a prose
    edit, an extra row and an edit to another row; the fixture stops with
    `<plan>: the branch changed the plan beyond its own row FX-A` and builds
    nothing.
  - `affected_projects` puts the owner first, then every other project whose
    production inputs hold a changed path. A `lib` unit ran lib's fake build
    then verify and app's fake `complete-production.sh`, each once, and its
    `Landing` section lists both checks and both builds; an `app` unit ran no
    `lib` entry. The suite cases keep their results.
  - `worktree.sh close` accepts a clean worktree whose branch commits are not
    on `origin/main` once `origin/main` tracks
    `app/docs/inventories/x/APP-1.numstat.tsv`, and still refuses before that.
  - The new step `pre_guards` runs `commit_scope.py --check`,
    `version_tool.py check`, the language guard and the architecture guard on
    the rebased tip: a failing language guard stopped at `guard` with no build
    and the state at `pre_guards`, and `--continue` then landed with one
    build. The fast-forward landing records the three early fake guards, then
    the five of the full chain; the existing guard failure after the seal
    still stops with the unit staged.
  - A branch whose inventory is already on `origin/main` stops at preflight
    with `FX-A already landed: origin/main tracks <inventory>`.
  - The state file writes non-ASCII text literally and escapes DEL, so a
    summary with an emoji, DEL and an accented letter round-trips.
- The documents now say that the landing, not the session, bumps, builds,
  verifies and completes, and that `check` is called by the deploy helper and
  the landing, not by the guards.

## Limits

- No real `cargo` ran, and the fixture's hooks and guards are doubles, as in
  `LND-1-B`; that the real hooks accept a sealed commit rests on running their
  commands first, and the author's first real landing is the real test.
- `scripts/production_artifact.py` is a verification input of every
  registered project, so its corrected comment again requires existing
  manifests to be re-verified before their next deployment. Nothing was built
  or deployed; this unit changes no deployable app.
- The plan merge compares whole lines, so a branch that only reformats its
  plan also stops; the author then resolves the plan by hand.

## Follow-up

None in this unit; the review's remaining findings stay deferred.
