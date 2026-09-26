# Evidence: the findings parked by the landing reviews

- **Date:** 2026-09-26
- **Scope:** `LND-1-E` of the
  [seal-at-landing plan](../plans/active/2026-09-25-seal-at-landing.md):
  `scripts/landing.py`, `scripts/land-unit.py`, `scripts/test-land-unit.py`,
  `scripts/worktree.sh`, `scripts/test-worktree.sh`,
  `scripts/production_artifact.py`, `scripts/test-production-artifacts.py`,
  `scripts/qmllint-cxxqt.sh`, the new `scripts/test-qmllint-target.sh` and
  its CI step in `.github/workflows/contracts.yml`, the
  [landing contract](../contracts/landing.md), `CONTRIBUTING.md` and one
  sentence of the
  [design](../superpowers/specs/2026-09-25-parallel-unit-landing-design.md)
- **Environment:** repository checkout on Linux; Git 2.43.0, Python 3.11,
  `/bin/sh` is dash, Cargo from rustup for the manual target-directory check;
  the fixture repositories of `scripts/test-land-unit.py` now give `app` a
  verify entry in the real entries' shape and deploy and status entries that
  record their runs, a fake `cargo` that fails on a manifest with conflict
  markers and can write a given lockfile, a Git wrapper that interrupts the
  checkout after the fast-forward or commits on local `main` before the push,
  and a pre-receive double whose racer can land the unit's inventory
- **Artifact:** not applicable

## Procedure

```sh
bash scripts/test-land-unit.sh
sh scripts/test-worktree.sh
sh scripts/test-qmllint-target.sh
bash scripts/test-production-artifacts.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
python3 scripts/version_tool.py check
python3 scripts/check-staged-units.py docs/inventories/2026-09-25-seal-at-landing/LND-1-E.numstat.tsv
git diff --cached --name-only | .githooks/commit-msg --check 'suite-maintenance: Fix the findings parked by the landing reviews'
for app in celestina siderita magnetita grafita fluorita hematita; do
    sh scripts/qmllint-cxxqt.sh --print-target-directory "$app"
done
```

Test-first order: every new or changed test ran and failed before its fix.

## Result

- **Exit:** every command above exited 0; the architecture guard printed
  `Architecture contract: OK`, the language guard `Language contract: OK`, the
  documentation guard `Documentation contract: OK` after its known `SID-G7-D`
  errata lines, and `version-contract: OK (8 owners)`. The loop printed
  `<checkout>/<app>/target` for each of the six applications, which is the
  directory the script read before.
- **Before the change:** `scripts/test-land-unit.py` ran 55 tests with 25
  failures and 18 errors, counting subtests; `test-worktree.sh` failed with
  `close app APP-1 with another owner's inventory: exit 0 instead of 1`;
  `test-qmllint-target.sh` failed with
  `the reported target directory was ignored`. Among the fixture failures: a
  retry after a racer landed the unit's inventory pushed four times; an
  interrupt after the fast-forward left `main` on the unpushed seal; a
  rejected push discarded a commit made on local `main`; a suite unit that
  renamed `lib/src` and registered the new name failed at `build_if_stale`;
  a unit that changed only `app/tests` ran `complete-production.sh`; the
  guards after the seal received `"app/docs/caf\303\251.md"` quoted; a `lib`
  unit with `--kind bug` failed inside `version_tool.py bump`; a conflicted
  `Cargo.toml` next to a conflicted lockfile stopped with
  `cargo metadata --offline failed, so merging the lockfile needs the network`.
- **Review round:** the new `affected_projects` cases for
  `scripts/production-common.sh`, a project's deploy entry and
  `scripts/complete-production.py` failed with `[] != ['app', 'lib']`;
  `test_verification_inputs_are_the_fingerprinted_set` failed because
  `verification_input_patterns` did not exist; the extended
  `test_rejected_push_keeps_local_main_commits` failed because the stop did
  not name the sealed commit; `test-qmllint-target.sh` failed with
  `cargo ran as '<app>|metadata --format-version 1 --no-deps'`. The new
  `test_library_verification_change_verifies_only` passed at once: it covers
  the existing verification-only path of a project that is not deployable.
