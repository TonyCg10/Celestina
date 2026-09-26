# Evidence: the landing accepts a stacked branch

- **Date:** 2026-09-26
- **Scope:** `AUD-1-C` (program unit P-0b, ruling R-A1) of the
  [monorepo hardening plan](../plans/active/2026-09-26-monorepo-hardening.md);
  no audit finding. `scripts/landing.py`, `scripts/land-unit.py`,
  `scripts/worktree.sh`, their fixture tests,
  [the landing contract](../contracts/landing.md) and the §10 amendment of
  [the landing design](../superpowers/specs/2026-09-25-parallel-unit-landing-design.md)
- **Environment:** session worktree `unit/suite/AUD-1-C` in a Linux container
  (kernel 6.18); Python 3.11.15, Git 2.43.0; the fixtures build their own
  repositories, bare origins and fixture production entries in temporary
  directories, so no production target, cache or live session was touched
- **Artifact:** not applicable; the landing runs no production entry for a
  suite unit that changes no registered production or verification input

## Procedure

```sh
python3 scripts/test-land-unit.py LandingFunctions
python3 scripts/test-land-unit.py -k stacked -k another_unit
python3 scripts/test-land-unit.py -k cross_prefix -k two_dependency
bash scripts/test-land-unit.sh
sh scripts/test-worktree.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
printf 'scripts/landing.py\0docs/contracts/landing.md' \
  | python3 scripts/commit_scope.py --check \
    "suite-maintenance: Support stacked unit branches in the landing (P-0b, ruling R-A1)"
```

The tests were written first and run against the unchanged scripts (RED),
then the scripts were changed and the same tests run again (GREEN). A review
round then added stacks across plans and prefixes and tightened five rules;
its new tests were run against the first round's scripts (RED) before the
second round's scripts made them pass (GREEN).

## Result

- **Exit:** every command above exited 0 on the final tree.
- **RED:** `LandingFunctions` ran 23 tests with 16 errors, all in the three
  new tests: `discover_unit` rejected the new `read_main_plan` argument,
  `merge_plan` stopped on the dependency's row, and `other_unit_record` did
  not exist (14 subtests). The five new fixture landings failed: the stacked
  landings stopped at `preflight` with
  `found 2: FX-A, FX-B` and no hint, and the landing whose retry met another
  unit's added inventory stopped at `rebase` with
  `conflict in app/docs/inventories/2026-09-25-fixture/FX-Z.numstat.tsv`.
  `test-worktree.sh` failed on `open app APP-3 --from unit/app/APP-2` with
  exit 2 (usage).
