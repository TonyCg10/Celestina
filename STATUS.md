# Suite status

- **Updated:** 2026-08-03
- **Current focus:** suite governance is idle; project work follows each local
  roadmap
- **Implementation checkpoint:** none
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
