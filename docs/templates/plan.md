# Plan title

- **Opened:** YYYY-MM-DD
- **Plan ID:** stable-plan-id
- **Status:** active
- **Scope:** project or suite
- **Implementation checkpoint:** ID
- **Author-validation checkpoint:** VAL-ID or none

## Hypothesis

One falsifiable sentence.

## Tangible outcome

What can be built, run or inspected when implementation closes.

## Scope

- Included work already authorized elsewhere.

## Exclusions

- Explicit boundary.

## Build order

1. Causal step.

## Implementation exit

Agent-executable integrated proof. Manual validation does not block it.

For a refactor, record the responsibility extracted, the removed/delegated old
path, dependency direction, search for equivalent recipes, characterization
tests and tests of the new boundary. Line count alone is never exit evidence.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| ID-A | `project:` | planned | paths plus stable sections/symbols | — | one result | command/evidence | `VAL-ID` or `None` |

The owner roadmap must be `active` or `blocked` and name this exact
implementation checkpoint; otherwise this plan is orphaned. A local plan uses
only its owner's primary registered commit prefix. Component prefixes are for
atomic code or manifest commits that do not close a ledger unit. Only a suite
plan uses `suite:`.

`Plan ID` is unique and immutable within the owner. Declare it when creating the
plan and no later than the first commit containing one of its inventories. A
short basename without the `YYYY-MM-DD-` prefix is recommended; a different id
is valid only when it is equally stable and unambiguous. Archiving never changes
it.

The ledger records the base scope (`project:`), not the final subject suffix.
At closure classify the atomic batch as `bug`, `milestone`, `release` or
`maintenance`. A versioned product delivery applies the matching SemVer change
and append-only history row before its production build; see
[the version contract](../contracts/versioning.md).

Before setting a unit to `done`, calculate tracked paths with
`git diff --numstat --no-renames` and each new path against `/dev/null`; preserve
the exact path inventory and record the sum as `N files, +X/-Y`. Replace
`Files / areas` with one relative Markdown link to that unit's
stable owner-local `docs/inventories/<plan-slug>/<unit>.numstat.tsv`, and replace
`Automated evidence` with a resolvable link to its dated record under
`evidence/`. `plan-slug` is this plan's exact basename without `.md`; `unit`
matches the ledger id exactly. The inventory starts with
`Base revision<TAB><40 hex>`, uses the header
`added<TAB>deleted<TAB>content<TAB>path`, and includes every changed path plus
the plan, evidence record and one row for itself whose content is `self`.
Before the row header, record one or more narrow `Pathspec<TAB>path` boundaries;
use a trailing `/` for a directory and no trailing slash for an exact file.
Every row and every Git change inside their union must match. Changes outside
the boundary belong to another unit and remain untouched. Multiple units in the
same plan may share only that host plan path; use exact Pathspecs for the shared
plan, each unit's inventory and its project-local `docs/evidence/` record. Only
a `suite:` unit may use `Pathspec<TAB>.`.
All uncommitted `done` units sharing this plan form one atomic commit batch.
Stage them together and run
`python3 scripts/check-staged-units.py INVENTORY...`; its union must match the
index exactly in paths, numstat and SHA-256.
Use the current `HEAD` immediately before the unit as Base revision. Binary
numstat values are `-/-`, mode-only values are `0/0`, and binary rows contribute
zero to `+X/-Y`. Other content values are the final SHA-256 or `deleted`. Line numbers may
supplement but never replace stable areas. Recheck the staged form with
`git diff --cached --numstat --no-renames` when a commit is requested. Update
the ledger when scope changes and when a unit closes. This plan records intent;
it grants no authority.

The documentation guard compares every row and hash with Base -> current
worktree until commit. Once the inventory is tracked and clean, it uses the
commit that last changed that TSV as the immutable endpoint and requires Base
revision to be its direct parent. Code, plan, evidence and inventory therefore
land in one commit; later work does not rewrite or invalidate the earlier unit.
Once versioned, an inventory is immutable: never edit, move, rename or reuse it.
A correction or later delivery gets a new unit and a new inventory.

Before moving a completed plan to `archive/`, add `Closed: YYYY-MM-DD` and
`Successor: ID/link` or `none`; every ledger unit must already be `done`. Move
only the plan, keep the same basename, checkpoint, `Plan ID`, units and links,
and leave inventories and evidence in their stable roots. If archiving needs a
separate traceable commit, add an administrative unit with a new inventory
before moving the plan; never rewrite an existing inventory.
