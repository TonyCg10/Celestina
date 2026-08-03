# Suite delivery inventories

This is the stable home for exact commit inventories owned by the suite.
Store each record as `<plan-slug>/<unit>.numstat.tsv`, where `plan-slug` is the
plan's exact basename without `.md` and `unit` is the exact ledger id.
The plan declares a unique, stable `Plan ID` before its first inventory commit.

Inventories stay here whether their plan is under [`../plans/active/`](../plans/active/)
or [`../plans/archive/`](../plans/archive/). Once an inventory is versioned it is
immutable: never edit, move, rename or reuse it. Corrections and later delivery
work receive a new unit and a new inventory.

Archiving moves only the plan and preserves its basename, checkpoint, `Plan ID`,
units and links. If that movement needs a separate traceable commit, record it
as a new administrative unit with its own inventory in this tree.
