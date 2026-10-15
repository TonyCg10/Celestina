# Evidence: the landing accepts a renamed halted owner's moved plans

- **Date:** 2026-10-15
- **Scope:** CEL-1-A2 — suite (`scripts/landing.py`, `scripts/land-unit.py`,
  `scripts/test-land-unit.py`, `docs/contracts/landing.md`), branch
  `unit/suite/CEL-1-A2` on `af3abd96`
- **Environment:** CachyOS, Python 3.14, Git; the probe read-only against the
  local branch `unit/suite/CEL-1-A` (`ad5466fa`, the author's prepared rename)
- **Artifact:** not applicable (landing tool only)

## Procedure

### The defect

Observed on 2026-10-15 by the controller, from the canonical checkout, with
`unit/suite/CEL-1-A` prepared and committed (`ad5466fa`: `git mv celestina
celestina-qt`, the registry renaming the halted owner with
`history_owner = "celestina"` and `halted = "2026-09-27"`, and a new
`celestina` registered as `rust-shell`):

```sh
python3 scripts/land-unit.py unit/suite/CEL-1-A --kind maintenance
```

stopped at preflight with

```
the branch deletes its plan celestina/docs/plans/active/2026-08-20-persistent-carriers.md
```

`discover_unit` reads the registry of `origin/main`, which knows only the old
owner `celestina` with `active_plans = "celestina/docs/plans/active"`. With
`--no-renames`, the halted shell's active plan is a deletion at that path and
an addition at `celestina-qt/docs/plans/active/…`, byte-identical. Two more
stops waited behind it, found with the probe below once the deletion was
skipped by hand: `the branch must change exactly one active plan; found 3`,
listing the suite plan, the new shell plan
`celestina/docs/plans/active/2026-10-14-cel-1-island.md` (only `planned`
rows) and `celestina-style/docs/plans/active/2026-08-04-shared-reading-controls.md`,
whose one difference from `origin/main`'s is the link of `STYLE-G7-K` to an
evidence record the rename moved (`celestina/docs/evidence/…` to
`celestina-qt/docs/evidence/…`).

### The rule

`scripts/landing.py`, dormant for a branch without a renamed halted owner:

- `renamed_owner_roots(main_registry, branch_registry)` pairs the
  `active_plans` directories of a halted branch owner that declares
  `history_owner` with the origin/main owner of that id, under the rule of
  `check-staged-units.py` `renamed_owner_moves` (the branch owner absent on
  main, the former owner carrying no `history_owner` itself).
- `moved_plan_paths(main_registry, branch_registry, changed_paths,
  read_branch, read_main)` names the plan files under the former directory
  that the branch lacks and that it holds unchanged at the same place under
  the new directory, where main has nothing. `discover_unit(…,
  moved_plans=…)` leaves them out before the deletion stop.
- `discover_unit` sets aside, like a settled plan and only while
  `renamed_roots=…` names a renamed owner, a changed plan that main and the
  fork point lack and whose branch text has no open row, and a plan that
  differs from main's only in links that follow the renamed owner's `docs/`
  tree (`only_follows_renamed_links`; the link rewrite is anchored on the
  closing delimiter, so a target never rewrites a longer target that starts
  with it). Both come back in `UnitRef.settled_plans`.
- `scripts/land-unit.py` `resolve_unit` reads the branch's registry only to
  compute both arguments; the registry that judges the unit stays
  `origin/main`'s. The moved files stay in `changed`: they are the unit's
  change, in its inventory and in its scope check, which the suite prefix
  allows.

### The probe

A read-only script in the session's scratchpad loads the branch's
`landing.py`, reads both registries with `git show`, computes the changed
paths with `git diff --name-only --no-renames origin/main...unit/suite/CEL-1-A`,
and runs `discover_unit` with the readers `resolve_unit` uses (the branch,
`origin/main`, the fork point `git merge-base origin/main unit/suite/CEL-1-A`,
`branch_unit="CEL-1-A"`). `land-unit.py` itself was not run.

```
origin/main af3abd96 branch ad5466fa fork da4647fc changed paths 1040
renamed_owner_roots: {'celestina/docs/plans/active': 'celestina-qt/docs/plans/active'}
moved_plan_paths: ['celestina/docs/plans/active/2026-08-20-persistent-carriers.md']
discover_unit: UnitRef(project_id='suite', prefix='suite', plan_path='docs/plans/active/2026-10-14-rust-shell.md', unit='CEL-1-A', summary='Move the halted shell to celestina-qt and register the Rust shell', evidence_path='docs/evidence/2026-10-14-shell-rename.md', settled_plans=('celestina/docs/plans/active/2026-10-14-cel-1-island.md', 'celestina-style/docs/plans/active/2026-08-04-shared-reading-controls.md'))
```

Before the rule, the same probe with the moved plan skipped by hand stopped
with `found 3` (above) and, with the new shell plan also skipped, with
`found 2: docs/plans/active/2026-10-14-rust-shell.md,
celestina-style/docs/plans/active/2026-08-04-shared-reading-controls.md;
origin/main has not closed STYLE-G7-K`.

### Tests and guards

`scripts/test-land-unit.py`, class `LandingFunctions`, gains four tests:
`moved_plan_paths` pairs only the unchanged move of a halted owner (not a
plan whose copy differs, nor a copy main tracks, nor an owner without
`halted` or without a former owner on main); `discover_unit` with
`moved_plans` no longer stops on the deleted path; a new plan with only
`planned` rows beside the unit's plan is set aside (the unit found; `found 2`
without a renamed owner, when the new plan has an open row or a `done` row
without an inventory link; the earlier stop when it has no ledger; the
archived-plan wording kept); a plan that only follows the renamed owner's
links is set aside with `renamed_roots` and remains a candidate without
them, with any other edit, or when the row's status changes along with its
link; the rewrite leaves a longer target that starts with a link's target
alone.

