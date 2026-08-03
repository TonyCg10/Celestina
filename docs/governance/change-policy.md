# Change and authorization policy

## Repository precedence

1. The author's explicit request defines objective and mutation level.
2. Root `AGENTS.md` defines suite invariants.
3. The nearest `AGENTS.md` adds sub-tree restrictions.
4. Standards, contracts, and accepted decisions explain an authorized change.
5. Status, roadmaps, and plans describe state and work; they grant no authority.

No repository file overrides a higher-precedence instruction. A plan records
intent and never authorizes its own scope.

## Request boundaries

| Request | Authorized work |
|---|---|
| Explain, audit, or review | reading, diagnosis, evidence-backed report |
| Fix a defect | minimum fix, tests, documentation, affected app completion |
| Implement a milestone | authorized milestone scope, tests, affected app completion |
| Install, deploy, or activate separately | only the explicitly requested destination and time |
| Commit or push | only on explicit request and after scope verification |

A request does not authorize adjacent refactors, dependencies, service changes,
or cleanup of unrelated work.

## Mandatory preflight

Before changing anything:

1. Read applicable instructions and active documents.
2. Inspect `git status` and separate pre-existing changes.
3. Search consumers, guards, manifests, and equivalent contracts.
4. Locate or create an authorized ledger unit.
5. Confirm that declared scope fits the request.

If a piece has no clear architecture destination, record the decision before
implementation. An ADR explains a decision but does not grant permission.

## Scope and unrelated work

- Preserve unrelated changes, committed or not.
- Do not revert or reformat outside the unit.
- Never raise baselines, allowlists, or limits to silence a guard.
- Separate mechanical and behavioral changes when independently verifiable.
- Update the ledger before expanding scope.

## Deployment authority

Fixing a bug or implementing a milestone permanently authorizes deploying the
verified bytes of each affected app to the author's normal test prefix and
checking them without a second build. It does not authorize replacing the live
shell, enabling inactive services, changing desktop defaults, touching hardware,
or choosing another prefix. Magnetita may restart its daemon only when already
active and atomic deployment requires it. Audits and docs-only work deploy
nothing. See [production artifacts](../contracts/production-artifacts.md).

Commit and push remain explicit. Destructive history/worktree operations are
never implicit migration steps.

## Ledger fields

Every active plan contains:

| Field | Contract |
|---|---|
| Unit | stable unique ID within the plan |
| Commit prefix | plan owner's primary prefix |
| Status | `planned`, `active`, `blocked`, or `done` |
| Files / areas | stable paths/symbols while open; one exact inventory link when done |
| Diffstat | placeholder while open; exact `N files, +X/-Y` when done |
| Intended change | one cohesive reviewable result |
| Automated evidence | expected command while open; evidence link when done |
| Author validation | validation ID/link or `None` |

Line numbers are secondary because they drift. Update the row at scope changes
and closure; future agents rely on it instead of conversation context.

## Plan identity and roadmap link

Every plan declares a unique immutable `Plan ID` within its owner before the
first inventory commit. Prefer a stable basename without the date prefix.

An `active` or `blocked` roadmap names exactly one active implementation
checkpoint and exactly one active plan of the same owner names that checkpoint.
For `planned`, `idle`, or `done`, the field is `none`. Orphan plans are invalid.
Suite ledgers use `suite:`; local ledgers use their owner's primary prefix.

## Exact immutable inventories

A `done` unit has one exclusive inventory at
`<owner>/docs/inventories/<plan-slug>/<unit>.numstat.tsv`, or root `docs/` for
suite work. It records one or more narrow Pathspecs and every changed path with
no-rename numstat plus final content identity.

The format contains:

```text
Base revision<TAB><40 hex>
Pathspec<TAB><exact file or directory/>
...
added<TAB>deleted<TAB>content<TAB>path
```

Use SHA-256 for final bytes, `deleted` for removed files, `self` for the
inventory row, `-/-` for binary numstat, and `0/0` for mode-only changes. Binary
rows contribute zero to aggregate line counts.

The inventory includes its plan, evidence, and itself. Before commit, its union
must match staged paths, numstat, and SHA-256 exactly. The base is the `HEAD`
immediately before the unit. Once tracked, the commit that introduced the
inventory must use that base as direct parent and contain the whole unit.

Tracked inventories are immutable. Never edit, move, rename, recalculate, or
reuse one. A correction or later delivery gets a new unit and inventory.

## Multiple units and batches

Units in one plan may overlap only on the host plan file; their inventory and
evidence Pathspecs remain exclusive. All uncommitted `done` units in one plan
form one atomic batch under the owner's primary prefix. The shared plan cannot
land with only part of the batch.

Several projects require several local plans/commits unless one authorized
suite unit genuinely owns the cross-suite invariant. `suite:` is not a wrapper
for unrelated changes.

## Commit enforcement

Before commit:

- the index equals the inventory union;
- all linked inventories and evidence are staged;
- one registered prefix covers every staged path;
- the subject is `<prefix>: <English imperative>`;
- partial staging does not change inventory truth;
- merges finish before delivery units close.

`.githooks/pre-commit`, `.githooks/commit-msg`,
`scripts/check-staged-units.py`, and `scripts/commit_scope.py` enforce these
rules locally and in CI where applicable.

## Archive transition

Archiving moves only the plan from `plans/active/` to `plans/archive/` with the
same basename, checkpoint, Plan ID, unit rows, and links. Evidence and inventory
roots do not move. If archive movement is not part of the already inventoried
final commit, add a new administrative unit and inventory before moving it.
