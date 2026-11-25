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

## Halted projects

No project is halted now. The author halts a project by marking it `halted`
in `docs/projects.toml` and describing the halt in this section. While a
project is halted:

- No agent plans, audits, implements, refactors, adopts shared owners into,
  builds, verifies, deploys, or activates it, and no agent proposes work on it.
- Monorepo-wide programs, audits, counts, and landings do not include it. A
  shared change never extends to it and never rebuilds it.
- A request that touches it is answered by pointing at this section, and the
  work stops there.
- `scripts/agent-context.py` announces the halt, `scripts/commit_scope.py`
  refuses its prefixes and paths, and the landing drops it from every
  production run.

Only the author lifts, amends or ends a halt, by editing this section. The
commit guard accepts a change to a halted project only in a commit that also
changes this section. A halted project removed from the tree stays in the
registry under `[[retired_projects]]`, so its history rows and old commits
still resolve (see
[docs/contracts/versioning.md](docs/contracts/versioning.md), "Retiring a
project").

The halt of the Qt/QML shell, its removal and the salvage record that copied
files cite are in
[docs/history/qt-shell-halt-and-salvage.md](docs/history/qt-shell-halt-and-salvage.md).

## Mandatory preflight

Before acting:

1. Run `python3 scripts/agent-context.py PATH` for the area you will touch.
   Read every document under "Read now" completely.
2. Each document under "Read when" names its trigger. Read it completely
   when that trigger applies, before you act on that matter; not before.
   `--full` prints every document in the old reading order.
3. When you work on a planned unit, read its plan's header and your unit's
   ledger row; the ledger is the cross-session hand-off and names the unit
   you may edit.
4. Inspect `git status`, the real checkout, consumers, manifests, and guards.
   Documentation may be stale.
5. Search first with `rg` or `rg --files`; reuse existing contracts instead of
   creating a parallel recipe.
6. Declare or update the ledger unit before editing planned milestone work, in
   a session worktree opened with `scripts/worktree.sh`.

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
another application's UI. Siderita may consume narrow Grafita, Fluorita and
Hematita domain/seams while retaining its own Qt state and composition. Accepted details
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

## Visible changes: preview before building

A unit that changes anything a person sees (a screen, component, layout,
motion, icon, colour, typography or visible text) shows the author a preview
and waits for approval before its plan, if it has one, and its code. Light
units too. A fix that only restores an approved look needs no preview; its
evidence says so.

- The preview is faithful, not a sketch: values from `celestina-style`'s
  `tokens.toml`, its fonts and icons, `DESIGN.md`, real sizes, real text, the
  states that apply (for example hover, focus, pressed, disabled, empty, error
  and loading) and both themes. The current screen, when there is one, sits
  beside the proposal.
- Two or three variants side by side when there is a real choice, with a
  recommendation. Motion is shown moving, not described.
- When an HTML preview cannot show it faithfully (glass over the wallpaper,
  compositor blur, fractional output scaling), say so and add a throwaway
  prototype in the real stack, with screenshots from a nested compositor.
- Iterate on the preview until approved. The unit's design or plan, or for a
  light unit its evidence, links the approved preview and gives the date of
  the approval; screenshots go under `docs/evidence/`. The preview is the
  reference for the implementation and the final review.
- Approving a preview does not replace the real-session check in
  `VALIDATION.md`.

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

Ledger rows, inventories, commit prefixes, hooks and ratchets:
`docs/governance/documentation.md`, `docs/governance/change-policy.md` and
`docs/contracts/landing.md`, before closing a unit or changing a guard.

## Production artifacts

Every project uses entries registered in `docs/projects.toml`:
`build-production.sh`, `verify-production.sh`, `deploy-production.sh`,
`complete-production.sh` and, for the shell, `activate-production.sh`.
`complete-production.sh` is the exit condition for every bug fix or milestone
that changes a deployable app, unless the author explicitly opts out. The
landing runs these entries on the tree it commits. A session never runs them
in its worktree. Do not run `clean`. Builds, caches, pruning and version bumps:
[docs/contracts/production-artifacts.md](docs/contracts/production-artifacts.md)
and [docs/contracts/versioning.md](docs/contracts/versioning.md).

## Minimum evidence

Run the common guard first:

```sh
bash scripts/check-architecture-contract.sh
```

Then run the affected project's fast checks and every guard for a changed
cross-cutting contract; the landing runs the registered production entries on
the tree it commits. A build proves compilation; a smoke proves startup.
Neither alone proves Wayland, compositor, portals, hardware, interaction,
appearance, or AT-SPI. Record exactly what ran and what remains in
`VALIDATION.md`.

Size the unit first (light units, review cap):
[verification.md](docs/standards/verification.md).

## Git and commits

Celestina is one repository. Do not commit or push without a request. When the
author requests a commit:

1. select the one unit its session branch carries; a landing lands one unit
   per commit;
2. keep unrelated work off that branch, because the landing commits the
   branch's whole diff against `origin/main`;
3. separate projects unless the change is genuinely cross-suite;
4. keep the ledger's registered base prefix, choose the delivered change kind,
   and use an imperative English subject:
   `<prefix>-<bug|milestone|release|maintenance>: <action>`;
5. leave the version alone: for a product bug, milestone or release, the
   landing applies the exact PATCH, MINOR or MAJOR transition and appends the
   matching `docs/version-history.tsv` row;
6. run `python3 scripts/land-unit.py <branch> --kind <kind>` from the
   canonical checkout, which runs the version, staged-inventory and
   commit-scope guards itself.

`.githooks/pre-commit` and `.githooks/commit-msg` verify the staged batch,
format, base scope, change kind, version transition, and the single
ledger-declared prefix; `.githooks/pre-merge-commit` runs the language and
documentation contracts over an automatic merge. `suite:` never wraps incompatible local batches. Enable hooks in each clone with
`git config core.hooksPath .githooks`; Git does not transport that setting.
Merges cannot close inventoried delivery units: finish the merge, then deliver
the unit in an ordinary prefixed commit. Normal, revert and fixup subjects must
start their inner action with a recognized English imperative and pass a
conservative non-English prose detector. This is an integrity heuristic, not a
complete linguistic parser. Historical replay uses an explicit scope-only mode
and does not retrofit grammar onto old subjects.
