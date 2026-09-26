# Landing contract

A unit is prepared in a session worktree and reaches `main` through one
landing. The session writes the change, its tests, its documents, its evidence
record and its open ledger row. The landing, run from the canonical checkout on
top of the current `origin/main`, produces everything that depends on the
parent commit: the version transition, the one production run the artifact
contract needs, the exact inventory, the ledger closure and the single commit.
The reason is recorded in [ADR 0011](../decisions/0011-seal-at-landing.md).

The landing produces the artifacts that
[ADR 0004](../decisions/0004-monorepo-change-ledger.md) and the
[change policy](../governance/change-policy.md) define, and it decides whether
to build with the manifest identity of
[ADR 0003](../decisions/0003-reusable-production-artifacts.md) and
[the production artifact contract](production-artifacts.md). Version
transitions follow [the version contract](versioning.md). No guard, hook,
inventory format or version rule is different for a landed unit.

## Session worktrees

Every path below is derived from the canonical checkout `<parent>/<name>/`:

```text
<parent>/<name>/                                 canonical checkout, on main
<parent>/<name>.worktrees/<project>-<unit>/      one session worktree per unit
<parent>/<name>.worktrees/.cargo-target/         Cargo target directory shared by sessions
<parent>/<name>.worktrees/.landing/              landing worktree and its state file
```

The worktrees live outside the repository, so the documentation guard, which
walks the repository tree, never sees them and no ignore rule is needed.

Open and close a session worktree from the canonical checkout:

```sh
scripts/worktree.sh open siderita SID-U2-A
scripts/worktree.sh close siderita SID-U2-A
```

`PROJECT` is a registered project id or `suite`, and `UNIT` is the ledger's
unit id. Both must start with a letter or digit and contain only letters,
digits, `.`, `_` and `-`. The entry refuses an unregistered project and refuses
to run inside a session worktree. It exits 1 on a refusal and 2 on a usage
error, with one line on stderr.

`open` runs `git fetch origin main`, creates the branch `unit/<project>/<unit>`
from `origin/main`, adds the worktree at
`<parent>/<name>.worktrees/<project>-<unit>/` and prints its path. It refuses
when that directory or that branch already exists. It writes two untracked
files at the worktree's root:

- `.cargo/config.toml`, whose `[build]` table sets `target-dir` to
  `<parent>/<name>.worktrees/.cargo-target`, so `cargo test` and
  `cargo clippy` from every session reuse one set of compiled crates. Cargo
  locks that directory and fingerprints crates by content.