- **After the change:** `scripts/test-land-unit.sh` ran 56 tests, all
  passing: 20 `LandingFunctions`, 1 `LandUnitFiles` and 35 `LandUnitFixture`.
  `test-worktree.sh` passed its eight cases, `test-qmllint-target.sh` its
  three, and `test-production-artifacts.sh` its 32 tests and the
  `production-common` fixtures. The verification fingerprints of all nine
  registered projects, computed on this checkout by the `eced4f9` module and
  by the new one, are identical.
  - **A1.** `close` counts only `<root>/<plan-slug>/<unit>.numstat.tsv` under
    the owner's inventory root (`docs/inventories` for `suite`,
    `<path>/docs/inventories` for a project): tracked
    `other/docs/inventories/x/APP-1.numstat.tsv` and
    `docs/inventories/x/APP-1.numstat.tsv` leave `close app APP-1` refused;
    `app/docs/inventories/x/APP-1.numstat.tsv` lets it close.
  - **A2.** `rebase` repeats the landed-unit refusal on its new base: the
    racer case stops at `rebase` with `FX-A already landed`, one push, one
    build, `main` fast-forwarded to the racer and clean.
  - **A3.** The `try` that restores `main` now starts at the fast-forward: an
    interrupt on the checkout of `main` after `update-ref` leaves `main` at
    its previous commit, and `--continue` lands the same sealed commit with
    one run of the hooks.
  - **A4.** After a rejected push, `main` moves by `merge --ff-only`; with a
    local commit on `main` and a moved `origin/main`, the landing stops at
    `push` with `local main cannot fast-forward`, names the unpushed sealed
    commit under the local commit and the recovery, and the local commit
    stays. A `--continue` without that recovery stops at `unbump` with
    `local main has commits that are not on origin/main`, with no second
    build.
  - **A5.** `build_if_stale` reads the tip's registry; the rename case lands
    and builds `app` and `lib`, and a registered input the tip lacks stops at
    `build_if_stale` naming `app` and `` `app/missing` ``.
  - **A6.** Changed paths also match the raw input patterns with `fnmatch`,
    including a directory's subtree: a deleted `lib/src/gone.rs` under
    `lib/src/*.rs` marks `lib`.
  - **A7.** A check that fails only with the two verification errors, whose
    strings `production_artifact.py` now names once, runs `verify`, `deploy`
    and `status` of `app` and no build, and the `Build` line starts
    `app verify:`; `lib`, which is not deployable, runs its verify entry
    alone. `production_artifact.py` now owns the verification input set in
    `verification_input_patterns`, which both its fingerprint and
    `affected_projects` use, so a change to any of those inputs, a shared
    script such as `scripts/production-common.sh` included, marks the
    project; a shared script marks every project.
  - **A8.** File reads, writes, removals and directory creation of the
    landing, and `final_digest`, turn an `OSError` into one `land-unit:` line
    naming the path; the unit test uses a path under a regular file.
  - **A9.** The staged names are read with `-z`: both `commit_scope.py` runs
    received the non-ASCII path unquoted between NULs.
  - **A10.** `--kind bug` for `lib` (`versioned = false`) stops at preflight
    with `lib is not versioned`, before any worktree exists.
  - **B1.** Lockfiles are resolved after every other conflicted path; a
    conflicted `Cargo.toml` of the same workspace stops naming it, with no
    cargo call, and `--continue` after resolving it merges the lockfile and
    lands. A lockfile that only gains the branch's package lands; one whose
    `otherdep` moved stops naming it. Both lockfile stops say to `git add` the
    lockfile in the landing worktree before `--continue`.
  - **B2.** The ratchet merge moves the comment line above a row with that
    row, on the real baseline's shape.
  - **B3.** The plan stop names the first differing line on each side, for
    example `first at line 13 on the branch and line 13 at the fork point`.
  - **C3, C5.** The suite `Build` line reads
    `none; no registered production input changed`; a run records
    `<project> build:` or `<project> verify:` and the manifest's
    `source_fingerprint` and `verification_fingerprint`.
  - **C4.** A guard stop after the seal and a hook stop add the sealed
    remedy (`--abort`, fix the branch, land again) instead of the generic
    `--continue` advice, also on a second `--continue`.
  - **D1, D2.** `inventory_path`, `owner_docs_root`, `subject` and the
    diffstat fixed point `sealed_diffstat` live in `landing.py` with a unit
    test; `discover_unit` lost its unused `root`. Every Git call of
    `numstat_rows`, `cat-file -e` included, goes through one helper with no
    terminal input and captured stderr.
  - **D3.** New cases: an evidence record missing on the branch stops at
    preflight; `--abort` during a rebase stop removes the landing worktree
    and leaves `main` and the branch as they were; numstat rows of a
    mode-only change (`0/0`) and of symlinks (hash of the link target).
  - **D4.** `qmllint-cxxqt.sh` reads the release module under the target
    directory `cargo metadata --no-deps --format-version 1 --offline`
    reports from the application root, else `<app>/target`; the fixture
    finds a module under a redirected directory, and a real Cargo project
    whose `.cargo/config.toml` sets `target-dir` resolved to that directory.
- A unit that deletes a file could not be sealed: `git add --all -- <paths>`
  refused the deleted path, which the index already stages. The seal now adds
  only the paths that exist; the rename case proves it.
- The expectations of four existing fixture tests changed where the brief
  changes behaviour: three `Build` lines now carry the project label and the
  fingerprints instead of `git_revision`, and the suite unit's `Build` line
  reads `none; no registered production input changed`. The unit test of
  `discover_unit` calls it without `root`. Every other existing assertion is
  unchanged.

## Limits

- No real `cargo` ran in the fixture, and its hooks, guards and production
  entries are doubles, as in `LND-1-B`; the author's first real landing is
  the real test.
- `scripts/production_artifact.py` and `scripts/qmllint-cxxqt.sh` are
  verification inputs of every registered project, so existing manifests
  must be re-verified before their next deployment. Nothing was built or
  deployed; this unit changes no deployable app.
- A change to a shared verification input marks every registered project,
  so its landing runs every project's `check` and each stale verification;
  a project with no current manifest is built in full.
- The `fnmatch` match lets `*` cross `/`, so a pattern can mark a project
  affected more often than its glob expansion would; the extra project then
  only runs its `check`.
- The plan merge still compares whole lines, so a branch that only
  reformats its plan stops; the stop now names the first differing lines.

## Follow-up

None.
