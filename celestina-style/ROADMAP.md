# CelestinaStyle implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** STYLE-G7
- **Related author validation:** `VAL-STYLE-01` through `VAL-STYLE-03` in
  [VALIDATION.md](VALIDATION.md); they do not block implementation

`STYLE-M1` remains the next settled maintenance checkpoint after `STYLE-G7`
and has no active execution plan.

## STYLE-G7 — Shared reading controls and demonstrated visual primitives

The falsifiable problem: two applications need the same two reading controls —
a scroll position and a line-number column — and the sharing contract admits a
control only once a second consumer proves the same semantics. Grafita built
both locally; Siderita's editor and quick look are that second consumer. Copying
them would leave the suite with two owners of one anatomy.

The tangible outcome is `CelestinaScrollBar` and `CelestinaLineGutter` in the
canonical module, registered in `qmldir` and the QML module, built from semantic
`CelestinaTheme` tokens rather than from a re-skinned `QtQuick.Controls`
template, and consumed through the canonical path by both applications.

Later demonstrated consumers extend the same active checkpoint without moving
application policy into Style. Celestina's compositor-backed contextual
sections require `GlassSurface` to render the canonical material over an
external backdrop while the shell retains protocol and region ownership. Its
reference-backed follow-up adds opt-in semantic roles: a dense matte
`ContentSurface` shared by menu cards and panel capsules, and a nearly
transparent `ContextualVeil` for the menu carrier. Existing consumers retain
the compatible default material.

The plan is
[Shared reading controls](docs/plans/active/2026-08-04-shared-reading-controls.md).
It excludes any further gutter content — diff or breakpoint markers, folding, a
minimap — and any keyboard of the scroll bar's own, because the surfaces it
reports on already reach every position they can.

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
