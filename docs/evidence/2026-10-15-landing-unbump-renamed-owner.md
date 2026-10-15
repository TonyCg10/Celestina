# Evidence: the landing unbumps a renamed halted owner at its new paths

- **Date:** 2026-10-15
- **Scope:** CEL-1-A4 — suite (`scripts/landing.py`, `scripts/land-unit.py`,
  `scripts/test-land-unit.py`, `docs/contracts/landing.md`), branch
  `unit/suite/CEL-1-A4` on `1fc6cbc3`
- **Environment:** CachyOS, Python 3.14, Git; the probe read-only against the
  local branch `unit/suite/CEL-1-A` (`5eb67a82`, the author's prepared rename
  merged with `origin/main`, whose fork point is `1fc6cbc3`)
- **Artifact:** not applicable (landing tool only)

## Procedure

### The defect

Foreseen by CEL-1-A3's evidence and confirmed on 2026-10-15 by calling
`origin/main`'s `unbumped_files` read-only on `unit/suite/CEL-1-A` (the
probe below, with `origin/main`'s `scripts/landing.py` on the path):

```
LandingError: cannot read the version of celestina/Cargo.toml: branch:celestina/Cargo.toml: expected package "celestina-shell", found "celestina"
```

`unbumped_files` read `origin/main`'s registry only and every owner's
version sources at their paths on the branch. On the rename branch,
`origin/main`'s halted owner `celestina` mirrors its version into
`celestina/Cargo.toml` and `celestina/Cargo.lock`, which now hold the Rust
shell's workspace (package `celestina`, `0.1.0`) or do not exist, while the
branch's registry carries that owner as `celestina-qt` (`history_owner =
"celestina"`, `halted`, sources under `celestina-qt/`) and registers a new
unversioned `celestina`.

### The rule

`scripts/landing.py`, dormant for a branch whose registry lists the same
owners with the same sources as `origin/main`'s:

- `unbumped_files(main_registry, branch_registry, read_base, read_branch)`
  takes the branch's registry as well and iterates main's owners. An owner
  the branch does not rename is read as before, by main's sources, base
  against branch at their own paths, whatever the branch's registry says of
  it: a branch forked before the owner adopted a version still has its bump
  dropped. An owner the branch renames (`history_owner` names a main id the
  branch's owner carried before the rename) pairs its sources with the
  former owner's by kind and name (`paired_sources`; the version contract
  keeps both through a rename): the base bytes at the former's path, the
  branch bytes at the new path, and a version that differs returns to the
  base's at the new path, since a rename never bumps. A new owner, including
  one that takes a renamed owner's id, has no base and nothing to unbump; a
  source without a counterpart is new. The history file returns to the fork
  point's bytes as before.
- `scripts/land-unit.py` `unbump` passes `registry_at(ctx, state.branch)`
  beside `ctx.registry` (`origin/main`'s, set by preflight).

### The probe

A read-only script in the session's scratchpad loads this branch's
`landing.py`, reads both registries with `git show`, takes `git merge-base
origin/main unit/suite/CEL-1-A` as the base and calls `unbumped_files` with
readers over `git show <rev>:<path>`. `land-unit.py` itself was not run.

```
origin/main 1fc6cbc3 branch 5eb67a82 base 1fc6cbc3
main owners: ['celestina', 'celestina-style', 'celestina-rs', 'siderita', 'magnetita', 'magnetita-android', 'grafita', 'fluorita', 'hematita', 'cuprita', 'calcita', 'selenita']
branch owners: ['celestina-qt', 'celestina', 'celestina-style', 'celestina-rs', 'siderita', 'magnetita', 'magnetita-android', 'grafita', 'fluorita', 'hematita', 'cuprita', 'calcita', 'selenita']
celestina-qt history_owner: celestina halted: 2026-09-27
celestina-qt sources: [{'kind': 'cmake-project', 'path': 'celestina-qt/CMakeLists.txt', 'project': 'celestina'}, {'kind': 'cargo-package', 'path': 'celestina-qt/Cargo.toml', 'package': 'celestina-shell'}, {'kind': 'cargo-lock', 'path': 'celestina-qt/Cargo.lock', 'package': 'celestina-shell'}]
new celestina versioned: False
unbumped_files: {}
```

The branch's `docs/version-history.tsv` equals the fork point's, so the
history rule has nothing to return either.

### Tests and guards

`scripts/test-land-unit.py`, class `LandingFunctions`: the existing
`unbumped_files` test passes unchanged with the same registry twice; one
test gains the renamed owner, on the fixture's `app` renamed to `app-qt`
with `history_owner = "app"` and `halted`: (a) sources moved with the
version kept, nothing rewritten and no error; (b) sources moved and bumped,
rewritten to the base version at the new paths; (c) the old id re-registered
as a new unversioned owner, ignored whatever the old paths hold; (d) the old
id re-registered as a new versioned owner with a cargo source, ignored even
with the former's package name; and, read with main's registry alone as
before, that manifest would have been taken for the former owner's. A
second test reads an owner main lists by main's sources where the branch's
registry, forked before the owner adopted a version, says `versioned =
false`: the branch's bump is dropped as with main's registry twice. The
`unbump` call itself has no fixture test, for the reason CEL-1-A3's evidence
gives: `LandUnitFixture` has no cheap pattern for a renamed halted owner.

| Command | Exit | Output |
|---|---|---|
| `python3 scripts/test-land-unit.py -k LandingFunctions` | 0 | Ran 36 tests, OK |
| `python3 scripts/test-land-unit.py` | 0 | Ran 92 tests, OK |
| `bash scripts/check-documentation-contract.sh` | 0 | Documentation contract: OK (the known errata of `SID-G7-D`'s inventory printed as on `main`) |
| `python3 scripts/check-language-contract.py` | 0 | Language contract: OK (131 legacy file(s) ratcheted) |
| `bash scripts/check-architecture-contract.sh` | 0 | Architecture contract: OK |

## Result

- **Exit:** 0 for every guard; the probe rewrites nothing and raises nothing
- **Observed:** on `unit/suite/CEL-1-A` the renamed `celestina-qt` is read
  at `celestina-qt/CMakeLists.txt`, `celestina-qt/Cargo.toml` and
  `celestina-qt/Cargo.lock` against the former `celestina`'s paths at the
  fork point, where the versions agree; the new `celestina` is unversioned
  and ignored; a branch whose registry equals `origin/main`'s is unbumped as
  before

## Limits

- The probe proves the `unbump` step's version rule only; the later steps of
  `land-unit.py` on the real branch are not run here. From reading them:
  `rebase` resolves hot files with `origin/main`'s registry
  (`other_unit_record`, `is_active_plan`), so a conflict in a moved plan of
  the halted owner would be judged by its old root; `bump_version` writes
  nothing for a maintenance landing; `build_if_stale` and `deploy_line` read
  the tip's registry, where the halted `celestina-qt` is never built and the
  new `celestina`, deployable with `celestina/scripts/build-production.sh`
  and `verify-production.sh`, is built and verified because every one of its
  production inputs changes; `pre_guards` and `run_guards` run the committed
  guards, which CEL-1-A1 taught the rename.
- A source the branch moves within an owner main also lists is read by
  main's sources, at its old path, where the branch may hold nothing; the
  landing's `pre_guards` refuses that change (the session's commit hook also
  does, but a hook can be bypassed).

## Follow-up

None.

## Landing

- **Base revision:** `1fc6cbc341661b1b2d6fc9f47aaa35feb6148524`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
