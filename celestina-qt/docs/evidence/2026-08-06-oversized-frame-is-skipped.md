# Evidence: 2026-08-06 an oversized frame is skipped, not fatal

- **Date:** 2026-08-06
- **Scope:** `LVR-3-D`; plan
  [late-provider-insertion](../plans/archive/2026-08-05-late-provider-insertion.md);
  the Niri finding in the medium section of the
  [light monorepo audit](../../../docs/evidence/2026-08-06-light-monorepo-audit.md)
- **Environment:** source and text only. The GPU safety hold stands, so nothing
  was compiled, tested, built, deployed or run against this project
- **Artifact:** none, and none may be produced during the hold

## What was wrong

`SharedWriter::emit` gained a `WriteError::TooLong` variant when the frame
budgets were aligned with the host. The provider helper handles it correctly:
`is_fatal()` distinguishes a write failure, which ends the channel, from a frame
the host would discard, which is skipped.

The Niri adapter was not updated with it. `stream_session` emitted through
`emit_json`, which wraps any `WriteError` in `AdapterError::Emit`, so a
`TooLong` left the function through `?` and `stream_forever` read it as the end
of the session: it printed the reason, published `unavailable` — which empties
the workspace strip — slept, reconnected, rebuilt the identical state and
produced the identical oversized frame again.

Before the budgets existed, the same situation cost one dropped line: the writer
wrote it and the host's decoder discarded it. The alignment turned a dropped
line into a reconnect loop against the compositor.

Reachability is a corner: the per-field bounds do not bound their product, so it
takes something like the full 512 workspaces carrying long multi-byte titles to
exceed a mebibyte. It is a real error-handling defect at an unlikely input
rather than a likely failure.

## What changed

- `src/niri_adapter.rs`, `stream_session` — the snapshot is emitted through
  `writer.emit` directly and the outcome is classified: a fatal write ends the
  session as before, an oversized frame is reported and skipped.
- The skipped snapshot is deliberately **not** stored as `last_snapshot`. The
  emit is guarded by a change comparison against that value, so remembering a
  frame that never left would suppress the next state change as a duplicate of
  something the host never saw.

## Procedure

None. No command was run against this project.

```text
The GPU safety hold forbids running any Celestina executable, provider,
build, test, deployment or activation. This unit is source and text.
```

## Result

Not verified by execution. Reviewed by reading: `WriteError` and `is_fatal` were
already in scope in this file, `emit` answers `Result<(), WriteError>`, and the
borrow of the snapshot ends before the move into `last_snapshot`. The suite
guards that do not build this project — architecture, language and documentation
— were run and pass.

## Limits

This code has not seen a compiler, and the loop it removes has not been observed
either happening or not happening. The classification it now applies is the one
the provider helper already applies to the same error, which is the argument for
believing it correct; it is not a substitute for the compiler or for a run.

The underlying shape stands: field bounds do not bound their product, so a
snapshot can still be built that no one can send. Skipping it is the honest
outcome, but bounding the assembled snapshot before emitting would be better and
is not attempted here.
