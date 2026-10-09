# Suite status

- **Updated:** 2026-10-09
- **Current focus:** EXT-1, reading and capture: Calcita, the PDF viewer,
  and Selenita, the screen capture and recording tool
  ([plan](docs/plans/active/2026-10-09-reading-and-capture.md))
- **Implementation checkpoint:** EXT-1
- **Author-validation checkpoint:** VAL-GOV-1

## Completed governance migration

GOV-1 replaced the mixed README/ROADMAP/agent notes with one neutral
documentation system and separate homes for current status, implementation,
author validation, decisions, discussions, evidence and history.

The completed execution plan and persistent commit inventory are in
[docs/plans/archive/2026-08-03-repository-governance.md](docs/plans/archive/2026-08-03-repository-governance.md).
Manual acceptance is tracked independently in [VALIDATION.md](VALIDATION.md).

## Completed governance alignment

GOV-2 closed the gaps an audit of GOV-1 found between the written contract and
the tools: the guard chain emitted Spanish the language guard could not see, the
architecture ratchet could not be lowered in the commit that earned the
reduction, `agent-context.py` omitted the standards local contracts require,
product changes had no durable commit-kind/SemVer link, and several documents
described commands and CI jobs the checkout does not have.
Its unit, exclusions and exit are in
[the archived GOV-2 plan](docs/plans/archive/2026-08-03-guard-contract-alignment.md).

Four items were deliberately left out because they change how the author works
and need an accepted decision first: hardening the language detector, requiring
inventories for project-prefixed source commits, defining proportionality for
`complete-production.sh` across shared-crate consumers, and collapsing the
ledger rules currently written in five documents.

## Active cross-project work

EXT-1 adds two applications in the suite's grammar: Calcita, the PDF viewer,
and Selenita, the screen capture and recording tool, each registered by a suite
unit and built by its own ledger. The suite ledger is
[the active plan](docs/plans/active/2026-10-09-reading-and-capture.md).

## Completed cross-project work

CONV-1 makes the first-party applications behave as one system: one
claim-first activation interface in `celestina-core` served by every
application, one appearance file, open-with inside the suite and
drag-and-drop between the applications. The ledger is
[the archived plan](docs/plans/archive/2026-10-09-suite-conventions.md).

AUD-1 carries the program of the 2026-09-26 monorepo audit: 184 findings
after de-duplication, 6 Critical, 78 Important and 100 Minor. The pure Rust
layer is strong; the problems sit at the edges: hostile input from files and
the network, the "never lose the source" promise, blocking IO on the Qt
thread, and a delivery pipeline whose CI has been red since 2026-09-25. The
suite plan carries the pipeline and cross-suite rows (`AUD-1-A` to
`AUD-1-F`); each affected project carries its own rows, in a new hardening
plan (Siderita, Hematita, Grafita, Fluorita, the Rust workspace) or as new
rows of its active plan (CelestinaStyle, Magnetita, Magnetita Android; the
Celestina shell's rows were withdrawn when the author halted the shell on
2026-09-27, see [Halted projects](AGENTS.md#halted-projects)). Only documentation-only suite units land from a container without
Qt, libmpv or the Android SDK; every other unit is prepared on its own branch
and landed by the author in program order. The findings and rulings are in
[the audit evidence](docs/evidence/2026-09-26-monorepo-audit.md) and the
ledger in [the archived plan](docs/plans/archive/2026-09-26-monorepo-hardening.md).

LND-1 moves the closure of a unit (inventory, ledger closure, version bump and
production build) from the session to a landing step that runs on the current
`main`, so sessions prepared in parallel worktrees no longer wait for each other
or reseal by hand. The build order, exclusions and ledger are in
[the archived plan](docs/plans/archive/2026-09-25-seal-at-landing.md).

LNG-1 makes product copy Spanish and leaves development truth English. The
standard listed "canonical UI copy" among the things English governs, and that
was enforced, so a Spanish desktop acquired an English media library. The
reasoning is in
[ADR 0007](docs/decisions/0007-spanish-product-copy.md); the unit is in the
[archived plan](docs/plans/archive/2026-08-04-spanish-product-copy.md).

ACT-1 amends the one activation-contract bullet that named Gallery and Music as
fixed standalone surfaces. The author specified the standalone library as a
sidebar of the folders they mapped, and the shipped two-tab surface contradicts
that; the projections themselves are unchanged and the embedded Siderita
surface is explicitly untouched. The reasoning is in
[ADR 0006](docs/decisions/0006-source-first-library-navigation.md) and the unit
is in the
[archived plan](docs/plans/archive/2026-08-04-source-first-library-navigation.md).

## Current truth boundary

The checkout and reproducible verification are authoritative for implemented
behaviour. Root and project documents now follow the registered taxonomy;
pre-migration detail is explicitly historical and is not current instruction.

## Delivery state

| Area | State | Canonical reference |
|---|---|---|
| Governance foundation | complete | archived GOV-1 plan |
| Project document migration | complete | root/project canonical documents and histories |
| Reusable production artifacts | entries complete for every project registered in [docs/projects.toml](docs/projects.toml); the Celestina shell's are halted since 2026-09-27 and no landing runs them | production evidence |
| Registry-backed guards and commit policy | complete | production/documentation/architecture/hook fixtures |
| Product version convention | complete | registered sources, typed commits and append-only history |
| Repository language | canonical rules and current guard success output are English; legacy diagnostics and code/UI debt remain ratcheted | language standard and guard |
| Vendor-specific bootstrap removal | complete | documentation inventory guard |

Artifact currency is not a status claim and is never recorded here: it changes
whenever a registered input or guard changes. Reproduce it instead, per project:

```sh
PROJECT/scripts/status-production.sh
```

Legacy language debt is likewise reproduced, not transcribed:

```sh
python3 scripts/check-language-contract.py
```

## Blockers

No governance blocker is recorded. The Celestina shell is halted by the
author's decision of 2026-09-27; that is a decision, not a blocker, and it is
recorded in [Halted projects](AGENTS.md#halted-projects). Product-specific blockers live only in
their project status documents.

## Evidence

The evidence of the active program is in
[the audit evidence](docs/evidence/2026-09-26-monorepo-audit.md) and in the
records its ledger rows link; the tooling unit `AUD-1-F` is in
[the tooling-integrity evidence](docs/evidence/2026-09-26-tooling-integrity.md).
Whether each artifact is current is reproduced with `status-production.sh`, as
above, and never transcribed here. The earlier GOV-2 record, including its
seven-manifest verification of that time, stays in
[the GOV-2 evidence](docs/evidence/2026-08-03-guard-contract-alignment.md) as
history; it is not the current state of any artifact.
