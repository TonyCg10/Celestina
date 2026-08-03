# Suite implementation plans

Plans translate settled decisions into executable work. They do not grant
authority and do not host open design debate.

- [active/](active/) contains current settled implementation plans.
- [archive/](archive/) preserves closed plans.

Every active plan contains a hypothesis, tangible result, scope, exclusions,
causal build order, implementation exit, and change/commit ledger. Manual
validation appears only as an ID linked to `VALIDATION.md`.

Inventories live outside the movable plan tree under
`docs/inventories/<plan-slug>/<unit>.numstat.tsv`, or the equivalent owner-local
root. They are immutable and never reused.

When all units close, move only the plan to `archive/` without changing its
basename, Plan ID, checkpoint, units, or links. Evidence and inventories stay at
their stable roots.
