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
scripts/worktree.sh open siderita SID-U2-B --from unit/siderita/SID-U2-A
scripts/worktree.sh close siderita SID-U2-A
```

`PROJECT` is a registered project id or `suite`, and `UNIT` is the ledger's
unit id. Both must start with a letter or digit and contain only letters,
digits, `.`, `_` and `-`. The entry refuses an unregistered project and refuses
to run inside a session worktree. It exits 1 on a refusal and 2 on a usage
error, with one line on stderr.

`open` runs `git fetch origin main`, creates the branch `unit/<project>/<unit>`
from `origin/main`, adds the worktree at
`<parent>/<name>.worktrees/<project>-<unit>/` and prints its path. With
`--from BRANCH`, which only `open` takes, the branch starts from the local
branch `BRANCH` instead, for a unit stacked on another unit's branch (see
[Stacked branches](#stacked-branches)); it refuses when `BRANCH` does not
exist. It refuses when that directory or that branch already exists. It
writes two untracked files at the worktree's root:

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
no inventory `<root>/<plan-slug>/<unit>.numstat.tsv` in the owner's inventory
root: `docs/inventories` for `suite`, and `<path>/docs/inventories` for a
project, with `path` from `docs/projects.toml`. An inventory of the same unit
id anywhere else, such as another project's unit or a tracked fixture, does
not count. Otherwise it removes those two files and the worktree, then deletes
the branch. The landing publishes one new sealed commit, so the session's own
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

`qmllint-cxxqt.sh` reads the release QML module under the target directory
that `cargo metadata --no-deps --format-version 1 --offline` reports from the
application's root, so in a session worktree it finds the shared target
directory; when Cargo gives no answer, it reads `<application>/target`.

For the landing to accept the branch, its diff against `origin/main` must
change exactly one active plan (a Markdown file other than `README.md` directly
in an owner's registered `active_plans` directory), and that plan must have
exactly one ledger row that is `active`, or `done` without an inventory link.
A row whose text equals `origin/main`'s row does not count: it is open on
`origin/main` as well, such as the author's own in-flight work, and the branch
did not change it. So the unit's row is the one open row the branch changed
or added. When the branch also changed a row that is open on `origin/main`,
`preflight` stops and says that row belongs to its own unit. What a unit that
already landed left on the branch does not count either, which is what lets
a [stacked branch](#stacked-branches) land:

- a changed active plan whose text on the branch equals `origin/main`'s or
  the fork point's once the rows `origin/main` settles are masked (see
  `merge_plan` under [Hot files](#hot-files)) is set aside while another
  changed plan remains;
- in the unit's plan, a row open on the branch that `origin/main` closed
  (`done` with an inventory link) is set aside while another open row
  remains.

The row of the unit a branch `unit/<project>/<unit>` is named after is never
set aside: when `origin/main` closed it, in any changed plan, it is the unit,
and `preflight` then refuses it as landed. The unit found must be the one
the branch is named after, or `preflight` stops naming both. When more than
one plan or row remains, the stop lists them and names the open rows
`origin/main` has not closed; for each one whose branch `unit/*/<unit>`
exists and is an ancestor of the landing's branch, it says to land that unit
first. A remaining plan that the fork point had and `origin/main` no longer
has under `active/` is named as such instead.

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
   inventory that exists on `origin/main`, or when `--kind` is `bug`,
   `milestone` or `release` and the owner has no version to bump: a `suite`
   unit, or a project the version contract does not version
   (`versioned = false`, such as `celestina-rs`). The
   [version contract](versioning.md) allows a `suite-bug`, `suite-milestone`
   or `suite-release` that bumps one or more products, but the tool refuses it
   because the author records those by hand. The scope check leaves out the
   changed paths that only carry units `origin/main` already holds: a path
   whose bytes on the branch equal `origin/main`'s, a plan set aside by the
   rules of the previous section, and another unit's evidence record or
   inventory that the fork point lacks and `origin/main` has, which the rebase
   resolves to `origin/main`'s copy. `pre_guards` and `run_guards` then judge
   what the rebased tip really changes. The registry is read from
   `origin/main`. Then it adds the landing worktree, detached on the branch,
   and writes the state file.
2. **`unbump`.** Stops when local `main` is not an ancestor of `origin/main`
   (it holds commits made elsewhere), before anything is rebuilt: keep those
   commits on another branch and reset `main` to `origin/main`. In the landing
   worktree, the branch becomes one temporary
   commit on its fork point with `origin/main`, made with `git commit-tree`.
   When the branch changed a registered version assignment, a mirror or
   `docs/version-history.tsv`, only those version bytes return to the fork
   point's, and the tool warns:

   ```text
   land-unit: warning: the session bumped <paths>; the landing drops that change and computes the version itself
   ```

   Other edits in the same manifest, such as an added dependency, stay.
3. **`rebase`.** Records `origin/main` as the base. Stops when the base
   tracks the unit's inventory, with the preflight's message
   `<unit> already landed: origin/main tracks <inventory>`: a retry after a
   rejected push rebases on a newer `origin/main`, which may carry the unit.
   Stops when the plan existed at the fork point but is no longer
   on `origin/main`: archiving a plan while a unit is open is the author's
   decision. Otherwise it runs
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
7. **`build_if_stale`.** The registry is the rebased tip's, so a unit that
   renames an input and registers the new name is read with the new name.
   The affected projects are the owning project first (a `suite` unit has
   none), then, in registry order, every other registered project with a
   production or verification input the unit changed, such as the deployable
   consumers of a library. A changed path is an input when it is one of the
   production inputs `production_artifact.py` fingerprints, expanded on the
   tip, or when it or a directory above it matches a raw production or
   verification input pattern (`fnmatch`), so a deleted file still counts.
   The verification inputs are exactly the set whose bytes make the
   verification fingerprint, which `production_artifact.py`'s
   `verification_input_patterns` returns: the project's
   `verification_inputs`, its registered verify, status, activate, complete
   and deploy entries, `scripts/complete-production.py` for a deployable
   project, and the shared paths such as `scripts/production_artifact.py`,
   `scripts/production-common.sh`, `scripts/qmllint-cxxqt.sh`,
   `docs/projects.toml` and the debt ratchets. A unit that changes a shared
   path therefore marks every registered project, and each one whose
   production inputs and artifacts are current takes the verification-only
   path below, with no build.
   A production input the tip's registry names but the tip lacks stops the
   landing at `build_if_stale`, naming the project and the pattern. For each
   affected project, it runs
   `production_artifact.py check <project> --require-verified`. Exit 0 means
   the artifact is current and verified for these exact inputs, and nothing
   runs. When the check fails only with the verification errors
   (`artifact is not verified yet` or `tests or rules changed`), the
   production inputs and artifacts are unchanged, so only the verification
   runs: the registered `verify_script`, then for a deployable project its
   `deploy_script` and `status_script`, the rest of what its
   `complete_script` runs. Otherwise a deployable project runs its registered
   `complete_script`, and a project that is not deployable runs its
   `build_script`, then its `verify_script`. A non-zero exit stops the
   landing. A `suite` unit that changes no registered production or
   verification input runs nothing.
8. **`seal`.** On the canonical checkout, the index holds the rebased tip and
   `HEAD` the base. It appends a `## Landing` section to the evidence record,
   closes the ledger row and writes the inventory, then stages the unit.
   The section reads:

   ```text
   ## Landing

   - **Base revision:** `<base>`
   - **Check:** <per project: the check command, its exit and its first output line>
   - **Build:** <per project that ran: `<project> build:` or `<project> verify:`, each entry with exit 0, then the manifest's source_fingerprint and verification_fingerprint>
   ```

   For example:
   `app verify: verify-production.sh exit 0, deploy-production.sh exit 0, status-production.sh exit 0, manifest source_fingerprint sha256:<digest>, verification_fingerprint sha256:<digest>`.
   The fingerprints identify the inputs and the verification the artifact was
   made from; the manifest's `git_revision` names only the temporary detached
   tip. When nothing ran, the `Build` line reads `artifact current; no build`.

   A `suite` unit that changes no registered production or verification input
   records the check
   `not applicable: the suite unit changes no registered production or verification input`
   and the build `none; no registered production input changed`. The row becomes
   `done`; `Files / areas` becomes an `inventory` link and `Automated evidence`
   an `evidence` link, both relative to the plan; and `Diffstat` becomes the
   exact `N files, +X/-Y` including the inventory's own row. The inventory is
   written at `<owner>/docs/inventories/<plan-slug>/<unit>.numstat.tsv`, or
   under root `docs/` for a suite unit, after the evidence and the plan, so it
   hashes their final bytes. It has `Base revision` = the base, one exact `Pathspec` per
   changed path, the plan, the evidence and itself, the `Calculation` and
   `Hashes` lines, and one row per path in the format of the
   [change policy](../governance/change-policy.md). A path the unit deletes
   is already staged as a deletion, so only the paths that exist are added.
9. **`run_guards`.** With the unit staged, the hooks' own commands in order:
   `check-staged-units.py <inventory>`, `commit_scope.py --check <subject>`
   with the staged paths on stdin, `version_tool.py check`,
   `check-architecture-contract.sh`, `check-language-contract.py` and
   `check-documentation-contract.sh`. The first failure stops the landing
   with its output.
10. **`commit_and_push`.** One `git commit -m <subject>` through the
    repository hooks; a hook failure stops the landing. Local `main` is
    fast-forwarded to the sealed commit with a compare-and-swap, checked out,
    and pushed with `git push --set-upstream origin main`. From the
    fast-forward to the end of the push, any failure or Ctrl-C first returns
    local `main` to where it was. On success the tool
    prints `land-unit: landed <commit> <subject>` and removes the landing
    worktree. When the push fails, local `main` returns to where it was and the
    tool fetches. If `origin/main` already is the sealed commit, the unit
    landed. Otherwise the sealed commit is discarded and local `main` is
    fast-forwarded to `origin/main` with `git merge --ff-only`, so a commit
    made on local `main` from elsewhere is never discarded; when it cannot
    fast-forward, the landing stops at `push`, saying whether those commits
    sit on this landing's unpushed sealed commit, and to keep them on another
    branch and run `git reset --hard origin/main` on `main`; `--continue` then
    restarts at `unbump`, which stops again while `main` still holds them.
    If `origin/main` moved, the landing restarts at
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
  unit's ledger row and without every other row that `main` already settles:
  a branch row equal to `main`'s row, and a branch row that is open where
  `main`'s is closed with every cell other than `Status`, `Files / areas`,
  `Diffstat` and `Automated evidence` (the cells the seal writes) equal,
  which is a landed dependency's row as its session left it. That second
  rule requires the row to have as many cells as the header on both sides,
  and never applies to a row the fork point had already closed. A row equal
  to the fork point's is unchanged and needs no rule; `settled_rows` decides
  the rest. When the texts still differ, the branch changed the plan beyond
  its own row, and the landing stops at `rebase` naming the plan, the first
  line that differs on each side and, when that line is a ledger row, its
  unit (the comparison is by whole lines, so a reformatting also stops).
  Otherwise it
  takes `main`'s text and replaces the ledger row whose `Unit` is this unit
  with the branch's row, or, when `main` has no such row, inserts the branch's
  row after the row that precedes it on the branch (or first in the table).
  Every other row and all prose are `main`'s.
- **Another active plan** the branch changes, which a stacked branch carries
  from a dependency in another plan. `merge_settled_plan` takes `main`'s text
  when `plan_settled` holds: once the rows `settled_rows` accepts are masked
  on every side, the branch's plan equals `main`'s or the fork point's. The
  tool then says
  `land-unit: <path> holds only rows origin/main settles; the landing keeps origin/main's copy`.
  Otherwise the landing stops at `rebase` naming the plan.
- **The debt ratchets** listed in `commit_policy.shared_ratchet_files` of
  `docs/projects.toml`. `merge_ratchet` keys each row by its non-integer
  cells; a row with other than exactly one integer cell stops the landing.
  The comment line immediately above a row belongs to that row.
  A key present on both sides takes the lower value and keeps `main`'s
  comment, unless only the branch changed it; a key one side removed stays
  removed, with its comment; a key only the branch added is appended with its
  comment; every other line, such as the file's head comments, and the order
  follow `main`. The guards then prove every row equals the measurement.
- **Every file named `Cargo.lock`.** Every other conflicted path is resolved
  first, and each lockfile last. When a `Cargo.toml` in the lockfile's
  directory or below is still in conflict, the landing stops at `rebase`
  naming that `Cargo.toml`: resolve it in the landing worktree, `git add` it,
  then run `land-unit.py --continue`, which merges the lockfile and continues
  the rebase. Otherwise the landing takes `main`'s file and runs
  `cargo metadata --offline --format-version 1` in its directory, so that
  Cargo records what the branch's manifests require. When cargo
  fails, which includes needing the network, the landing stops. When a
  package `main` already locks changes version or checksum, `lockfile_upgrades`
  names it and the landing stops; a package the branch added or removed is
  accepted. Either lockfile stop says to merge the lockfile by hand,
  `git add` it in the landing worktree, then run `land-unit.py --continue`.

- **Another unit's added evidence record or inventory.** In an add/add
  conflict, a Markdown file directly in an owner's `docs/evidence/` other than
  its `README.md` and other than this unit's evidence record, and an
  inventory at `<owner docs>/inventories/<plan-slug>/` other than this
  unit's, are another unit's records; `other_unit_record` decides which
  paths these are. An inventory takes `main`'s copy. An evidence record takes
  `main`'s copy only when `merge_other_evidence` finds that the branch's copy,
  without trailing newlines, equals `main`'s without its trailing
  `\n\n## Landing` section, which is exactly what the seal appended; any
  other difference means the branch edited another unit's evidence, and the
  landing stops at `rebase` naming the file. The tool says:

  ```text
  land-unit: <path> is another unit's evidence record; the landing keeps origin/main's copy, which adds only its landing section
  land-unit: warning: <path> is another unit's inventory; the landing keeps origin/main's copy
  ```

  A modify/modify conflict on another unit's record, which the fork point
  already had, is an edit of an older record and stops like any other file.

A hot file deleted on one side stops the landing, and so does another unit's
record deleted on one side. Version
sources, their mirrors and `docs/version-history.tsv` are not hot files: after
`unbump` the branch carries no version change, and `bump_version` writes them
on the rebased tree. Every other conflict, including this unit's own evidence
record, `STATUS.md`, `ROADMAP.md`, `VALIDATION.md` and plan indexes, stops the
landing for the author to resolve. Two branches that set different active checkpoints
in one `ROADMAP.md` conflict in prose and stop; the one-active-checkpoint rule
is unchanged.

## Stacked branches

A unit that needs another unit's unlanded work is prepared on a branch
stacked on that unit's branch, its dependency. Open it from the canonical
checkout with

```sh
scripts/worktree.sh open siderita SID-U2-B --from unit/siderita/SID-U2-A
```

or by hand with
`git worktree add -b unit/siderita/SID-U2-B <parent>/<name>.worktrees/siderita-SID-U2-B unit/siderita/SID-U2-A`
(then write the two files `open` writes), or with `scripts/worktree.sh open`
followed by `git reset --hard unit/siderita/SID-U2-A` in the new worktree.
The stacked branch then carries the dependency's commits: its change, its
ledger row as its session left it, and its evidence record.

A unit that needs two unlanded units starts from one dependency's branch and
merges the other's (`git merge unit/<project>/<unit>`) in its worktree. A
dependency may belong to another plan and another prefix than the stacked
unit.

Units land in dependency order. While a dependency has not landed, the
stacked branch carries its open row: in the unit's plan, that plan has two
open rows; in another plan, the branch changes two plans that are not set
aside. Either way `preflight` stops and names the dependency's row; it says
to land that unit first only when a branch `unit/<project>/<dependency>`
exists and is an ancestor of the stacked branch. The dependency lands as one sealed commit, and its branch is
never rewritten, so the stacked branch keeps its commits. The stacked unit
then lands with the ordinary command, and the tool accepts what the
dependency's landing left on both sides:

- the dependency's row, open on the branch and closed on `main`, is set
  aside by `discover_unit` and masked by `merge_plan`; when it lies in
  another plan, that plan is set aside, the scope check leaves it out, and
  `merge_settled_plan` keeps `main`'s copy;
- the dependency's evidence record exists on both sides, and `main`'s copy,
  with its `## Landing` section, is kept when the branch's copy is the one
  the dependency's session left; an edit the stacked session made to it
  stops the landing; the scope check leaves the record out;
- the dependency's change exists on both sides, and Git's three-way merge
  resolves every hunk that is identical on both sides; a path whose bytes
  equal `main`'s is left out of the scope check;
- the dependency's version bump and inventory exist only on `main`, since a
  session writes neither and `unbump` drops a bump it did write, so they
  merge cleanly.

The sealed commit's diff against the previous `main` is then the stacked
unit's own change, plan row, evidence record and inventory. The rebase takes
the old fork point as its base, so a stacked change that touches lines next to
a line the dependency changed conflicts as an ordinary file and stops the
landing for the author to resolve. A file of the dependency that another
unit changed on `main` after the dependency landed differs from `main` on
the branch, so under another prefix it stops the scope check at
`preflight`; merge `origin/main` into the stacked branch in its session
worktree, keeping `main`'s side of the dependency's files, then land again.

## Stops and resumption

A stop prints `land-unit: stopped at <step>: <reason>`, where the reason names
the file at fault when there is one, and exits 1. The step label is
`preflight`, `unbump`, `rebase`, `build_if_stale`, `seal`, `guard` (for
`pre_guards` and `run_guards`), `commit` or `push`.
When the landing worktree exists, the tool adds
`land-unit: resolve it, then run land-unit.py --continue, or --abort to give up`,
unless the stop names its own remedy instead. After the seal, a `guard` stop of
`run_guards` and a `commit` stop judge the staged unit, so they add:

```text
land-unit: the unit is sealed, so --continue only repeats this step on the same bytes, which helps only when the cause lies outside the unit, such as a missing tool; otherwise run land-unit.py --abort, fix the branch in its session worktree, and land it again
```

A `rebase` stop for a unit that already landed adds
`land-unit: the unit is on main already: run land-unit.py --abort`, and a
lockfile stop names the lockfile to `git add` in the landing worktree.
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
  seal was not pushed, or by fast-forward to `origin/main` after a rejected
  push.
- Edit a tracked inventory; it writes one new inventory, in the commit that
  lands it.
- Run with a dirty canonical checkout, off `main`, inside a session worktree,
  or while another landing is in progress.
- Land a path outside the row prefix's registered commit scope, or more than
  one unit in one commit.
- Pass `--no-verify`. The temporary commits of the landing worktree are made
  with `git commit-tree` and never published; the sealed commit is the only one
  that reaches `main`, and a hook failure fails the landing.
- Build in a session worktree, deploy without running the project's verify
  entry first, rebuild a project whose check reports only the verification
  errors, or run any entry for a project whose artifact
  `check --require-verified` already accepts.
- Bump a product for a `suite` unit, or resolve a prose conflict.