- `.celestina-worktree`, the session marker, which records `project`, `unit`
  and `canonical` (the canonical checkout's path).

Both paths are added to `info/exclude` in the common Git directory, so no
session stages them. `core.hooksPath` is repository configuration, so the
hooks run in every worktree.

`close` runs `git fetch origin main` and refuses while the worktree holds any
change other than the two files `open` wrote, or while `unit/<project>/<unit>`
has a commit that is not reachable from `origin/main` and `origin/main` tracks
no inventory named `<unit>.numstat.tsv` under an `inventories/` directory.
Otherwise it removes those two files and the worktree, then deletes the
branch. The landing publishes one new sealed commit, so the session's own
commits never reach `origin/main`; after a landing, the unit's inventory on
`origin/main` is what lets `close` accept the branch.

While `.celestina-worktree` is a file at the repository root,
`scripts/production_artifact.py` refuses `run-build`, `run-verification` and
`status`, which the no-argument build, verify and status entries delegate to,
with exit 1 and:

```text
production-artifact: this is a session worktree; production runs happen at landing (scripts/land-unit.py)
```

`check` only reads, so it keeps working there; its callers are
`production-common.sh`'s deploy helper and `land-unit.py`. `land-unit.py`
refuses to run in a session worktree as well.

A session does, in its worktree: the whole unit (code, tests and documents);
the dated evidence record under the owner's `docs/evidence/`; the ledger row
with its intent; and the fast checks (`cargo test`, `cargo clippy`,
`qmllint-cxxqt.sh`, the guard chain). It commits there as often as it likes;
those commits are temporary and never published. A session does not build
production, deploy, bump a version, set its row to `done` with an inventory
link, or write an inventory. Those are landing steps.

For the landing to accept the branch, its diff against `origin/main` must
change exactly one active plan (a Markdown file other than `README.md` directly
in an owner's registered `active_plans` directory), and that plan must have
exactly one ledger row that is `active`, or `done` without an inventory link.
That row's `Commit prefix` must be the plan owner's registered prefix, its
`Intended change` is the subject's imperative text unless `--summary` replaces
it, and its `Automated evidence` must link an existing `.md` record under the
owner's `docs/evidence/` (root `docs/evidence/` for a suite plan).

## Landing

The author requests the landing; it commits and pushes `main`. From the
canonical checkout:

```sh
python3 scripts/land-unit.py unit/siderita/SID-U2-A --kind bug
python3 scripts/land-unit.py unit/siderita/SID-U2-A --kind bug --summary "Fix picker synchronization"
python3 scripts/land-unit.py --continue
python3 scripts/land-unit.py --abort
```

`--kind` is `bug`, `milestone`, `release` or `maintenance`. The subject is
`<prefix>-<kind>: <summary>`, where the summary is `--summary` or the row's
`Intended change`. For a versioned kind the same text is the version-history
summary, so the subject and the history row agree as the version contract
requires. `--continue` and `--abort` take no branch, `--kind` or `--summary`.
The tool prints `land-unit: <step>` as each step starts, and exits 0 when the
unit landed, 1 when the landing stopped or failed, and 2 on a usage error.

1. **`preflight`.** Stops when the landing worktree already exists, when the
   canonical checkout is not a clean `main` (the message lists the
   `git status --porcelain` lines), when local `main` has commits that are not
   on `origin/main` after `git fetch origin main`, when the branch does not
   exist or has no changes, when the branch does not satisfy the plan and row
   rules of the previous section, when the unit's inventory already exists on
   `origin/main` (the unit already landed), when a changed path lies outside
   the row prefix's registered commit scope, when the branch changes an
   inventory that exists on `origin/main`, or when a `suite` unit asks for `bug`,
   `milestone` or `release` (a suite unit that bumps products is recorded by
   hand, as the version contract says). The registry is read from
   `origin/main`. Then it adds the landing worktree, detached on the branch,
   and writes the state file.
2. **`unbump`.** In the landing worktree, the branch becomes one temporary
   commit on its fork point with `origin/main`, made with `git commit-tree`.
   When the branch changed a registered version assignment, a mirror or
   `docs/version-history.tsv`, only those version bytes return to the fork
   point's, and the tool warns:

   ```text
   land-unit: warning: the session bumped <paths>; the landing drops that change and computes the version itself
   ```

   Other edits in the same manifest, such as an added dependency, stay.
3. **`rebase`.** Stops when the plan existed at the fork point but is no longer
   on `origin/main`: archiving a plan while a unit is open is the author's
   decision. Otherwise it records `origin/main` as the base and runs
   `git rebase --no-autosquash <base>` in the landing worktree. Conflicts in hot
   files are resolved as the next section says; any other conflict stops.
4. **`bump_version`.** For `bug`, `milestone` and `release`, it runs the landing
   worktree's version tool on the rebased tree and records the result as
   another temporary `git commit-tree` commit; `maintenance` changes nothing:

   ```sh
   python3 scripts/version_tool.py --root <landing worktree> bump <project> <kind> --unit <unit> --summary <summary>
   ```
5. **`checkout_canonical`.** The canonical checkout checks out the rebased tip
   detached. Its `target/` and `build/` directories are the production
   caches, so a build below reuses them.
6. **`pre_guards`.** On the rebased tip, before anything is built or deployed,
   the guards whose verdict does not depend on the seal, in order:
   `commit_scope.py --check <subject>` with the paths the unit changes on
   stdin, `version_tool.py check`, `check-language-contract.py` and
   `check-architecture-contract.sh`. The first failure stops the landing with
   its output.
7. **`build_if_stale`.** The affected projects are the owning project first (a
   `suite` unit has none), then every other registered project whose
   production inputs hold a path the unit changed, in registry order, such as
   the deployable consumers of a library. For each, it runs
   `production_artifact.py check <project> --require-verified`. Exit 0 means
   the artifact is current and verified for these exact inputs, and nothing is
   built. Otherwise a deployable project runs its registered
   `complete_script`, and a project that is not deployable runs its
   `build_script`, then its `verify_script`; a non-zero exit stops the
   landing. A `suite` unit that changes no registered production input runs
   nothing.
8. **`seal`.** On the canonical checkout, the index holds the rebased tip and
   `HEAD` the base. It appends a `## Landing` section to the evidence record,
   closes the ledger row and writes the inventory, then stages the unit.
   The section reads:

   ```text
   ## Landing

   - **Base revision:** `<base>`
   - **Check:** <per project: the check command, its exit and its first output line>
   - **Build:** <per built project: each entry it ran with exit 0, then manifest git_revision <revision>>
   ```

   When nothing was built, the `Build` line reads `artifact current; no build`.

   A `suite` unit without production inputs records the check
   `not applicable: the suite unit changes no registered production input` and
   the build `none; a suite unit has no production owner`. The row becomes
   `done`; `Files / areas` becomes an `inventory` link and `Automated evidence`
   an `evidence` link, both relative to the plan; and `Diffstat` becomes the
   exact `N files, +X/-Y` including the inventory's own row. The inventory is
   written at `<owner>/docs/inventories/<plan-slug>/<unit>.numstat.tsv`, or
   under root `docs/` for a suite unit, after the evidence and the plan, so it
   hashes their final bytes. It has `Base revision` = the base, one exact `Pathspec` per
   changed path, the plan, the evidence and itself, the `Calculation` and
   `Hashes` lines, and one row per path in the format of the
   [change policy](../governance/change-policy.md).
9. **`run_guards`.** With the unit staged, the hooks' own commands in order:
   `check-staged-units.py <inventory>`, `commit_scope.py --check <subject>`
   with the staged paths on stdin, `version_tool.py check`,
   `check-architecture-contract.sh`, `check-language-contract.py` and
   `check-documentation-contract.sh`. The first failure stops the landing
   with its output.
10. **`commit_and_push`.** One `git commit -m <subject>` through the
    repository hooks; a hook failure stops the landing. Local `main` is
    fast-forwarded to the sealed commit with a compare-and-swap, checked out,
    and pushed with `git push --set-upstream origin main`. On success the tool
    prints `land-unit: landed <commit> <subject>` and removes the landing
    worktree. When the push fails, local `main` returns to where it was and the
    tool fetches. If `origin/main` already is the sealed commit, the unit
    landed. Otherwise local `main` is reset to `origin/main` and the sealed
    commit is discarded: if `origin/main` moved, the landing restarts at
    `unbump` from the branch's unsealed commits, at most three times (four
    pushes in total); if it did not move, or after the third retry, the landing
    stops.

The sealed commit is derived, never rebased: every retry recomputes the
version, the build decision and the inventory against the parent the commit
will have.

## Hot files

Git's own merge stands for every file it merges cleanly. When the rebase stops
on a conflict, only these paths receive a semantic resolution; each is a pure
function in `scripts/landing.py`:

- **The unit's plan** (the active plan the branch changes). `merge_plan`
  first compares the branch's plan with the fork point's, both without this
  unit's ledger row. When they differ, the branch changed the plan beyond its
  own row, and the landing stops at `rebase` naming the plan. Otherwise it
  takes `main`'s text and replaces the ledger row whose `Unit` is this unit
  with the branch's row, or, when `main` has no such row, inserts the branch's
  row after the row that precedes it on the branch (or first in the table).
  Every other row and all prose are `main`'s.
- **The debt ratchets** listed in `commit_policy.shared_ratchet_files` of
  `docs/projects.toml`. `merge_ratchet` keys each row by its non-integer
  cells; a row with other than exactly one integer cell stops the landing.
  A key present on both sides takes the lower value; a key one side removed
  stays removed; a key only the branch added is appended; comments and order
  follow `main`. The guards then prove every row equals the measurement.
- **Every file named `Cargo.lock`.** The landing takes `main`'s file and runs
  `cargo metadata --offline --format-version 1` in its directory, so that
  Cargo records what the branch's manifests require. When cargo
  fails, which includes needing the network, the landing stops. When a
  package `main` already locks changes version or checksum, `lockfile_upgrades`
  names it and the landing stops; a package the branch added or removed is
  accepted.

A hot file deleted on one side stops the landing. Version sources, their
mirrors and `docs/version-history.tsv` are not hot files: after `unbump` the
branch carries no version change, and `bump_version` writes them on the
rebased tree. Every other conflict, including inventories, evidence records,
`STATUS.md`, `ROADMAP.md`, `VALIDATION.md` and plan indexes, stops the landing
for the author to resolve. Two branches that set different active checkpoints
in one `ROADMAP.md` conflict in prose and stop; the one-active-checkpoint rule
is unchanged.

## Stops and resumption

A stop prints `land-unit: stopped at <step>: <reason>`, where the reason names
the file at fault when there is one, and exits 1. The step label is
`preflight`, `rebase`, `build_if_stale`, `seal`, `guard` (for `pre_guards` and
`run_guards`), `commit` or `push`.
When the landing worktree exists, the tool adds
`land-unit: resolve it, then run land-unit.py --continue, or --abort to give up`.
Any other failure prints `land-unit: <reason>` and exits 1; Ctrl-C prints
`land-unit: interrupted during <step>; run land-unit.py --continue or --abort`.

A `preflight` stop creates nothing: fix the cause and run the same command
again. From `unbump` on, the tool keeps its state in
`<parent>/<name>.worktrees/.landing/.land-state.toml`: the step that was
running, the branch, the kind, the summary, the project, the unit, the retry
count, and what later steps recorded (base, landing tip, check and build
results, inventory path, and `main`'s previous and sealed commits).
`land-unit.py --continue` reads it and resumes at that step. For a rebase
conflict, the message says to resolve the files in the landing worktree,
`git add` them, run `git rebase --continue` there, then
`land-unit.py --continue`. When a push fails and `origin` cannot be fetched,
local `main` is restored and the sealed commit kept, so `--continue` pushes
it; after a rejected push that stopped the landing, `--continue` restarts at
`unbump`.

The canonical checkout is detached on the rebased tip from
`checkout_canonical` until `commit_and_push` moves `main`. A stop there leaves
it detached on purpose, with `main` untouched, so the author can inspect the
tree that was about to land. `land-unit.py --abort` moves local `main` back
to its previous commit while it still is the sealed commit, checks out `main`
with `--force` (discarding the staged seal), deletes the unit's untracked
inventory if the seal wrote one, removes the landing worktree with its state
file, and prints `land-unit: aborted; the canonical checkout is on main`. The
branch is never changed, so a new landing starts from the same session work.

`LAND_UNIT_GIT`, `LAND_UNIT_CARGO` and `LAND_UNIT_PYTHON` replace the programs
the tool runs; the tests use them.

## What the tool never does

- Force-push, move the branch, or rewrite its commits. Local `main` moves only
  by fast-forward to the sealed commit, back to its previous commit when that
  seal was not pushed, or to `origin/main` after a rejected push.
- Edit a tracked inventory; it writes one new inventory, in the commit that
  lands it.
- Run with a dirty canonical checkout, off `main`, inside a session worktree,
  or while another landing is in progress.
- Land a path outside the row prefix's registered commit scope, or more than
  one unit in one commit.
- Pass `--no-verify`. The temporary commits of the landing worktree are made
  with `git commit-tree` and never published; the sealed commit is the only one
  that reaches `main`, and a hook failure fails the landing.
- Build in a session worktree, run a deploy entry on its own, or build a
  project whose artifact `check --require-verified` already accepts.
- Bump a product for a `suite` unit, or resolve a prose conflict.
