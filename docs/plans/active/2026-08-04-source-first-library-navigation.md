# Source-first library navigation contract

- **Opened:** 2026-08-04
- **Plan ID:** source-first-library-navigation
- **Status:** active
- **Scope:** suite
- **Implementation checkpoint:** ACT-1
- **Author-validation checkpoint:** none

## Hypothesis

The activation contract can name the configured media source as the standalone
library's top-level axis without weakening any invariant it already enforces,
because the `Space` versus double-click mapping, the browsing-starts-no-decoder
rule and the minimal embedded surface are all independent of which axis the
standalone library is entered through.

## Tangible outcome

`docs/contracts/content-activation.md` describes Gallery and Music as catalogue
projections selected by a source's kinds rather than as two fixed standalone
surfaces, and an accepted ADR records why. Fluorita's implementation plan can
then be authorized against a contract it does not contradict.

## Scope

- ADR 0006 and its index row.
- The single behavioural-invariant bullet in the activation contract that names
  the standalone surfaces.
- The root roadmap checkpoint and status entry this plan is linked from.

## Exclusions

- Every Fluorita code, QML, crate and document change. Those belong to the
  Fluorita-owned plan and its own prefix; this unit changes no product
  behaviour and bumps no product version.
- The `Space` versus double-click mapping, the directory and unsupported-file
  rows, and the embedded-surface boundary. They are reaffirmed, not edited.
- Siderita and Grafita behaviour of any kind.

## Build order

1. Record ADR 0006 with the context, the ruling and the boundary it does not
   extend to the embedded surface.
2. Amend the one contract bullet and link the ADR.
3. Link the checkpoint from the root roadmap and status.

## Implementation exit

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-documentation-contract.py
python3 scripts/check-language-contract.py
```

The documentation guard is the check that matters here: it verifies the ADR
index, the roadmap-to-plan link, local link validity and, at closure, the
inventory union. No product version moves, so `version_tool.py check` must
report the same declarations before and after.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| ACT-1-A | `suite:` | done | [inventory](../../inventories/2026-08-04-source-first-library-navigation/ACT-1-A.numstat.tsv) | 8 files, +260/-9 | Make the activation contract describe source-first standalone navigation and record the accepted decision behind it | [evidence](../../evidence/2026-08-04-source-first-library-navigation.md) | None |

This unit is documentation only and closes as `suite-maintenance`. It delivers
no product behaviour and appends no version-history row.
