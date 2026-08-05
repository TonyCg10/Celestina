# Celestina implementation plans

Active plans turn a settled roadmap milestone into causal implementation units
and a persistent commit ledger. They record execution; they do not authorize
repository, package or live-session changes.

- Active: none
- Archived: [R3 session verbs](archive/2026-08-03-r3-session-verbs.md),
  [R4 notifications](archive/2026-08-04-r4-notifications.md)

Each unit stores its immutable inventory under
[`../inventories/<plan-slug>/<unit>.numstat.tsv`](../inventories/), outside both
plan-state directories. Completed plans move alone to [`archive/`](archive/)
with the same basename, checkpoint, `Plan ID`, units and links. A separate
archive commit requires a new administrative unit and inventory. Pre-migration
phase work orders remain separately under [`../history/`](../history/) because
they predate the plan lifecycle and ledger contract.
