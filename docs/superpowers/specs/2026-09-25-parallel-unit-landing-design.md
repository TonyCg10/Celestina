# Parallel unit landing — design

- **Date:** 2026-09-25
- **Status:** approved by the author in brainstorming; awaiting spec review
- **Scope:** suite — how a unit prepared in one session lands on `main`
  while other sessions keep working
- **Owners:** `suite:` (scripts, governance documents, one decision record)
- **Versions at closure:** none; this is `suite-maintenance` and ships no
  product behaviour

## 1. Goal and scope

The author runs several agent sessions at once on one local clone. Today they
step on each other, and a unit that closes second has to wait for the first
one to publish and then redo its closure by hand. The reason is mechanical and
lives in the guards, not in the documents:

- The inventory seals `Base revision = HEAD` and `check-staged-units.py`
  refuses any other value. Afterwards `documentation_contract.py` requires
  that base to be the direct parent of the unit's commit, forever. A rebase
  changes the parent, so a sealed unit cannot be rebased.
- A merge would keep the parent, but `commit_scope.py` rejects a merge that
  carries an inventory or changes a debt ratchet, and the published-history
  replays in `contracts.yml` audit only non-merge commits. Landing is
  therefore fast-forward only.
- A product's SemVer bump and history row are chosen at closure. Two units of
  the same product both claim the next number.
- Nearly every unit touches the same hot files: `docs/version-history.tsv`,
  the product's `Cargo.toml` and `Cargo.lock`, `celestina-rs/Cargo.lock`, the
  three `scripts/*-baseline.tsv` ratchets, and the owner's `STATUS.md`,
  `ROADMAP.md`, plan ledger and plan index.
- One clone means one Git index, which is what the hooks and
  `check-staged-units.py` read, one `target/` per product, one
  `celestina-style/build/` and one deploy prefix.
- No script writes an inventory. Each `.numstat.tsv` is produced by hand from
  `git diff --numstat --no-renames` and `sha256sum`, which is why resealing
  hurts.
- Every landed unit runs `complete-production.sh` at closure; a unit that
  must be resealed builds twice. The author builds on a personal machine and
  wants exactly the builds the contract needs.

In scope:

1. one worktree per session, outside the repository, with a shared Cargo
   cache for the fast checks a session runs, and a refusal to run production
   entries there;
2. `scripts/land-unit.py`, which lands one unit on `main` from the canonical
   checkout: rebase, semantic merge of the hot files, version bump against
   the current `main`, the one production run the artifact contract needs,
   inventory and ledger closure, the full guard chain, one typed commit,
   push, and automatic retry when `main` moved;
3. tests for both scripts with fixture repositories and fake production
   entries;
4. a decision record and the document changes that move closure from "compute
   the inventory by hand" to "request the landing".

Out of scope:

- any change to a hook, a guard, the inventory format, the version contract
  or the one-active-checkpoint rule; the design keeps linear history, base =
  direct parent, immutable inventories and one commit per unit exactly as
  they are;
- landing by merge; it opens the audit gap `.github/workflows/README.md`
  documents and does not solve same-product versions;
- reserving version numbers when a unit opens;
- more than one active checkpoint per project;
- a shared cache for CMake builds (the shell and `celestina-style`); those
  build only at landing, in the canonical checkout, as today;
- merging prose conflicts; a `STATUS.md`, `ROADMAP.md`, `VALIDATION.md` or
  plan index conflict stops the landing and is resolved by the author.

## 2. Constraints the design honours

- **Linear published history.** `audit-version-commits.py` replays each
  non-merge commit against its first parent; `test-commit-scope.sh` replays
  every non-merge commit. The landing produces ordinary commits on `main`,
  never a merge.
- **Base = direct parent.** The inventory is written on top of the `main`
  the commit will have as parent. It is generated, so it is regenerated on
  every retry and never rebased.
- **Immutable inventories.** The tool writes an inventory once, in the
  commit that lands it. It never edits a tracked one.
- **One atomic batch per plan.** A branch carries one unit, so the batch is
  that unit. Two units of one plan on two branches land as two commits, in
  arrival order.
- **Versioned deliveries carry their build.** `bug`, `milestone` and
  `release` change a registered version before the canonical build and the
  deployed bytes contain it. The tool bumps first and builds after, on the
  same tree it commits.
