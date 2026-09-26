# Suite status

- **Updated:** 2026-09-26
- **Current focus:** AUD-1, the monorepo hardening program that follows the
  2026-09-26 audit; project units of the same program follow each local
  roadmap
- **Implementation checkpoint:** AUD-1
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

AUD-1 carries the program of the 2026-09-26 monorepo audit: 184 findings
after de-duplication, 6 Critical, 78 Important and 100 Minor. The pure Rust
layer is strong; the problems sit at the edges: hostile input from files and
the network, the "never lose the source" promise, blocking IO on the Qt
thread, and a delivery pipeline whose CI has been red since 2026-09-25. The
suite plan carries the pipeline and cross-suite rows (`AUD-1-A` to
`AUD-1-F`); each affected project carries its own rows, in a new hardening
plan (Siderita, Hematita, Grafita, Fluorita, the Rust workspace) or as new
rows of its active plan (Celestina, CelestinaStyle, Magnetita, Magnetita
Android). Only documentation-only suite units land from a container without
Qt, libmpv or the Android SDK; every other unit is prepared on its own branch
and landed by the author in program order. The findings and rulings are in
[the audit evidence](docs/evidence/2026-09-26-monorepo-audit.md) and the
ledger in [the active plan](docs/plans/active/2026-09-26-monorepo-hardening.md).

## Completed cross-project work

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
| Reusable production artifacts | entries complete for all seven projects | production evidence |
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

No governance blocker is recorded. Product-specific blockers live only in
their project status documents.

## Evidence

The complete guard commands, prior seven-manifest verification, installed-state
audit, exact language-baseline movement and final runner invalidation are in
[the GOV-2 evidence](docs/evidence/2026-08-03-guard-contract-alignment.md). Its
final runner integration changed registered build scripts, so existing manifests are
intentionally stale. No release artifact was rebuilt or deployed. The earlier
verification pass compiled test/debug targets only and is not reported as real
Wayland, hardware or assistive-technology validation.