| Command | Exit | Output |
|---|---|---|
| `python3 scripts/test-land-unit.py` (before and after the fix round) | 0 | Ran 89 tests, OK (a first run beside the staged-units and commit-scope fixtures had the two interrupted-landing tests fail on an `exit 130` of a Git child, a timing of the SIGINT under load; both pass alone and the file passes alone) |
| `sh scripts/test-staged-units.sh` | 0 | Staged inventories: OK |
| `bash scripts/test-commit-scope.sh` | 0 | Commit scope: OK |
| `bash scripts/check-documentation-contract.sh` | 0 | Documentation contract: OK (the known errata of `SID-G7-D`'s inventory printed as on `main`) |
| `python3 scripts/check-language-contract.py` | 0 | Language contract: OK (131 legacy file(s) ratcheted) |
| `bash scripts/check-architecture-contract.sh` | 0 | Architecture contract: OK |

## Result

- **Exit:** 0 for every guard; the probe returns the suite plan's `CEL-1-A`
  row
- **Observed:** on `unit/suite/CEL-1-A` the moved plan is paired, the new
  shell plan and the style plan are set aside as settled plans, and
  `discover_unit` finds `CEL-1-A`; every earlier `discover_unit` behaviour
  keeps its tests and messages

## Limits

- The probe proves preflight's discovery only; the later steps of
  `land-unit.py` on the real branch (scope check, rebase, seal, guards) are
  not run here. From reading them: the scope check passes every path under
  the suite prefix; the rebase of `ad5466fa` onto `af3abd96` touches no plan
  `origin/main` changed since the fork, so no hot merge runs; should the
  style plan conflict in a later attempt, `merge_settled_plan` would keep
  `origin/main`'s copy and lose the retargeted link, which the documentation
  contract then refuses at the seal.
- A link into a renamed owner's tree outside its `docs/` (for example its
  `README.md`) is not followed by the rule; such a plan stays a candidate.
- `moved_plan_paths` compares the decoded texts the readers return, as
  `discover_unit` does; the staged-units guard compares the bytes.

## Follow-up

None.

## Landing

- **Base revision:** `af3abd9683225fac35263a31cca67bf8995cce2f`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