- **Build once, verify the same bytes, deploy without compiling.**
  `production_artifact.py` decides currency from the registered
  `production_inputs`; the tool asks it instead of deciding on its own.
- **Squash before publishing.** The version contract allows temporary
  commits on a branch as long as they are squashed before delivery. Every
  branch commit before the landing is such a temporary commit.
- **English development truth; Spanish dialogue.** Scripts, diagnostics and
  documents are English.

## 3. Decisions taken in brainstorming

- Sessions share one clone today; the author also works on the same product
  from two sessions, not only on different products.
- Rebuilding at landing is accepted, provided a unit builds once and a unit
  whose product inputs did not change does not build at all.
- Approach A, "seal at landing", was chosen over "land by merge" and over
  "worktrees plus a manual recipe".
- Worktrees live outside the repository, because `documentation_contract.py`
  walks the tree with `os.walk` and does not read `.gitignore`.
- The five hot-file kinds of §5 are merged semantically; everything else in
  conflict stops the landing.

## 4. `scripts/worktree.sh`

A POSIX shell entry with two commands, run from any checkout of the
repository.

`worktree.sh open PROJECT UNIT` creates the branch `unit/<project>/<unit>`
from `origin/main` after `git fetch origin main`, adds the worktree at
`<repository parent>/<repository basename>.worktrees/<project>-<unit>/`, and
writes two untracked files inside it:

- `.cargo/config.toml` with `build.target-dir` set to the shared directory
  `<repository parent>/<repository basename>.worktrees/.cargo-target/`, so
  `cargo test` and `cargo clippy` from every session reuse one compiled set of
  crates. Cargo serialises concurrent access to that directory with its own
  lock and fingerprints every crate by content, so two sessions on different
  branches cost one compilation per changed crate, never two full builds.
- `.celestina-worktree`, a marker naming the unit and the canonical checkout.

`PROJECT` is a registered project id or `suite`, and `UNIT` is the ledger's
unit id, for example `worktree.sh open siderita SID-U2-A`. An unregistered
project is refused. `core.hooksPath` is repository configuration, so the
hooks already run in every worktree.

`worktree.sh close PROJECT UNIT` refuses while the branch has commits that
are not on `origin/main`, then removes the worktree and deletes the branch.

`production_artifact.py` refuses `run-build`, `run-verification` and `status`
when `.celestina-worktree` exists at the repository root, with the message
`production-artifact: this is a session worktree; production runs happen at
landing (scripts/land-unit.py)`. `check` keeps working because
`production-common.sh`'s deploy helper and `land-unit.py` call it. The
refusal is a registered rule change of the artifact tool and has its own test.

What a session does in its worktree: the whole unit — code, tests,
documents, the dated evidence record under the owner's `docs/evidence/`, and
the ledger row with its intent — plus the fast checks (`cargo test`,
`cargo clippy`, `qmllint-cxxqt.sh`, the guard chain). What it does not do:
build production, deploy, bump a version, or write an inventory. Those are
landing steps.

## 5. `scripts/land-unit.py`

Invoked by the author, from the canonical checkout, when they request the
commit:

```sh
python3 scripts/land-unit.py unit/siderita/SID-U2-A --kind bug
python3 scripts/land-unit.py --continue
```

`--kind` is one of `bug`, `milestone`, `release`, `maintenance` and becomes
the subject suffix. The subject's imperative text is taken from the ledger
row's `Intended change` unless `--summary` overrides it; the version tool
requires the two to be equal, and the landing enforces that.

### 5.1 Steps

Each step stops cleanly on failure (§6).

1. **Preconditions.** The canonical checkout is on `main` and clean;
   `git fetch origin main`; the branch exists; the branch's diff against
   `origin/main` touches exactly one active plan whose ledger has exactly
   one row that is `active`, or `done` without an inventory link, and whose
   prefix is registered; that row's project owns every changed path under
   the prefix's `commit_roots` plus the registered workspace manifests, or
   the prefix is `suite`.
2. **Unbump.** If the branch changed a registered version source, a mirror
   or `docs/version-history.tsv`, those changes are reverted on a landing
   worktree of the branch, so the branch carries no version diff. The
   reverted row is remembered only to warn the author that the session bumped
   where it should not have.
3. **Rebase.** The landing worktree, at
   `<repository parent>/<repository basename>.worktrees/.landing/`, rebases
   the branch onto `origin/main` with `git rebase --no-autosquash`. When a
   conflict is only in hot files, §5.2 resolves it and the rebase continues.
   Any other conflict stops the landing.
