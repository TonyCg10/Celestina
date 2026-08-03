# Architecture standard

## Responsibility boundaries

| Responsibility | Canonical destination | Boundary |
|---|---|---|
| Pure domain, operations, protocol, and testable IO | core/ops/engine/net crates in `celestina-rs/` | no Qt, QML, or visual composition |
| Shared Qt/C++ seam CXX-Qt cannot express | `fluorita-qt` or a future approved exception in `celestina-rs/` | no domain, QML, or app composition |
| UI state and Qt/D-Bus/XDG adaptation | application `src/` | no extractable domain rules |
| Surface presentation | application `qml/` | no IO or domain decisions |
| Common visual tokens and controls | `celestina-style/` | no application state or workflow |
| App-local CXX-Qt gap | application `cpp/` | no logic Rust/CXX-Qt can cover |
| Inter-process contract | stable backward-compatible API | no internal QML IDs or types |

Dependencies point from presentation and adapters toward pure contracts, never
in reverse. One application does not import another application's UI.
Integrated consumers share narrow domain and seams while retaining their own Qt
state and QML composition. [ADR 0005](../decisions/0005-bounded-qt-bridge-crates.md)
bounds the real Qt exception: it exists only for a stable shared host seam or a
CXX-Qt gap and never turns `celestina-rs` into presentation. The historical
name `siderita-qt` is not such an exception; it is safe Rust with opaque view
contracts and no Qt linkage.

## Decide location before writing

A new piece needs a named responsibility, owner, consumers, and lifecycle. If
it does not fit the table, stop and record a decision. Proximity to the open
file does not determine ownership.

Search the whole monorepo before creating an abstraction. A second appearance
requires a semantic comparison: extract the real intersection or document why
the concepts differ. Do not add application flags to simulate reuse.

## Cohesion, extraction, and refactoring

Size finds surfaces worth reviewing; it does not approve architecture. There is
no universal line limit. Extract when a real boundary exists even in a small
file, and do not split a cohesive unit just to lower a counter.

Every extraction must satisfy all of these:

1. The responsibility can be named without “misc”, “utils”, “manager”, or a
   list of unrelated domains.
2. It has a distinct reason to change, lifecycle/state, dependencies, or test
   boundary.
3. Its API is minimal and typed; it owns its invariants and does not reach into
   caller-private IDs or state.
4. Dependencies follow the canonical direction and remain acyclic.
5. The old path disappears or delegates to the single owner; the rule is not
   implemented twice.
6. Characterization tests protect prior behavior and boundary tests prove the
   extraction.

A host coordinates; a coherent region owns a component. A QObject does not
accumulate independent domains. A CXX-Qt bridge declares a contract and
delegates instead of hosting logic. Every new QML file is registered through
the project build.

Reject refactors that merely move lines, create pass-through wrappers, rename a
monolith to `Manager`, add consumer mode flags, or leave two active paths. The
legacy coordinators in `scripts/architecture-baseline.tsv` may not grow. That
file is a ratchet for concrete debt, not a size standard for new code. Raising
it requires author approval, justification, and a removal condition.

## Reuse and non-redundancy

Search by responsibility, not only name. Every domain rule, transformation,
protocol decision, and shared control has one canonical owner.

- Compare contract and consumers at the second appearance; extract the real
  intersection or explain the semantic distinction in evidence.
- Do not copy validation, conversion, state, or domain decisions between QML,
  adapters, and crates. Layers delegate to the correct owner.
- A shared abstraction does not accept application branches or booleans to
  pretend several behaviors are one.
- Visual similarity alone does not imply a shared component. Equivalent
  behavior and interaction require evaluating one.
- Pure domain may be extracted with one consumer when its boundary is stable
  and tested. A new visual control remains local until two consumers prove the
  same semantics.

## Sharing and assets

`celestina-style` is the single style source. Every host declares its supported
consumption path in the build, such as a relative link or source import, and
guards verify it. Copying or renaming shared QML is forbidden.

## External contracts

- Evolve D-Bus and persisted formats backward-compatibly.
- A best-effort integration failure degrades the feature instead of blocking UI.
- Treat network and filesystem input as hostile, bounded, and sanitized.
- Blocking IO never runs on the Qt thread.
- Workers publish only current results and shut down deterministically.
- A write never destroys the source before confirming the destination.

Cross-cutting interaction contracts live in [../contracts/](../contracts/) and
change together with a decision, consumers, and tests in one unit.
