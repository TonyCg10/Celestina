# Active Celestina plans

The active shell implementation plan is
[R8 — polkit authentication agent](2026-08-14-polkit-authentication-agent.md),
authorized and bounded by
[ADR 0005](../../decisions/0005-first-party-polkit-agent.md). The first-party
session lock closed on 2026-08-14 on its implementation exit, with `VAL-R6`
deliberately unclaimed: the lock is built and tested, and nobody has yet
unlocked their own machine with it.

Plans that are authorized but whose checkpoint has not opened wait under
[`../pending/`](../pending/).
Completed plans remain under [`../archive/`](../archive/).
Each plan owns a persistent change ledger and remains separate from author-only
validation.

Unit inventories live under
[`../../inventories/<plan-slug>/<unit>.numstat.tsv`](../../inventories/), not in
this directory, and remain there when only the plan is archived.