4. **Version.** For `bug`, `milestone` and `release`, run
   `version_tool.py bump <owner> <kind> --unit <unit> --summary <text>` on
   the rebased tree. For `maintenance`, nothing. `suite-<kind>` with a
   product bump is refused: a suite unit that bumps products is a decision
   the author records by hand, as the version contract says.
5. **Land on the canonical checkout.** The canonical checkout checks out the
   rebased tip detached. Its `target/` and `build/` directories are the
   production caches, so the build below reuses them.
6. **The one build the contract needs.** For the owning project, if it is
   deployable: `production_artifact.py check <project> --require-verified`.
   When that passes, the artifact is current for these exact inputs and
   nothing is built. When it fails, `complete-production.sh` runs: build,
   verify, deploy, status. For a project that is not deployable, such as
   `celestina-style` or `celestina-rs`, only `verify-production.sh` runs, and
   only when `check` fails. A `suite` unit that changes no registered
   production input runs nothing. A unit whose product inputs moved because `main` advanced
   builds once, here, as the artifact contract already requires.
7. **Seal.** Generate the inventory at
   `<owner>/docs/inventories/<plan-slug>/<unit>.numstat.tsv`, or under root
   `docs/` for suite work: `Base revision` = `origin/main`, one `Pathspec`
   per changed path as the current units do, `Calculation` and `Hashes`
   lines, `git diff --numstat --no-renames` rows against the base, `/dev/null`
   numstat for new paths, `-/-` for binary, `0/0` for mode-only, `deleted`
   for removed files, final SHA-256 for the rest, `self` for the inventory
   row. Close the ledger row: status `done`, `Files / areas` as the relative
   inventory link, `Diffstat` as exact `N files, +X/-Y`, `Automated evidence`
   as the link to the evidence record, which must exist. Append a `## Landing`
   section to the evidence record stating the base revision, the `check`
   result, and either the `complete-production.sh` run with its exit and the
   manifest's `git_revision`, or `artifact current; no build`. The inventory
   is computed after that append so it hashes the final evidence bytes.
8. **Guards.** With the whole change staged: `check-staged-units.py`,
   `commit_scope.py --check` with the final subject, `version_tool.py check`,
   `check-architecture-contract.sh`, `check-language-contract.py`,
   `check-documentation-contract.sh`. These are the hooks' own commands, so a
   later hook failure is a bug in the tool, not a way around the hooks.
9. **Commit and push.** One commit `<prefix>-<kind>: <imperative>` through
   the normal hooks. Then `git checkout main`, `git merge --ff-only`, and
   `git push -u origin main`. A rejected push means `main` moved: the tool
   resets `main` to `origin/main`, discards the sealed commit, and returns to
   step 3 with the unsealed tip, at most three times.

The sealed commit is derived, never rebased. On every retry it is rebuilt
from the branch's unsealed tip, so the inventory and the version are always
computed against the parent the commit will have.

### 5.2 Semantic merge of hot files

Only these paths receive a resolution beyond Git's own; each is a pure
function of the two sides, tested in isolation:

- **The active plan ledger** (`<owner>/docs/plans/active/<plan>.md`). The
  `Change and commit ledger` table is merged by the `Unit` column: our row
  for our unit, `main`'s content for every other row and for the prose.
  When `main` no longer has the plan under `active/`, the landing stops:
  archiving while a unit is open is the author's decision.
- **`docs/version-history.tsv`, the product's registered version source and
  its mirrors.** Never in conflict after step 2, because the branch carries
  no version diff; step 4 writes them on the rebased tree.
- **`celestina-rs/Cargo.lock` and a product `Cargo.lock`.** Take `main`'s
  file, then `cargo metadata --offline --format-version 1` in that workspace,
  which rewrites the lockfile only if the branch added or changed a
  dependency. If cargo needs the network, or the resulting lockfile differs
  from `main`'s outside the packages the branch's manifests changed, the
  landing stops.
- **The three ratchets** (`scripts/architecture-baseline.tsv`,
  `scripts/language-baseline.tsv`, `scripts/qmllint-baseline.tsv`). Rows are
  merged by key. A key changed on both sides takes the lower value; a key
  removed on either side stays removed; comments and order follow `main`.
  Step 8 then proves every row equals the measurement, as it does today.
