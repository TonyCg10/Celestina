# Contributing to Celestina

This monorepo is one Git history and several products with independent
boundaries. These rules apply equally to people and agents.

## Minimum context

Before proposing, changing, or reviewing:

1. Read the root and nearest local `AGENTS.md`.
2. Read the applicable `README.md`, `STATUS.md`, `ROADMAP.md`, and
   `VALIDATION.md`.
3. Read the active plan ledger when one exists.
4. Inspect `git status`, consumers, manifests, and guards in the checkout.
5. Search for equivalent contracts with `rg` or `rg --files` before creating
   another.

The source map is [docs/README.md](docs/README.md), the project registry is
[docs/projects.toml](docs/projects.toml), and repository language is defined by
[docs/standards/language.md](docs/standards/language.md).

## Language and technical level

Repository content is English. Agents speak to the author in Spanish unless
asked otherwise. Work with expert Rust, modern C++, Qt 6, QML, and CXX-Qt
judgment: verify ownership, lifecycle, thread affinity, Qt models/signals, QML
registration, FFI, errors, consumers, and toolchain before deciding.

## Authority and scope

- An audit authorizes reading and diagnosis only.
- A fix authorizes the minimum correction, tests, documentation, and canonical
  deployment of the affected app.
- A plan or document never expands authority granted by the author and
  applicable `AGENTS.md` files.
- Local `AGENTS.md` files only add restrictions.
- Verify does not install or activate. Completing a bug or milestone deploys
  verified bytes through `complete-production.sh`; replacing the live shell or
  another live-session mutation still requires an explicit request.

See [change-policy.md](docs/governance/change-policy.md).

## Two delivery lanes

`ROADMAP.md` contains implementation, required documentation, and
agent-executable evidence. Implementation closes when its automated exit is
satisfied; a pending manual check does not keep it open.

Author-run checks live in `VALIDATION.md` with independent IDs and states. A
manual failure creates a linked remediation unit; it never inserts code work
into the validation queue or rewrites the closed milestone.

## Plans and monorepo ledger

Every active plan has a change ledger. Before editing, its active unit declares:

- commit prefix;
- stable paths, areas, or symbols;
- provisional diffstat placeholder;
- intended change;
- expected automated evidence;
- related author validation, if any.

Update the ledger when scope or status changes. Line numbers may supplement but
never replace stable locations. The ledger is the cross-session hand-off.

An `active` or `blocked` roadmap names exactly one `Active implementation
checkpoint` and has one active plan with the same owner and checkpoint. For
`planned`, `idle`, or `done`, the field is `none`. A plan without that link is
orphaned. Each plan has a unique immutable `Plan ID` before its first inventory
commit; prefer a durable basename without the date prefix.

Local plan units use only their owner's primary prefix. Component prefixes are
for atomic code/manifest commits that do not close a ledger unit. Only suite
plans use `suite:`.

## Closing a unit

Before setting a unit to `done`:

1. Run its complete agent-executable exit.
2. Calculate tracked paths with `git diff --numstat --no-renames` and each new
   path against `/dev/null`.
3. Record exact `N files, +X/-Y`.
4. Replace `Files / areas` with one link to
   `<owner>/docs/inventories/<plan-slug>/<unit>.numstat.tsv`, or root `docs/`
   for suite work.
5. Link the dated evidence record under the owner's `docs/evidence/`.

The inventory contains:

- `Base revision<TAB><40 hex>` using the `HEAD` immediately before the unit;
- one or more narrow `Pathspec<TAB>path` boundaries;
- `added<TAB>deleted<TAB>content<TAB>path` rows for every changed path;
- the plan, evidence record, and its own `self` row;
- final SHA-256 values, `deleted`, `-/-` for binary numstat, and `0/0` for
  mode-only changes.

Every row and every Git change inside the Pathspec union must match. Only a
`suite:` unit may use `Pathspec<TAB>.`. Boundaries must fit the registered
prefix. The primary project prefix includes its tree, associated crates, and
exact workspace manifests. A component prefix includes only component roots and
those manifests, never plans or evidence.

Before commit, `scripts/check-staged-units.py` compares staged paths, numstat,
and SHA-256 with the inventory union. After commit, the inventory's base must be
the direct parent of the single commit containing code, plan, evidence, and
inventory.

Tracked inventories are immutable. Never edit, move, rename, recalculate, or
reuse one. Later corrections get new units and inventories. Units in one plan
may share only their host plan path; each keeps separate exact Pathspecs,
inventory, and evidence.

All uncommitted `done` units in one plan form one atomic batch. Do not commit the
shared plan with only part of that batch.

## Archiving

Archive a completed plan by moving only it from `plans/active/` to
`plans/archive/` without changing basename, checkpoint, Plan ID, units, or
links. Evidence and inventories stay at stable roots. If the move needs a
separate traceable commit, first add an administrative ledger unit and new
inventory; never rewrite an old one.

## Build, verify, and deploy

Each buildable project registers separate entries for:

- one canonical production build;
- verification of those exact bytes without installation/activation;
- deployment of the current verified artifact;
- installed-byte status;
- for deployable apps, one `complete-production.sh` that chains the exit.

Do not clean caches or rebuild release between verify and deploy. Shell
completion updates its bundle but activation remains separate. See
[production-artifacts.md](docs/contracts/production-artifacts.md).

## Git and commits

Do not commit or push without an explicit request. When requested:

1. choose one unit or the atomic batch of uncommitted `done` units in its plan;
2. compare inventory paths with the index and exclude unrelated work;
3. separate projects unless the unit is genuinely cross-suite;
4. use the single ledger prefix and an imperative English subject;
5. run staged inventory and commit-scope guards;
6. include code, plan, inventory, and evidence in the same commit.

Finish merges before closing delivery units. Enable local hooks with
`git config core.hooksPath .githooks`.
