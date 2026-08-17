# Active Celestina plans

[`LOCK-1`](2026-08-17-lock-depth-transition.md) is the active Celestina
implementation plan: the locked session recedes behind its own blurred
wallpaper instead of vanishing into an opaque slab, and is uncovered
continuously. It changes what a locked screen looks like and nothing about what
unlocks it.

Plans that are authorized but whose checkpoint has not opened wait under
[`../pending/`](../pending/).
Completed plans remain under [`../archive/`](../archive/).
Each plan owns a persistent change ledger and remains separate from author-only
validation.

Unit inventories live under
[`../../inventories/<plan-slug>/<unit>.numstat.tsv`](../../inventories/), not in
this directory, and remain there when only the plan is archived.