- **Inventories and evidence records.** Unique per unit; a conflict there is
  a bug and stops the landing.

Two branches that set different active checkpoints in the same `ROADMAP.md`
conflict in prose and stop. That is the contract, unchanged.

## 6. Stops, resumption and what the tool never does

A stop leaves the landing worktree with the rebase in progress, prints the
step, the file and the exact reason, and exits non-zero. After the author
resolves the conflict there, `land-unit.py --continue` resumes at the step
that stopped; the tool records the step, the branch, the kind and the summary
in `<repository parent>/<repository basename>.worktrees/.landing/.land-state.toml`
for that.

The tool never: force-pushes; rewrites `main` or the branch's unsealed
history; edits a tracked inventory; runs with a dirty canonical checkout;
lands paths of two prefixes in one commit; passes `--no-verify`; builds in a
session worktree; deploys when `complete-production.sh` did not verify. A
hook failure fails the landing.

The canonical checkout is detached on the rebased tip between step 5 and
step 9. Any stop leaves it there on purpose, with `main` untouched, so the
author can inspect the tree that was about to land; `--continue` and
`--abort` both return it to `main`.

## 7. Documents and the decision

- **ADR 0011, "Seal at landing".** Context: the closure work is a function of
  the parent commit, so it belongs to the moment the parent is known. Decision:
  inventories, ledger closure fields, version bumps and the canonical
  production run are produced by `land-unit.py` on top of the current
  `main`; sessions work in worktrees and never produce them. Consequences:
  parallel sessions land in arrival order without manual resealing, one
  build per landed unit and none when the product's inputs did not move,
  prose conflicts remain the author's. Extends ADR 0004 and ADR 0003; changes
  no guard.
- **`docs/contracts/landing.md`.** The canonical description of §4–§6:
  worktree layout, the refusal rule, the landing steps, the hot-file merge
  rules, the stop and resume behaviour. Registered in `suite.shared_rules` so
  `agent-context.py` prints it.
- **`AGENTS.md` and `CONTRIBUTING.md`.** "Git and commits" and "Closing a
  unit" say: sessions work in a worktree opened with `worktree.sh`; the ledger
  row is declared and kept current there; the evidence record is written
  there; when the author requests the commit, `land-unit.py` performs the
  closure. The inventory format stays documented as the artifact the tool
  produces. The production-artifact section keeps `complete-production.sh`
  as the exit condition and says the landing runs it.
- **`docs/governance/change-policy.md` and `docs/templates/plan.md`.** The
  "calculate tracked paths by hand" instructions become "the landing computes
  them"; the rules they state about the result do not change.
- **`docs/README.md`.** One row: "How does a finished unit reach `main`?" →
  the landing contract.

## 8. Tests and evidence

- **`scripts/test-land-unit.sh`.** Builds throwaway repositories the way
  `test-staged-units.sh` does, with a registry of one deployable project
  whose production entries are fake scripts that record their invocation,
  as `test-production-artifacts.py` already does, and a bare `origin`. Cases:
  1. branch on top of `main`: lands fast-forward, one commit, inventory base
     equals the parent, the ledger row is `done`, the evidence has a
     `Landing` section;
  2. `main` advanced with another product's unit: no conflict, `check`
     passes, the fake build is not invoked;
  3. `main` advanced with the same product: the version is bumped against
     `main`'s new number, the ledger rows of both units survive, the ratchet
     row both lowered takes the lower value, the fake build runs once;
  4. branch with its own bump: the bump is stripped, a warning is printed,
     the landed version is the one computed at landing;
  5. plan archived on `main`: stop, with the plan path in the message;
  6. `STATUS.md` conflict: stop, with the file in the message; `--continue`
     after a manual resolution lands;
  7. lockfile conflict that needs the network: stop;
  8. push rejected once: the sealed commit is discarded and the landing
     repeats from the rebase with a new base, then succeeds; rejected four
     times: stop;
  9. `production_artifact.py run-build` inside a session worktree: refusal
     with the registered message; `check` still works;
  10. every stop leaves `main` unchanged and the landing state file present;
      `--abort` restores the canonical checkout to `main`.
- **`scripts/test-worktree.sh`.** `open` creates the branch, the worktree,
  the marker and the Cargo config; a second `open` of the same unit is
  refused; `close` refuses with unlanded commits and succeeds after they are
  on `origin/main`.
