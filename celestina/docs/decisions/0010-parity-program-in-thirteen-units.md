# ADR 0010: The parity program is delivered in thirteen units

- **Date:** 2026-10-16
- **Status:** accepted

## Context

The Celestina shell must take over what Noctalia does for the author's session
(bar, idle, caffeine, night light, wallpaper, OSD, media, clipboard, polkit,
lock, phone and the rest of the inventory). That is too much for one unit and
too coupled to ship as one change. The inventory and the parity matrix are in
[the parity program design](../../../docs/superpowers/specs/2026-10-16-celestina-parity-program-design.md),
§3 and §5.

## Decision

The program is delivered in thirteen units, A to M, in the order of the
design's §5. Each unit is a ledger row of
`celestina/docs/plans/active/2026-10-16-cel-2-parity.md`, lands through
`scripts/land-unit.py`, and queues its author checks as `VAL-CEL2-*` rows that
never block the unit. Noctalia keeps running until unit M, the last one, which
is the only point where the author stops it.

## Consequences

- Each unit can be validated live beside Noctalia, one duty at a time (ADR
  0011), so a regression is attributable to one unit.
- The order in the design's §5 is the order of the units' plans; it is not
  a claim about which units can run in parallel.
- The program opening (the ledger and the roadmap's active checkpoint) is a
  separate author decision and is not implied by this record.

## Revisit when

The author reorders or merges units, or a unit proves that its duty cannot be
switched on without a later one (then the decomposition, not this record, is
amended).
