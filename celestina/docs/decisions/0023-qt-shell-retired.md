# ADR 0023: The Qt shell is retired and removed from the tree

- **Date:** 2026-10-23
- **Status:** accepted
- **Number:** written by the CEL-QT-REMOVE unit; provisional until landing,
  by the same rule as ADR 0022. If another unit lands a number first, the
  landing renumbers this file and its index row.
- **Supersedes:** [ADR 0002](0002-halted-qt-shell-is-history.md)

## Context

ADR 0002 kept the halted Qt/QML shell in the repository at `celestina-qt/`
as history and rollback, and let this project salvage from it by copy. The
author decided on 2026-10-23 that the Qt shell is not kept: not as a tree,
not as a tag, not as a branch. What this project needed from it has been
copied (the salvage list in the root contract, and CEL-QT-RESCUE for the
Niri compositor patch and the Niri colour generator).

## Decision

`celestina-qt/` and `celestina-rs/crates/celestina-shell-core` are removed
from the tree by the unit CEL-QT-REMOVE. The registry records the shell as a
retired project (`[[retired_projects]]` in `docs/projects.toml`): its version
rows stay as a closed chain, its old commit prefixes resolve only when the
history is replayed, and no new work may use them. The salvage list in the
root contract's "Halted projects" section stays as the record of provenance
for the head comments that cite the removed paths and their commits.

## Consequences

- Nothing further is copied from the Qt shell; a module the salvage list
  never named is written anew, in this project's own terms.
- The head comments that cite `celestina-qt/...` or
  `celestina-shell-core/...` with a commit are exact: those commits remain
  in `main`'s history, which is the only place the bytes still exist.
- Rollback to the Qt shell is no longer a supported operation.

## Revisit when

Never, unless the author lifts this decision in writing.
