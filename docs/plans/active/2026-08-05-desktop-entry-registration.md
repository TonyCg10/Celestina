# PRD-1 — the shell's desktop entry as a registered artifact

- **Opened:** 2026-08-05
- **Plan ID:** desktop-entry-registration
- **Status:** active
- **Scope:** suite
- **Implementation checkpoint:** PRD-1
- **Author-validation checkpoint:** `VAL-SHELL-03` in
  [`../../../celestina/VALIDATION.md`](../../../celestina/VALIDATION.md)

## Hypothesis

A file the shell deploys is a file the manifest seals. The registry is the only
place that decides which those are, so registering `celestina.desktop` there is
what makes deploying it legitimate rather than a script copying bytes nobody
verified.

## Tangible outcome

`celestina/celestina.desktop` is a production input and a sealed artifact of the
`celestina` project, so its digest is recorded by the manifest, deploy copies
only what verification sealed, and both `status-production.sh` and
`activate-production.sh` can report on the installed copy.

## Scope

The registry entry and this plan's evidence and inventory. The preceding
language-plan archive is owned by its own administrative `LNG-1-B` unit in the
same suite batch.

## Exclusions

The desktop entry's own content, its deployment and its installed checks belong
to Celestina and land under `celestina:` — this unit registers, it does not
implement. No other project's registration is touched.

## Build order

1. Let `LNG-1-B` archive the delivered previous suite checkpoint through its
   own exact inventory, so the roadmap names exactly one active checkpoint.
2. Register the desktop entry as a production input and artifact.

## Implementation exit

```sh
python3 scripts/check-documentation-contract.sh
bash scripts/check-architecture-contract.sh
python3 scripts/version_tool.py check
python3 scripts/check-staged-units.py
```

No product version moves: registering a file ships no product behaviour.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| PRD-1-A | `suite:` | done | [inventory](../../inventories/2026-08-05-desktop-entry-registration/PRD-1-A.numstat.tsv) | 4 files, +141/-2 | Register the shell's desktop entry as a production input and sealed artifact after `LNG-1-B` closes the preceding suite checkpoint | [evidence](../../evidence/2026-08-05-desktop-entry-registration.md) | `VAL-SHELL-03` |

## Paired suite transition

The root roadmap may name one active implementation checkpoint, and exactly one
active plan may name it. LNG-1's product unit was delivered in `8008056`, but
its plan stayed in `active/`. Its own administrative `LNG-1-B` unit therefore
owns the archive transition, evidence and exact inventory. `PRD-1-A` shares the
suite commit and prefix without claiming those paths.

## Delivery order

This unit must land **before** Celestina's `LVR-1-A`. Until the registry names
the desktop entry, `deploy-production.sh` would be copying bytes the manifest
does not seal, which is what the production-artifact contract exists to prevent.
