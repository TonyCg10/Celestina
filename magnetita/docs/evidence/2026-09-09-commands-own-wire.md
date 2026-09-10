# Registered commands published and run by id — MAG-P5-A

- **Date:** 2026-09-09
- **Scope:** `MAG-P5-A` of
  [`../plans/archive/2026-09-09-remote-control.md`](../plans/archive/2026-09-09-remote-control.md):
  `celestina-rs/crates/magnetitad/src/link_wire/commands.rs`, the
  `commands` and `input` settings, `Devices1.ListCommands`, `SetCommand`
  and `RemoveCommand`, the desktop app's `src/commands.rs`,
  `qml/components/CommandRow.qml` and the settings section, this record
- **Environment:** the workspace's loopback tests; the daemon and app
  deployed through `magnetita/scripts/complete-production.sh`
- **Artifact:** `magnetitad` and the desktop app, deployed

## Design

- The registry is `commands.json` under the daemon's configuration
  directory: id, name, program, arguments as a list. The phone receives
  ids and names only, as the session opens and again whenever the
  registry changes (`Command::CommandsChanged` on every queue).
- A run names an id; the daemon spawns the program with its arguments as
  a process group under the subprocess discipline every other tool uses,
  with a sixty-second budget, and answers whether it exited zero. An
  unknown id runs nothing.
- The desktop app's settings gain a section: the registered commands with
  a remove glyph, and a row of three fields (name, program, arguments
  split on spaces) with an add glyph.

## Procedure

```sh
cd celestina-rs && cargo test -p magnetitad
busctl --user call … SetCommand usss 0 "Say hi" "true" 0
magnetita-peer connect 10.0.0.134:1760 --run 1 --hold 5
```

## Result

- **Exit:** 0. The registry's test numbers, publishes names only, runs
  `true` and `false` by id and refuses an unknown id; the loopback test
  sees the list in the greeting, registers a command, sees the new list
  after `CommandsChanged`, runs it and receives the result, and gets a
  failure for an unknown id.

## Limits

- The program runs with the daemon's environment; a command that needs
  the session's display variables is a script that sets them.
