# The owner, the guard and the coalescer — MAG-M1-A

- **Date:** 2026-09-10
- **Scope:** `MAG-M1-A` of
  [`../plans/archive/2026-09-10-app-lifecycle.md`](../plans/archive/2026-09-10-app-lifecycle.md):
  `src/lifecycle.rs`, `src/main.rs`, this record
- **Environment:** the workspace's tests
- **Artifact:** none; the models adopt the owner in `MAG-M1-B`

## Ownership map

Every thread the app spawned before this unit, and who owns it now.

| Thread | Before | Now |
|---|---|---|
| Device snapshot read | detached, one per request | `DevicesModel.owned`, coalesced by `Reload` |
| Connection-log read | detached | `DevicesModel.owned`, coalesced by `Reload` |
| Settings read | detached | `DevicesModel.owned`, coalesced by `Reload` |
| Mirror snapshot read | detached | `DevicesModel.owned` |
| Pairing window read | detached | `DevicesModel.owned` |
| `Changed` watch loop | detached, blocked on the bus forever | `DevicesModel.owned`; returns within a quarter second of close |
| `Event` watch loop | detached, blocked forever | `DevicesModel.owned`; returns within a quarter second of close |
| The two bus iterator threads behind each watch | detached | end with the connection the returning loop drops; deliver nothing once the guard closes |
| UI action worker | owned, channel closed and joined on drop | unchanged; closed after the owner so its queue drains |
| Messages refresh | detached | `MessagesModel.owned` |
| SMS send worker | owned, joined on drop | unchanged |
| Commands refresh, add, remove | detached | `CommandsModel.owned` |

## Design

`lifecycle::Owned` is one owner per QObject: `spawn` hands the worker a
`Guard`, `close` (run by `Drop`) flips the guard and joins every worker,
and a spawn after closing does nothing. Every worker checks the guard
before queueing onto the GUI thread, so a snapshot that finished after
shutdown began is dropped. `lifecycle::Reload` is the coalescing the
model kept in four booleans per read, now one type: a request during a
read folds into one follow-up, so a burst costs at most two round trips
and the newest snapshot wins. The bus watches take the guard and wake
every quarter second to notice it.

## Procedure

```sh
cd magnetita && cargo test lifecycle && cargo clippy --all-targets -- -D warnings
```

## Result

- **Exit:** 0; three tests: the burst coalesced into one follow-up, the
  owner closed under sixteen workers twenty times with the slow half
  refused and the late spawn never run, and the guard waking from a
  ten-second wait at once.

## Limits

- The bus iterator threads are not joined: zbus blocks them until the
  connection drops, which the returning loop does; they hold no model.
