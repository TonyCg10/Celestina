# Closing PANEL-1: what shipped, and what its ledger never recorded

- **Date:** 2026-08-14
- **Scope:** Celestina unit `PANEL-1-Z`
- **Artifact:** Celestina 0.20.3 with CelestinaStyle 1.7.0
- **Environment:** the repository's own history and its automated suite; no
  compositor session is involved in this unit
- **Plan:** [panel glass redesign ledger](../plans/archive/2026-08-08-panel-glass-redesign.md)
- **Validation:** `VAL-PANEL-1`

This unit delivers no behaviour. It closes a plan whose work was delivered and
whose ledger stopped keeping up with it, and it says plainly which of those two
things this record is.

## Procedure

The plan's nine ledger units were compared against the version history and the
commits that carry them. Two — `PANEL-1-A` and `PANEL-1-B` — were closed the
way the contract asks, each with its own inventory. The remaining seven were
written as intent, delivered as code, and never returned to.

Their subjects are traceable through the version ledger rather than through
inventories of their own:

- `PANEL-1-I`, `PANEL-1-J`, `PANEL-1-K` — the edge-attached prototype and the
  droplet membrane, delivered in 0.11.0 and 0.12.0.
- `PANEL-1-L`, `PANEL-1-M` — per-output sizing, raster fidelity and the DDC
  smoke correction, delivered in 0.13.0 and 0.13.1.
- `PANEL-1-N`, `PANEL-1-O` — contextual surfaces at the right per-output size
  and the factor derived from size rather than density, delivered in 0.14.0
  and 0.14.1.

Everything after those went further than the ledger's own decomposition ever
described: 0.15.0 through 0.19.0 carried the reading menus, the quiet surfaces
onto the shell's glass, the physical arrival and departure of every falling
drop, one contextual surface at a time answering on the press, and the dense
glass's own blur. Those are recorded in the version history and in this
project's dated evidence records, which are the honest trail.

## Result

The plan is archived with its two properly closed units intact and this
administrative unit standing for the rest. No inventory is reconstructed after
the fact: writing seven inventories today against base revisions chosen in
hindsight would produce documents that look like evidence and are not.

The automated suite passes at the moment of closure — CTest 18/18, the
production QML lint clean — which is the state the plan is being closed in, not
a proof of any single unit inside it.

## Limits

This record does not claim the plan's hypothesis was validated. `VAL-PANEL-1`
was never run: the redesign has been judged on the author's nested session and
on screen recordings, never on the three real monitors, and the material is
still being iterated on. Closing the plan releases the implementation
checkpoint so other work can hold it; it does not declare the panel finished.

Design work on the shell's glass continues under a future checkpoint, opened
when the author decides what it should contain. That is the author's stated
intent for this milestone: something that iterates rather than something that
concludes.
