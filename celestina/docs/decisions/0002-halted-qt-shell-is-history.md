# ADR 0002: The halted Qt shell is history at `celestina-qt/`

- **Date:** 2026-10-14
- **Status:** superseded
- **Superseded by:** [ADR 0023](0023-qt-shell-retired.md), written by the
  unit CEL-QT-REMOVE, which removed `celestina-qt/` and
  `celestina-rs/crates/celestina-shell-core` from the tree and registered the
  shell as a retired project; the salvage list stays as a record in
  [AGENTS.md "Halted projects"](../../../AGENTS.md#halted-projects).

## Context

The new shell takes the halted shell's identity (id, path and prefix
`celestina`), so the halted tree must move. Its provider code, its niri
adapter and the pure state machines of `celestina-shell-core` are tested and
worth keeping, but the halt forbids editing them. See [the design](../../../docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md), §2.

## Decision

The halted Qt/QML shell lives at `celestina-qt/` (id and prefix
`celestina-qt`), still halted, as history and rollback. This project salvages
from it only by copy: the root contract's "Halted projects" section names the
files that may be copied, each copy records in its head comment the source
path and the commit it came from, and the halted bytes are never edited.

## Consequences

- `celestina-qt/` and `celestina-rs/crates/celestina-shell-core` are never
  edited from this project; a module the list does not name is not copied.
- Later deliveries that need another module extend the list through the
  author's edit of "Halted projects" first.
- The history rows named `celestina` up to this project's baseline belong to
  `celestina-qt` (`history_owner`, the version contract's rename rule).

## Revisit when

The author lifts the halt of `celestina-qt` or removes it from the registry.