- **RED, review round:** against the first round's scripts,
  `LandingFunctions` ran 25 tests with 6 failures and 3 errors (the missing
  `read_base_plan`, `stacked_on` and `merge_settled_plan`; a reworded,
  closed-and-edited, reopened or extra-cell row masked silently or without
  its row named; a modify/modify evidence record resolved to main's copy),
  and the four cross-plan fixtures failed at `preflight` with
  `the branch must change exactly one active plan; found 2` (one dependency)
  or `found 3` (two dependencies) and no dependency named.
- **GREEN:** `bash scripts/test-land-unit.sh` ran 70 tests, OK (56 before this
  unit, 14 new); `sh scripts/test-worktree.sh` printed 9 `ok` lines (8 before,
  1 new).

## Observed facts

- **Stacked branch after its dependency landed**
  (`test_stacked_branch_lands_after_its_dependency`). main plans `FX-A`,
  `FX-B` and `FX-C`; `unit/app/FX-B` is created with `git worktree add` from
  `unit/app/FX-A`; `FX-A` lands as a bug (version 0.1.1), then `FX-B` lands as
  maintenance. The result is one commit on top of `FX-A`'s sealed commit whose
  diff against it holds exactly `app/src/stacked.rs`, the plan, `FX-B`'s
  evidence record and `FX-B`'s inventory; the plan differs from the previous
  main in `FX-B`'s row alone (closed with its inventory link), `FX-A`'s row
  stays `done`, `app/src/main.rs` carries `FX-A`'s line once, the version stays
  0.1.1, and `FX-A`'s evidence record keeps main's bytes with its landing
  section, with the warning
  `<path> is another unit's evidence record; the landing keeps origin/main's copy`.
  This confirms that Git's three-way merge resolves the dependency's change,
  which is identical on both sides.
- **Dependency not landed**
  (`test_stacked_branch_stops_until_its_dependency_lands`). The landing stops
  at `preflight` with
  `found 2: FX-A, FX-B; origin/main has not closed FX-A: a stacked branch lands after the unit it is stacked on, so land FX-A first`,
  creates no landing worktree and builds nothing.
- **Stacked session edited the dependency's evidence**
  (`test_stacked_branch_keeps_mains_dependency_evidence`). The landing keeps
  main's copy, warns, and lands; the record is absent from the sealed diff.
- **Foreign plan edit** (`test_stacked_branch_editing_another_row_stops`, and
  the unchanged `test_plan_edited_beyond_own_row_stops`). A stacked branch that
  rewords `FX-C`'s row stops at `rebase` with
  `the branch changed the plan beyond its own row FX-B`. The pure test
  `test_merge_plan_accepts_rows_main_already_has` also stops a dependency row
  reworded beyond the seal's cells and a closed row that differs from main's.
- **Cross-prefix stack** (`test_cross_prefix_stack_lands_after_its_dependency`).
  The app unit `FX-A` is stacked on the library unit `LIB-A`, which has its
  own plan and the `lib:` prefix. After `LIB-A` landed, `FX-A` lands as one
  commit whose diff holds only `app/src/stacked.rs`, the app plan, `FX-A`'s
  evidence and its inventory; the library plan, evidence and source keep
  main's bytes, and the tool says the library plan
  `holds only rows origin/main settles`. Before `LIB-A` landed, the landing
  stops at `preflight` with `found 2` plans and
  `origin/main has not closed LIB-A: ... so land LIB-A first`.
- **Two dependencies** (`test_two_dependency_stack_lands_after_both`,
  `test_two_dependency_stack_stops_naming_the_unlanded_one`). `FX-A` starts
  from `unit/lib/LIB-A` and merges `unit/suite/SU-D`. With both landed it
  lands as one commit of its own paths; with only `LIB-A` landed it stops
  naming `SU-D` alone.
- **Tightened rules** (pure tests). A branch whose own row main closed
  returns that row, so preflight reports it as landed, whatever main did with
  the dependency's row. The landing-order wording appears only when a
  `unit/*/<unit>` branch of the dependency is an ancestor of the branch;
  otherwise the stop only names the open rows. A modify/modify conflict on
  another unit's evidence record is not resolved. A row with an extra cell,
  or a row the fork point had closed and the branch reopened, is not masked,
  and the stop names its ledger row.
- **Another unit's added inventory**
  (`test_added_inventory_of_another_unit_takes_mains_copy`). A branch carrying
  a file at another unit's inventory path, which main gains from a racing
  push, meets an add/add conflict on the retry; the landing keeps main's copy,
  warns, and lands on the second push.
- **Owners.** `SEAL_COLUMNS` names the four cells `close_ledger_row` writes and
  `settled_rows` ignores; `is_open_row` and `is_closed_row` are the one
  definition of an open and a sealed row for `discover_unit` and
  `settled_rows`; `other_unit_record` reuses `owner_tables` and
  `owner_docs_root` for the evidence and inventory roots.
- **Adjacent edits.** A probe with `git merge-file` on a base `fn main() {}`,
  one side adding `// A` and the other `// A` then `// B`, reports a conflict:
  a stacked change on lines next to its dependency's change conflicts as an
  ordinary file and stops the landing, as the contract now says.

## Limits

- The fixtures replace every guard and hook with a recording double, as the
  existing landing fixtures do; the real guard chain ran on this worktree,
  not inside a fixture landing.
- No real stacked branch of this program was landed: the landing needs the
  canonical checkout and a push to `origin/main`, which a session does not do.
- A file of a dependency under another prefix that a later unit changed on
  main differs from main on the stacked branch, so the preflight scope check
  still refuses it; the author resolves such a stack by hand.
- A dependency whose plan was archived on main after it landed leaves the
  stacked branch with a plan main lacks, which is not set aside and stops.

## Follow-up

None.
