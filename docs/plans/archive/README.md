# Archived suite plans

Closed plans preserve scope, ledger, evidence, and outcome. They are not active
backlog or authority. A superseded plan remains active until every unit is moved
or closed and is archived only when all units are done.

Archiving preserves the exact basename, checkpoint, Plan ID, units, and links:

1. close every implementation unit;
2. record `Closed` and `Successor` (`none` when absent);
3. keep validation IDs independent;
4. move only the plan; inventories remain immutable under
   `docs/inventories/<plan-slug>/` and evidence remains at its stable root.

If archive movement requires a separate commit, add an administrative unit and
new inventory before the move.
