# Active Celestina plans

The active shell implementation plan is
[R6 — first-party session lock](2026-08-14-first-party-session-lock.md),
authorized and bounded by
[ADR 0004](../../decisions/0004-first-party-session-lock.md). The panel glass
redesign closed on 2026-08-14 without being finished — it shipped
continuously and stopped keeping its ledger — and design work on the shell's
glass reopens under a future checkpoint.

Plans that are authorized but whose checkpoint has not opened wait under
[`../pending/`](../pending/).
Completed plans remain under [`../archive/`](../archive/).
Each plan owns a persistent change ledger and remains separate from author-only
validation.

Unit inventories live under
[`../../inventories/<plan-slug>/<unit>.numstat.tsv`](../../inventories/), not in
this directory, and remain there when only the plan is archived.
