# AGENTS.md — canonical agent contract for Celestina

This file is the vendor-neutral, mandatory entry point for every agent that
analyzes, changes, or reviews this monorepo. The closest `AGENTS.md` may add
local constraints; it never relaxes this root contract. Do not maintain
provider-specific copies.

## Repository language

The repository stores two kinds of text. **Development truth is English**:
rules, documentation, roadmaps, decisions, plans, evidence, identifiers, code
comments, diagnostics, test names, fixtures, protocol tokens, and commit
subjects. **Product copy is Spanish**: the words a person reads while using the
products, declared as the literal arguments of `qsTr()` in QML or the string
literals of a file whose head says `language-contract: product-copy`. A surface
is Spanish throughout; a half-translated screen is a defect. Other non-English
text is allowed only in explicit localization resources or fixtures that test
international input, and must be labelled as such. Historical material keeps
its original bytes until a dedicated migration translates it without changing
meaning. Agents speak to the author in Spanish unless the author requests
another language. See
[docs/standards/language.md](docs/standards/language.md) and
[ADR 0007](docs/decisions/0007-spanish-product-copy.md).

## Mandatory preflight

Before acting:

1. Run `python3 scripts/agent-context.py PATH` for the area you will touch and
   read every document it prints, completely. Its output is the reading order,
   not a summary: root and local `AGENTS.md`, the registered cross-cutting
   rules, the owner's `README.md`, `STATUS.md`, `ROADMAP.md`, `VALIDATION.md`,
   applicable contracts, and any active plan ledger.
2. Read the active plan ledger completely when one exists; it is the
   cross-session hand-off and it names the unit you may edit.
3. Inspect `git status`, the real checkout, consumers, manifests, and guards.
   Documentation may be stale.
4. Search first with `rg` or `rg --files`; reuse existing contracts instead of
   creating a parallel recipe.
5. Declare or update the ledger unit before editing planned milestone work.

Canonical sources are mapped in [docs/README.md](docs/README.md), the workflow
is in [CONTRIBUTING.md](CONTRIBUTING.md), and executable project metadata is in
[docs/projects.toml](docs/projects.toml).

## Authority and scope

- The author's request defines the objective and mutation level. An audit does
  not authorize a fix; a fix does not authorize an adjacent refactor.
- A README, roadmap, plan, discussion, decision, or evidence record never grants
  authority. It records state or intent.
- Preserve unrelated worktree changes. Do not revert, delete, reformat, or
  include them for convenience.
- Do not commit, push, activate a live surface, enable a service, or change
  anything outside the repository without an explicit request.
- Completing an authorized bug fix or milestone does authorize the registered
  deployment of verified bytes to the author's normal test prefix. It does not
  authorize replacing the live shell session.
- If scope must grow, update the ledger first and compare it with the request
  again. The complete policy is in
  [docs/governance/change-policy.md](docs/governance/change-policy.md).

## Required technical level

Act as an expert in Rust, modern C++, Qt 6, QML, and CXX-Qt. Before deciding,
verify ownership, lifetimes, thread affinity, signals/models, FFI, QML
registration, toolchain compatibility, errors, and consumers. If an API or
version is uncertain, inspect the code and primary documentation; do not guess.
See [docs/standards/rust-cpp-qt-qml.md](docs/standards/rust-cpp-qt-qml.md).

## Architecture direction

| Responsibility | Canonical destination |
|---|---|
| Pure domain, protocol, operations, and testable IO | `celestina-rs/` |
| Reusable Qt/C++ seam that CXX-Qt cannot cover | `fluorita-qt` or a future approved exception in `celestina-rs/` |
| Qt/D-Bus/XDG state and adaptation | application `src/` |
| Surface presentation | application `qml/` |
| Reusable tokens and controls | `celestina-style/` |
| Concrete CXX-Qt gap | application `cpp/` |
| Inter-process contract | stable, backward-compatible API |

Pure crates never depend on adapters or UI. One application never imports
another application's UI. Siderita may consume narrow Grafita and Fluorita
domain/seams while retaining its own Qt state and composition. Accepted details
and exceptions, including the bounded `fluorita-qt` bridge, live in
[docs/standards/architecture.md](docs/standards/architecture.md).

If a piece has no clear destination, stop and record the decision before
implementing it. Proximity to the open file does not determine ownership.

## Reuse and modularity

- Before adding logic, search the whole monorepo for the rule, operation,
  component, and contract. Every invariant has one owner; equivalent
  implementations are not synchronized by convention.
- Compare every second appearance of a recipe. Extract only the real semantic
  intersection or record why the concepts differ. Syntactic similarity alone
  does not justify abstraction, and small differences do not justify copying a
  domain rule.
- A refactor removes or delegates the old path in the same unit. Do not leave
  two active paths, boundary-free pass-through wrappers, domainless `utils`
  modules, or application flags that hide multiple behaviors in one API.
- Extract a component when it gains a named responsibility, an independent
  reason to change, its own state/lifecycle, distinct dependencies, or an
  independently testable boundary. Keep its API minimal, typed, domain-oriented,
  free of internal caller IDs, and acyclic.
