# The models under their owner — MAG-M1-B

- **Date:** 2026-09-10
- **Scope:** `MAG-M1-B` of
  [`../plans/active/2026-09-10-app-lifecycle.md`](../plans/active/2026-09-10-app-lifecycle.md):
  `src/controller.rs`, `src/messages.rs`, `src/commands.rs`,
  `src/devices.rs`, the plan indexes and the archived `MAG-P7` plan, this
  record
- **Environment:** the workspace's tests
- **Artifact:** `magnetita`, deployed in `MAG-M1-C`

## Design

Each of the three models holds one `lifecycle::Owned`. The device model's
snapshot reads go through `read_owned`, which runs the read on an owned
thread and queues the result onto the GUI thread only while the guard is
open; the four booleans of each coalesced read became one `Reload`. The
two bus watches take the guard, wake every quarter second to notice it,
and return when it closes; their callbacks deliver nothing after that.
The mirror snapshot and the pairing window are owned reads too. The
messages refresh and the commands refresh, add and remove are owned. On
drop the owner closes first, then the action worker's channel is closed
and it is joined, so a queued action still runs against a live daemon.

`MAG-P7` moved to the archive with its ledger done and `MAG-P7-D` noted
as waiting on `VAL-MAG-14`; `MAG-M1` opened as the active checkpoint.

## Procedure

```sh
cd magnetita && cargo test && cargo clippy --all-targets -- -D warnings
```

## Result

- **Exit:** 0; 17 tests; no detached `thread::spawn` remains outside the
  two owned action workers and the bus iterator threads.

## Limits

- The bus iterator threads are not joined: zbus blocks them until the
  connection drops, which the returning loop does; they hold no model.
