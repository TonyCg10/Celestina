# Evidence: the landing accepts a renamed halted owner's moved inventories

- **Date:** 2026-10-15
- **Scope:** CEL-1-A3 — suite (`scripts/landing.py`, `scripts/land-unit.py`,
  `scripts/test-land-unit.py`, `docs/contracts/landing.md`), branch
  `unit/suite/CEL-1-A3` on `53038724`
- **Environment:** CachyOS, Python 3.14, Git; the probe read-only against the
  local branch `unit/suite/CEL-1-A` (`4030f001`, the author's prepared rename
  merged with `origin/main`)
- **Artifact:** not applicable (landing tool only)

## Procedure

### The defect

Observed on 2026-10-15 by the controller, from the canonical checkout, with
`unit/suite/CEL-1-A` at its merge commit `4030f001`:

```sh
python3 scripts/land-unit.py unit/suite/CEL-1-A --kind maintenance
```

passed unit discovery (CEL-1-A2's rules) and stopped at preflight with

```
the branch edits tracked inventories, which are immutable: celestina/docs/inventories/2026-08-03-r3-session-verbs/R3-A.numstat.tsv, …
```

listing 63 paths. The check names every changed `.numstat.tsv` that
`origin/main` holds. With `--no-renames`, the halted shell's inventories,
moved byte-identically to `celestina-qt/docs/inventories/…`, are deletions at
their old paths, which `origin/main` holds; `check-staged-units.py`
`renamed_owner_moves` already accepts the same pairs when they are staged.

### The rule

`scripts/landing.py`, dormant for a branch without a renamed halted owner:

- `moved_inventory_paths(main_registry, branch_registry, changed_paths,
  read_branch, read_main)` takes the root pairs of `renamed_owner_roots`
  (CEL-1-A2: a halted branch owner with `history_owner`, absent on main,
  whose former owner on main declares no `history_owner`) and derives each
  owner's inventory root, `<docs root>/inventories/`, as `owner_docs_root`
  does. It names the changed `.numstat.tsv` files under the former root that
  the branch lacks and that it holds with the same content at the same place
  under the new root, where main has nothing.
- `scripts/land-unit.py` preflight reads the branch's registry, computes the
  pairs with byte readers (`blob` of the branch and of `origin/main`) and
  leaves their old paths out of the immutable-inventory stop. They stay in
  `changed`: the deletions and additions are the unit's change, listed in
  its inventory.

### The probe

A read-only script in the session's scratchpad loads this branch's
`landing.py`, reads both registries with `git cat-file blob`, computes the
changed paths with `git diff --name-only --no-renames -z
origin/main...unit/suite/CEL-1-A`, and computes the tracked inventories as
preflight does, before and after leaving out `moved_inventory_paths`.
`land-unit.py` itself was not run.

```
origin/main 53038724 branch 4030f001 changed paths 1018
renamed_owner_roots: {'celestina/docs/plans/active': 'celestina-qt/docs/plans/active'}
celestina/docs/inventories/**/*.numstat.tsv old paths in changed: 63
tracked inventories before the rule: 63
moved_inventory_paths: 63; every old path paired: True
tracked inventories after the rule: 0 []
```

### Tests and guards

`scripts/test-land-unit.py`, class `LandingFunctions`, gains one test:
`moved_inventory_paths` pairs only the unchanged move of an inventory in a
plan-slug subdirectory, and not an inventory whose copy differs, nor a copy
main already tracks, nor a `.numstat.tsv` outside the inventory root, nor
anything for an owner without `halted`, without a former owner on main, or
whose former owner was renamed before, nor without a rename. The preflight
call itself has no fixture test: `LandUnitFixture` has no cheap pattern for
a renamed halted owner (it would need a registry rename, a moved plan and a
moved inventory committed on the fixture's `origin/main`), so the pure
function's test and the probe above stand for it.

| Command | Exit | Output |
|---|---|---|
| `python3 scripts/test-land-unit.py -k LandingFunctions` | 0 | Ran 34 tests, OK |
| `python3 scripts/test-land-unit.py` | 0 | Ran 90 tests, OK |
| `bash scripts/check-documentation-contract.sh` | 0 | Documentation contract: OK (the known errata of `SID-G7-D`'s inventory printed as on `main`) |
| `python3 scripts/check-language-contract.py` | 0 | Language contract: OK (131 legacy file(s) ratcheted) |
| `bash scripts/check-architecture-contract.sh` | 0 | Architecture contract: OK |

## Result

- **Exit:** 0 for every guard; the probe leaves no tracked inventory
- **Observed:** on `unit/suite/CEL-1-A` all 63 old inventory paths are
  paired with their byte-identical copies under `celestina-qt/`, and the
  immutable-inventory stop has nothing left to name; a branch without a
  renamed halted owner computes an empty pairing and meets the stop as before

## Limits

- The probe proves preflight's inventory check only; the later steps of
  `land-unit.py` on the real branch are not run here. From reading them and
  calling `unbumped_files` read-only on the branch: `unbump` stops with
  `cannot read the version of celestina/Cargo.toml: branch:celestina/Cargo.toml:
  expected package "celestina-shell", found "celestina"`, because it reads
  `origin/main`'s registry, whose halted `celestina` owner mirrors its version
  into `celestina/Cargo.toml`, now the Rust shell's workspace. Past that, it
  would also return `docs/version-history.tsv` to the fork's bytes, dropping
  the branch's `celestina 0.1.0 baseline CEL-1-A` row.
- An inventory outside `<docs root>/inventories/` is not paired, as in
  `check-staged-units.py`.

## Follow-up

The `unbump` stop above, before `CEL-1-A` can land.

## Landing

- **Base revision:** `53038724e38491cb69a9b22858f0cc89bb4b3e68`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