- Do not fragment for appearance. Every extraction must improve ownership,
  dependency direction, or testability. A short file does not rescue a confused
  API, and a long file does not by itself prove a monolith.
- Pure domain may be extracted with one consumer when it already has a stable,
  tested boundary. An unspecified visual control stays local until two
  consumers prove the same semantics.
- Shared style QML uses the canonical path and explicit host registration;
  never copy it.
- A host coordinates; a coherent region owns a component, state, and lifecycle.
  A QObject or bridge does not accumulate independent domains. Run
  characterization tests before and boundary tests after a refactor.
- Line count is only a review signal. There is no boundary where 799 lines are
  correct and 801 are wrong. The baseline only prevents known legacy
  coordinators from growing until their debt is resolved; never raise it to
  silence CI or replace architectural judgment.

## Implementation invariants

### Rust, IO, and external contracts

- `unsafe` is forbidden unless a prior, isolated, documented exception exists.
- Do not add production `unwrap`, `expect`, or `panic!` unless the invariant is
  demonstrated at the use site. Do not hide debt with `#[allow]`, TODO, FIXME,
  or HACK.
- Typed errors retain context and source. Treat network, filesystem, D-Bus, and
  process input as hostile and bounded.
- Blocking IO never runs on the Qt thread. Bound or coalesce bursts; workers
  shut down deterministically and publish only current results.
- A write never removes the source before confirming the destination. A
  best-effort D-Bus failure degrades a feature instead of blocking or crashing
  the application.
- Published APIs and persisted data evolve compatibly. Every domain feature
  includes tests in the same unit.
- Justify each dependency in its manifest. Heavy runtimes or frameworks require
  approval.

### Qt, QML, and accessibility

- QML presents state; it does not open sockets, launch processes, or decide
  domain policy.
- Use typed properties, `required property`, signals, and narrow APIs. A
  component never reaches parent IDs, and `x: x` injection is forbidden.
- Register every new QML file through the project's build mechanism.
- Colors, typography, radii, control anatomy, opacity, and motion come from
  semantic `CelestinaTheme` tokens; do not hard-code QML colors.
- Every action works with keyboard and assistive technology. Custom controls
  expose role, name, state, and action; visible focus uses `visualFocus`.
- Dialogs contain and restore focus. New or changed motion honors
  `CelestinaTheme.reducedMotion`. Normal text reaches 4.5:1 contrast and large
  text 3:1.
- Manual C++ names the concrete CXX-Qt limitation that requires it, respects
  RAII and QObject affinity, and delegates pure testable logic to Rust.

## Documents and two delivery lanes

- `README.md`: current product, use, and structure.
- `STATUS.md`: volatile truth, focus, and blockers.
- `ROADMAP.md`: implementation and agent-executable evidence only.
- `VALIDATION.md`: author-only real-session, hardware, or perceptual tests.
- `docs/plans/active/`: execution order and durable change ledger.
- `docs/inventories/`: exact immutable inventories for closed units.
- `docs/version-history.tsv`: append-only product version history.
- `docs/decisions/`, `discussions/`, `evidence/`, and `history/`: decisions,
  debate, proof, and history respectively.

An implementation checkpoint closes when code, documentation, and automated
evidence are complete. Pending manual validation does not keep it open. A
manual failure is recorded and creates a new corrective unit; never rewrite the
milestone's history. See
[docs/governance/documentation.md](docs/governance/documentation.md).

Every plan has a ledger with ID, prefix, status, stable paths or symbols,
intent, diffstat, evidence, and related validation. Line numbers are secondary
because they drift; the ledger is the cross-session hand-off and commit source.

The roadmap-to-plan link is strict. An `active` or `blocked` `ROADMAP.md` names
one `Active implementation checkpoint` and has exactly one active plan owned by
the same project with that ID. For `planned`, `idle`, or `done`, the field is
`none`. Every plan declares a unique immutable `Plan ID` before its first
inventory commit; prefer the basename without its date prefix. Archiving never
changes it.

A local plan ledger accepts only its owner's primary prefix; `suite:` is for a
cross-suite plan. Component prefixes are for atomic code or manifest commits
that do not close a ledger unit. A `done` unit records exact `N files, +X/-Y`,
links one `.numstat.tsv` inventory from `Files / areas`, and links its real
record from `Automated evidence`.

Inventories live at
`<owner>/docs/inventories/<plan-slug>/<unit>.numstat.tsv`, or under root `docs/`
for suite work. They contain the base revision, every path, added/deleted lines,
one or more narrow `Pathspec` boundaries, the plan, evidence, and their own
`self` row. Only `suite:` may use `Pathspec<TAB>.`. Before commit, the guard
compares exhaustive paths, numstat, and SHA-256 inside those boundaries while
leaving external changes untouched. The base is the `HEAD` immediately before
the unit and later must be the direct parent of the single commit containing
change, plan, evidence, and inventory. Binary rows use `-/-`; mode-only rows use
`0/0`.