- **Unit tests for the merges** in `scripts/test-land-unit.py`: ledger table
  merge by `Unit`, ratchet merge by key, the lockfile decision, each on
  literal inputs.
- **CI.** Both shell tests join the `Commit scope` step of `contracts.yml`.
- **Guards** for the unit itself: architecture, documentation, language,
  version, `check-staged-units.py`; `agent-context.py` prints the new
  contract for every path.
- **Author validation.** None for the scripts. The first real landing of a
  product unit through the tool is recorded in that unit's evidence, not in a
  validation entry.

## 9. Delivery

One suite plan, `LND-1` "Seal at landing", which replaces `PRD-1` as the
root roadmap's active checkpoint. `PRD-1-A` was delivered in `36ed52a` but
its plan is still under `active/`, so the same paired transition `PRD-1`
used for `LNG-1` applies: an administrative `PRD-1-B` unit archives it with
its own inventory, in the same suite commit as the first `LND-1` unit. Then
three units in causal order, each `suite-maintenance`:

1. `LND-1-A`: `worktree.sh`, the `production_artifact.py` refusal and their
   tests. The worktree directory is outside the repository, so no ignore
   rule is needed.
2. `LND-1-B`: `land-unit.py` with the merge functions, `test-land-unit.sh`,
   `test-land-unit.py`, the CI step.
3. `LND-1-C`: ADR 0011, `docs/contracts/landing.md`, the registry row in
   `suite.shared_rules`, and the amendments to `AGENTS.md`,
   `CONTRIBUTING.md`, `change-policy.md`, `plan.md` and `docs/README.md`.

`LND-1-C` is the first unit landed by `land-unit.py` itself; its evidence
record carries that `Landing` section. No product version moves.

## 10. Amendments after implementation (2026-09-25)

The review of the whole implementation changed four rules above, and on
2026-09-26 `AUD-1-C` amended §5.1 and §5.2 for stacked branches. The
[landing contract](../../contracts/landing.md) states the amended behaviour.

- **§5.2, the active plan ledger.** Taking `main`'s content for every other
  row and for the prose discarded any other edit the branch made to the plan,
  and the inventory then sealed the wrong bytes. The ledger merge now applies
  only when the branch's plan equals the fork point's with the unit's own row
  masked on both sides; otherwise the landing stops at the rebase, naming the
  plan and saying that the branch changed it beyond its own row.
- **§5.1 step 6, the one build.** One rule now covers every unit kind. The
  affected projects are the owner first (a `suite` unit has none), then every
  other registered project whose production inputs the changed paths touch,
  in registry order. Each gets `check --require-verified`; when it fails, a
  deployable project runs `complete-production.sh`, and one that is not runs
  `build-production.sh`, then `verify-production.sh`. A verify entry alone
  refuses changed production inputs, and the artifact contract requires every
  affected deployable consumer of a library to be completed.
- **§4, `worktree.sh close`.** The sealed commit is a squash, so the session's
  commits never reach `origin/main`. `close` now also accepts a clean
  worktree whose branch has such commits when `origin/main` tracks an
  inventory named `<unit>.numstat.tsv` under an `inventories/` directory.
- **§5.1, guards before the build.** A new step `pre_guards` runs between
  steps 5 and 6: on the rebased tip, `commit_scope.py --check <subject>` with
  the changed paths on stdin, `version_tool.py check`, the language guard and
  the architecture guard, so that a failure stops the landing before anything
  is built or deployed. The full chain of step 8 still runs after the seal; a
  failure in either stops with the label `guard`.
- **§5.1 and §5.2, stacked branches (`AUD-1-C`, 2026-09-26).** A branch
  created from another unit's branch lands after that dependency landed.
  Rows `origin/main` closed are settled: discovery sets aside an open row
  that `main` closed, and a changed plan that holds only such rows, unless
  the row is the unit's own; the plan merge masks a row equal to `main`'s and
  an open row `main` closed with the session's cells unchanged; another
  active plan holding only settled rows takes `main`'s text; and the
  preflight scope check leaves out three kinds of path: those whose bytes
  equal `main`'s, the plans set aside as settled, and another unit's
  evidence record or inventory that the fork point lacks and `main` has. In
  an add/add conflict, another unit's inventory takes `main`'s copy, and
  another unit's evidence record takes it only when the branch's copy equals
  `main`'s without its landing section; otherwise the landing stops. A
  dependency that has not landed stops the preflight, which names it.
