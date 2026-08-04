# CelestinaStyle implementation roadmap

- **Status:** planned
- **Active implementation checkpoint:** none
- **Related author validation:** `VAL-STYLE-01` through `VAL-STYLE-03` in
  [VALIDATION.md](VALIDATION.md); they do not block implementation

`STYLE-M1` is the next settled maintenance checkpoint and has no active
execution plan.

## STYLE-M1 — Stable and motion-complete public contract

## Hypothesis and tangible outcome

A finite public API with an enforced compatibility policy and an auditable
reduced-motion route can evolve without silently breaking consumers or leaving
legacy animation outside the accessibility contract. The tangible outcome is a
versioned QML surface whose inventories, aliases and motion checks fail on drift.

## Scope

- Write and enforce the compatibility/deprecation policy for public QML types,
  properties, roles and aliases.
- Inventory every current `Behavior`, `Transition` and spatial/scale animation;
  add or correct its `CelestinaTheme.reducedMotion` route.
- Add the narrowest scanner/tests that detect a newly unguarded motion path.
- Complete the mono-face and fallback policy without depending on an accidental
  host font.
- Reconcile public documentation with `qmldir`, CMake, QRC and real consumers.

## Exclusions

- Building speculative controls without demonstrated reusable demand.
- An installed module before an external consumer exists.
- App layout, product workflows or compositor protocol ownership.
- Manual appearance, blur, keyboard and AT-SPI acceptance.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| STYLE-M1-A | planned | none | Public compatibility/deprecation contract and inventory test | Registry/parity guard plus focused negative fixture |
| STYLE-M1-B | planned | STYLE-M1-A | Complete legacy-motion inventory and reduced route | Style guard, QML tests and negative fixture |
| STYLE-M1-C | planned | STYLE-M1-A | Explicit mono and fallback contract | Module lint plus affected consumer builds |
| STYLE-M1-D | planned | STYLE-M1-B, STYLE-M1-C | One verified production module and affected consumers deployed with it | Style production verify plus each affected deployable consumer's registered completion script |

## Implementation exit

Close `STYLE-M1` when public inventory drift and unguarded motion both fail
automatically, the font contract is explicit, and the canonical module passes
its `scripts/verify-production.sh` while every affected deployable consumer
passes its registered `complete-production.sh`, placing those exact bytes in the
author's normal test destination. Real compositor and assistive-technology
results remain independent validation rows.

## Closed evidence

The completed S1-S6 migration and later semantic component work are archived in
the [roadmap history](docs/history/roadmap-through-2026-08-03.md).