The main project prefix covers its tree, associated crates, and exact registered
manifests. A component prefix covers only component code and those manifests,
never plans, ledgers, status, or evidence. Both additionally cover
`commit_policy.shared_ratchet_files`, so a change that shrinks a guarded file
lowers its baseline row in the same commit instead of publishing a revision
whose relevant guard is red. Python rules committed in HEAD interpret source,
baselines and registry TOML from INDEX; staged or unstaged rule modules never
execute in the current hook. HEAD and INDEX must both authorize every normal
commit path and prefix, so staged policy cannot authorize its own expansion.
Delivery discovery uses the conservative union of HEAD and INDEX layouts and
rejects conflicting ownership. Merge commits cannot change ratchets and their
staged guarded sources must still match the INDEX baselines. A semantic rule
change becomes authority after landing: first add compatible dormant behavior,
then activate it with any measurement update in a later commit. Hooks are
repository-integrity controls, not an adversarial sandbox. A `lines`
row may disappear only when the same staged unit changes or deletes that source
and its evidence contains the exact field
``- **Resolved architecture debt:** `path/to/source` ``. A language row may fall
without its source only for a declared scanner migration, which needs both
`scripts/check-language-contract.py` staged and the exact field
``- **Resolved language debt:** `scripts/check-language-contract.py` `` in the
same unit's evidence. This is qualitative
architectural closure, not a line threshold. Local evidence lives under the
project's canonical `docs/evidence/`; suite evidence lives under root
`docs/evidence/`. A component prefix cannot create a nested lookalike evidence
directory to retire debt; use the owning project's primary prefix.

A tracked inventory is immutable: never edit, move, rename, or reuse it. Later
work gets a new unit and inventory. Archiving moves only the plan from
`plans/active/` to `plans/archive/` with the same basename, checkpoint, Plan ID,
units, and links. Evidence and inventories stay at stable roots. If the move
needs a separate traceable commit, first add an administrative unit with a new
inventory.

All uncommitted `done` units in one plan form one atomic batch under the owner's
primary prefix. Do not commit the shared plan while leaving another inventory
pending. `.githooks/pre-commit` discovers inventories and requires their union
to match staged paths, numstat, and SHA-256 through
`scripts/check-staged-units.py`.

## Production artifacts

Every project uses entries registered in `docs/projects.toml`:

1. `build-production.sh` builds the canonical release artifact once.
2. `verify-production.sh` verifies those same bytes while reusing caches.
3. `deploy-production.sh` copies only a current verified artifact to the
   author's normal test destination and never compiles.
4. `complete-production.sh` runs build, verify, deploy, and status. It is the
   exit condition for every bug fix or milestone that changes a deployable app,
   unless the author explicitly opts out.
5. The shell additionally has `activate-production.sh`; completion updates the
   on-disk bundle but never replaces a live session.

For a product `bug`, `milestone` or `release`, bump the registered SemVer source
and append its history row before this build. `maintenance` changes do neither.
See [docs/contracts/versioning.md](docs/contracts/versioning.md).

Do not use a parallel Cargo/CMake build as final evidence when it leaves a
different deployable binary. Do not run `clean`; production targets and caches
are reusable monorepo resources. See
[docs/contracts/production-artifacts.md](docs/contracts/production-artifacts.md).

## Minimum evidence

Run the common guard first:

```sh
bash scripts/check-architecture-contract.sh
```

Then use the affected project's registered `verify_script` and every guard for
a changed cross-cutting contract. A build proves compilation; a smoke proves
startup. Neither alone proves Wayland, compositor, portals, hardware,
interaction, appearance, or AT-SPI. Record exactly what ran and what remains in
`VALIDATION.md`.

## Git and commits

Celestina is one repository. Do not commit or push without a request. When the
author requests a commit:

1. select one coherent unit or the full batch of uncommitted `done` units in
   the same plan;
2. compare its paths with the index and exclude unrelated work;
3. separate projects unless the change is genuinely cross-suite;
4. keep the ledger's registered base prefix, choose the delivered change kind,
   and use an imperative English subject:
   `<prefix>-<bug|milestone|release|maintenance>: <action>`;
5. for a product bug, milestone or release, apply the exact PATCH, MINOR or
   MAJOR transition and append the matching `docs/version-history.tsv` row;
6. run `python3 scripts/version_tool.py check` and
   `python3 scripts/check-staged-units.py INVENTORY...`, then include code,
   version declarations, history, plan, inventories, and evidence in the same
   commit.

`.githooks/pre-commit` and `.githooks/commit-msg` verify the staged batch,
format, base scope, change kind, version transition, and the single
ledger-declared prefix. `suite:` never wraps incompatible local batches. Enable hooks in each clone with
`git config core.hooksPath .githooks`; Git does not transport that setting.
Merges cannot close inventoried delivery units: finish the merge, then deliver
the unit in an ordinary prefixed commit. Normal, revert and fixup subjects must
start their inner action with a recognized English imperative and pass a
conservative non-English prose detector. This is an integrity heuristic, not a
complete linguistic parser. Historical replay uses an explicit scope-only mode
and does not retrofit grammar onto old subjects.
