# ADR 0013: Everything the bar holds is a piece behind one contract

- **Date:** 2026-10-16
- **Status:** accepted

## Context

The Island's tabs were a closed enum of pages, and each new bar item would
otherwise add its own special case in the Island. The design's §6.3 asks for
one contract that the bar, the Island's tabs and `island open <page>` all read
from. See [the piece contract](../../src/pieces/mod.rs), which documents the
trait, the static table and the rules for adding a piece.

## Decision

A piece is one thing the bar can hold. The contract is the `Piece` trait in
`src/pieces/mod.rs` (`id`, `name`, `bar`, `page`, `activate`), with the static
`PIECES` table as the only list of pieces. A piece reads only its typed,
read-only `BarCx`, asks for things only through `PieceAction`, and never touches
a surface, a bus or a process. A piece with a page appears as an Island tab
in the table's order. The CEL-1 pages become the pieces' pages without a change
of behaviour.

## Consequences

- Adding a unit's bar item is a new piece file, one table row and, if needed,
  one read-only slice and one action variant; no unit edits another piece.
- Piece ids are the stable keys of the layout in `shell.json`, so they are
  part of a persisted, compatibly evolving format.
- The table is shared code: a change to `BarCx` or `PieceAction` touches every
  piece and must stay additive.

## Revisit when

A piece needs state that cannot be expressed as a read-only slice, or the
table cannot express a placement the layout requires (for example one piece in
two zones).
